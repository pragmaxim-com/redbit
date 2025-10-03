use std::collections::hash_map::Entry;
use std::collections::HashMap;
use redbit::TaskResult;

#[derive(Clone, Debug, PartialEq)]
pub struct ReportRow {
    pub name: String,
    pub last: u128,
    pub avg:  u128,
    pub dev:  u128,
    pub cv:   f64,   // percent
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report(pub Vec<ReportRow>);

impl Report {
    pub fn printable(&mut self, buffer_size: usize, name_width: usize) -> String {
        self.0.sort_by(|a, b| b.last.cmp(&a.last));
        let mut lines = Vec::with_capacity(self.0.len() + 1);
        lines.push(format!(
            "{:<name_width$}  {:>10}  {:>10}  {:>10}  {:>7}",
            "TASK", "last ms", "avg ms", "dev ms", "cv %", name_width = name_width
        ));

        for r in self.0.iter() {
            lines.push(format!(
                "{:<name_width$}  {:>10}  {:>10}  {:>10}  {:>7.1}",
                r.name, r.last, r.avg, r.dev, r.cv, name_width = name_width
            ));
        }
        format!("Persist_buffer {} and writing task performance :\n{}", buffer_size, lines.join("\n"))
    }
}

#[derive(Default)]
pub struct TaskStats {
    pub totals: HashMap<String, u128>,   // running sum per task
    pub sumsqs: HashMap<String, f64>, // running sum of squares per task
    pub iters:  u64,         // how many times we've updated
}

impl TaskStats {
    pub fn update(&mut self, batch: &HashMap<String, TaskResult>) {
        self.iters = self.iters.saturating_add(1);
        for tr in batch.values() {
            match self.totals.entry(tr.name.clone()) {
                Entry::Vacant(e) => { e.insert(tr.took); }
                Entry::Occupied(mut e) => { *e.get_mut() = e.get().saturating_add(tr.took); }
            }
            let sq = (tr.took as f64) * (tr.took as f64);
            *self.sumsqs.entry(tr.name.clone()).or_insert(0.0) += sq;
        }
    }

    pub fn build_report(&self, batch: &HashMap<String, TaskResult>) -> Report {
        let iters_u64  = self.iters;
        let iters_u128 = iters_u64 as u128;
        let iters_f64  = iters_u64 as f64;

        let mut rows = Vec::with_capacity(batch.len());
        for tr in batch.values() {
            let total = *self.totals.get(&tr.name).unwrap_or(&0);
            let sumsq = *self.sumsqs.get(&tr.name).unwrap_or(&0.0);

            let avg_u128 = if iters_u128 == 0 { 0 } else { total / iters_u128 };

            // population stddev
            let mean_f = if iters_f64 == 0.0 { 0.0 } else { (total as f64) / iters_f64 };
            let ex2    = if iters_f64 == 0.0 { 0.0 } else { sumsq / iters_f64 };
            let var    = (ex2 - mean_f * mean_f).max(0.0);
            let dev_f  = var.sqrt();
            let dev_u128 = dev_f.round() as u128;

            let cv_pct = if mean_f > 0.0 { 100.0 * (dev_f / mean_f) } else { 0.0 };

            rows.push(ReportRow {
                name: tr.name.clone(),
                last: tr.took,
                avg:  avg_u128,
                dev:  dev_u128,
                cv:   cv_pct,
            });
        }
        Report(rows)
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use super::*;

    fn tr(name: &str, took: u128) -> TaskResult {
        TaskResult { name: name.to_string(), took }
    }

    fn batch(pairs: &[(&str, u128)]) -> HashMap<String, TaskResult> {
        let mut m = HashMap::with_capacity(pairs.len());
        for (n, t) in pairs {
            m.insert((*n).to_string(), tr(n, *t));
        }
        m
    }

    #[test]
    fn update_stats_accumulates_and_counts() {
        let mut s = TaskStats::default();
        let b1 = batch(&[("flush", 10), ("index", 5)]);
        s.update(&b1);
        assert_eq!(s.iters, 1);
        assert_eq!(s.totals.get("flush").copied(), Some(10));
        assert_eq!(s.totals.get("index").copied(), Some(5));

        let b2 = batch(&[("flush", 7), ("index", 13)]);
        s.update(&b2);
        assert_eq!(s.iters, 2);
        assert_eq!(s.totals.get("flush").copied(), Some(17));
        assert_eq!(s.totals.get("index").copied(), Some(18));

        // sumsqs sanity
        let flush_sq = 10f64*10f64 + 7f64*7f64;
        let index_sq = 5f64*5f64 + 13f64*13f64;
        assert!((s.sumsqs.get("flush").unwrap() - flush_sq).abs() < 1e-12);
        assert!((s.sumsqs.get("index").unwrap() - index_sq).abs() < 1e-12);
    }

    #[test]
    fn compute_rows_constant_latency_zero_dev_and_cv() {
        let mut s = TaskStats::default();
        let b1 = batch(&[("flush", 10)]);
        s.update(&b1);
        s.update(&b1);
        // iters = 2; total = 20; sumsqs = 2 * 100

        let report = s.build_report(&b1);
        assert_eq!(report.0.len(), 1);
        let r = &report.0[0];
        assert_eq!(r.name, "flush");
        assert_eq!(r.last, 10);
        assert_eq!(r.avg, 10);
        assert_eq!(r.dev, 0);
        assert!((r.cv - 0.0).abs() < 1e-12);
    }

    #[test]
    fn compute_rows_alternating_0_20_gives_avg10_dev10_cv100pct() {
        let mut s = TaskStats::default();
        let b0 = batch(&[("index", 0)]);
        let b1 = batch(&[("index", 20)]);
        s.update(&b0);
        s.update(&b1);

        let report = s.build_report(&b1); // last seen is 20
        let r = report.0.iter().find(|r| r.name == "index").unwrap();
        assert_eq!(r.avg, 10);
        assert_eq!(r.dev, 10);
        assert!((r.cv - 100.0).abs() < 1e-9);
        assert_eq!(r.last, 20);
    }

    #[test]
    fn build_report_formats_and_sorts() {
        let mut s = TaskStats::default();
        let b = batch(&[("a", 5), ("z", 30), ("m", 10)]);
        s.update(&b); // iters=1

        let mut report = s.build_report(&b);
        let text = report.printable(42, 12);
        assert!(text.contains("Persist_buffer 42"));
        // sorted by last desc: z(30), m(10), a(5)
        let zpos = text.find("\nz").unwrap_or(usize::MAX);
        let mpos = text.find("\nm").unwrap_or(usize::MAX);
        let apos = text.find("\na").unwrap_or(usize::MAX);
        assert!(zpos < mpos && mpos < apos, "rows not sorted by last desc:\n{}", text);
        // header columns present
        assert!(text.contains("TASK") && text.contains("last ms") && text.contains("avg ms") && text.contains("dev ms") && text.contains("cv %"));
    }
}
