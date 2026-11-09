//! 게이트 조건 판정(스펙 3.5). 조건은 "참이면 실패"다.
use crate::model::Metrics;
use crate::settings::{GateCondition, Metric, Op};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConditionStatus { Passed, Failed, Skipped }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub metric: Metric,
    pub op: Op,
    pub value: f64,
    pub actual: Option<f64>,
    pub status: ConditionStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateResult { pub passed: bool, pub conditions: Vec<Condition> }

/// 신규 핫스팟 중 검토된 비율(%). 핫스팟이 없으면 100(SonarQube와 같음).
pub fn hotspots_reviewed_pct(new_hotspots: u64, reviewed: u64) -> f64 {
    if new_hotspots == 0 { 100.0 } else { 100.0 * reviewed as f64 / new_hotspots as f64 }
}

fn actual_of(metric: Metric, m: &Metrics, reviewed_pct: Option<f64>) -> Option<f64> {
    match metric {
        Metric::NewIssues => m.new_issues.map(|v| v as f64),
        Metric::NewHotspotsReviewed => reviewed_pct,
        Metric::NewCoverage => m.new_coverage,
        Metric::NewDuplicatedLines => m.new_duplicated_lines,
        Metric::Issues => Some(m.issues as f64),
        Metric::Coverage => m.coverage,
        Metric::DuplicatedLines => m.duplicated_lines,
    }
}

/// 조건을 판정한다. 측정하지 못한 지표의 조건은 건너뛰지만, `coverage_required_missing`이면
/// 커버리지 조건(`new_coverage`, `coverage`)은 실패다.
pub fn evaluate(conds: &[GateCondition], m: &Metrics, reviewed_pct: Option<f64>, coverage_required_missing: bool) -> GateResult {
    let conditions: Vec<Condition> = conds.iter().map(|c| {
        let actual = actual_of(c.metric, m, reviewed_pct);
        let cov_missing = coverage_required_missing && matches!(c.metric, Metric::NewCoverage | Metric::Coverage);
        let status = match actual {
            _ if cov_missing => ConditionStatus::Failed,
            None => ConditionStatus::Skipped,
            Some(a) => {
                let fails = match c.op { Op::Gt => a > c.value, Op::Lt => a < c.value };
                if fails { ConditionStatus::Failed } else { ConditionStatus::Passed }
            }
        };
        Condition { metric: c.metric, op: c.op, value: c.value, actual, status }
    }).collect();
    let passed = conditions.iter().all(|c| c.status != ConditionStatus::Failed);
    GateResult { passed, conditions }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cond(metric: Metric, op: Op, value: f64) -> GateCondition { GateCondition { metric, op, value } }

    fn one(c: GateCondition, m: &Metrics) -> ConditionStatus { evaluate(&[c], m, None, false).conditions[0].status }

    #[test]
    fn gt_fails_above_and_lt_fails_below_with_boundary_passing() {
        let gt = cond(Metric::NewIssues, Op::Gt, 0.0);
        assert_eq!(one(gt.clone(), &Metrics { new_issues: Some(1), ..Default::default() }), ConditionStatus::Failed);
        assert_eq!(one(gt, &Metrics { new_issues: Some(0), ..Default::default() }), ConditionStatus::Passed);
        let lt = cond(Metric::NewCoverage, Op::Lt, 80.0);
        assert_eq!(one(lt.clone(), &Metrics { new_coverage: Some(79.9), ..Default::default() }), ConditionStatus::Failed);
        assert_eq!(one(lt, &Metrics { new_coverage: Some(80.0), ..Default::default() }), ConditionStatus::Passed);
    }

    #[test]
    fn unmeasured_metric_is_skipped_and_gate_passes() {
        let r = evaluate(&[cond(Metric::NewCoverage, Op::Lt, 80.0), cond(Metric::NewIssues, Op::Gt, 0.0)], &Metrics::default(), None, false);
        assert!(r.passed);
        assert!(r.conditions.iter().all(|c| c.status == ConditionStatus::Skipped && c.actual.is_none()));
    }

    #[test]
    fn required_but_missing_coverage_fails_coverage_conditions_only() {
        let conds = [cond(Metric::NewCoverage, Op::Lt, 80.0), cond(Metric::Coverage, Op::Lt, 10.0), cond(Metric::DuplicatedLines, Op::Gt, 3.0)];
        let r = evaluate(&conds, &Metrics::default(), None, true);
        assert!(!r.passed);
        let s: Vec<_> = r.conditions.iter().map(|c| c.status).collect();
        assert_eq!(s, [ConditionStatus::Failed, ConditionStatus::Failed, ConditionStatus::Skipped]);
    }

    #[test]
    fn reviewed_pct_counts_zero_hotspots_as_fully_reviewed() {
        assert_eq!(hotspots_reviewed_pct(0, 0), 100.0);
        assert_eq!(hotspots_reviewed_pct(4, 1), 25.0);
    }

    #[test]
    fn hotspots_reviewed_condition_uses_the_argument() {
        let c = cond(Metric::NewHotspotsReviewed, Op::Lt, 100.0);
        let m = Metrics::default();
        assert_eq!(evaluate(std::slice::from_ref(&c), &m, None, false).conditions[0].status, ConditionStatus::Skipped);
        assert_eq!(evaluate(std::slice::from_ref(&c), &m, Some(25.0), false).conditions[0].status, ConditionStatus::Failed);
        let r = evaluate(&[c], &m, Some(100.0), false);
        assert_eq!((r.passed, r.conditions[0].actual), (true, Some(100.0)));
    }

    #[test]
    fn total_metrics_map_to_their_fields() {
        let m = Metrics { issues: 2, coverage: Some(50.0), duplicated_lines: Some(5.0), ..Default::default() };
        let conds = [cond(Metric::Issues, Op::Gt, 1.0), cond(Metric::Coverage, Op::Lt, 60.0), cond(Metric::DuplicatedLines, Op::Gt, 5.0)];
        let r = evaluate(&conds, &m, None, false);
        let a: Vec<_> = r.conditions.iter().map(|c| c.actual).collect();
        assert_eq!(a, [Some(2.0), Some(50.0), Some(5.0)]);
        let s: Vec<_> = r.conditions.iter().map(|c| c.status).collect();
        assert_eq!(s, [ConditionStatus::Failed, ConditionStatus::Failed, ConditionStatus::Passed]);
    }

    #[test]
    fn empty_conditions_pass() {
        let r = evaluate(&[], &Metrics::default(), None, true);
        assert!(r.passed && r.conditions.is_empty());
    }
}
