use crate::api::BlockHeaderLike;
use crate::api::{BlockLike, BlockPersistence};
use crate::api::{BlockProvider, ChainSyncError};
use crate::monitor::ProgressMonitor;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use redbit::{error, info};
use crate::task;
use crate::settings::IndexerSettings;

pub struct ChainSyncer<FB: Send + Sync + 'static, TB: BlockLike + 'static> {
    pub block_provider: Arc<dyn BlockProvider<FB, TB>>,
    pub block_persistence: Arc<dyn BlockPersistence<TB>>,
    pub monitor: Arc<ProgressMonitor>,
}

impl<FB: Send + Sync + 'static, TB: BlockLike + 'static> ChainSyncer<FB, TB> {
    pub fn new(block_provider: Arc<dyn BlockProvider<FB, TB>>, block_persistence: Arc<dyn BlockPersistence<TB>>) -> Self {
        Self { block_provider, block_persistence, monitor: Arc::new(ProgressMonitor::new(1000)) }
    }

    pub async fn sync(&self, indexer_conf: IndexerSettings) {
        let block_provider = Arc::clone(&self.block_provider);
        let persistence = Arc::clone(&self.block_persistence);
        let monitor = Arc::clone(&self.monitor);

        let node_chain_tip_header = block_provider.get_chain_tip().await.expect("Failed to get chain tip header");
        let chain_tip_height = node_chain_tip_header.height();
        let last_persisted_header = persistence.get_last_header().expect("Failed to get last header");
        let last_persisted_height = last_persisted_header.as_ref().map_or(0, |h| h.height());
        let heights_to_fetch = chain_tip_height - last_persisted_height;

        let indexing_par: usize = indexer_conf.processing_parallelism.clone().into();

        if heights_to_fetch <= 0 {
            return;
        }
        info!("Indexing from {} to {} with processing parallelism : {}", last_persisted_height, chain_tip_height, indexing_par);
        let buffer_size = 8192;
        let batch_buffer_size = std::cmp::max(16, buffer_size / indexer_conf.min_batch_size);
        let (fetch_tx, fetch_rx) = mpsc::channel::<FB>(buffer_size);
        let (proc_tx, mut proc_rx) = mpsc::channel::<TB>(buffer_size);
        let (sort_tx, mut sort_rx) = mpsc::channel::<Vec<TB>>(batch_buffer_size);

        // Process + batch stage (consumes provider.stream() directly)
        let fetch_handle = {
            let block_provider = Arc::clone(&self.block_provider);
            task::spawn_named("fetch", async move {
                let mut s = block_provider.stream(node_chain_tip_header.clone(), last_persisted_header);
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
            task::spawn_named("sort",async move {
                let mut batch: Vec<TB> = Vec::with_capacity(indexer_conf.min_batch_size);
                let mut weight: usize = 0;
                let min_weight = indexer_conf.min_batch_size;

                while let Some(block) = proc_rx.recv().await {
                    weight += block.weight() as usize;
                    let header = block.header();
                    let height = header.height();
                    let idx = batch.binary_search_by_key(&height, |b| b.header().height()).unwrap_or_else(|i| i);

                    if weight >= min_weight {
                        monitor.log(height, &header.timestamp_str(), &header.hash_str(), &weight, proc_rx.len());
                        batch.insert(idx, block);
                        if sort_tx.send(std::mem::take(&mut batch)).await.is_err() {
                            break;
                        }
                        weight = 0;
                    } else {
                        batch.insert(idx, block);
                    }
                }
                if !batch.is_empty() {
                    let _ = sort_tx.send(batch).await;
                }
                drop(sort_tx);
            })
        };

        let persist_handle = {
            let block_provider = Arc::clone(&block_provider);
            let persistence   = Arc::clone(&persistence);
            task::spawn_blocking_named("persist",move || {
                while let Some(batch) = sort_rx.blocking_recv() {
                    let do_chain_link = batch.last().is_some_and(|b| b.header().height() + 100 > chain_tip_height);
                    match Self::persist_blocks(batch, do_chain_link, Arc::clone(&block_provider), Arc::clone(&persistence)) {
                        Ok(()) => {},
                        Err(e) => {
                            error!("persist: persist_blocks returned error {}", e);
                            return;
                        }
                    }
                }
            })
        };

        if let Err(e) = tokio::try_join!(fetch_handle, process_handle, sort_handle, persist_handle) {
            error!("One of the pipeline tasks failed {}", e);
        }
    }

    fn chain_link(block: TB, block_provider: Arc<dyn BlockProvider<FB, TB>>, block_persistence: Arc<dyn BlockPersistence<TB>>) -> Result<Vec<TB>, ChainSyncError> {
        let header = block.header();
        let prev_headers = block_persistence.get_header_by_hash(header.prev_hash())?;

        // Base case: genesis
        if header.height() == 1 {
            return Ok(vec![block]);
        }

        // If the DB already has the direct predecessor, we can stop here
        if prev_headers.first().map(|ph| ph.height() == header.height() - 1).unwrap_or(false) {
            return Ok(vec![block]);
        }

        // Otherwise we need to fetch the parent and prepend it
        if prev_headers.is_empty() {
            info!("Fork detected at {}@{}, downloading parent {}", header.height(), hex::encode(header.hash()), hex::encode(header.prev_hash()),);

            // fetch parent
            let parent_header = header.clone();
            let parent_block = block_provider.get_processed_block(parent_header)?;
            // recurse to build the earlier part of the chain
            let mut chain = Self::chain_link(parent_block, block_provider, block_persistence)?;
            // now append our current block at the end
            chain.push(block);
            return Ok(chain);
        }

        // If we got here, there were multiple candidates in DB → panic or handle specially
        panic!("Unexpected condition in chain_link: multiple parent candidates for {}@{}", header.height(), hex::encode(header.hash()));
    }

    pub fn persist_blocks(mut blocks: Vec<TB>, do_chain_link: bool, block_provider: Arc<dyn BlockProvider<FB, TB>>, block_persistence: Arc<dyn BlockPersistence<TB>>) -> Result<(), ChainSyncError> {
        if !do_chain_link {
            block_persistence.store_blocks(blocks)
        } else {
            for block in blocks.drain(..) {
                let chain = Self::chain_link(block, Arc::clone(&block_provider), Arc::clone(&block_persistence))?;

                match chain.len() {
                    0 => unreachable!("chain_link never returns empty Vec"),
                    1 => block_persistence.store_blocks(chain)?,
                    _ => block_persistence.update_blocks(chain)?,
                }
            }
            Ok(())
        }

    }
}
