use redbit::TaskResult;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct ReportRow {
    pub name: String,
    pub collect: ReportData,
    pub sort: ReportData,
    pub write: ReportData,
    pub flush: ReportData,
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
    pub fn printable(&mut self) -> String {
        fn sum_last(r: &ReportRow) -> u128 {
            r.collect.last + r.sort.last + r.write.last + r.flush.last
        }
        self.0.sort_by(|a, b| sum_last(b).cmp(&sum_last(a)));

        // ---- formatting constants ----
        const NAME_WIDTH: usize = 40; // (1)
        const NUM_WIDTH: usize = 5;
        const GAP: &str = "  ";
        const SEP: &str = " | ";

        fn plus_eq_fmt(vals: &[u128]) -> String {
            format!(
                "{:>w$}{g}{:>w$}{g}{:>w$}{g}{:>w$} = {:>w$}",
                vals[0], vals[1], vals[2], vals[3],
                vals[0] + vals[1] + vals[2] + vals[3],
                w = NUM_WIDTH, g = GAP
            )
        }

        // (3) header shape with custom label in the last cell
        fn plus_eq_header(label: &str) -> String {
            format!(
                "{:>w$}{g}{:>w$}{g}{:>w$}{g}{:>w$} = {:>w$}",
                "c", "s", "w", "f", label,
                w = NUM_WIDTH, g = GAP
            )
        }

        // build both composite headers and size the column to the maximum to guarantee fit
        let header_last = plus_eq_header("last");
        let header_avg  = plus_eq_header("avg");
        let sum_col_w = header_last.len().max(header_avg.len());

        let mut lines = Vec::with_capacity(self.0.len() + 1);

        // (2) single header line with both shaped columns, separated by |
        lines.push(format!(
            "{:<name_w$}{sep}{:>sum_w$}{sep}{:>sum_w$}{sep}{:>8}{sep}{:>8}",
            "TASK (c)ollect,(s)ort,(w)rite,(f)lush ms",
            header_last,
            header_avg,
            "dev",
            "coefov %",
            name_w = NAME_WIDTH,
            sum_w = sum_col_w,
            sep = SEP
        ));

        for r in self.0.iter() {
            let lasts = [r.collect.last, r.sort.last, r.write.last, r.flush.last];
            let avgs  = [r.collect.avg,  r.sort.avg,  r.write.avg,  r.flush.avg];
            let devs  = [r.collect.dev,  r.sort.dev,  r.write.dev,  r.flush.dev];
            let cvs   = [r.collect.cv,   r.sort.cv,   r.write.cv,   r.flush.cv];

            lines.push(format!(
                "{:<name_w$}{sep}{:>sum_w$}{sep}{:>sum_w$}{sep}{:>8}{sep}{:>8}",
                r.name,
                plus_eq_fmt(&lasts),
                plus_eq_fmt(&avgs),
                devs.iter().copied().sum::<u128>(),
                cvs.iter().map(|v| v.round() as u128).sum::<u128>(),
                name_w = NAME_WIDTH,
                sum_w = sum_col_w,
                sep = SEP
            ));
        }
        format!("Task performance :\n{}", lines.join("\n"))
    }
}

#[derive(Default)]
pub struct TaskAcc {
    // one map per phase, kept in a compact struct to avoid duplication elsewhere
    write_totals: HashMap<String, u128>,
    write_sumsqs: HashMap<String, f64>,
    flush_totals: HashMap<String, u128>,
    flush_sumsqs: HashMap<String, f64>,
    collect_totals: HashMap<String, u128>,
    collect_sumsqs: HashMap<String, f64>,
    sort_totals: HashMap<String, u128>,
    sort_sumsqs: HashMap<String, f64>,
    pub iters:  u64,
}

impl TaskAcc {
    fn update_phase_maps(totals: &mut HashMap<String, u128>, sumsqs: &mut HashMap<String, f64>, name: &str, took: u128) {
        match totals.entry(name.to_string()) {
            Entry::Vacant(e) => { e.insert(took); }
            Entry::Occupied(mut e) => { *e.get_mut() = e.get().saturating_add(took); }
        }
        let sq = (took as f64) * (took as f64);
        *sumsqs.entry(name.to_string()).or_insert(0.0) += sq;
        // O(1) expected per update (hash operations), per phase
    }

    pub fn update(&mut self, batch: &HashMap<String, TaskResult>) {
        self.iters = self.iters.saturating_add(1);

        for tr in batch.values() {
            // NOTE: this assumes TaskResult now carries collect_took and sort_took.
            // If not yet available, set them to 0 at call site or extend TaskResult accordingly.
            Self::update_phase_maps(&mut self.collect_totals, &mut self.collect_sumsqs, &tr.name, tr.stats.collect_took);
            Self::update_phase_maps(&mut self.sort_totals,    &mut self.sort_sumsqs,    &tr.name, tr.stats.sort_took);
            Self::update_phase_maps(&mut self.write_totals,   &mut self.write_sumsqs,   &tr.name, tr.stats.write_took);
            Self::update_phase_maps(&mut self.flush_totals,   &mut self.flush_sumsqs,   &tr.name, tr.stats.flush_took);
        }
    }

    pub fn build_data(&self, took: u128, total: u128, sumsq: f64) -> ReportData {
        let iters_u64  = self.iters;
        let iters_u128 = iters_u64 as u128;
        let iters_f64  = iters_u64 as f64;

        // O(1)
        let mean_f = if iters_f64 == 0.0 { 0.0 } else { (total as f64) / iters_f64 };
        let ex2    = if iters_f64 == 0.0 { 0.0 } else { sumsq / iters_f64 };
        let var    = (ex2 - mean_f * mean_f).max(0.0);
        let dev_f  = var.sqrt();

        ReportData {
            last: took,
            avg: if iters_u128 == 0 { 0 } else { total / iters_u128 },
            dev: dev_f.round() as u128,
            cv:  if mean_f > 0.0 { 100.0 * (dev_f / mean_f) } else { 0.0 },
        }
    }

    pub fn build_report(&self, batch: &HashMap<String, TaskResult>) -> Report {
        let mut rows: Vec<ReportRow> = Vec::with_capacity(batch.len());

        for tr in batch.values() {
            let (c_tot, c_sq) = (
                *self.collect_totals.get(&tr.name).unwrap_or(&0),
                *self.collect_sumsqs.get(&tr.name).unwrap_or(&0.0),
            );
            let (s_tot, s_sq) = (
                *self.sort_totals.get(&tr.name).unwrap_or(&0),
                *self.sort_sumsqs.get(&tr.name).unwrap_or(&0.0),
            );
            let (w_tot, w_sq) = (
                *self.write_totals.get(&tr.name).unwrap_or(&0),
                *self.write_sumsqs.get(&tr.name).unwrap_or(&0.0),
            );
            let (f_tot, f_sq) = (
                *self.flush_totals.get(&tr.name).unwrap_or(&0),
                *self.flush_sumsqs.get(&tr.name).unwrap_or(&0.0),
            );

            let collect = self.build_data(tr.stats.collect_took, c_tot, c_sq);
            let sort    = self.build_data(tr.stats.sort_took,    s_tot, s_sq);
            let write   = self.build_data(tr.stats.write_took,   w_tot, w_sq);
            let flush   = self.build_data(tr.stats.flush_took,   f_tot, f_sq);

            rows.push(ReportRow {
                name: tr.name.clone(),
                collect, sort, write, flush,
            });
        }
        Report(rows)
    }
}

#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use super::*;
    use redbit::storage::table_writer_api::TaskStats;

    // ----- helpers (keep minimal changes) -----

    fn tr(name: &str, collect_took: u128, sort_took: u128, write_took: u128, flush_took: u128) -> TaskResult {
        TaskResult::new(name, TaskStats::new(collect_took, sort_took, write_took, flush_took))
    }

    fn batch_equal_phases(pairs: &[(&str, u128)]) -> HashMap<String, TaskResult> {
        let mut m = HashMap::with_capacity(pairs.len());
        for (n, t) in pairs {
            // use same took for all phases to keep old invariants/simple expectations where needed
            m.insert((*n).to_string(), tr(n, *t, *t, *t, *t));
        }
        m
    }

    #[test]
    fn update_stats_accumulates_and_counts() {
        let mut s = TaskAcc::default();
        let b1 = batch_equal_phases(&[("flush", 10), ("index", 5)]);
        s.update(&b1);
        assert_eq!(s.iters, 1);
        assert_eq!(s.write_totals.get("flush").copied(), Some(10));
        assert_eq!(s.write_totals.get("index").copied(), Some(5));

        let b2 = batch_equal_phases(&[("flush", 7), ("index", 13)]);
        s.update(&b2);
        assert_eq!(s.iters, 2);
        assert_eq!(s.write_totals.get("flush").copied(), Some(17));
        assert_eq!(s.write_totals.get("index").copied(), Some(18));

        // sumsqs sanity (write as exemplar)
        let flush_sq = 10f64*10f64 + 7f64*7f64;
        let index_sq = 5f64*5f64 + 13f64*13f64;
        assert!((s.write_sumsqs.get("flush").unwrap() - flush_sq).abs() < 1e-12);
        assert!((s.write_sumsqs.get("index").unwrap() - index_sq).abs() < 1e-12);
    }

    #[test]
    fn compute_rows_constant_latency_zero_dev_and_cv() {
        let mut s = TaskAcc::default();
        let b1 = batch_equal_phases(&[("flush", 10)]);
        s.update(&b1);
        s.update(&b1);
        // iters = 2; per-phase total = 20; per-phase sumsqs = 2 * 100

        let report = s.build_report(&b1);
        assert_eq!(report.0.len(), 1);
        let r = &report.0[0];
        assert_eq!(r.name, "flush");
        assert_eq!(r.write.last, 10);
        assert_eq!(r.write.avg, 10);
        assert_eq!(r.write.dev, 0);
        assert!((r.write.cv - 0.0).abs() < 1e-12);
        // spot-check new phases exist
        assert_eq!(r.collect.last, 10);
        assert_eq!(r.sort.last, 10);
    }

    #[test]
    fn compute_rows_alternating_0_20_gives_avg10_dev10_cv100pct() {
        let mut s = TaskAcc::default();
        let b0 = {
            let mut m = HashMap::new();
            m.insert("index".to_string(), tr("index", 0, 0, 0, 0));
            m
        };
        let b1 = {
            let mut m = HashMap::new();
            m.insert("index".to_string(), tr("index", 20, 20, 20, 20));
            m
        };
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
        // NOTE: rename to TaskStats if that's the concrete type in your module
        let mut s = TaskAcc::default();
        let b = {
            // vary only write to preserve previous test’s sorting intuition; other phases 0
            let mut m = HashMap::new();
            m.insert("a".to_string(), tr("a", 0, 0, 5, 0));
            m.insert("z".to_string(), tr("z", 0, 0, 30, 0));
            m.insert("m".to_string(), tr("m", 0, 0, 10, 0));
            m
        };
        s.update(&b); // iters=1

        let mut report = s.build_report(&b);
        let text = report.printable();

        // sorted by total last desc: z(30), m(10), a(5)
        let zpos = text.find("\nz").unwrap_or(usize::MAX);
        let mpos = text.find("\nm").unwrap_or(usize::MAX);
        let apos = text.find("\na").unwrap_or(usize::MAX);
        assert!(zpos < mpos && mpos < apos, "rows not sorted by last desc:\n{}", text);

        // header presence & shape (single line with separators and new title)
        assert!(
            text.contains("TASK (c)ollect,(s)ort,(w)rite,(f)lush"),
            "missing/changed header title:\n{}",
            text
        );
        assert!(
            text.contains(" | "),
            "expected ' | ' separators between columns:\n{}",
            text
        );
        // these labels must appear (only in the header now)
        assert!(text.contains("last"), "missing 'last' header:\n{}", text);
        assert!(text.contains("avg"),  "missing 'avg' header:\n{}", text);
        assert!(text.contains("dev"), "missing 'dev' header:\n{}", text);
        assert!(text.contains("coefov %"), "missing 'coefov %' header:\n{}", text);

        // sanity: no legacy markers from old format
        assert!(!text.contains("c+s+w+f"), "legacy header artifact present:\n{}", text);
        assert!(!text.contains("(c)collect"), "legacy header artifact present:\n{}", text);
    }
}
