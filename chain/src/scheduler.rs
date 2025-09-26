use crate::api::{BlockLike, SizeLike};
use crate::settings::IndexerSettings;
use crate::syncer::ChainSyncer;
use redbit::{error, info, WriteTxContext};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;

pub struct Scheduler<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext + 'static> {
    pub syncer: Arc<ChainSyncer<FB, TB, CTX>>,
}

impl<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext + 'static> Scheduler<FB, TB, CTX> {
    pub fn new(syncer: Arc<ChainSyncer<FB, TB, CTX>>) -> Self {
        Scheduler { syncer }
    }

    pub async fn schedule(&self, indexer_conf: &IndexerSettings, mut shutdown: watch::Receiver<bool>) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    info!("Shutting down syncer ...");
                    break;
                }
                _ = interval.tick() => {
                    match self.syncer.sync(indexer_conf, None, shutdown.clone()).await {
                        Ok(_) => {},
                        Err(e) => error!("Sync failed: {:?}", e),
                    }
                }
           }
        }
    }
}
