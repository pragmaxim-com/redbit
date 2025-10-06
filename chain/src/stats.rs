use std::collections::hash_map::Entry;
use std::collections::HashMap;
use redbit::TaskResult;

#[derive(Clone, Debug, PartialEq)]
pub struct ReportRow {
    pub name: String,
    pub write: ReportData,
    pub commit: ReportData,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReportData {
    pub last: u128,
    pub avg:  u128,
    pub dev:  u128,
    pub cv:   f64,   // percent
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report(pub Vec<ReportRow>);

impl Report {
    pub fn printable(&mut self, buffer_size: usize) -> String {
        self.0.sort_by(|a, b| (b.commit.last + b.write.last).cmp(&(a.commit.last + a.write.last)));
        let mut lines = Vec::with_capacity(self.0.len() + 1);
        lines.push(format!(
            "{:<name_width$}  {:>16}  {:>16}  {:>16}  {:>14}",
            "TASK (c)commit, (w)rite", "c+w= last ms", "c+w= avg ms", "c/w dev ms", "c/w coefov %", name_width = 30
        ));

        fn slash_fmt(a: u128, b: u128) -> String {
            format!("{}/{}", a, b)
        }
        fn slash_eq_fmt(a: u128, b: u128) -> String {
            format!("{}+{}={}", a, b, a + b)
        }

        for r in self.0.iter() {
            lines.push(format!(
                "{:<name_width$}  {:>16}  {:>16}  {:>16}  {:>14}",
                r.name,
                slash_eq_fmt(r.commit.last, r.write.last),
                slash_eq_fmt(r.commit.avg, r.write.avg),
                slash_fmt(r.commit.dev, r.write.dev),
                slash_fmt(r.commit.cv.round() as u128, r.write.cv.round() as u128),
                name_width = 30
            ));
        }
        format!("Persist_buffer {} and writing task performance :\n{}", buffer_size, lines.join("\n"))
    }
}

#[derive(Default)]
pub struct TaskStats {
    pub write_totals: HashMap<String, u128>,   // running sum per task
    pub write_sumsqs: HashMap<String, f64>, // running sum of squares per task
    pub commit_totals: HashMap<String, u128>,   // running sum per task
    pub commit_sumsqs: HashMap<String, f64>, // running sum of squares per task
    pub iters:  u64,         // how many times we've updated
}

impl TaskStats {
    pub fn update(&mut self, batch: &HashMap<String, TaskResult>) {
        self.iters = self.iters.saturating_add(1);
        for tr in batch.values() {
            match self.write_totals.entry(tr.name.clone()) {
                Entry::Vacant(e) => { e.insert(tr.write_took); }
                Entry::Occupied(mut e) => { *e.get_mut() = e.get().saturating_add(tr.write_took); }
            }
            match self.commit_totals.entry(tr.name.clone()) {
                Entry::Vacant(e) => { e.insert(tr.commit_took); }
                Entry::Occupied(mut e) => { *e.get_mut() = e.get().saturating_add(tr.commit_took); }
            }
            let write_sq = (tr.write_took as f64) * (tr.write_took as f64);
            *self.write_sumsqs.entry(tr.name.clone()).or_insert(0.0) += write_sq;
            let commit_sq = (tr.commit_took as f64) * (tr.commit_took as f64);
            *self.commit_sumsqs.entry(tr.name.clone()).or_insert(0.0) += commit_sq;
        }
    }

    pub fn build_data(&self, took: u128, total: u128, sumsq: f64) -> ReportData {
        let iters_u64  = self.iters;
        let iters_u128 = iters_u64 as u128;
        let iters_f64  = iters_u64 as f64;
        let write_mean_f = if iters_f64 == 0.0 { 0.0 } else { (total as f64) / iters_f64 };
        let write_ex2 = if iters_f64 == 0.0 { 0.0 } else { sumsq / iters_f64 };
        let write_var = (write_ex2 - write_mean_f * write_mean_f).max(0.0);
        let write_dev_f = write_var.sqrt();
        ReportData {
            last: took,
            avg: if iters_u128 == 0 { 0 } else { total / iters_u128 },
            dev: write_dev_f.round() as u128,
            cv: if write_mean_f > 0.0 { 100.0 * (write_dev_f / write_mean_f) } else { 0.0 },
        }
    }


    pub fn build_report(&self, batch: &HashMap<String, TaskResult>) -> Report {
        let mut rows: Vec<ReportRow> = Vec::with_capacity(batch.len());
        for tr in batch.values() {
            let write_total = *self.write_totals.get(&tr.name).unwrap_or(&0);
            let write_sumsq = *self.write_sumsqs.get(&tr.name).unwrap_or(&0.0);
            let commit_total = *self.commit_totals.get(&tr.name).unwrap_or(&0);
            let commit_sumsq = *self.commit_sumsqs.get(&tr.name).unwrap_or(&0.0);
            let write = self.build_data(tr.write_took, write_total, write_sumsq);
            let commit = self.build_data(tr.commit_took, commit_total, commit_sumsq);
            rows.push(ReportRow {name: tr.name.clone(), write, commit });
        }
        Report(rows)
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use super::*;

    fn tr(name: &str, write_took: u128, commit_took: u128) -> TaskResult {
        TaskResult { name: name.to_string(), write_took, commit_took }
    }

    fn batch(pairs: &[(&str, u128)]) -> HashMap<String, TaskResult> {
        let mut m = HashMap::with_capacity(pairs.len());
        for (n, t) in pairs {
            m.insert((*n).to_string(), tr(n, *t, *t));
        }
        m
    }

    #[test]
    fn update_stats_accumulates_and_counts() {
        let mut s = TaskStats::default();
        let b1 = batch(&[("flush", 10), ("index", 5)]);
        s.update(&b1);
        assert_eq!(s.iters, 1);
        assert_eq!(s.write_totals.get("flush").copied(), Some(10));
        assert_eq!(s.write_totals.get("index").copied(), Some(5));

        let b2 = batch(&[("flush", 7), ("index", 13)]);
        s.update(&b2);
        assert_eq!(s.iters, 2);
        assert_eq!(s.write_totals.get("flush").copied(), Some(17));
        assert_eq!(s.write_totals.get("index").copied(), Some(18));

        // sumsqs sanity
        let flush_sq = 10f64*10f64 + 7f64*7f64;
        let index_sq = 5f64*5f64 + 13f64*13f64;
        assert!((s.write_sumsqs.get("flush").unwrap() - flush_sq).abs() < 1e-12);
        assert!((s.write_sumsqs.get("index").unwrap() - index_sq).abs() < 1e-12);
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
        assert_eq!(r.write.last, 10);
        assert_eq!(r.write.avg, 10);
        assert_eq!(r.write.dev, 0);
        assert!((r.write.cv - 0.0).abs() < 1e-12);
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
        assert_eq!(r.write.avg, 10);
        assert_eq!(r.write.dev, 10);
        assert!((r.write.cv - 100.0).abs() < 1e-9);
        assert_eq!(r.write.last, 20);
    }

    #[test]
    fn build_report_formats_and_sorts() {
        let mut s = TaskStats::default();
        let b = batch(&[("a", 5), ("z", 30), ("m", 10)]);
        s.update(&b); // iters=1

        let mut report = s.build_report(&b);
        let text = report.printable(42);
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
