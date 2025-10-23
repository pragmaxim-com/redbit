use crate::api::Height;
use crate::settings::Parallelism;
use crate::size_batcher::SizeBatcher;
use crate::{combine, BlockHeaderLike, BlockLike, ChainError, SizeLike};
use redbit::{info, StreamExt};
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

#[async_trait::async_trait]
pub trait RestClient<CBOR> {
    async fn get_block_by_height(&self, height: Height) -> Result<CBOR, ChainError>;
}

pub trait BlockStream<FB: SizeLike, TB: BlockLike>: Send + Sync {
    fn stream(
        &self,
        remote_chain_tip_header: TB::Header,
        last_persisted_header: Option<TB::Header>,
        shutdown: watch::Receiver<bool>,
        batch: bool,
    ) -> (Receiver<Vec<FB>>, JoinHandle<()>);
}

pub struct RestBlockStream<C, FB, TB>
where
    C: RestClient<FB>,
    FB: SizeLike + Send + 'static,
    TB: BlockLike,
{
    pub client: Arc<C>,
    pub fetching_par: Parallelism,
    pub max_entity_buffer_kb_size: usize,
    pub phantom_f: std::marker::PhantomData<FB>,
    pub phantom_t: std::marker::PhantomData<TB>,
}

impl<C: RestClient<FB>, FB: SizeLike, TB: BlockLike> RestBlockStream<C, FB, TB> {
    pub fn new(client: Arc<C>, fetching_par: Parallelism, max_entity_buffer_kb_size: usize) -> Self {
        RestBlockStream {
            client,
            fetching_par,
            max_entity_buffer_kb_size,
            phantom_f: std::marker::PhantomData,
            phantom_t: std::marker::PhantomData,
        }
    }
}

impl<C, FB, TB> BlockStream<FB, TB> for RestBlockStream<C, FB, TB>
where
    C: RestClient<FB> + Send + Sync + 'static,
    FB: SizeLike + Send + 'static,
    TB: BlockLike + 'static,
    TB::Header: Send + Sync + 'static,
{
    fn stream(
        &self,
        remote_chain_tip_header: TB::Header,
        last_persisted_header: Option<TB::Header>,
        shutdown: watch::Receiver<bool>,
        batch: bool,
    ) -> (Receiver<Vec<FB>>, JoinHandle<()>) {
        let height_to_index_from = last_persisted_header.map_or(1, |h| h.height() + 1);
        let heights = height_to_index_from..=remote_chain_tip_header.height();
        let client = Arc::clone(&self.client);
        let buffer_size = 64;
        let fetching_par = self.fetching_par.0;
        let min_batch_kb_size = core::cmp::max(self.max_entity_buffer_kb_size, 256) / buffer_size;
        let (tx, rx) = mpsc::channel::<Vec<FB>>(buffer_size);

        let handle = tokio::spawn(async move {
            // Producer: fetch blocks with the same concurrency semantics you had.
            let s = tokio_stream::iter(heights).map(move |height| {
                let client = Arc::clone(&client);
                async move {
                    client
                        .get_block_by_height(height)
                        .await
                        .expect("Failed to fetch block by height")
                }
            });

            let mut batcher = SizeBatcher::<FB>::from_kb(min_batch_kb_size, !batch);

            let mut s = if batch {
                s.buffer_unordered(fetching_par).boxed()
            } else {
                s.buffered(fetching_par).boxed()
            };

            loop {
                tokio::select! {
                    biased;
                    _ = combine::await_shutdown(shutdown.clone()) => {
                        info!("fetch: shutdown");
                        if let Some(tail) = batcher.take_all() { let _ = tx.send(tail).await; }
                        break;
                    }
                    next = s.next() => {
                        match next {
                            Some(btc_cbor) => {
                                if let Some(batch) = batcher.push(btc_cbor) {
                                    if tx.send(batch).await.is_err() { break; }
                                }
                            }
                            None => {
                                if let Some(tail) = batcher.take_all() { let _ = tx.send(tail).await; }
                                break;
                            }
                        }
                    }
                }
            }
        });

        (rx, handle)
    }
}
