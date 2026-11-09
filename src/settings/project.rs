use super::GateCondition;
use crate::config::{CoverageCfg, DupCfg, PublishCfg, ScanCfg};
use serde::Deserialize;

fn days() -> u32 { 30 }

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema: u32,
    pub repo: String,
    pub ruleset: String,
    #[serde(default)]
    pub languages: Option<Vec<String>>,
    #[serde(default = "days")]
    pub new_code_days: u32,
    #[serde(default)]
    pub gate: Vec<GateCondition>,
    #[serde(default)]
    pub scan: ScanCfg,
    #[serde(default)]
    pub coverage: CoverageCfg,
    #[serde(default)]
    pub duplication: DupCfg,
    #[serde(default)]
    pub publish: PublishCfg,
}

impl Project {
    /// 프로젝트 파일이 없는 저장소의 기본값.
    pub fn unregistered(repo: &str, ruleset: &str, gate: Vec<GateCondition>) -> Project {
        Project {
            schema: 1,
            repo: repo.to_string(),
            ruleset: ruleset.to_string(),
            languages: None,
            new_code_days: days(),
            gate,
            scan: ScanCfg::default(),
            coverage: CoverageCfg::default(),
            duplication: DupCfg::default(),
            publish: PublishCfg::default(),
        }
    }
}
