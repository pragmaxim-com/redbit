use crate::api::BlockHeaderLike;
use crate::api::{BlockLike, BlockPersistence};
use crate::api::{BlockProvider, ChainSyncError};
use crate::{error, info};
use crate::monitor::ProgressMonitor;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
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
        let chain_tip_for_persist = node_chain_tip_header.clone();
        let last_persisted_header = persistence.get_last_header().expect("Failed to get last header");

        let heights_to_fetch = node_chain_tip_header.clone().height() - last_persisted_header.as_ref().map_or(0, |h| h.height());
        if heights_to_fetch <= 0 {
            return;
        }

        type Batch<T> = (Vec<T>, usize);
        let (fetch_tx, mut fetch_rx) = mpsc::channel::<FB>(8192);
        let (proc_tx, mut proc_rx) = mpsc::channel::<Batch<TB>>(512);

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

        // Process + batch stage: read raw, parse on blocking thread, batch, send to persist
        let process_handle = {
            let indexer_conf = indexer_conf.clone();
            let block_provider = Arc::clone(&self.block_provider);
            task::spawn_named("process",async move {
                let mut batch: Vec<TB> = Vec::with_capacity(indexer_conf.min_batch_size);
                let mut weight: usize = 0;
                let min_weight = indexer_conf.min_batch_size;

                while let Some(raw) = fetch_rx.recv().await {
                    let bp = Arc::clone(&block_provider);
                    let block: TB = match tokio::task::spawn_blocking(move || bp.process_block(&raw)).await {
                        Ok(Ok(b)) => b,
                        Ok(Err(e)) => {
                            error!("process: failed to process block due to {}", e);
                            return;
                        }
                        Err(join_err) => {
                            error!("process: failed to process block due to {}", join_err);
                            return;
                        }
                    };
                    weight += block.weight() as usize;
                    batch.push(block);

                    if weight >= min_weight {
                        if proc_tx.send((std::mem::take(&mut batch), weight)).await.is_err() {
                            break;
                        }
                        weight = 0;
                    }
                }
                if !batch.is_empty() {
                    let _ = proc_tx.send((batch, weight)).await;
                }
                drop(proc_tx);
            })
        };

        let persist_handle = task::spawn_named("persist",async move {
            while let Some((block_batch, batch_weight)) = proc_rx.recv().await {
                let chain_link: bool = block_batch.last().is_some_and(|curr_block| curr_block.header().height() + 100 > chain_tip_for_persist.height());

                let last_block = block_batch.last().expect("Block batch should not be empty");
                let height = last_block.header().height();
                let timestamp = last_block.header().timestamp();

                monitor.log(height, timestamp, block_batch.len(), &batch_weight, proc_rx.len());
                // Move blocking IO off the async executor
                let block_provider = Arc::clone(&block_provider);
                let persistence = Arc::clone(&persistence);
                match tokio::task::spawn_blocking(move || Self::persist_blocks(block_batch, chain_link, block_provider, persistence)).await {
                    Ok(Ok(())) => {},
                    Ok(Err(e)) => {
                        error!("persist: persist_blocks returned error {}", e);
                        return;
                    }
                    Err(join_err) => {
                        error!("persist: spawn_blocking panicked {}", join_err);
                        return;
                    }
                }
            }
        });
        if let Err(e) = tokio::try_join!(fetch_handle, process_handle, persist_handle) {
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

    pub fn persist_blocks(blocks: Vec<TB>, do_chain_link: bool, block_provider: Arc<dyn BlockProvider<FB, TB>>, block_persistence: Arc<dyn BlockPersistence<TB>>) -> Result<(), ChainSyncError> {
        for block in blocks {
            // consume each block by value, build its chain
            let chain = if do_chain_link { Self::chain_link(block, Arc::clone(&block_provider), Arc::clone(&block_persistence))? } else { vec![block] };

            match chain.len() {
                0 => unreachable!("chain_link never returns empty Vec"),
                1 => block_persistence.store_blocks(chain)?,
                _ => block_persistence.update_blocks(chain)?,
            }
        }
        Ok(())
    }
}
