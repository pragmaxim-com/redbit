use crate::stats::TaskAcc;
use crate::{BlockHeaderLike, BlockLike};
use redbit::info;
use redbit::storage::table_writer_api::TaskResult;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;
use redb::Durability;

pub type BatchWeight = usize;
pub type BoxWeight = usize;

pub struct ProgressMonitor<B: BlockLike> {
    start_time: Instant,
    task_stats: RefCell<TaskAcc>,
    total_weight: RefCell<usize>,
    phantom: std::marker::PhantomData<B>,
}

impl<B: BlockLike> ProgressMonitor<B> {
    pub fn new() -> Self {
        ProgressMonitor {
            start_time: Instant::now(),
            task_stats: RefCell::new(TaskAcc::default()),
            total_weight: RefCell::new(0),
            phantom: std::marker::PhantomData,
        }
    }

    pub fn log_batch(&self, batch: &Vec<B>, durability: Durability, buffer_size: usize) {
        if let Some(first) = batch.first() {
            let batch_weight = batch.iter().map(|x| x.header().weight() as usize).sum::<usize>();
            let total_weight_before = self.total_weight.replace_with(|old| *old + batch_weight);
            let total_weight_now = total_weight_before + batch_weight;
            let lh = first.header();
            let height = lh.height();
            let timestamp = &lh.timestamp().to_string();
            let hash = &lh.hash().to_string();
            let total_time = self.start_time.elapsed().as_secs();
            let txs_per_sec = format!("{:.1}", total_weight_now as f64 / total_time as f64);
            info!(
                "Batch[{}] @ {} : {} from {} at {} ins+outs+assets/s, total {}, durability: {:?}, proc_buffer {}",
                batch.len(), height, &hash[..12], timestamp, txs_per_sec, total_weight_now, durability, buffer_size
            );
        }
    }

    pub fn log_task_results(&self, tasks_by_name: HashMap<String, TaskResult>) {
        let mut s = self.task_stats.borrow_mut();
        s.update(&tasks_by_name);

        if s.iters % 10 != 0 {
            return;
        } else {
            let mut report = s.build_report(&tasks_by_name);
            let report = report.printable();
            info!("Task report:\n{}", report);
        }
    }
}
