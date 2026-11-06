use anyhow::{Context, Result};
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
impl Default for PublishCfg { fn default() -> Self { Self { max_new_issues: 20, labels: s(&["qualgate"]) } } }

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
        if !p.exists() { return Ok(Config::default()); }
        let text = std::fs::read_to_string(&p).with_context(|| format!("{} 읽기 실패", p.display()))?;
        toml::from_str(&text).with_context(|| format!("{} 형식 오류", p.display()))
    }
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
}
