use crate::catalog::{Catalog, Kind};
use crate::model::*;
use crate::newcode::ChangedLines;
use crate::{coverage, dup, fsutil, gate, newcode, rules, select, settings};
use anyhow::{bail, Context, Result};
use std::path::Path;

/// 설정 디렉터리의 `source.json`. 없거나 JSON 객체가 아니면 오류다.
pub fn read_source(config_dir: &Path) -> Result<serde_json::Value> {
    let p = config_dir.join("source.json");
    let text = std::fs::read_to_string(&p).with_context(|| format!("{}을(를) 읽지 못했습니다. fetch-config가 성공했는지 확인하세요", p.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text).with_context(|| format!("{} 형식 오류", p.display()))?;
    if !v.is_object() { bail!("{} 형식 오류: JSON 객체가 아닙니다", p.display()); }
    Ok(v)
}

/// `source.json`의 대상 저장소 이름(`repo`).
pub fn source_repo(source: &serde_json::Value) -> Result<String> {
    match source.get("repo").and_then(|v| v.as_str()) {
        Some(r) if !r.is_empty() => Ok(r.to_string()),
        _ => bail!("source.json에 repo 값이 없습니다"),
    }
}

/// git 이력으로 신규 코드를 계산하고, 안 되면(얕은 클론 등) `newcode.json`을 쓴다.
/// `newcode.json`은 그 `head`가 스캔하는 커밋과 같을 때만 쓴다.
fn new_code(src: &Path, config_dir: &Path, days: u32, head: &str) -> Result<Option<ChangedLines>> {
    match newcode::changed(src, days) {
        Ok(Some(c)) => return Ok(Some(c)),
        Ok(None) => {}
        Err(e) => eprintln!("경고: git 이력으로 신규 코드를 계산하지 못했습니다: {e:#}"),
    }
    Ok(match newcode::read_newcode_json(&config_dir.join("newcode.json"))? {
        Some(n) if n.head == head => Some(n.changed_lines()),
        Some(n) => {
            eprintln!("경고: newcode.json의 커밋 {}이(가) 스캔한 커밋 {head}와 달라 신규 코드를 측정하지 않습니다", n.head);
            None
        }
        None => None,
    })
}

fn on_lines(f: &Finding, changed: &ChangedLines) -> bool { changed.get(&f.file).is_some_and(|s| s.contains(&f.line)) }

pub fn scan(src: &Path, config_dir: &Path, repo: &str, rules_root: &Path, opengrep: &str) -> Result<ScanResult> {
    if src.join(".qualgate.toml").symlink_metadata().is_ok() {
        eprintln!("경고: 저장소의 .qualgate.toml은 쓰지 않습니다. 설정 저장소의 설정으로 스캔합니다.");
    }
    let config_source = read_source(config_dir)?;
    let catalog = Catalog::load(rules_root)?;
    let res = settings::resolve(config_dir, repo, &catalog).map_err(|errs| {
        anyhow::anyhow!("설정 오류 {}건:\n{}", errs.len(), errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))
    })?;
    let p = &res.project;
    let (commit, branch) = newcode::head_info(src)?;
    let exclude = fsutil::globset(&p.scan.exclude)?;
    let tests = fsutil::globset(&p.scan.test_paths)?;
    let all = fsutil::walk(src, &exclude)?;
    let (code, excluded): (Vec<String>, Vec<String>) = all.iter().filter(|p| Lang::of(p).is_some()).cloned()
        .partition(|p| !exclude.is_match(p.as_str()));
    let prod: Vec<String> = code.iter().filter(|p| !tests.is_match(p.as_str())).cloned().collect();

    let dirs = select::active_dirs(&res.ruleset, p.languages.as_deref(), &code);
    let sel = select::select(rules_root, &catalog, &res.ruleset, &dirs)?;
    let mut outcome = if sel.rule_ids.is_empty() {
        eprintln!("경고: 실행할 규칙이 없어 규칙 검사를 건너뜁니다(중복도와 커버리지만 측정합니다)");
        rules::RulesOutcome::default()
    } else {
        // 꺼진 디렉터리만 대상으로 하는 파일은 설정에 따른 제외다(스펙 4.1). 분석 실패로 기록하지
        // 않도록 실행하는 디렉터리의 대상 파일만 넘긴다.
        let targeted: Vec<String> = code.iter()
            .filter(|f| select::dirs_for_file(f).iter().any(|d| sel.dirs.iter().any(|s| s == d)))
            .cloned().collect();
        rules::run(&rules::RunOpts {
            opengrep: opengrep.to_string(),
            src: src.to_path_buf(),
            rule_paths: vec![sel.rules_dir.path().to_path_buf()],
            exclude: p.scan.exclude.clone(),
            disabled: res.ruleset.languages.values().flat_map(|l| l.disabled_rules.iter().cloned()).collect(),
            code: targeted,
        })?
    };
    select::apply_overrides(&mut outcome.findings, &catalog, &res.ruleset);

    let d = dup::detect(src, &prod, &p.duplication);
    let loaded = coverage::load(src, &all, &code, &excluded, &tests, &p.coverage);
    for e in &loaded.error_details { eprintln!("경고: 커버리지 리포트를 읽지 못했습니다: {e}"); }

    let changed = new_code(src, config_dir, p.new_code_days, &commit)?;
    let has_cov = !loaded.reports.is_empty();
    let count = |kind: Kind, only: Option<&ChangedLines>| {
        outcome.findings.iter().filter(|f| f.kind == kind && only.is_none_or(|c| on_lines(f, c))).count() as u64
    };
    let new_hotspots: Vec<Finding> = changed.as_ref().map_or_else(Vec::new, |c| {
        outcome.findings.iter().filter(|f| f.kind == Kind::Hotspot && on_lines(f, c)).cloned().collect()
    });
    let metrics = Metrics {
        new_issues: changed.as_ref().map(|c| count(Kind::Issue, Some(c))),
        issues: count(Kind::Issue, None),
        new_hotspots: changed.as_ref().map(|_| new_hotspots.len() as u64),
        coverage: gate::coverage_pct(&loaded.map, None),
        new_coverage: changed.as_ref().filter(|_| has_cov).and_then(|c| gate::coverage_pct(&loaded.map, Some(c))),
        duplicated_lines: gate::duplication(&d.clones, &d.total_lines, None),
        new_duplicated_lines: changed.as_ref().and_then(|c| gate::duplication(&d.clones, &d.total_lines, Some(c))),
    };

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
        unanalyzed,
        publish: PublishConfig { max_new_issues: p.publish.max_new_issues, labels: p.publish.labels.clone() },
        active_dirs: sel.dirs,
        active_rules: sel.rule_ids,
        gate_conditions: p.gate.clone(),
        config_source,
        new_hotspots,
        new_code_measured: changed.is_some(),
    })
}
