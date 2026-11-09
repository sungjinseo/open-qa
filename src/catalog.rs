use crate::model::Severity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
