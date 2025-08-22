use crate::api::BlockLike;
use crate::settings::IndexerSettings;
use crate::syncer::ChainSyncer;
use redbit::error;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;

pub struct Scheduler<FB: Send + Sync + 'static, TB: BlockLike + 'static> {
    pub syncer: Arc<ChainSyncer<FB, TB>>,
}

impl<FB: Send + Sync + 'static, TB: BlockLike + 'static> Scheduler<FB, TB> {
    pub fn new(syncer: Arc<ChainSyncer<FB, TB>>) -> Self {
        Scheduler { syncer }
    }

    pub async fn schedule(&self, indexer_conf: &IndexerSettings, mut shutdown: watch::Receiver<bool>) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    println!("Shutting down syncer ...");
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
