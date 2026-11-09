use crate::config::Config;
use crate::model::*;
use crate::{coverage, dup, fsutil, gate, newcode, rules};
use anyhow::Result;
use std::path::Path;

pub fn scan(src: &Path, rules_root: &Path, opengrep: &str) -> Result<ScanResult> {
    let cfg = Config::load(src)?;
    let (commit, branch) = newcode::head_info(src)?;
    let exclude = fsutil::globset(&cfg.scan.exclude)?;
    let tests = fsutil::globset(&cfg.scan.test_paths)?;
    let all = fsutil::walk(src, &exclude)?;
    let (code, excluded): (Vec<String>, Vec<String>) = all.iter().filter(|p| Lang::of(p).is_some()).cloned()
        .partition(|p| !exclude.is_match(p.as_str()));
    let prod: Vec<String> = code.iter().filter(|p| !tests.is_match(p.as_str())).cloned().collect();

    let mut rule_paths = vec![rules_root.to_path_buf()];
    rule_paths.extend(rules::extra_rule_paths(src, &cfg.rules.extra_paths)?);
    let outcome = rules::run(&rules::RunOpts {
        opengrep: opengrep.to_string(),
        src: src.to_path_buf(),
        rule_paths,
        exclude: cfg.scan.exclude.clone(),
        disabled: cfg.rules.disabled.clone(),
        code: code.clone(),
    })?;

    let d = dup::detect(src, &prod, &cfg.duplication);
    let loaded = coverage::load(src, &all, &code, &excluded, &tests, &cfg.coverage);
    for e in &loaded.error_details { eprintln!("경고: 커버리지 리포트를 읽지 못했습니다: {e}"); }

    let changed = match newcode::changed(src, cfg.new_code.days) {
        Ok(c) => c,
        Err(e) => { eprintln!("경고: 신규 코드를 계산하지 못했습니다: {e:#}"); None }
    };
    let has_cov = !loaded.reports.is_empty();
    let metrics = Metrics {
        coverage: gate::coverage_pct(&loaded.map, None),
        new_coverage: changed.as_ref().filter(|_| has_cov).and_then(|c| gate::coverage_pct(&loaded.map, Some(c))),
        duplication: gate::duplication(&d.clones, &d.total_lines, None),
        new_duplication: changed.as_ref().and_then(|c| gate::duplication(&d.clones, &d.total_lines, Some(c))),
        new_blocking: gate::blocking_on_changed(&outcome.findings, changed.as_ref(), &cfg.gate.blocking_severities),
    };
    let required_missing = cfg.coverage.required && loaded.map.is_empty();
    let gate_result = gate::evaluate(&cfg.gate, &metrics, required_missing);

    let not_in_report: Vec<String> = if has_cov {
        prod.iter().filter(|p| !loaded.map.contains_key(p.as_str())).cloned().collect()
    } else { vec![] };
    let mut unanalyzed = outcome.unanalyzed;
    unanalyzed.extend(d.unanalyzed);
    unanalyzed.sort();
    unanalyzed.dedup();

    Ok(ScanResult {
        schema: SCHEMA, commit, branch,
        findings: outcome.findings,
        clones: d.clones,
        metrics,
        coverage_status: CoverageStatus { reports: loaded.reports, unresolved: loaded.unresolved, not_in_report },
        gate: gate_result,
        unanalyzed,
        publish: PublishConfig { max_new_issues: cfg.publish.max_new_issues, labels: cfg.publish.labels },
    })
}
