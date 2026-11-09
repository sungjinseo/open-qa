//! 룰셋과 저장소 파일로 실행할 규칙 디렉터리와 규칙을 고르고, 결과에 오버라이드를 적용한다.
use crate::catalog::{Catalog, Kind};
use crate::model::Finding;
use crate::settings::Ruleset;
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use std::path::Path;

/// 규칙 디렉터리 이름. 룰셋 `languages`의 키와 같다.
pub const DIRS: [&str; 7] = ["java", "kotlin", "python", "go", "ts", "react", "vue"];

/// 파일을 대상으로 하는 규칙 디렉터리(스펙 4.1). 대상이 아니면 빈 목록.
pub fn dirs_for_file(path: &str) -> &'static [&'static str] {
    let Some((stem, ext)) = path.rsplit('/').next().and_then(|n| n.rsplit_once('.')) else { return &[] };
    if stem.is_empty() { return &[]; }
    match ext.to_ascii_lowercase().as_str() {
        "java" => &["java"],
        "kt" | "kts" => &["kotlin"],
        "py" => &["python"],
        "go" => &["go"],
        "js" | "mjs" | "cjs" | "ts" => &["ts"],
        "jsx" | "tsx" => &["ts", "react"],
        "vue" => &["ts", "vue"],
        _ => &[],
    }
}

/// 실행할 디렉터리 = 룰셋에서 켜짐 ∩ (프로젝트 `languages` 또는 파일로 자동 감지). 정렬된 목록.
pub fn active_dirs(rs: &Ruleset, project_langs: Option<&[String]>, files: &[String]) -> Vec<String> {
    let wanted: BTreeSet<&str> = match project_langs {
        Some(l) => l.iter().map(String::as_str).collect(),
        None => files.iter().flat_map(|f| dirs_for_file(f).iter().copied()).collect(),
    };
    let mut v: Vec<String> = rs.languages.iter()
        .filter(|(d, sel)| sel.enabled && wanted.contains(d.as_str()))
        .map(|(d, _)| d.clone())
        .collect();
    v.sort();
    v
}

pub struct Selection {
    /// 실행할 규칙이 하나 이상 있는 디렉터리.
    pub dirs: Vec<String>,
    pub rule_ids: Vec<String>,
    /// `<dir>/<file>.yaml`로 복사한 실행할 규칙 파일만 담은 임시 디렉터리.
    pub rules_dir: tempfile::TempDir,
}

/// `dirs`의 규칙 중 `disabled_rules`에 없는 규칙 파일만 임시 디렉터리에 복사한다.
pub fn select(rules_root: &Path, catalog: &Catalog, rs: &Ruleset, dirs: &[String]) -> Result<Selection> {
    let tmp = tempfile::tempdir()?;
    let mut sel_dirs = vec![];
    let mut rule_ids = vec![];
    for dir in dirs {
        let disabled: &[String] = rs.languages.get(dir).map_or(&[], |l| l.disabled_rules.as_slice());
        let mut any = false;
        for r in catalog.rules.values().filter(|r| &r.dir == dir && !disabled.contains(&r.id)) {
            let file = format!("{}.yaml", &r.id[dir.len() + 1..]);
            let to = tmp.path().join(dir);
            std::fs::create_dir_all(&to)?;
            std::fs::copy(rules_root.join(dir).join(&file), to.join(&file))
                .with_context(|| format!("규칙 파일 복사 실패: {dir}/{file}"))?;
            rule_ids.push(r.id.clone());
            any = true;
        }
        if any { sel_dirs.push(dir.clone()); }
    }
    rule_ids.sort();
    Ok(Selection { dirs: sel_dirs, rule_ids, rules_dir: tmp })
}

/// 카탈로그 기본 종류를 기록하고 룰셋 `overrides`의 등급과 종류를 적용한다.
pub fn apply_overrides(findings: &mut [Finding], catalog: &Catalog, rs: &Ruleset) {
    for f in findings {
        f.kind = catalog.rules.get(&f.rule).map_or(Kind::Issue, |r| r.kind);
        if let Some(o) = rs.overrides.get(&f.rule) {
            if let Some(s) = o.severity { f.severity = s; }
            if let Some(k) = o.kind { f.kind = k; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Severity;

    fn ruleset(text: &str) -> Ruleset { toml::from_str(&format!("schema = 1\nname = \"t\"\n{text}")).unwrap() }

    fn s(v: &[&str]) -> Vec<String> { v.iter().map(|x| x.to_string()).collect() }

    fn real_root() -> std::path::PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("rules") }

    #[test]
    fn dirs_for_file_follows_the_spec_table() {
        assert_eq!(dirs_for_file("a/B.java"), ["java"]);
        assert_eq!(dirs_for_file("build.gradle.kts"), ["kotlin"]);
        assert_eq!(dirs_for_file("a.kt"), ["kotlin"]);
        assert_eq!(dirs_for_file("a.py"), ["python"]);
        assert_eq!(dirs_for_file("a.go"), ["go"]);
        assert_eq!(dirs_for_file("a.mjs"), ["ts"]);
        assert_eq!(dirs_for_file("a.ts"), ["ts"]);
        assert_eq!(dirs_for_file("a.JS"), ["ts"]);
        assert_eq!(dirs_for_file("a.jsx"), ["ts", "react"]);
        assert_eq!(dirs_for_file("a.tsx"), ["ts", "react"]);
        assert_eq!(dirs_for_file("a.vue"), ["ts", "vue"]);
        assert!(dirs_for_file("README.md").is_empty());
        assert!(dirs_for_file(".java").is_empty());
        assert!(dirs_for_file("src/java").is_empty());
    }

    const RS: &str = "[languages.java]\n[languages.ts]\n[languages.react]\n[languages.go]\nenabled = false\n";

    #[test]
    fn active_dirs_intersects_ruleset_with_detected_or_declared() {
        let rs = ruleset(RS);
        let files = s(&["a.java", "b.tsx", "c.go", "d.md"]);
        assert_eq!(active_dirs(&rs, None, &files), s(&["java", "react", "ts"]));
        assert_eq!(active_dirs(&rs, Some(&s(&["java"])), &files), s(&["java"]));
        // 룰셋에서 꺼진 디렉터리(go)와 없는 디렉터리(python)는 프로젝트가 지정해도 실행하지 않는다.
        assert_eq!(active_dirs(&rs, Some(&s(&["go", "python", "ts"])), &files), s(&["ts"]));
        assert!(active_dirs(&rs, None, &s(&["x.py"])).is_empty());
    }

    #[test]
    fn select_copies_only_enabled_rules_of_active_dirs() {
        let catalog = Catalog::load(&real_root()).unwrap();
        let rs = ruleset("[languages.java]\ndisabled_rules = [\"java.system-out\"]\n");
        let sel = select(&real_root(), &catalog, &rs, &s(&["java"])).unwrap();
        assert_eq!(sel.dirs, s(&["java"]));
        assert!(!sel.rule_ids.contains(&"java.system-out".to_string()));
        assert!(sel.rule_ids.contains(&"java.catch-throwable".to_string()));
        assert!(sel.rule_ids.iter().all(|r| r.starts_with("java.")));
        let java_count = catalog.rules.values().filter(|r| r.dir == "java").count();
        assert_eq!(sel.rule_ids.len(), java_count - 1);
        let root = sel.rules_dir.path();
        assert!(!root.join("java/system-out.yaml").exists());
        assert!(root.join("java/catch-throwable.yaml").is_file());
        let entries: Vec<_> = std::fs::read_dir(root).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(entries, vec![std::ffi::OsString::from("java")]);
        assert_eq!(std::fs::read_dir(root.join("java")).unwrap().count(), java_count - 1);
    }

    #[test]
    fn select_drops_dirs_whose_rules_are_all_disabled() {
        let catalog = Catalog::load(&real_root()).unwrap();
        let all: Vec<String> = catalog.rules.values().filter(|r| r.dir == "go").map(|r| format!("\"{}\"", r.id)).collect();
        let rs = ruleset(&format!("[languages.go]\ndisabled_rules = [{}]\n", all.join(",")));
        let sel = select(&real_root(), &catalog, &rs, &s(&["go"])).unwrap();
        assert!(sel.dirs.is_empty());
        assert!(sel.rule_ids.is_empty());
    }

    fn finding(rule: &str) -> Finding {
        Finding {
            rule: rule.into(), file: "A.java".into(), line: 1, line_hash: "h".into(), message: "m".into(),
            severity: Severity::High, category: "c".into(), kind: Kind::Issue,
        }
    }

    #[test]
    fn overrides_change_severity_and_kind_and_catalog_sets_default_kind() {
        let catalog = Catalog::load(&real_root()).unwrap();
        let rs = ruleset("[overrides.\"java.catch-throwable\"]\nseverity = \"low\"\nkind = \"hotspot\"\n");
        let mut fs = vec![finding("java.catch-throwable"), finding("go.math-rand"), finding("go.sql-concat"), finding("ext.unknown")];
        fs[1].kind = Kind::Issue;
        fs[2].kind = Kind::Hotspot;
        apply_overrides(&mut fs, &catalog, &rs);
        assert_eq!((fs[0].severity, fs[0].kind), (Severity::Low, Kind::Hotspot));
        assert_eq!((fs[1].severity, fs[1].kind), (Severity::High, Kind::Hotspot));
        assert_eq!(fs[2].kind, Kind::Issue);
        assert_eq!(fs[3].kind, Kind::Issue);
    }
}
