use crate::sync::IGNORED_LABEL;
use anyhow::{bail, ensure, Result};
use serde::Deserialize;

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
pub struct PublishCfg { pub max_new_issues: usize, pub labels: Vec<String> }
impl Default for PublishCfg { fn default() -> Self { Self { max_new_issues: 20, labels: vec![] } } }

/// 중복도 설정 검증. 설정 저장소의 프로젝트 파일 검증이 쓴다.
pub fn validate_dup(d: &DupCfg) -> Result<()> {
    ensure!(d.block_lines >= 1, "duplication.block_lines는 1 이상이어야 합니다");
    ensure!(d.min_tokens >= 1, "duplication.min_tokens는 1 이상이어야 합니다");
    Ok(())
}

/// 게시 설정 검증. `max_new_issues`는 1~100으로 조정한다.
pub fn validate_publish(p: &mut PublishCfg) -> Result<()> {
    if p.labels.iter().any(|l| l.eq_ignore_ascii_case(IGNORED_LABEL)) {
        bail!("publish.labels에 {IGNORED_LABEL}을(를) 넣을 수 없습니다");
    }
    let n = p.max_new_issues.clamp(1, 100);
    if n != p.max_new_issues {
        eprintln!("경고: publish.max_new_issues {}을(를) {n}(으)로 조정합니다 (허용 범위 1~100)", p.max_new_issues);
        p.max_new_issues = n;
    }
    Ok(())
}

/// 바이트 위치를 1부터 시작하는 줄과 열로 바꾼다.
pub(crate) fn position(text: &str, offset: usize) -> (usize, usize) {
    let before = &text.as_bytes()[..offset.min(text.len())];
    let line = before.iter().filter(|b| **b == b'\n').count() + 1;
    let col = before.iter().rev().take_while(|b| **b != b'\n').count() + 1;
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_documented_values() {
        assert_eq!(DupCfg::default().block_lines, 10);
        assert_eq!(PublishCfg::default().max_new_issues, 20);
        assert!(PublishCfg::default().labels.is_empty());
    }

    #[test]
    fn duplication_sizes_must_be_positive() {
        assert!(validate_dup(&DupCfg { block_lines: 0, min_tokens: 100 }).is_err());
        assert!(validate_dup(&DupCfg { block_lines: 10, min_tokens: 0 }).is_err());
        assert!(validate_dup(&DupCfg::default()).is_ok());
    }

    fn publish(max: usize, labels: &[&str]) -> Result<usize> {
        let mut p = PublishCfg { max_new_issues: max, labels: labels.iter().map(|s| s.to_string()).collect() };
        validate_publish(&mut p).map(|_| p.max_new_issues)
    }

    #[test]
    fn ignored_label_is_refused_and_empty_labels_are_fine() {
        assert!(publish(20, &["qualgate:ignored"]).is_err());
        assert!(publish(20, &["QualGate:Ignored"]).is_err());
        assert!(publish(20, &[]).is_ok());
    }

    #[test]
    fn max_new_issues_is_clamped() {
        assert_eq!(publish(0, &[]).unwrap(), 1);
        assert_eq!(publish(5000, &[]).unwrap(), 100);
        assert_eq!(publish(7, &[]).unwrap(), 7);
    }

    #[test]
    fn position_is_one_based() {
        assert_eq!(position("ab\ncd", 4), (2, 2));
        assert_eq!(position("ab", 99), (1, 3));
    }
}
