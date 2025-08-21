use crate::api::{BlockChainLike, BlockLike};
use crate::api::BlockHeaderLike;
use crate::api::{BlockProvider, ChainError};
use crate::monitor::ProgressMonitor;
use crate::settings::IndexerSettings;
use crate::task;
use futures::StreamExt;
use redbit::{error, info};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use crate::batcher::{Batcher, ReorderBuffer, SyncMode};

pub struct ChainSyncer<FB: Send + Sync + 'static, TB: BlockLike + 'static> {
    pub block_provider: Arc<dyn BlockProvider<FB, TB>>,
    pub chain: Arc<dyn BlockChainLike<TB>>,
    pub monitor: Arc<ProgressMonitor>,
}

impl<FB: Send + Sync + 'static, TB: BlockLike + 'static> ChainSyncer<FB, TB> {
    pub fn new(block_provider: Arc<dyn BlockProvider<FB, TB>>, chain: Arc<dyn BlockChainLike<TB>>) -> Self {
        Self { block_provider, chain, monitor: Arc::new(ProgressMonitor::new(1000)) }
    }

    pub async fn sync(&self, indexer_conf: &IndexerSettings, mut shutdown: watch::Receiver<bool>,) -> Result<(), ChainError> {
        let block_provider = Arc::clone(&self.block_provider);
        let chain = Arc::clone(&self.chain);
        let monitor = Arc::clone(&self.monitor);

        let node_chain_tip_header = block_provider.get_chain_tip().await?;
        let chain_tip_height = node_chain_tip_header.height();
        let last_persisted_header = chain.get_last_header()?;
        let height_to_index_from = last_persisted_header.as_ref().map_or(1, |h| h.height() + 1);
        let heights_to_fetch = chain_tip_height - last_persisted_header.as_ref().map_or(0, |h| h.height());

        let indexing_par: usize = indexer_conf.processing_parallelism.clone().into();
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
            indexing_how, heights_to_fetch, height_to_index_from, chain_tip_height, indexing_par, fork_detection_height
        );
        let buffer_size = 8192;
        let batch_buffer_size = std::cmp::max(16, buffer_size / indexer_conf.min_batch_size);
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
                    .for_each_concurrent(indexing_par, move |raw| {
                        let tx = proc_tx_stream.clone();
                        let proc_fn = proc_fn.clone();
                        async move {
                            match proc_fn(&raw) {
                                Ok(block)   => { let _ = tx.send(block).await; }
                                Err(e)      => { error!("process: {e}"); }
                            }
                        }
                    })
                    .await;
                drop(proc_tx);
            })
        };

        // Sort + process stage is executed at parallel so blocks are not coming in order
        let sort_handle = {
            let indexer_conf = indexer_conf.clone();
            task::spawn_named("sort", async move {
                let mut reorder: ReorderBuffer<TB> = ReorderBuffer::new(height_to_index_from, buffer_size);
                let mut batcher: Batcher<TB> = Batcher::new(indexer_conf.min_batch_size, buffer_size, indexing_mode);

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
                                    &lh.timestamp_str(),
                                    &lh.hash_str(),
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
            let chain   = Arc::clone(&chain);
            task::spawn_blocking_named("persist",move || {
                while let Some(batch) = sort_rx.blocking_recv() {
                    match Self::persist_or_link(batch, fork_detection_height, Arc::clone(&block_provider), Arc::clone(&chain)) {
                        Ok(()) => {},
                        Err(e) => {
                            error!("persist: persist_blocks returned error {}", e);
                            return;
                        }
                    }
                }
            })
        };

        let shutdown_handle: JoinHandle<Result<(), ChainError>> = tokio::spawn(async move {
            shutdown.changed().await.ok();
            if *shutdown.borrow() {
                info!("shutdown signal received");
            }
            // return an error to break try_join!
            Err(ChainError::Custom("Shutdown signal received".to_string()))
        });

        let _ = tokio::try_join!(fetch_handle, process_handle, sort_handle, persist_handle, shutdown_handle)?;
        Ok(())
    }

    fn chain_link(block: TB, block_provider: Arc<dyn BlockProvider<FB, TB>>, chain: Arc<dyn BlockChainLike<TB>>) -> Result<Vec<TB>, ChainError> {
        let header = block.header();
        let prev_headers = chain.get_header_by_hash(header.prev_hash())?;

        if header.height() == 1 {
            // Base case: genesis
            Ok(vec![block])
        } else if prev_headers.first().map(|ph| ph.height() == header.height() - 1).unwrap_or(false) {
            // If the DB already has the direct predecessor, we can stop here
            info!("Block @ {} : {} linked with parent {}", header.height(), &header.hash_str()[..12], &header.prev_hash_str()[..12]);
            Ok(vec![block])
        } else if prev_headers.is_empty() {
            // Otherwise we need to fetch the parent and prepend it
            info!("Fork detected @ {} : {} - downloading his parent {}", header.height(), &header.hash_str()[..12], &header.prev_hash_str()[..12]);
            let parent_header = header.clone();
            let parent_block = block_provider.get_processed_block(parent_header)?;
            // recurse to fetch the missing fork
            let mut chain = Self::chain_link(parent_block, block_provider, chain)?;
            // now append our current block at the end of the fork
            chain.push(block);
            Ok(chain)
        } else {
            if let Some(prev_header) = prev_headers.first() {
                panic!(
                    "Found prev header {} with different height {} @ {} : {} -> {}",
                    &prev_header.hash_str()[..12],
                    prev_header.height(),
                    header.height(),
                    &header.hash_str()[..12],
                    &header.prev_hash_str()[..12]
                );
            } else {
                panic!("Found {} prev headers", prev_headers.len())
            }
        }

    }

    pub fn persist_or_link(mut blocks: Vec<TB>, fork_detection_height: u32, block_provider: Arc<dyn BlockProvider<FB, TB>>, block_chain: Arc<dyn BlockChainLike<TB>>) -> Result<(), ChainError> {
        if blocks.is_empty() {
            error!("Received empty block batch, nothing to persist");
            Ok(())
        } else if blocks.last().is_some_and(|b| b.header().height() <= fork_detection_height) {
            block_chain.populate_inputs(&mut blocks)?;
            block_chain.store_blocks(blocks)
        } else {
            block_chain.populate_inputs(&mut blocks)?;
            for block in blocks.drain(..) {
                let chain = Self::chain_link(block, Arc::clone(&block_provider), Arc::clone(&block_chain))?;
                match chain.len() {
                    0 => unreachable!("chain_link never returns empty Vec"),
                    1 => block_chain.store_blocks(chain)?,
                    _ => block_chain.update_blocks(chain)?,
                }
            }
            Ok(())
        }

    }
}
