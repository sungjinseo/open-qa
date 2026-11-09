use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// `result.json` 형식 버전. `publish`는 다른 버전을 거부한다.
pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity { Info, Low, Medium, High, Critical }

impl Severity {
    pub fn parse(s: &str) -> Option<Severity> {
        match s.to_ascii_lowercase().as_str() {
            "info" => Some(Severity::Info),
            "low" => Some(Severity::Low),
            "medium" => Some(Severity::Medium),
            "high" => Some(Severity::High),
            "critical" => Some(Severity::Critical),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang { Java, Kotlin, Python, Js, Ts, Tsx, Go, Vue }

impl Lang {
    pub fn of(path: &str) -> Option<Lang> {
        // 확장자 없는 파일(`go`, `py`)은 코드가 아니다.
        // 점으로 시작하는 이름(`.java`)도 확장자가 아니라 이름이므로 줄기가 비어 있으면 제외한다.
        let (stem, ext) = path.rsplit('/').next()?.rsplit_once('.')?;
        if stem.is_empty() { return None; }
        let ext = ext.to_ascii_lowercase();
        Some(match ext.as_str() {
            "java" => Lang::Java,
            "kt" | "kts" => Lang::Kotlin,
            "py" => Lang::Python,
            "js" | "jsx" | "mjs" | "cjs" => Lang::Js,
            "ts" | "mts" | "cts" => Lang::Ts,
            "tsx" => Lang::Tsx,
            "go" => Lang::Go,
            "vue" => Lang::Vue,
            _ => return None,
        })
    }
    /// 복제 탐지에서 서로 비교할 언어 묶음
    pub fn dup_group(self) -> &'static str {
        match self {
            Lang::Java => "java",
            Lang::Kotlin => "kotlin",
            Lang::Python => "python",
            Lang::Go => "go",
            Lang::Js | Lang::Ts | Lang::Tsx | Lang::Vue => "web",
        }
    }
}

pub fn line_hash(line: &str) -> String {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    let digest = Sha256::digest(compact.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub rule: String,
    pub file: String,
    pub line: u32,
    pub line_hash: String,
    pub message: String,
    pub severity: Severity,
    pub category: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ClonePart { pub file: String, pub start_line: u32, pub end_line: u32 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneGroup { pub key: String, pub parts: Vec<ClonePart>, pub lines: u32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LineCov { pub covered: bool, pub conditions: u32, pub covered_conditions: u32 }

pub type CoverageMap = BTreeMap<String, BTreeMap<u32, LineCov>>;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub coverage: Option<f64>,
    pub new_coverage: Option<f64>,
    pub duplication: Option<f64>,
    pub new_duplication: Option<f64>,
    pub new_blocking: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CoverageStatus {
    pub reports: Vec<String>,
    pub unresolved: Vec<String>,
    pub not_in_report: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConditionStatus { Passed, Failed, Skipped }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub name: String,
    pub actual: Option<f64>,
    pub limit: f64,
    pub status: ConditionStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateResult { pub passed: bool, pub conditions: Vec<Condition> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishConfig { pub max_new_issues: usize, pub labels: Vec<String> }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanResult {
    pub schema: u32,
    pub commit: String,
    pub branch: Option<String>,
    pub findings: Vec<Finding>,
    pub clones: Vec<CloneGroup>,
    pub metrics: Metrics,
    pub coverage_status: CoverageStatus,
    pub gate: GateResult,
    pub unanalyzed: Vec<String>,
    pub publish: PublishConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_hash_ignores_whitespace() {
        assert_eq!(line_hash("  a = 1;\t"), line_hash("a=1;"));
        assert_ne!(line_hash("a=1;"), line_hash("a=2;"));
    }

    #[test]
    fn lang_groups_web_together() {
        assert_eq!(Lang::of("a/B.tsx").unwrap().dup_group(), "web");
        assert_eq!(Lang::of("a/B.vue").unwrap().dup_group(), "web");
        assert_eq!(Lang::of("a/B.KT"), Some(Lang::Kotlin));
        assert_eq!(Lang::of("a/README.md"), None);
        assert_eq!(Lang::of("bin/go"), None);
        assert_eq!(Lang::of("a.b/py"), None);
        assert_eq!(Lang::of("java"), None);
        assert_eq!(Lang::of("src/java"), None);
        assert_eq!(Lang::of(".java"), None);
        assert_eq!(Lang::of("a/.py"), None);
        assert_eq!(Lang::of("a.b/C.java"), Some(Lang::Java));
    }

    #[test]
    fn severity_orders_by_risk() {
        assert!(Severity::Critical > Severity::High);
        assert_eq!(Severity::parse("HIGH"), Some(Severity::High));
    }
}
