use crate::config::GateCfg;
use crate::model::*;
use crate::newcode::ChangedLines;
use std::collections::{BTreeMap, BTreeSet};

pub fn coverage_pct(map: &CoverageMap, only: Option<&ChangedLines>) -> Option<f64> {
    let (mut total, mut covered) = (0u64, 0u64);
    for (file, lines) in map {
        for (n, c) in lines {
            if let Some(ch) = only {
                if !ch.get(file).is_some_and(|s| s.contains(n)) { continue; }
            }
            total += 1 + c.conditions as u64;
            covered += c.covered as u64 + c.covered_conditions as u64;
        }
    }
    (total > 0).then(|| 100.0 * covered as f64 / total as f64)
}

fn dup_lines(clones: &[CloneGroup]) -> BTreeMap<&str, BTreeSet<u32>> {
    let mut m: BTreeMap<&str, BTreeSet<u32>> = BTreeMap::new();
    for g in clones {
        for p in &g.parts {
            m.entry(p.file.as_str()).or_default().extend(p.start_line..=p.end_line);
        }
    }
    m
}

pub fn duplication(clones: &[CloneGroup], total_lines: &BTreeMap<String, u32>, only: Option<&ChangedLines>) -> Option<f64> {
    let dups = dup_lines(clones);
    let (mut total, mut dup) = (0u64, 0u64);
    match only {
        None => {
            total = total_lines.values().map(|v| *v as u64).sum();
            dup = dups.iter().filter(|(f, _)| total_lines.contains_key(**f)).map(|(_, s)| s.len() as u64).sum();
        }
        Some(ch) => for (f, lines) in ch {
            if !total_lines.contains_key(f) { continue; }
            total += lines.len() as u64;
            if let Some(s) = dups.get(f.as_str()) { dup += lines.intersection(s).count() as u64; }
        },
    }
    (total > 0).then(|| 100.0 * dup as f64 / total as f64)
}

pub fn blocking_on_changed(findings: &[Finding], changed: Option<&ChangedLines>, severities: &[String]) -> Option<u64> {
    let ch = changed?;
    let blocking: Vec<Severity> = severities.iter().filter_map(|s| Severity::parse(s)).collect();
    Some(findings.iter().filter(|f| blocking.contains(&f.severity) && ch.get(&f.file).is_some_and(|s| s.contains(&f.line))).count() as u64)
}

pub fn evaluate(cfg: &GateCfg, m: &Metrics, coverage_required_missing: bool) -> GateResult {
    let min = |name: &str, a: Option<f64>, limit: f64, force_fail: bool| Condition {
        name: name.into(), actual: a, limit,
        status: match a {
            _ if force_fail => ConditionStatus::Failed,
            None => ConditionStatus::Skipped,
            Some(v) if v >= limit => ConditionStatus::Passed,
            Some(_) => ConditionStatus::Failed,
        },
    };
    let max = |name: &str, a: Option<f64>, limit: f64| Condition {
        name: name.into(), actual: a, limit,
        status: match a {
            None => ConditionStatus::Skipped,
            Some(v) if v <= limit => ConditionStatus::Passed,
            Some(_) => ConditionStatus::Failed,
        },
    };
    let conditions = vec![
        min("new_coverage", m.new_coverage, cfg.new_coverage_min, coverage_required_missing),
        max("new_duplication", m.new_duplication, cfg.new_duplication_max),
        max("new_blocking", m.new_blocking.map(|v| v as f64), cfg.new_blocking_max as f64),
    ];
    GateResult { passed: conditions.iter().all(|c| c.status != ConditionStatus::Failed), conditions }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cov() -> CoverageMap {
        let mut m = CoverageMap::new();
        let f = m.entry("a".into()).or_default();
        f.insert(1, LineCov { covered: true, conditions: 2, covered_conditions: 1 });
        f.insert(2, LineCov { covered: false, conditions: 0, covered_conditions: 0 });
        m
    }

    fn only(lines: &[u32]) -> ChangedLines {
        let mut c = ChangedLines::new();
        c.insert("a".into(), lines.iter().copied().collect());
        c
    }

    #[test]
    fn coverage_counts_lines_and_conditions() {
        // (커버 줄 1 + 커버 조건 1) / (줄 2 + 조건 2) = 50%
        assert_eq!(coverage_pct(&cov(), None), Some(50.0));
    }

    #[test]
    fn new_coverage_limits_to_changed() {
        assert_eq!(coverage_pct(&cov(), Some(&only(&[2]))), Some(0.0));
        assert_eq!(coverage_pct(&cov(), Some(&only(&[9]))), None);
        assert_eq!(coverage_pct(&CoverageMap::new(), None), None);
    }

    fn clones() -> Vec<CloneGroup> {
        vec![CloneGroup { key: "k".into(), lines: 2, parts: vec![
            ClonePart { file: "a".into(), start_line: 1, end_line: 2 },
            ClonePart { file: "a".into(), start_line: 2, end_line: 3 },
        ]}]
    }

    #[test]
    fn duplication_counts_each_line_once() {
        let mut t = BTreeMap::new();
        t.insert("a".to_string(), 10u32);
        assert_eq!(duplication(&clones(), &t, None), Some(30.0));
        assert_eq!(duplication(&clones(), &BTreeMap::new(), None), None);
    }

    #[test]
    fn new_duplication_limits_to_changed() {
        let mut t = BTreeMap::new();
        t.insert("a".to_string(), 10u32);
        assert_eq!(duplication(&clones(), &t, Some(&only(&[3, 4]))), Some(50.0));
    }

    #[test]
    fn blocking_counts_changed_lines_only() {
        let f = |line, sev| Finding { rule: "r".into(), file: "a".into(), line, line_hash: "".into(), message: "".into(), severity: sev, category: "c".into() };
        let fs = vec![f(1, Severity::High), f(2, Severity::Low), f(3, Severity::Critical)];
        let sev = vec!["critical".to_string(), "high".to_string()];
        assert_eq!(blocking_on_changed(&fs, Some(&only(&[1, 2])), &sev), Some(1));
        assert_eq!(blocking_on_changed(&fs, None, &sev), None);
    }

    #[test]
    fn gate_passes_at_limits_and_skips_unmeasured() {
        let cfg = GateCfg::default();
        let m = Metrics { new_coverage: Some(80.0), new_duplication: Some(3.0), new_blocking: Some(0), ..Default::default() };
        assert!(evaluate(&cfg, &m, false).passed);
        let g = evaluate(&cfg, &Metrics::default(), false);
        assert!(g.passed);
        assert!(g.conditions.iter().all(|c| c.status == ConditionStatus::Skipped));
    }

    #[test]
    fn gate_fails_past_limits_and_when_required_missing() {
        let cfg = GateCfg::default();
        let m = Metrics { new_coverage: Some(79.9), ..Default::default() };
        assert!(!evaluate(&cfg, &m, false).passed);
        let m = Metrics { new_duplication: Some(3.1), ..Default::default() };
        assert!(!evaluate(&cfg, &m, false).passed);
        let m = Metrics { new_blocking: Some(1), ..Default::default() };
        assert!(!evaluate(&cfg, &m, false).passed);
        assert!(!evaluate(&cfg, &Metrics::default(), true).passed);
    }
}
