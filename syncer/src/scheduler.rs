use crate::api::{BlockLike, BlockPersistence, BlockProvider};
use crate::monitor::ProgressMonitor;
use crate::settings::IndexerSettings;
use crate::syncer::ChainSyncer;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time;

pub struct Scheduler<FB: Send + Sync + 'static, TB: BlockLike + 'static> {
    pub syncer: ChainSyncer<FB, TB>,
}

impl<FB: Send + Sync + 'static, TB: BlockLike + 'static> Scheduler<FB, TB> {
    pub fn new(block_provider: Arc<dyn BlockProvider<FB, TB>>, block_persistence: Arc<dyn BlockPersistence<TB>>) -> Self {
        let syncer = ChainSyncer { block_provider, block_persistence, monitor: Arc::new(ProgressMonitor::new(1000)) };
        Scheduler { syncer }
    }

    pub async fn sync(&self, indexer_conf: IndexerSettings) {
        self.syncer.sync(indexer_conf.clone()).await;
    }

    pub async fn schedule(&self, indexer_conf: IndexerSettings, mut shutdown: watch::Receiver<bool>) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    println!("Shutting down syncer ...");
                    break;
                }
                _ = interval.tick() => {
                    self.syncer.sync(indexer_conf.clone()).await;
                }
           }
        }
    }
}
