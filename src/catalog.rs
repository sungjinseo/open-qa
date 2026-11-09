use crate::model::Severity;
use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// 규칙의 이슈 종류. 확신도가 낮은 규칙은 `Hotspot`이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind { #[default] Issue, Hotspot }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleInfo {
    pub id: String,
    pub dir: String,
    pub severity: Severity,
    pub kind: Kind,
    pub category: String,
    pub mois: Option<String>,
    pub cwe: Option<String>,
    pub title: String,
}

#[derive(Debug, Clone, Default)]
pub struct Catalog { pub rules: BTreeMap<String, RuleInfo> }

impl Catalog {
    pub fn from_rules(v: Vec<RuleInfo>) -> Catalog {
        Catalog { rules: v.into_iter().map(|r| (r.id.clone(), r)).collect() }
    }
}

/// 규칙 폴더. `rules/tests/`는 규칙이 아니므로 포함하지 않는다.
pub const RULE_DIRS: [&str; 7] = ["java", "kotlin", "python", "react", "vue", "go", "ts"];

impl Catalog {
    /// `rules_root/<dir>/*.yaml`을 읽어 카탈로그를 만든다. 규칙 ID는 `<dir>.<파일명>`이어야 한다.
    pub fn load(rules_root: &Path) -> anyhow::Result<Catalog> {
        let mut v = vec![];
        for dir in RULE_DIRS {
            let d = rules_root.join(dir);
            if !d.is_dir() { continue; }
            let mut files: Vec<_> = std::fs::read_dir(&d)
                .with_context(|| format!("{} 읽기 실패", d.display()))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
                .collect();
            files.sort();
            for f in files { v.push(read_rule(dir, &f)?); }
        }
        if v.is_empty() {
            bail!("{}에서 규칙 파일(<dir>/*.yaml)을 하나도 찾지 못했습니다. QUALGATE_RULES 경로를 확인하세요", rules_root.display());
        }
        Ok(Catalog::from_rules(v))
    }

    /// 항목 필드는 `RuleInfo` 필드명 그대로인 배열.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self.rules.values().collect::<Vec<_>>()).expect("RuleInfo는 JSON으로 직렬화된다")
    }

    pub fn dir_of(&self, id: &str) -> Option<&str> { self.rules.get(id).map(|r| r.dir.as_str()) }
}

fn read_rule(dir: &str, file: &Path) -> anyhow::Result<RuleInfo> {
    let name = file.display();
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let doc: serde_yaml::Value = serde_yaml::from_str(&std::fs::read_to_string(file).with_context(|| format!("{name} 읽기 실패"))?)
        .with_context(|| format!("{name} YAML 오류"))?;
    let rule = doc.get("rules").and_then(|r| r.get(0)).ok_or_else(|| anyhow!("{name}: rules[0] 없음"))?;
    let s = |v: Option<&serde_yaml::Value>| v.and_then(|v| v.as_str()).map(str::to_string);
    let id = s(rule.get("id")).ok_or_else(|| anyhow!("{name}: id 없음"))?;
    let expected = format!("{dir}.{stem}");
    if id != expected { bail!("{name}: 규칙 ID는 {expected} 이어야 함(현재 {id})"); }
    let meta = rule.get("metadata");
    let m = |k: &str| s(meta.and_then(|m| m.get(k)));
    let severity = m("qualgate_severity").ok_or_else(|| anyhow!("{name}: qualgate_severity 없음"))?;
    let severity = Severity::parse(&severity).ok_or_else(|| anyhow!("{name}: qualgate_severity 값 오류({severity})"))?;
    let kind = match m("qualgate_kind").as_deref() {
        None | Some("issue") => Kind::Issue,
        Some("hotspot") => Kind::Hotspot,
        Some(o) => bail!("{name}: qualgate_kind는 issue 또는 hotspot이어야 함({o})"),
    };
    let title = s(rule.get("message")).and_then(|t| t.lines().next().map(str::to_string)).unwrap_or_default();
    Ok(RuleInfo {
        id, dir: dir.to_string(), severity, kind,
        category: m("qualgate_category").unwrap_or_default(),
        mois: m("qualgate_mois"), cwe: m("cwe"), title,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(dir: &Path, name: &str, id: &str, extra: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(format!("{name}.yaml")), format!(
            "rules:\n  - id: {id}\n    languages: [go]\n    severity: ERROR\n    message: \"첫 줄 제목\\n둘째 줄\"\n    pattern: foo()\n    metadata:\n      qualgate_id: {id}\n      qualgate_severity: high\n      qualgate_category: security\n{extra}      qualgate_mois: SF-06\n      cwe: \"CWE-798\"\n"
        )).unwrap();
    }

    #[test]
    fn load_reads_kind_and_metadata() {
        let t = tempfile::tempdir().unwrap();
        rule(&t.path().join("go"), "a", "go.a", "      qualgate_kind: hotspot\n");
        rule(&t.path().join("go"), "b", "go.b", "");
        let c = Catalog::load(t.path()).unwrap();
        assert_eq!(c.rules["go.a"].kind, Kind::Hotspot);
        assert_eq!(c.rules["go.b"].kind, Kind::Issue);
        let a = &c.rules["go.a"];
        assert_eq!((a.dir.as_str(), a.severity, a.mois.as_deref(), a.cwe.as_deref()), ("go", Severity::High, Some("SF-06"), Some("CWE-798")));
        assert_eq!(a.title, "첫 줄 제목");
        assert_eq!(c.dir_of("go.a"), Some("go"));
        assert_eq!(c.dir_of("go.zz"), None);
        let j = c.to_json();
        assert_eq!(j.as_array().unwrap().len(), 2);
        assert_eq!(j[0]["id"], "go.a");
        assert_eq!(j[0]["kind"], "hotspot");
    }

    #[test]
    fn id_must_match_dir_and_file() {
        let t = tempfile::tempdir().unwrap();
        rule(&t.path().join("go"), "a", "java.a", "");
        assert!(Catalog::load(t.path()).unwrap_err().to_string().contains("go.a"));
    }

    #[test]
    fn tests_dir_is_not_read() {
        let t = tempfile::tempdir().unwrap();
        rule(&t.path().join("tests").join("go"), "a", "bogus", "");
        std::fs::write(t.path().join("tests").join("x.yaml"), "not: [valid").unwrap();
        rule(&t.path().join("go"), "a", "go.a", "");
        let c = Catalog::load(t.path()).unwrap();
        assert_eq!(c.rules.keys().collect::<Vec<_>>(), ["go.a"]);
    }

    #[test]
    fn no_rule_files_is_an_error() {
        let t = tempfile::tempdir().unwrap();
        assert!(Catalog::load(t.path()).unwrap_err().to_string().contains("QUALGATE_RULES"));
        // 규칙 폴더는 있지만 YAML이 없는 경우, 그리고 존재하지 않는 경로도 오류다.
        std::fs::create_dir_all(t.path().join("go")).unwrap();
        assert!(Catalog::load(t.path()).is_err());
        assert!(Catalog::load(&t.path().join("missing")).is_err());
    }

    #[test]
    fn real_rules_load() {
        let c = Catalog::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("rules")).unwrap();
        assert!(!c.rules.is_empty());
        assert!(c.rules.values().all(|r| RULE_DIRS.contains(&r.dir.as_str())));
        assert_eq!(c.rules["go.math-rand"].kind, Kind::Hotspot);
        assert_eq!(c.rules["go.sql-concat"].kind, Kind::Issue);
    }
}
