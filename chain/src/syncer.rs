use crate::api::{BlockChainLike, BlockLike};
use crate::api::{BlockHeaderLike, SizeLike};
use crate::api::{BlockProvider, ChainError};
use crate::combine::ShutdownReason;
use crate::monitor::ProgressMonitor;
use crate::reorder_buffer::ReorderBuffer;
use crate::settings::{IndexerSettings, Parallelism};
use crate::weight_batcher::WeightBatcher;
use crate::{combine, task};
use futures::StreamExt;
use redb::Durability;
use redbit::storage::table_writer_api::TaskResult;
use redbit::{error, info, warn, WriteTxContext};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::{mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;

pub struct ChainSyncer<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext> {
    pub block_provider: Arc<dyn BlockProvider<FB, TB>>,
    pub chain: Arc<dyn BlockChainLike<TB, CTX>>,
}

impl<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext + 'static> ChainSyncer<FB, TB, CTX> {
    pub fn new(block_provider: Arc<dyn BlockProvider<FB, TB>>, chain: Arc<dyn BlockChainLike<TB, CTX>>) -> Self {
        Self { block_provider, chain }
    }

    pub async fn sync(&self, indexer_conf: &IndexerSettings, last_header: Option<TB::Header>, shutdown: watch::Receiver<bool>) -> Result<(), ChainError> {
        let block_provider = Arc::clone(&self.block_provider);
        let chain = Arc::clone(&self.chain);

        let node_chain_tip_header = block_provider.get_chain_tip().await?;
        let chain_tip_height = node_chain_tip_header.height();
        let last_persisted_header = last_header.or(chain.get_last_header()?);
        let height_to_index_from = last_persisted_header.as_ref().map_or(1, |h| h.height() + 1);
        let heights_to_fetch = chain_tip_height - last_persisted_header.as_ref().map_or(0, |h| h.height());

        let indexing_par: Parallelism = indexer_conf.processing_parallelism;
        let fork_detection_height: u32 = chain_tip_height - indexer_conf.fork_detection_heights as u32;

        if heights_to_fetch < 1 {
            return Ok(());
        }
        let (indexing_mode, batching, default_durability) =
             if heights_to_fetch > indexer_conf.fork_detection_heights as u32 {
                 ("batch", true, Durability::None)
             } else {
                 ("continuously", false, Durability::Immediate)
             };

        info!(
            "Going to {} index {} blocks from {} to {}, parallelism : {}, fork_detection @ {}",
            indexing_mode, heights_to_fetch, height_to_index_from, chain_tip_height, indexing_par.0, fork_detection_height
        );

        let process_persist_batch_size_ratio = 32;
        let min_entity_persist_batch_size = indexer_conf.min_entity_batch_size;
        let min_entity_process_batch_size = min_entity_persist_batch_size / process_persist_batch_size_ratio;
        let non_durable_batches = indexer_conf.non_durable_batches;

        let (fetch_rx, fetch_handle) =
            self.block_provider.block_stream(node_chain_tip_header.clone(), last_persisted_header, shutdown.clone(), batching);
        let (proc_tx, mut proc_rx) = mpsc::channel::<Vec<TB>>(4 * process_persist_batch_size_ratio); // holds processed batch
        let (sort_tx, mut sort_rx) = mpsc::channel::<Vec<TB>>(4); // holds persist batch

        let process_handle = {
            let proc_tx_stream = proc_tx.clone();
            let proc_fn = block_provider.block_processor();
            let shutdown = shutdown.clone();
            task::spawn_named("process", async move {
                let mut proc_buf: Vec<TB> = Vec::new();
                let mut proc_weight: usize = 0;

                let proc_stream =
                    ReceiverStream::new(fetch_rx)
                        .map(move |batch| {
                            let proc_fn = proc_fn.clone();
                            tokio::task::spawn_blocking(move || {
                                batch
                                    .into_iter()
                                    .map(|raw| proc_fn(&raw))
                                    .collect::<Vec<_>>()
                            })
                        })
                        .buffer_unordered(indexing_par.0);

                futures::pin_mut!(proc_stream);
                loop {
                    tokio::select! {
                        biased;
                        _ = combine::await_shutdown(shutdown.clone()) => {
                            info!("process: shutdown");
                            break;
                        }
                        maybe = proc_stream.next() => {
                            match maybe {
                                Some(join_res) => {
                                    match join_res {
                                        Ok(results) => {
                                            for r in results {
                                                match r {
                                                    Ok(block) => {
                                                        // accumulate by weight; send when limit crossed
                                                        proc_weight = proc_weight.saturating_add(block.header().weight() as usize);
                                                        proc_buf.push(block);
                                                        if proc_weight > min_entity_process_batch_size {
                                                            let _ = proc_tx_stream.send(std::mem::take(&mut proc_buf)).await;
                                                            proc_weight = 0;
                                                        }
                                                    }
                                                    Err(e) => {
                                                        warn!("Processing warning: {e}");
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!("spawn_blocking join error: {e}");
                                        }
                                    }
                                }
                                None => break, // upstream closed
                            }
                        }
                    }
                }
                drop(proc_tx);
            })
        };

        // Sort + process stage is executed at parallel so blocks are not coming in order
        let sort_handle = {
            let shutdown = shutdown.clone();
            task::spawn_named("sort", async move {
                let safety_cap = 1024 * 8;
                let mut reorder: ReorderBuffer<TB> = ReorderBuffer::new(height_to_index_from, safety_cap);
                let mut batcher: WeightBatcher<TB> = WeightBatcher::new(min_entity_persist_batch_size, safety_cap, default_durability);

                loop {
                    tokio::select! {
                        maybe = proc_rx.recv() => {
                            match maybe {
                                Some(blocks) => {
                                    for block in blocks {
                                        let header = block.header();
                                        let h = header.height();

                                        // 1) Strict global reordering; returns only contiguous-from-next items.
                                        let ready = reorder.insert(h, block);

                                        // Optional observability/backpressure hint on wide gaps:
                                        if reorder.is_saturated() && let Some((need, seen)) = reorder.gap_span() {
                                            warn!("Block @ {} not fetched, currently @ {} ... pending {} blocks", need, seen, reorder.pending_len());
                                        }

                                        // 2) Feed in-order items into weight batcher.
                                        for b in ready {
                                            if let Some(out) = batcher.push_with(b, |x| x.header().weight() as usize) {
                                                if sort_tx.send(out).await.is_err() { break; }
                                            }
                                        }
                                    }
                                }
                                None => break, // upstream closed
                            }
                        }
                        _ = combine::await_shutdown(shutdown.clone()) => {
                            info!("sort: shutdown");
                            break;
                        }
                    }
                }

                if let Some(out) = batcher.flush() {
                    let _ = sort_tx.send(out).await;
                }
                drop(sort_tx);
            })
        };

        let mut persist_handle = {
            let block_provider = Arc::clone(&block_provider);
            let chain = Arc::clone(&chain);
            let shutdown = shutdown.clone();
            task::spawn_blocking_named("persist", move || {
                if let Ok(index_context) = chain.new_indexing_ctx() {
                    let tick = std::time::Duration::from_millis(50);
                    let monitor = ProgressMonitor::new();
                    let mut batch_counter = 0;
                    loop {
                        if *shutdown.borrow() {
                            info!("persist: shutdown signal received");
                            break;
                        } else {
                            match sort_rx.try_recv() {
                                Ok(batch) => {
                                    let durability = if batch_counter > 0 && batch_counter % non_durable_batches == 0 {
                                        Durability::Immediate
                                    } else {
                                        default_durability
                                    };
                                    monitor.log_batch(&batch, durability, sort_rx.len());
                                    match Self::persist_or_link(
                                        &index_context,
                                        batch,
                                        fork_detection_height,
                                        Arc::clone(&block_provider),
                                        Arc::clone(&chain),
                                        durability
                                    ) {
                                        Ok(tasks) => {
                                            batch_counter += 1;
                                            monitor.log_task_results(tasks);
                                        }
                                        Err(e) => {
                                            error!("persist: persist_or_link returned error {e}");
                                            return;
                                        }
                                    }
                                }
                                Err(TryRecvError::Empty) => {
                                    std::thread::sleep(tick);
                                    continue;
                                }
                                Err(TryRecvError::Disconnected) => {
                                    break;
                                }
                            }
                        }
                    }
                    if let Err(e) = index_context.stop_writing() {
                        error!("persist: stop_writing error: {e}");
                    }
                } else {
                    panic!("persist: cannot create indexing context");
                }
            })
        };

        tokio::select! {
            result = &mut persist_handle => {
                result?;
            }
            reason = combine::await_shutdown(shutdown.clone()) => {
                match reason {
                    ShutdownReason::AlreadyTrue | ShutdownReason::ChangedToTrue => {
                        info!("sync: shutdown observed, aborting workers");
                    }
                    ShutdownReason::SenderDropped => {
                        info!("sync: shutdown sender dropped, aborting workers");
                    }
                }
                fetch_handle.abort();
                process_handle.abort();
                sort_handle.abort();
                let _ = fetch_handle.await;
                let _ = process_handle.await;
                let _ = sort_handle.await;
                persist_handle.await?;
            }
        }
        Ok(())
    }

    fn chain_link(block: TB, block_provider: Arc<dyn BlockProvider<FB, TB>>, chain: Arc<dyn BlockChainLike<TB, CTX>>) -> Result<Vec<TB>, ChainError> {
        let header = block.header();
        let prev_headers = chain.get_header_by_hash(header.prev_hash())?;
        let height = header.height();
        let hash_str = &header.hash().to_string();
        let prev_hash_str = &header.prev_hash().to_string();

        if height == 1 {
            // Base case: genesis
            Ok(vec![block])
        } else if prev_headers.first().map(|ph| ph.height() == height - 1).unwrap_or(false) {
            // If the DB already has the direct predecessor, we can stop here
            info!("Block @ {} : {} linked with parent {}", height, hash_str, prev_hash_str);
            Ok(vec![block])
        } else if prev_headers.is_empty() {
            // Otherwise we need to fetch the parent and prepend it
            info!("Fork detected @ {} : {} - downloading his parent {}", height, hash_str, prev_hash_str);
            match block_provider.get_processed_block(header.prev_hash())? {
                None => {
                    warn!("Fork cannot be formed because parent {} @ {} cannot be fetched from node", prev_hash_str, height);
                    Ok(vec![])
                },
                Some(parent_block) => {
                    let mut fork = Self::chain_link(parent_block, block_provider, chain)?;
                    fork.push(block);
                    Ok(fork)
                }
            }
        } else if let Some(prev_header) = prev_headers.first() {
            panic!(
                "Found prev header {} with different height {} @ {} : {} -> {}",
                &prev_header.hash(),
                prev_header.height(),
                height,
                hash_str,
                prev_hash_str
            );
        } else {
            panic!("Found {} prev headers", prev_headers.len())
        }
    }

    pub fn persist_or_link(indexing_context: &CTX, mut blocks: Vec<TB>, fork_detection_height: u32, block_provider: Arc<dyn BlockProvider<FB, TB>>, block_chain: Arc<dyn BlockChainLike<TB, CTX>>, durability: Durability) -> Result<HashMap<String, TaskResult>, ChainError> {
        if blocks.is_empty() {
            error!("Received empty block batch, nothing to persist");
            Ok(HashMap::new())
        } else if blocks.last().is_some_and(|b| b.header().height() <= fork_detection_height) {
            block_chain.store_blocks(indexing_context, blocks, durability)
        } else {
            let mut last_tasks: HashMap<String, TaskResult> = HashMap::new();
            for block in blocks.drain(..) {
                let chain = Self::chain_link(block, Arc::clone(&block_provider), Arc::clone(&block_chain))?;
                match chain.len() {
                    0 => (),
                    1 => { last_tasks = block_chain.store_blocks(indexing_context, chain, durability)? },
                    _ => { last_tasks = block_chain.update_blocks(indexing_context, chain)? },
                }
            }
            Ok(last_tasks)
        }

    }
}
