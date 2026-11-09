use crate::catalog::Kind;
use crate::model::Severity;
use serde::Deserialize;
use std::collections::BTreeMap;

fn yes() -> bool { true }

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageSel {
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub disabled_rules: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleOverride {
    #[serde(default)]
    pub severity: Option<Severity>,
    #[serde(default)]
    pub kind: Option<Kind>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ruleset {
    pub schema: u32,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub languages: BTreeMap<String, LanguageSel>,
    #[serde(default)]
    pub overrides: BTreeMap<String, RuleOverride>,
}
