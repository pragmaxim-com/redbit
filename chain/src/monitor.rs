use crate::{BlockHeaderLike, BlockLike};
use redbit::storage::table_writer_api::TaskResult;
use redbit::{info, warn};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use crate::stats::TaskStats;

pub type BatchWeight = usize;
pub type BoxWeight = usize;

pub struct ProgressMonitor<B: BlockLike> {
    weight_report_interval: usize,
    start_time: Instant,
    task_stats: Mutex<TaskStats>,
    total_and_last_report_weight: Mutex<(usize, usize)>,
    phantom: std::marker::PhantomData<B>,
}

impl<B: BlockLike> ProgressMonitor<B> {
    pub fn new(weight_report_interval: usize) -> Self {
        ProgressMonitor {
            weight_report_interval,
            start_time: Instant::now(),
            task_stats: Mutex::new(TaskStats::default()),
            total_and_last_report_weight: Mutex::new((0, 0)),
            phantom: std::marker::PhantomData,
        }
    }

    pub fn warn_gap(&self, need_height: u32, seen_height: u32, pending_heights: usize) {
        warn!("Block @ {} not fetched, currently @ {} ... pending {} blocks", need_height, seen_height, pending_heights);
    }

    pub fn log_batch(&self, batch: &Vec<B>, buffer_size: usize) {
        if let Some(first) = batch.first() {
            let batch_weight = batch.iter().map(|x| x.header().weight() as usize).sum::<usize>();
            let lh = first.header();
            let mut total_weight = self.total_and_last_report_weight.lock().unwrap();
            let new_total_weight = total_weight.0 + batch_weight;
            if new_total_weight > total_weight.1 + self.weight_report_interval {
                let height = lh.height();
                let timestamp = &lh.timestamp().to_string();
                let hash = &lh.hash().to_string();

                *total_weight = (new_total_weight, new_total_weight);
                let total_time = self.start_time.elapsed().as_secs();
                let txs_per_sec = format!("{:.1}", new_total_weight as f64 / total_time as f64);
                info!(
                    "Batch[{}] @ {} : {} from {} at {} ins+outs+assets/s, total {}, proc_buffer {}",
                    batch.len(), height, &hash[..12], timestamp, txs_per_sec, new_total_weight, buffer_size
                );
            } else {
                *total_weight = (new_total_weight, total_weight.1);
            }
        }
    }

    pub fn log_task_results(&self, tasks_by_name: HashMap<String, TaskResult>, buffer_size: usize) {
        let mut s = self.task_stats.lock().expect("stats poisoned");
        s.update(&tasks_by_name);

        if s.iters % 100 != 0 {
            return;
        } else {
            let mut report = s.build_report(&tasks_by_name);
            let report = report.printable(buffer_size);
            info!("Task report:\n{}", report);
        }
    }
}
