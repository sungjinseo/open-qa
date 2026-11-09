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
}
