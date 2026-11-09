//! 설정 저장소(전역 설정, 룰셋, 프로젝트)의 형식과 검증.
mod global;
mod project;
mod ruleset;
mod validate;

pub use global::Global;
pub use project::Project;
pub use ruleset::{LanguageSel, RuleOverride, Ruleset};
pub use validate::{load_dir_all, resolve};

use serde::Deserialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    NewIssues,
    NewHotspotsReviewed,
    NewCoverage,
    NewDuplicatedLines,
    Issues,
    Coverage,
    DuplicatedLines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Op { Gt, Lt }

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateCondition { pub metric: Metric, pub op: Op, pub value: f64 }

/// 기본 게이트. 전역 `default_gate`가 비어 있을 때 쓴다.
pub const DEFAULT_GATE: [(Metric, Op, f64); 4] = [
    (Metric::NewIssues, Op::Gt, 0.0),
    (Metric::NewHotspotsReviewed, Op::Lt, 100.0),
    (Metric::NewCoverage, Op::Lt, 80.0),
    (Metric::NewDuplicatedLines, Op::Gt, 3.0),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError { pub file: String, pub message: String }

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}: {}", self.file, self.message) }
}
impl std::error::Error for ConfigError {}

#[derive(Debug, Clone)]
pub struct Resolved { pub global: Global, pub ruleset: Ruleset, pub project: Project, pub registered: bool }
