use crate::model::{line_hash, Finding, Severity};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default, PartialEq)]
pub struct RulesOutcome { pub findings: Vec<Finding>, pub unanalyzed: Vec<String> }

#[derive(Debug, Clone)]
pub struct RunOpts {
    pub opengrep: String,
    pub src: PathBuf,
    pub rule_paths: Vec<PathBuf>,
    pub exclude: Vec<String>,
    pub disabled: Vec<String>,
}

#[derive(Deserialize)]
struct Output { #[serde(default)] results: Vec<RawResult>, #[serde(default)] errors: Vec<RawError> }
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
struct RawError { #[serde(default)] path: Option<String> }

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
        if let Some(p) = e.path { res.unanalyzed.push(clean(&p)); }
    }
    res.unanalyzed.sort();
    res.unanalyzed.dedup();
    Ok(res)
}

fn invoke(opts: &RunOpts, target: &Path, exclude: &[String]) -> Result<String> {
    let out_dir = tempfile::tempdir()?;
    let out_file = out_dir.path().join("o.json");
    let mut c = Command::new(&opts.opengrep);
    c.current_dir(target).arg("scan").arg(format!("--json-output={}", out_file.display()));
    for r in &opts.rule_paths { c.arg("-f").arg(r); }
    for e in exclude { c.arg("--exclude").arg(e); }
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

pub fn run(opts: &RunOpts) -> Result<RulesOutcome> {
    let excl = crate::fsutil::globset(&opts.exclude)?;
    let src = opts.src.clone();
    let read_src = move |f: &str, l: u32| std::fs::read_to_string(src.join(f)).ok().and_then(|t| line_at(&t, l));
    let json = invoke(opts, &opts.src, &opts.exclude)?;
    let mut all = parse_output(&json, &opts.disabled, &read_src)?;
    all.findings.retain(|f| !excl.is_match(&f.file));
    all.unanalyzed.retain(|f| !excl.is_match(f));

    // Vue: script만 남긴 임시 파일을 `<경로>.vue.<ext>`로 만들어 별도 스캔
    let tmp = tempfile::tempdir()?;
    let mut any_vue = false;
    for rel in crate::fsutil::walk(&opts.src) {
        if !rel.ends_with(".vue") || excl.is_match(&rel) { continue; }
        let Ok(text) = std::fs::read_to_string(opts.src.join(&rel)) else {
            all.unanalyzed.push(rel);
            continue;
        };
        if let Some(e) = crate::vue::extract(&text) {
            let dst = tmp.path().join(format!("{rel}.{}", e.ext));
            std::fs::create_dir_all(dst.parent().unwrap())?;
            std::fs::write(dst, e.text)?;
            any_vue = true;
        }
    }
    if any_vue {
        let strip = |f: &str| f.rsplit_once(".vue.").map(|(a, _)| format!("{a}.vue")).unwrap_or_else(|| f.to_string());
        let src2 = opts.src.clone();
        let read_vue = move |f: &str, l: u32| std::fs::read_to_string(src2.join(strip(f))).ok().and_then(|t| line_at(&t, l));
        let json = invoke(opts, tmp.path(), &[])?;
        let v = parse_output(&json, &opts.disabled, &read_vue)?;
        for mut f in v.findings { f.file = strip(&f.file); all.findings.push(f); }
        for f in v.unanalyzed { all.unanalyzed.push(strip(&f)); }
    }
    all.unanalyzed.sort();
    all.unanalyzed.dedup();
    Ok(all)
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
      "paths": {"scanned":[]}
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
    }

    #[test]
    fn malformed_json_is_error() {
        assert!(parse_output("{", &[], &|_, _| None).is_err());
    }

    fn write_rule(dir: &Path) -> PathBuf {
        let r = dir.join("r.yaml");
        std::fs::write(&r, "rules:\n  - id: t\n    languages: [javascript, typescript]\n    severity: ERROR\n    message: no eval\n    pattern: eval(...)\n    metadata: { qualgate_id: t.eval, qualgate_severity: high, qualgate_category: security }\n").unwrap();
        r
    }

    #[test]
    fn runs_opengrep_and_maps_vue_back_to_original_path() {
        let d = tempfile::tempdir().unwrap();
        let rd = tempfile::tempdir().unwrap();
        let rule = write_rule(rd.path());
        let src = d.path();
        std::fs::create_dir_all(src.join("web/node_modules/x")).unwrap();
        std::fs::write(src.join("web/node_modules/x/i.js"), "eval('1')\n").unwrap();
        std::fs::write(src.join("web/a.js"), "eval('1')\n").unwrap();
        std::fs::write(src.join("web/C.vue"), "<template><p/></template>\n<script>\neval('2')\n</script>\n").unwrap();
        let o = run(&RunOpts {
            opengrep: "opengrep".into(),
            src: src.to_path_buf(),
            rule_paths: vec![rule],
            exclude: vec!["**/node_modules/**".into()],
            disabled: vec![],
        }).unwrap();
        let mut got: Vec<(String, u32)> = o.findings.iter().map(|f| (f.file.clone(), f.line)).collect();
        got.sort();
        assert_eq!(got, vec![("web/C.vue".to_string(), 3), ("web/a.js".to_string(), 1)]);
    }

    #[test]
    fn opengrep_failure_is_error() {
        let d = tempfile::tempdir().unwrap();
        let r = run(&RunOpts {
            opengrep: "opengrep".into(),
            src: d.path().to_path_buf(),
            rule_paths: vec![d.path().join("missing.yaml")],
            exclude: vec![],
            disabled: vec![],
        });
        assert!(r.is_err());
    }
}
