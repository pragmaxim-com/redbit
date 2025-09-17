use crate::api::{BlockHeaderLike, SizeLike};
use crate::api::{BlockChainLike, BlockLike};
use crate::api::{BlockProvider, ChainError};
use crate::batcher::{Batcher, ReorderBuffer, SyncMode};
use crate::monitor::ProgressMonitor;
use crate::settings::{IndexerSettings, Parallelism};
use crate::task;
use futures::StreamExt;
use redbit::{error, info, warn, WriteTxContext};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;

pub struct ChainSyncer<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext> {
    pub block_provider: Arc<dyn BlockProvider<FB, TB>>,
    pub chain: Arc<dyn BlockChainLike<TB, CTX>>,
    pub monitor: Arc<ProgressMonitor>,
}

impl<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext + 'static> ChainSyncer<FB, TB, CTX> {
    pub fn new(block_provider: Arc<dyn BlockProvider<FB, TB>>, chain: Arc<dyn BlockChainLike<TB, CTX>>) -> Self {
        Self { block_provider, chain, monitor: Arc::new(ProgressMonitor::new(1000)) }
    }

    pub async fn sync(&self, indexer_conf: &IndexerSettings, last_header: Option<TB::Header>, shutdown: watch::Receiver<bool>) -> Result<(), ChainError> {
        let block_provider = Arc::clone(&self.block_provider);
        let chain = Arc::clone(&self.chain);
        let monitor = Arc::clone(&self.monitor);

        let node_chain_tip_header = block_provider.get_chain_tip().await?;
        let chain_tip_height = node_chain_tip_header.height();
        let last_persisted_header = last_header.or(chain.get_last_header()?);
        let height_to_index_from = last_persisted_header.as_ref().map_or(1, |h| h.height() + 1);
        let heights_to_fetch = chain_tip_height - last_persisted_header.as_ref().map_or(0, |h| h.height());

        let indexing_par: Parallelism = indexer_conf.processing_parallelism.clone().into();
        let fork_detection_height: u32 = chain_tip_height - indexer_conf.fork_detection_heights as u32;

        if heights_to_fetch <= 0 {
            return Ok(());
        }
        let (indexing_how, indexing_mode) =
             if heights_to_fetch > indexer_conf.fork_detection_heights as u32 {
                 ("batch", SyncMode::Batching)
             } else {
                 ("continuously", SyncMode::Continuous)
             };

        info!(
            "Going to {} index {} blocks from {} to {}, parallelism : {}, fork_detection @ {}",
            indexing_how, heights_to_fetch, height_to_index_from, chain_tip_height, indexing_par.0, fork_detection_height
        );
        let buffer_size = 512;
        let block_byte_size_blocking_limit = 128 * 1024;
        let min_batch_size = indexer_conf.min_batch_size;
        let batch_buffer_size = buffer_size / 4;
        let (fetch_tx, fetch_rx) = mpsc::channel::<FB>(buffer_size);
        let (proc_tx, mut proc_rx) = mpsc::channel::<TB>(buffer_size);
        let (sort_tx, mut sort_rx) = mpsc::channel::<Vec<TB>>(batch_buffer_size);

        // Process + batch stage (consumes provider.stream() directly)
        let fetch_handle = {
            let block_provider = Arc::clone(&self.block_provider);
            task::spawn_named("fetch", async move {
                let mut s = block_provider.stream(node_chain_tip_header.clone(), last_persisted_header, indexing_mode);
                while let Some(raw) = s.next().await {
                    if let Err(_) = fetch_tx.send(raw).await {
                        error!("fetch: raw_rx closed, stopping fetcher");
                        return;
                    }
                }
                drop(fetch_tx);
            })
        };

        let process_handle = {
            let proc_tx_stream = proc_tx.clone();
            let proc_fn = block_provider.block_processor();
            task::spawn_named("process", async move {
                ReceiverStream::new(fetch_rx)
                    .for_each_concurrent(indexing_par.0, move |raw_block| {
                        let tx = proc_tx_stream.clone();
                        let proc_fn = proc_fn.clone();
                        async move {
                            if raw_block.size() < block_byte_size_blocking_limit {
                                match proc_fn(&raw_block) {
                                    Ok(block)    => { let _ = tx.send(block).await; }
                                    Err(e)  => { error!("Small block processing failure: {e}"); }
                                }
                            } else {
                                match tokio::task::spawn_blocking(move || proc_fn(&raw_block)).await {
                                    Ok(Ok(block))    => { let _ = tx.send(block).await; }
                                    Ok(Err(e))  => { error!("Big block processing failure: {e}"); }
                                    Err(e)       => { error!("spawn_blocking join error: {e}"); }
                                }
                            }
                        }
                    })
                    .await;
                drop(proc_tx);
            })
        };

        // Sort + process stage is executed at parallel so blocks are not coming in order
        let sort_handle = {
            task::spawn_named("sort", async move {
                let mut reorder: ReorderBuffer<TB> = ReorderBuffer::new(height_to_index_from, buffer_size * 4);
                let mut batcher: Batcher<TB> = Batcher::new(min_batch_size, buffer_size, indexing_mode);

                while let Some(block) = proc_rx.recv().await {
                    let header = block.header();
                    let h = header.height();

                    // 1) Strict global reordering; returns only contiguous-from-next items.
                    let ready = reorder.insert(h, block);

                    // Optional observability/backpressure hint on wide gaps:
                    if reorder.is_saturated() {
                        if let Some((need, seen)) = reorder.gap_span() {
                            monitor.warn_gap(need, seen, reorder.pending_len());
                        }
                    }

                    // 2) Feed in-order items into weight batcher.
                    for b in ready {
                        if let Some(out) = batcher.push_with(b, |x| x.header().weight() as usize) {
                            if let Some(last) = out.last() {
                                let lh = last.header();
                                monitor.log(
                                    lh.height(),
                                    &lh.timestamp().to_string(),
                                    &lh.hash().to_string(),
                                    out.len(),
                                    out.iter().map(|x| x.header().weight() as usize).sum::<usize>(),
                                    proc_rx.len(),
                                );
                            }
                            if sort_tx.send(out).await.is_err() { break; }
                        }
                    }
                }

                if let Some(out) = batcher.flush() {
                    let _ = sort_tx.send(out).await;
                }
                drop(sort_tx);
            })
        };

        let persist_handle = {
            let block_provider = Arc::clone(&block_provider);
            let chain = Arc::clone(&chain);
            let shutdown = shutdown.clone();
            task::spawn_blocking_named("persist", move || {
                if let Ok(index_context) = chain.new_indexing_ctx() {
                    loop {
                        if *shutdown.borrow() {
                            info!("persist: shutdown signal received");
                            break;
                        } else {
                            match sort_rx.blocking_recv() {
                                Some(batch) => {
                                    if let Err(e) = Self::persist_or_link(
                                        &index_context,
                                        batch,
                                        fork_detection_height,
                                        Arc::clone(&block_provider),
                                        Arc::clone(&chain),
                                    ) {
                                        error!("persist: persist_blocks returned error {e}");
                                        return;
                                    }
                                }
                                None => break,
                            }
                        }
                    }
                    index_context.stop_writing().unwrap();
                } else {
                    panic!("persist: cannot create indexing context");
                }
            })
        };

        let _ = tokio::try_join!(fetch_handle, process_handle, sort_handle, persist_handle)?;
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
        } else {
            if let Some(prev_header) = prev_headers.first() {
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

    }

    pub fn persist_or_link(indexing_context: &CTX, mut blocks: Vec<TB>, fork_detection_height: u32, block_provider: Arc<dyn BlockProvider<FB, TB>>, block_chain: Arc<dyn BlockChainLike<TB, CTX>>) -> Result<(), ChainError> {
        if blocks.is_empty() {
            error!("Received empty block batch, nothing to persist");
            Ok(())
        } else if blocks.last().is_some_and(|b| b.header().height() <= fork_detection_height) {
            block_chain.store_blocks(indexing_context, blocks)
        } else {
            for block in blocks.drain(..) {
                let chain = Self::chain_link(block, Arc::clone(&block_provider), Arc::clone(&block_chain))?;
                match chain.len() {
                    0 => (),
                    1 => block_chain.store_blocks(indexing_context, chain)?,
                    _ => block_chain.update_blocks(indexing_context, chain)?,
                }
            }
            Ok(())
        }

    }
}
