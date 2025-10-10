use crate::stats::TaskAcc;
use crate::{BlockHeaderLike, BlockLike};
use redbit::info;
use redbit::storage::table_writer_api::TaskResult;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

pub type BatchWeight = usize;
pub type BoxWeight = usize;

pub struct ProgressMonitor<B: BlockLike> {
    weight_report_interval: usize,
    start_time: Instant,
    task_stats: RefCell<TaskAcc>,
    total_and_last_report_weight: RefCell<(usize, usize)>,
    phantom: std::marker::PhantomData<B>,
}

impl<B: BlockLike> ProgressMonitor<B> {
    pub fn new(weight_report_interval: usize) -> Self {
        ProgressMonitor {
            weight_report_interval,
            start_time: Instant::now(),
            task_stats: RefCell::new(TaskAcc::default()),
            total_and_last_report_weight: RefCell::new((0, 0)),
            phantom: std::marker::PhantomData,
        }
    }

    pub fn log_batch(&self, batch: &Vec<B>, buffer_size: usize) {
        if let Some(first) = batch.first() {
            let batch_weight = batch.iter().map(|x| x.header().weight() as usize).sum::<usize>();
            let mut total_weight = self.total_and_last_report_weight.borrow_mut();
            let new_total_weight = total_weight.0 + batch_weight;
            if new_total_weight > total_weight.1 + self.weight_report_interval {
                let lh = first.header();
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

    pub fn log_task_results(&self, tasks_by_name: HashMap<String, TaskResult>) {
        let mut s = self.task_stats.borrow_mut();
        s.update(&tasks_by_name);

        if s.iters % 100 != 0 {
            return;
        } else {
            let mut report = s.build_report(&tasks_by_name);
            let report = report.printable();
            info!("Task report:\n{}", report);
        }
    }
}
