use crate::model::Severity;
use crate::sync::IGNORED_LABEL;
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use std::path::Path;

fn s(v: &[&str]) -> Vec<String> { v.iter().map(|x| x.to_string()).collect() }

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScanCfg { pub exclude: Vec<String>, pub test_paths: Vec<String> }
impl Default for ScanCfg {
    fn default() -> Self {
        Self {
            exclude: s(&["**/node_modules/**", "**/build/**", "**/dist/**", "**/vendor/**"]),
            test_paths: s(&["**/src/test/**", "**/*_test.go", "**/test_*.py", "**/*.spec.*", "**/*.test.*"]),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RulesCfg { pub extra_paths: Vec<String>, pub disabled: Vec<String> }

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DupCfg { pub block_lines: usize, pub min_tokens: usize }
impl Default for DupCfg { fn default() -> Self { Self { block_lines: 10, min_tokens: 100 } } }

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoverageCfg {
    pub jacoco: Vec<String>,
    pub cobertura: Vec<String>,
    pub lcov: Vec<String>,
    pub go: Vec<String>,
    pub required: bool,
}
impl Default for CoverageCfg {
    fn default() -> Self {
        Self {
            jacoco: s(&["**/build/reports/jacoco/**/*.xml", "**/target/site/jacoco/jacoco.xml"]),
            cobertura: s(&["coverage.xml"]),
            lcov: s(&["coverage/lcov.info"]),
            go: s(&["coverage.out"]),
            required: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NewCodeCfg { pub days: u32 }
impl Default for NewCodeCfg { fn default() -> Self { Self { days: 30 } } }

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GateCfg {
    pub new_coverage_min: f64,
    pub new_duplication_max: f64,
    pub new_blocking_max: u64,
    pub blocking_severities: Vec<String>,
}
impl Default for GateCfg {
    fn default() -> Self {
        Self { new_coverage_min: 80.0, new_duplication_max: 3.0, new_blocking_max: 0, blocking_severities: s(&["critical", "high"]) }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PublishCfg { pub max_new_issues: usize, pub labels: Vec<String> }
impl Default for PublishCfg { fn default() -> Self { Self { max_new_issues: 20, labels: vec![] } } }

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub scan: ScanCfg,
    pub rules: RulesCfg,
    pub duplication: DupCfg,
    pub coverage: CoverageCfg,
    pub new_code: NewCodeCfg,
    pub gate: GateCfg,
    pub publish: PublishCfg,
}

impl Config {
    pub fn load(src: &Path) -> Result<Config> {
        let p = src.join(".qualgate.toml");
        // 저장소는 신뢰하지 않는다. 심볼릭 링크를 따라가면 /proc/self/environ 같은 파일을 읽게 된다.
        let meta = match std::fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(e) => return Err(e).with_context(|| format!("{} 읽기 실패", p.display())),
        };
        ensure!(meta.is_file(), "{}은(는) 일반 파일이어야 합니다 (심볼릭 링크 불가)", p.display());
        let text = std::fs::read_to_string(&p).with_context(|| format!("{} 읽기 실패", p.display()))?;
        let mut cfg: Config = toml::from_str(&text).map_err(|e| {
            // 파서 오류 메시지는 원문 줄을 포함하므로 위치만 알린다.
            let (line, col) = e.span().map_or((0, 0), |r| position(&text, r.start));
            anyhow::anyhow!("{} 형식 오류 (줄 {line}, 열 {col})", p.display())
        })?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&mut self) -> Result<()> {
        for sev in &self.gate.blocking_severities {
            // 오타를 무시하면 해당 등급이 차단되지 않아 게이트가 열린다.
            if Severity::parse(sev).is_none() { bail!("gate.blocking_severities 값이 잘못되었습니다: {sev}"); }
        }
        ensure!(self.duplication.block_lines >= 1, "duplication.block_lines는 1 이상이어야 합니다");
        ensure!(self.duplication.min_tokens >= 1, "duplication.min_tokens는 1 이상이어야 합니다");
        if self.publish.labels.iter().any(|l| l.eq_ignore_ascii_case(IGNORED_LABEL)) {
            bail!("publish.labels에 {IGNORED_LABEL}을(를) 넣을 수 없습니다");
        }
        let n = self.publish.max_new_issues.clamp(1, 100);
        if n != self.publish.max_new_issues {
            eprintln!("경고: publish.max_new_issues {}을(를) {n}(으)로 조정합니다 (허용 범위 1~100)", self.publish.max_new_issues);
            self.publish.max_new_issues = n;
        }
        Ok(())
    }
}

/// 바이트 위치를 1부터 시작하는 줄과 열로 바꾼다.
fn position(text: &str, offset: usize) -> (usize, usize) {
    let before = &text.as_bytes()[..offset.min(text.len())];
    let line = before.iter().filter(|b| **b == b'\n').count() + 1;
    let col = before.iter().rev().take_while(|b| **b != b'\n').count() + 1;
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_uses_defaults() {
        let d = tempfile::tempdir().unwrap();
        let c = Config::load(d.path()).unwrap();
        assert_eq!(c.duplication.block_lines, 10);
        assert_eq!(c.publish.max_new_issues, 20);
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join(".qualgate.toml"), "[gate]\nnew_coverage_min = 60.0\n").unwrap();
        let c = Config::load(d.path()).unwrap();
        assert_eq!(c.gate.new_coverage_min, 60.0);
        assert_eq!(c.gate.new_duplication_max, 3.0);
    }

    #[test]
    fn unknown_key_is_error() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join(".qualgate.toml"), "[gate]\nbogus = 1\n").unwrap();
        assert!(Config::load(d.path()).is_err());
    }

    fn load_str(text: &str) -> Result<Config> {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join(".qualgate.toml"), text).unwrap();
        Config::load(d.path())
    }

    #[test]
    fn toml_error_reports_position_without_source() {
        let e = load_str("[gate]\nsecret-value-xyz = = 1\n").unwrap_err();
        let m = format!("{e:#}");
        assert!(m.contains("줄 2"), "{m}");
        assert!(!m.contains("secret-value-xyz"), "{m}");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_config_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let t = d.path().join("real.toml");
        std::fs::write(&t, "").unwrap();
        std::os::unix::fs::symlink(&t, d.path().join(".qualgate.toml")).unwrap();
        assert!(Config::load(d.path()).is_err());
    }

    #[test]
    fn typo_in_blocking_severities_is_error() {
        assert!(load_str("[gate]\nblocking_severities = [\"critcal\"]\n").is_err());
        assert!(load_str("[gate]\nblocking_severities = [\"HIGH\"]\n").is_ok());
    }

    #[test]
    fn duplication_sizes_must_be_positive() {
        assert!(load_str("[duplication]\nblock_lines = 0\n").is_err());
        assert!(load_str("[duplication]\nmin_tokens = 0\n").is_err());
    }

    #[test]
    fn ignored_label_is_refused_and_empty_labels_are_fine() {
        assert!(load_str("[publish]\nlabels = [\"qualgate:ignored\"]\n").is_err());
        assert!(load_str("[publish]\nlabels = []\n").is_ok());
        assert!(Config::default().publish.labels.is_empty());
    }

    #[test]
    fn max_new_issues_is_clamped() {
        assert_eq!(load_str("[publish]\nmax_new_issues = 0\n").unwrap().publish.max_new_issues, 1);
        assert_eq!(load_str("[publish]\nmax_new_issues = 5000\n").unwrap().publish.max_new_issues, 100);
        assert_eq!(load_str("[publish]\nmax_new_issues = 7\n").unwrap().publish.max_new_issues, 7);
    }
}
