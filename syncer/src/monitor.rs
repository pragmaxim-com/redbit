use std::{time::Instant};

use std::sync::Mutex;
use redbit::info;

pub type BatchWeight = usize;
pub type BoxWeight = usize;

pub struct ProgressMonitor {
    min_weight_report: usize,
    start_time: Instant,
    total_and_last_report_weight: Mutex<(usize, usize)>,
}

impl ProgressMonitor {
    pub fn new(min_tx_count_report: usize) -> Self {
        ProgressMonitor {
            min_weight_report: min_tx_count_report,
            start_time: Instant::now(),
            total_and_last_report_weight: Mutex::new((0, 0)),
        }
    }

    pub fn log(
        &self,
        height: u32,
        timestamp: &str,
        hash: &str,
        batch_weight: &BatchWeight,
        buffer_size: usize,
    ) {
        let mut total_weight = self.total_and_last_report_weight.lock().unwrap();
        let new_total_weight = total_weight.0 + batch_weight;
        if new_total_weight > total_weight.1 + self.min_weight_report {
            *total_weight = (new_total_weight, new_total_weight);
            let total_time = self.start_time.elapsed().as_secs();
            let txs_per_sec = format!("{:.1}", new_total_weight as f64 / total_time as f64);
            info!(
                "{} @ {} from {} at {} ins+outs+assets/s, total {}, proc_buffer {}",
                &hash[..12], height, timestamp, txs_per_sec, new_total_weight, buffer_size
            );
        } else {
            *total_weight = (new_total_weight, total_weight.1);
        }
    }
}
