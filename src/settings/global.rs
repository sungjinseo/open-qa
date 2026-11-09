use super::GateCondition;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Global {
    pub schema: u32,
    /// 비어 있으면 미등록 저장소는 오류다.
    #[serde(default)]
    pub default_ruleset: String,
    #[serde(default)]
    pub default_gate: Vec<GateCondition>,
}
