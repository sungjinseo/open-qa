use crate::model::{line_hash, Finding, Lang, Severity};
use crate::vue::Sfc;
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default, PartialEq)]
pub struct RulesOutcome { pub findings: Vec<Finding>, pub unanalyzed: Vec<String>, pub scanned: Vec<String> }

#[derive(Debug, Clone)]
pub struct RunOpts {
    pub opengrep: String,
    pub src: PathBuf,
    pub rule_paths: Vec<PathBuf>,
    pub exclude: Vec<String>,
    pub disabled: Vec<String>,
    /// 분석해야 하는 파일. 스캔 결과에 없는 파일은 분석 실패로 기록한다.
    pub code: Vec<String>,
}

#[derive(Deserialize)]
struct Output {
    #[serde(default)] results: Vec<RawResult>,
    #[serde(default)] errors: Vec<RawError>,
    // 일부러 기본값을 두지 않는다. 없으면 무엇을 분석했는지 알 수 없으므로 오류다.
    paths: Paths,
    #[serde(default)] skipped_rules: Vec<serde_json::Value>,
}
#[derive(Deserialize)]
struct Paths { scanned: Vec<String> }
#[derive(Deserialize)]
struct RawResult { check_id: String, path: String, start: Pos, extra: Extra }
#[derive(Deserialize)]
struct Pos { line: u32 }
#[derive(Deserialize)]
struct Extra {
    #[serde(default)] message: String,
    #[serde(default)] severity: String,
    #[serde(default)] metadata: serde_json::Value,
}
#[derive(Deserialize)]
struct RawError {
    #[serde(default)] path: Option<String>,
    #[serde(default)] level: String,
    #[serde(default)] rule_id: Option<String>,
    #[serde(default)] message: String,
}

fn clean(p: &str) -> String { p.trim_start_matches("./").replace('\\', "/") }

fn fallback_severity(s: &str) -> Severity {
    match s.to_ascii_uppercase().as_str() {
        "ERROR" => Severity::High,
        "WARNING" => Severity::Medium,
        _ => Severity::Low,
    }
}

pub fn parse_output(json: &str, disabled: &[String], read_line: &dyn Fn(&str, u32) -> Option<String>) -> Result<RulesOutcome> {
    let out: Output = serde_json::from_str(json).context("Opengrep JSON 파싱 실패")?;
    // 빠진 규칙이 있으면 그 규칙의 이슈가 전부 "고쳐짐"으로 닫힌다.
    if let Some(r) = out.skipped_rules.first() {
        bail!("Opengrep이 규칙을 건너뛰었습니다: {}", r.get("rule_id").and_then(|v| v.as_str()).unwrap_or("(이름 없음)"));
    }
    let mut res = RulesOutcome::default();
    for r in out.results {
        let meta = |k: &str| r.extra.metadata.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let rule = meta("qualgate_id").unwrap_or_else(|| r.check_id.clone());
        if disabled.contains(&rule) { continue; }
        let severity = meta("qualgate_severity").and_then(|s| Severity::parse(&s))
            .unwrap_or_else(|| fallback_severity(&r.extra.severity));
        let category = meta("qualgate_category").unwrap_or_else(|| "external".into());
        let file = clean(&r.path);
        let line_text = read_line(&file, r.start.line).unwrap_or_default();
        res.findings.push(Finding {
            rule, line: r.start.line, line_hash: line_hash(&line_text),
            message: r.extra.message.clone(), severity, category, file,
        });
    }
    for e in out.errors {
        match e.path {
            Some(p) => res.unanalyzed.push(clean(&p)),
            None if e.level.eq_ignore_ascii_case("error") => {
                let rule = e.rule_id.as_deref().unwrap_or("(규칙 미상)");
                bail!("Opengrep 엔진 오류, 규칙 {rule}: {}", e.message.lines().next().unwrap_or(""));
            }
            None => {}
        }
    }
    res.unanalyzed.sort();
    res.unanalyzed.dedup();
    res.scanned = out.paths.scanned.iter().map(|p| clean(p)).collect();
    Ok(res)
}

/// Opengrep에 넘겨도 되는 제외 패턴만 고른다. `**/이름/**`만 양쪽 의미가 같다.
/// 나머지(`h.py`, `docs/*.py`)는 Opengrep이 모든 깊이에 적용해 qualgate보다 많이 제외하므로
/// 넘기지 않고 qualgate가 결과를 걸러낸다. 이 패턴들은 대형 디렉터리를 건너뛰는 성능 목적이다.
fn engine_excludes(exclude: &[String]) -> Vec<String> {
    exclude.iter().filter(|p| {
        p.strip_prefix("**/").and_then(|x| x.strip_suffix("/**")).is_some_and(|name| {
            !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        })
    }).cloned().collect()
}

fn invoke(opts: &RunOpts, target: &Path, exclude: &[String]) -> Result<String> {
    let out_dir = tempfile::tempdir()?;
    let out_file = out_dir.path().join("o.json");
    let mut c = Command::new(&opts.opengrep);
    c.current_dir(target).env_remove("GITEA_TOKEN").arg("scan").arg(format!("--json-output={}", out_file.display()));
    // 저장소가 무엇을 분석할지 정하지 못하게 Opengrep 자체 필터를 끈다.
    c.args(["--no-git-ignore", "--x-ignore-semgrepignore-files", "--max-target-bytes=0"]);
    for r in &opts.rule_paths { c.arg("-f").arg(r); }
    for e in exclude { c.arg(format!("--exclude={e}")); }
    c.arg(".");
    let o = c.output().with_context(|| format!("{} 실행 실패", opts.opengrep))?;
    if !o.status.success() {
        bail!("Opengrep 실패 ({}): {}", o.status, String::from_utf8_lossy(&o.stderr));
    }
    std::fs::read_to_string(&out_file).context("Opengrep 결과 파일 없음")
}

fn line_at(text: &str, line: u32) -> Option<String> {
    text.lines().nth(line.checked_sub(1)? as usize).map(str::to_string)
}

fn is_vue(path: &str) -> bool { Lang::of(path) == Some(Lang::Vue) }

/// 기본 규칙은 `.jsx`, `.tsx`, Vue script만 대상으로 하고 Opengrep은 `.mts`, `.cts`를 아예 스캔하지
/// 않는다. 이 확장자들은 스캔되지 않는 것이 정상이므로(중복도와 커버리지만 측정) 항상 분석 실패가
/// 되지 않도록 스캔 여부를 요구하지 않는다.
fn must_be_scanned(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    !matches!(ext.as_str(), "js" | "ts" | "mjs" | "cjs" | "mts" | "cts")
}

pub fn run(opts: &RunOpts) -> Result<RulesOutcome> {
    let excl = crate::fsutil::globset(&opts.exclude)?;
    let src = opts.src.clone();
    let read_src = move |f: &str, l: u32| std::fs::read_to_string(src.join(f)).ok().and_then(|t| line_at(&t, l));
    let json = invoke(opts, &opts.src, &engine_excludes(&opts.exclude))?;
    let mut all = parse_output(&json, &opts.disabled, &read_src)?;
    all.findings.retain(|f| !excl.is_match(&f.file));
    all.unanalyzed.retain(|f| !excl.is_match(f));
    // `.vue` 원본이 스캔된 것은 script를 분석했다는 뜻이 아니다. 아래에서 따로 판단한다.
    let mut scanned: BTreeSet<String> = all.scanned.drain(..).filter(|f| !is_vue(f)).collect();

    // Vue: script만 남긴 임시 파일을 `<경로>.<ext>`로 만들어 별도 스캔
    let tmp = tempfile::tempdir()?;
    let mut any_vue = false;
    for rel in opts.code.iter().filter(|p| is_vue(p)) {
        let Ok(text) = std::fs::read_to_string(opts.src.join(rel)) else {
            all.unanalyzed.push(rel.clone());
            continue;
        };
        match crate::vue::extract(&text) {
            Sfc::Script(e) => {
                let dst = tmp.path().join(format!("{rel}.{}", e.ext));
                std::fs::create_dir_all(dst.parent().unwrap())?;
                std::fs::write(dst, e.text)?;
                any_vue = true;
            }
            Sfc::NoScript => { scanned.insert(rel.clone()); }
            Sfc::Malformed => all.unanalyzed.push(rel.clone()),
        }
    }
    if any_vue {
        let strip = |f: &str| f.rsplit_once('.').map_or_else(|| f.to_string(), |(a, _)| a.to_string());
        let src2 = opts.src.clone();
        let read_vue = move |f: &str, l: u32| std::fs::read_to_string(src2.join(strip(f))).ok().and_then(|t| line_at(&t, l));
        let json = invoke(opts, tmp.path(), &[])?;
        let v = parse_output(&json, &opts.disabled, &read_vue)?;
        for mut f in v.findings { f.file = strip(&f.file); all.findings.push(f); }
        for f in v.unanalyzed { all.unanalyzed.push(strip(&f)); }
        for f in v.scanned { scanned.insert(strip(&f)); }
    }

    let failed: BTreeSet<String> = all.unanalyzed.iter().cloned().collect();
    let required: Vec<&String> = opts.code.iter().filter(|f| must_be_scanned(f) && !failed.contains(*f)).collect();
    ensure!(required.is_empty() || required.iter().any(|f| scanned.contains(*f)),
        "Opengrep이 분석 대상 파일 {}개 중 하나도 스캔하지 않았습니다", required.len());
    for f in required {
        if !scanned.contains(f) { all.unanalyzed.push(f.clone()); }
    }
    // 스캔을 요구하지 않는 파일도 외부 규칙의 대상일 수 있으므로 읽을 수 있는지는 확인한다.
    for f in opts.code.iter().filter(|f| !must_be_scanned(f)) {
        if std::fs::File::open(opts.src.join(f)).is_err() { all.unanalyzed.push(f.clone()); }
    }
    all.scanned = scanned.into_iter().collect();
    all.unanalyzed.sort();
    all.unanalyzed.dedup();
    Ok(all)
}

/// `rules.extra_paths`를 검증해 절대 경로로 돌려준다. 저장소 밖을 가리키면 오류다.
pub fn extra_rule_paths(src: &Path, extra: &[String]) -> Result<Vec<PathBuf>> {
    let root = src.canonicalize().with_context(|| format!("{} 경로 확인 실패", src.display()))?;
    let inside = |p: &Path| p.canonicalize().is_ok_and(|c| c.starts_with(&root));
    extra.iter().map(|rel| {
        let p = src.join(rel);
        ensure!(inside(&p), "rules.extra_paths는 저장소 안에 있어야 합니다: {rel}");
        // 폴더 안의 심볼릭 링크도 저장소 밖으로 나갈 수 있다.
        for e in walkdir::WalkDir::new(&p) {
            let e = e.with_context(|| format!("rules.extra_paths 탐색 실패: {rel}"))?;
            ensure!(!e.path_is_symlink() || inside(e.path()), "rules.extra_paths의 심볼릭 링크가 저장소 밖을 가리킵니다: {rel}");
        }
        Ok(p.canonicalize()?)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTPUT: &str = r#"{
      "results": [
        {"check_id":"rules.java.x","path":"./src/A.java","start":{"line":2,"col":1},
         "extra":{"message":"bad","severity":"WARNING",
           "metadata":{"qualgate_id":"java.x","qualgate_severity":"critical","qualgate_category":"security"}}},
        {"check_id":"ext.rule","path":"b.py","start":{"line":1,"col":1},
         "extra":{"message":"m","severity":"ERROR","metadata":{}}},
        {"check_id":"off","path":"c.py","start":{"line":1,"col":1},
         "extra":{"message":"m","severity":"INFO","metadata":{"qualgate_id":"off.rule"}}}
      ],
      "errors": [
        {"level":"warn","message":"syntax","path":"./bad.go"},
        {"level":"warn","message":"no path"}
      ],
      "paths": {"scanned":["./src/A.java"]}
    }"#;

    #[test]
    fn maps_metadata_and_fallbacks() {
        let read = |_: &str, line: u32| Some(format!("line{line}"));
        let o = parse_output(OUTPUT, &["off.rule".to_string()], &read).unwrap();
        assert_eq!(o.findings.len(), 2);
        let a = &o.findings[0];
        assert_eq!((a.rule.as_str(), a.file.as_str(), a.line), ("java.x", "src/A.java", 2));
        assert_eq!(a.severity, Severity::Critical);
        assert_eq!(a.category, "security");
        assert_eq!(a.line_hash, line_hash("line2"));
        let b = &o.findings[1];
        assert_eq!(b.rule, "ext.rule");
        assert_eq!(b.severity, Severity::High);
        assert_eq!(b.category, "external");
        assert_eq!(o.unanalyzed, vec!["bad.go".to_string()]);
        assert_eq!(o.scanned, vec!["src/A.java".to_string()]);
    }

    #[test]
    fn malformed_json_is_error() {
        assert!(parse_output("{", &[], &|_, _| None).is_err());
    }

    #[test]
    fn missing_paths_is_error() {
        assert!(parse_output(r#"{"results":[],"errors":[]}"#, &[], &|_, _| None).is_err());
        assert!(parse_output(r#"{"results":[],"errors":[],"paths":{}}"#, &[], &|_, _| None).is_err());
    }

    #[test]
    fn pathless_engine_error_names_the_rule() {
        let j = r#"{"errors":[{"level":"error","rule_id":"x.broken","message":"bad"}],"paths":{"scanned":[]}}"#;
        let e = parse_output(j, &[], &|_, _| None).unwrap_err();
        assert!(e.to_string().contains("x.broken"), "{e}");
    }

    #[test]
    fn skipped_rule_is_error_and_names_the_rule() {
        let j = r#"{"skipped_rules":[{"rule_id":"x.skipped","details":"d"}],"paths":{"scanned":[]}}"#;
        let e = parse_output(j, &[], &|_, _| None).unwrap_err();
        assert!(e.to_string().contains("x.skipped"), "{e}");
    }

    #[test]
    fn only_directory_excludes_reach_the_engine() {
        let got = engine_excludes(&["**/node_modules/**".into(), "**/a.b_c-d/**".into(), "h.py".into(), "docs/*.py".into(), "**/x*/**".into(), "**/a/b/**".into(), "**/*.py".into()]);
        assert_eq!(got, vec!["**/node_modules/**".to_string(), "**/a.b_c-d/**".to_string()]);
    }

    fn write_rule(dir: &Path) -> PathBuf {
        let r = dir.join("r.yaml");
        std::fs::write(&r, "rules:\n  - id: t\n    languages: [javascript, typescript]\n    severity: ERROR\n    message: no eval\n    pattern: eval(...)\n    metadata: { qualgate_id: t.eval, qualgate_severity: high, qualgate_category: security }\n  - id: p\n    languages: [python]\n    severity: ERROR\n    message: no eval\n    pattern: eval(...)\n").unwrap();
        r
    }

    // opengrep은 동시 실행이 많으면 Bus error로 죽을 수 있어 실제 실행 테스트를 직렬화한다.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn serial() -> std::sync::MutexGuard<'static, ()> { SERIAL.lock().unwrap_or_else(|e| e.into_inner()) }

    fn opts(src: &Path, rule: PathBuf, exclude: &[&str]) -> RunOpts {
        let excl = crate::fsutil::globset(&exclude.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        RunOpts {
            opengrep: "opengrep".into(),
            src: src.to_path_buf(),
            rule_paths: vec![rule],
            exclude: exclude.iter().map(|s| s.to_string()).collect(),
            disabled: vec![],
            code: crate::fsutil::walk(src).unwrap().into_iter()
                .filter(|p| Lang::of(p).is_some() && !excl.is_match(p.as_str())).collect(),
        }
    }

    fn put(src: &Path, rel: &str, body: &str) {
        let p = src.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn files(o: &RulesOutcome) -> Vec<String> {
        let mut v: Vec<String> = o.findings.iter().map(|f| f.file.clone()).collect();
        v.sort();
        v
    }

    #[test]
    fn runs_opengrep_and_maps_vue_back_to_original_path() {
        let _g = serial();
        let d = tempfile::tempdir().unwrap();
        let rd = tempfile::tempdir().unwrap();
        let rule = write_rule(rd.path());
        let src = d.path();
        put(src, "web/node_modules/x/i.js", "eval('1')\n");
        put(src, "web/a.js", "eval('1')\n");
        put(src, "web/C.vue", "<template><p/></template>\n<script>\neval('2')\n</script>\n");
        let o = run(&opts(src, rule, &["**/node_modules/**"])).unwrap();
        let mut got: Vec<(String, u32)> = o.findings.iter().map(|f| (f.file.clone(), f.line)).collect();
        got.sort();
        assert_eq!(got, vec![("web/C.vue".to_string(), 3), ("web/a.js".to_string(), 1)]);
        assert_eq!(o.unanalyzed, Vec::<String>::new());
    }

    #[test]
    fn opengrep_failure_is_error() {
        let _g = serial();
        let d = tempfile::tempdir().unwrap();
        let r = run(&opts(d.path(), d.path().join("missing.yaml"), &[]));
        assert!(r.is_err());
    }

    /// 예전에는 아래 파일들이 조용히 사라져 이슈가 "고쳐짐"으로 닫혔다.
    #[test]
    fn engine_filters_do_not_hide_files() {
        let _g = serial();
        let d = tempfile::tempdir().unwrap();
        let rd = tempfile::tempdir().unwrap();
        let rule = write_rule(rd.path());
        let src = d.path();
        Command::new("git").current_dir(src).args(["init", "-q"]).status().unwrap();
        let v = "eval(input())\n";
        let expected = ["big.py", "com/acme/test/d.py", "ignored.py", "semgrepignored.py", "src/test/c.py", "test/b.py", "tests/a.py"];
        for f in expected { put(src, f, v); }
        put(src, "big.py", &format!("{}{v}", "\n".repeat(1_500_000)));
        put(src, ".gitignore", "ignored.py\n");
        put(src, ".semgrepignore", "semgrepignored.py\n");
        let o = run(&opts(src, rule, &[])).unwrap();
        assert_eq!(files(&o), expected.to_vec());
        assert_eq!(o.unanalyzed, Vec::<String>::new());
        for f in expected { assert!(o.scanned.iter().any(|s| s == f), "{f}"); }
    }

    #[test]
    fn semgrepignore_star_does_not_hide_files() {
        let _g = serial();
        let d = tempfile::tempdir().unwrap();
        let rd = tempfile::tempdir().unwrap();
        let rule = write_rule(rd.path());
        put(d.path(), ".semgrepignore", "*\n");
        put(d.path(), "a.py", "eval(input())\n");
        let o = run(&opts(d.path(), rule, &[])).unwrap();
        assert_eq!(files(&o), vec!["a.py".to_string()]);
    }

    #[test]
    fn qualgate_exclude_is_the_only_filter() {
        let _g = serial();
        let d = tempfile::tempdir().unwrap();
        let rd = tempfile::tempdir().unwrap();
        let rule = write_rule(rd.path());
        // Opengrep의 --exclude=h.py는 x/h.py까지 제외하지만 qualgate의 glob은 루트만 뜻한다.
        put(d.path(), "h.py", "eval(input())\n");
        put(d.path(), "x/h.py", "eval(input())\n");
        put(d.path(), "build/g.py", "eval(input())\n");
        let o = run(&opts(d.path(), rule, &["h.py", "**/build/**"])).unwrap();
        assert_eq!(files(&o), vec!["x/h.py".to_string()]);
        assert_eq!(o.unanalyzed, Vec::<String>::new());
    }

    #[test]
    fn unscanned_code_is_unanalyzed_and_excluded_or_plain_js_is_not() {
        let _g = serial();
        let d = tempfile::tempdir().unwrap();
        let rd = tempfile::tempdir().unwrap();
        let rule = write_rule(rd.path());
        put(d.path(), "a.py", "x = 1\n");
        // 규칙 파일에 Go 규칙이 없으므로 a.go는 스캔되지 않는다.
        put(d.path(), "a.go", "package a\n");
        put(d.path(), "a.mts", "const a = 1\n");
        let mut o = opts(d.path(), rule, &[]);
        o.code.push("deleted-meanwhile.mts".into());
        let r = run(&o).unwrap();
        assert_eq!(r.unanalyzed, vec!["a.go".to_string(), "deleted-meanwhile.mts".to_string()]);
    }

    #[test]
    fn nothing_scanned_is_an_error() {
        let _g = serial();
        let d = tempfile::tempdir().unwrap();
        let rd = tempfile::tempdir().unwrap();
        let rule = write_rule(rd.path());
        put(d.path(), "a.go", "package a\n");
        assert!(run(&opts(d.path(), rule, &[])).is_err());
    }

    #[test]
    fn repo_of_only_plain_ts_is_not_an_error_when_no_rule_targets_it() {
        let _g = serial();
        let d = tempfile::tempdir().unwrap();
        let rd = tempfile::tempdir().unwrap();
        let rule = rd.path().join("r.yaml");
        std::fs::write(&rule, "rules:\n  - id: p\n    languages: [python]\n    severity: ERROR\n    message: m\n    pattern: eval(...)\n").unwrap();
        put(d.path(), "a.ts", "const a = 1\n");
        let r = run(&opts(d.path(), rule, &[])).unwrap();
        assert_eq!(r.unanalyzed, Vec::<String>::new());
    }

    #[test]
    fn vue_routes_keep_findings_or_report_unanalyzed() {
        let _g = serial();
        let d = tempfile::tempdir().unwrap();
        let rd = tempfile::tempdir().unwrap();
        let rule = write_rule(rd.path());
        let s = "<script>\neval('1')\n</script>\n";
        put(d.path(), "SelfClosing.vue", &format!("<template src=\"./x.html\"/>\n{s}"));
        put(d.path(), "Commented.vue", &format!("<!-- <template> old -->\n<template><p/></template>\n{s}"));
        put(d.path(), "Unclosed.vue", "<script>\neval('1')\n");
        put(d.path(), "UnclosedTemplate.vue", &format!("<template><p/>\n{s}"));
        put(d.path(), "NoScript.vue", "<template><p/></template>\n");
        let r = run(&opts(d.path(), rule, &[])).unwrap();
        assert_eq!(files(&r), vec!["Commented.vue".to_string(), "SelfClosing.vue".to_string()]);
        assert_eq!(r.unanalyzed, vec!["Unclosed.vue".to_string(), "UnclosedTemplate.vue".to_string()]);
        assert!(r.scanned.iter().any(|f| f == "NoScript.vue"));
    }

    #[test]
    fn only_a_malformed_vue_is_reported_not_fatal() {
        let _g = serial();
        let d = tempfile::tempdir().unwrap();
        let rd = tempfile::tempdir().unwrap();
        let rule = write_rule(rd.path());
        put(d.path(), "Unclosed.vue", "<script>\neval('1')\n");
        let r = run(&opts(d.path(), rule, &[])).unwrap();
        assert_eq!(r.unanalyzed, vec!["Unclosed.vue".to_string()]);
    }

    #[test]
    fn extra_paths_must_stay_inside_the_repo() {
        let d = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        put(d.path(), "rules/a.yaml", "rules: []\n");
        std::fs::write(outside.path().join("o.yaml"), "secret: 1\n").unwrap();
        assert_eq!(extra_rule_paths(d.path(), &["rules".to_string()]).unwrap(), vec![d.path().canonicalize().unwrap().join("rules")]);
        for bad in ["../x".to_string(), outside.path().display().to_string(), "missing".to_string()] {
            assert!(extra_rule_paths(d.path(), &[bad.clone()]).is_err(), "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn extra_paths_symlink_leaving_the_repo_is_rejected() {
        use std::os::unix::fs::symlink;
        let d = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("o.yaml");
        std::fs::write(&secret, "secret: 1\n").unwrap();
        symlink(&secret, d.path().join("direct.yaml")).unwrap();
        assert!(extra_rule_paths(d.path(), &["direct.yaml".to_string()]).is_err());
        put(d.path(), "rules/ok.yaml", "rules: []\n");
        symlink(&secret, d.path().join("rules/leak.yaml")).unwrap();
        let e = extra_rule_paths(d.path(), &["rules".to_string()]).unwrap_err();
        assert!(!format!("{e:#}").contains("secret"));
        // 저장소 안을 가리키는 링크는 허용한다.
        let d2 = tempfile::tempdir().unwrap();
        put(d2.path(), "rules/ok.yaml", "rules: []\n");
        symlink(d2.path().join("rules/ok.yaml"), d2.path().join("rules/alias.yaml")).unwrap();
        assert!(extra_rule_paths(d2.path(), &["rules".to_string()]).is_ok());
    }
}
