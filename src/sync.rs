use crate::gitea::{Issue, IssueApi};
use crate::model::ScanResult;
use crate::render::{self, parse_state, IssueState, SUMMARY_KEY};
use crate::track::track;
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};

pub const IGNORED_LABEL: &str = "qualgate:ignored";

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct SyncReport { pub created: usize, pub updated: usize, pub closed: usize, pub reopened: usize, pub skipped: usize }

struct Existing { issue: Issue, state: IssueState }

fn apply_part(api: &mut dyn IssueApi, e: &Existing, title: &str, body: &str, rep: &mut SyncReport, items: &[crate::track::Item]) -> Result<()> {
    let t = track(items, &e.state.items);
    if !e.issue.open {
        api.edit_issue(e.issue.number, title, body, true)?;
        api.comment(e.issue.number, "같은 문제가 다시 발견되어 이슈를 다시 열었습니다.")?;
        rep.reopened += 1;
    } else if e.issue.title != title || e.issue.body != body {
        api.edit_issue(e.issue.number, title, body, true)?;
        if !t.new_items.is_empty() || !t.gone.is_empty() {
            api.comment(e.issue.number, &format!("변경 사항: 신규 {}건, 해결 {}건", t.new_items.len(), t.gone.len()))?;
        }
        rep.updated += 1;
    }
    Ok(())
}

pub fn sync(api: &mut dyn IssueApi, r: &ScanResult, base_url: &str, repo: &str) -> Result<SyncReport> {
    let labels = &r.publish.labels;
    let primary = labels.first().map(String::as_str).unwrap_or("qualgate");
    for l in labels { api.ensure_label(l)?; }
    api.ensure_label(IGNORED_LABEL)?;

    let mut existing: BTreeMap<String, Vec<Existing>> = BTreeMap::new();
    for issue in api.list_issues(primary)? {
        // 상태 주석이 없는 이슈는 사람이 만든 것이므로 건드리지 않는다.
        let Some(state) = parse_state(&issue.body) else { continue };
        existing.entry(state.key.clone()).or_default().push(Existing { issue, state });
    }
    for v in existing.values_mut() { v.sort_by_key(|e| e.state.part); }

    let mut rep = SyncReport::default();
    let mut desired = render::desired(r, base_url, repo);
    desired.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.key.cmp(&b.key)));
    let keys: BTreeSet<String> = desired.iter().map(|d| d.key.clone()).collect();
    let mut created_groups = 0usize;

    for d in &desired {
        let cur = existing.remove(&d.key).unwrap_or_default();
        if cur.iter().any(|e| e.issue.labels.iter().any(|l| l == IGNORED_LABEL)) { continue; }
        if cur.is_empty() {
            if created_groups >= r.publish.max_new_issues { rep.skipped += 1; continue; }
            created_groups += 1;
        }
        for (i, p) in d.parts.iter().enumerate() {
            match cur.get(i) {
                Some(e) => apply_part(api, e, &p.title, &p.body, &mut rep, &p.items)?,
                None => { api.create_issue(&p.title, &p.body, labels)?; rep.created += 1; }
            }
        }
        for e in cur.iter().skip(d.parts.len()) {
            if e.issue.open {
                api.edit_issue(e.issue.number, &e.issue.title, &e.issue.body, false)?;
                rep.closed += 1;
            }
        }
    }

    let summary_existing = existing.remove(SUMMARY_KEY);
    for (key, group) in &existing {
        debug_assert!(!keys.contains(key));
        for e in group {
            let ignored = e.issue.labels.iter().any(|l| l == IGNORED_LABEL);
            let blocked = e.state.items.iter().any(|i| r.unanalyzed.contains(&i.file));
            if !e.issue.open || ignored || blocked { continue; }
            api.comment(e.issue.number, &format!("커밋 `{}`에서 더 이상 발견되지 않아 이슈를 닫습니다.", r.commit))?;
            api.edit_issue(e.issue.number, &e.issue.title, &e.issue.body, false)?;
            rep.closed += 1;
        }
    }

    let (title, body) = render::summary(r, rep.skipped);
    match summary_existing.as_ref().and_then(|v| v.first()) {
        Some(e) if e.issue.title == title && e.issue.body == body && e.issue.open => {}
        Some(e) => { api.edit_issue(e.issue.number, &title, &body, true)?; rep.updated += 1; }
        None => { api.create_issue(&title, &body, labels)?; rep.created += 1; }
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    #[derive(Default)]
    struct Fake { issues: Vec<Issue>, comments: Vec<(u64, String)>, calls: usize, labels: Vec<String>, next: u64 }

    impl IssueApi for Fake {
        fn default_branch(&mut self) -> Result<String> { Ok("main".into()) }
        fn ensure_label(&mut self, n: &str) -> Result<()> { self.labels.push(n.into()); Ok(()) }
        fn list_issues(&mut self, _: &str) -> Result<Vec<Issue>> { Ok(self.issues.clone()) }
        fn create_issue(&mut self, t: &str, b: &str, l: &[String]) -> Result<u64> {
            self.calls += 1;
            self.next += 1;
            self.issues.push(Issue { number: self.next, title: t.into(), body: b.into(), open: true, labels: l.to_vec() });
            Ok(self.next)
        }
        fn edit_issue(&mut self, n: u64, t: &str, b: &str, open: bool) -> Result<()> {
            self.calls += 1;
            let i = self.issues.iter_mut().find(|i| i.number == n).unwrap();
            i.title = t.into(); i.body = b.into(); i.open = open;
            Ok(())
        }
        fn comment(&mut self, n: u64, b: &str) -> Result<()> { self.calls += 1; self.comments.push((n, b.into())); Ok(()) }
    }

    fn f(rule: &str, file: &str, line: u32, sev: Severity) -> Finding {
        Finding { rule: rule.into(), file: file.into(), line, line_hash: format!("h{line}"), message: "m".into(), severity: sev, category: "c".into() }
    }

    fn result(findings: Vec<Finding>) -> ScanResult {
        ScanResult {
            schema: 1, commit: "abc".into(), branch: Some("main".into()), findings, clones: vec![],
            metrics: Metrics::default(), coverage_status: CoverageStatus::default(),
            gate: GateResult { passed: true, conditions: vec![] }, unanalyzed: vec![],
            publish: PublishConfig { max_new_issues: 20, labels: vec!["qualgate".into()] },
        }
    }

    fn run(api: &mut Fake, r: &ScanResult) -> SyncReport { sync(api, r, "http://g", "o/r").unwrap() }

    fn by_key(api: &Fake, key: &str) -> Issue {
        api.issues.iter().find(|i| parse_state(&i.body).is_some_and(|s| s.key == key)).unwrap().clone()
    }

    const K: &str = "rule:r:a.java";

    #[test]
    fn creates_issue_and_summary() {
        let mut a = Fake::default();
        let rep = run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        assert_eq!(rep.created, 2);
        assert!(by_key(&a, K).open);
        assert!(by_key(&a, SUMMARY_KEY).open);
        assert!(a.labels.contains(&IGNORED_LABEL.to_string()));
    }

    #[test]
    fn rerun_without_change_makes_no_calls() {
        let mut a = Fake::default();
        let r = result(vec![f("r", "a.java", 1, Severity::High)]);
        run(&mut a, &r);
        a.calls = 0;
        let rep = run(&mut a, &r);
        assert_eq!(a.calls, 0);
        assert_eq!(rep, SyncReport::default());
    }

    #[test]
    fn fixed_group_is_closed_with_comment() {
        let mut a = Fake::default();
        run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        let rep = run(&mut a, &result(vec![]));
        assert_eq!(rep.closed, 1);
        let i = by_key(&a, K);
        assert!(!i.open);
        assert!(a.comments.iter().any(|(n, _)| *n == i.number));
    }

    #[test]
    fn unanalyzed_file_keeps_issue_open() {
        let mut a = Fake::default();
        run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        let mut r = result(vec![]);
        r.unanalyzed = vec!["a.java".into()];
        let rep = run(&mut a, &r);
        assert_eq!(rep.closed, 0);
        assert!(by_key(&a, K).open);
    }

    #[test]
    fn recurrence_reopens_with_comment() {
        let mut a = Fake::default();
        let r = result(vec![f("r", "a.java", 1, Severity::High)]);
        run(&mut a, &r);
        run(&mut a, &result(vec![]));
        let rep = run(&mut a, &r);
        assert_eq!(rep.reopened, 1);
        assert!(by_key(&a, K).open);
    }

    #[test]
    fn ignored_label_is_left_alone() {
        let mut a = Fake::default();
        let r = result(vec![f("r", "a.java", 1, Severity::High)]);
        run(&mut a, &r);
        let n = by_key(&a, K).number;
        let i = a.issues.iter_mut().find(|i| i.number == n).unwrap();
        i.open = false;
        i.labels.push(IGNORED_LABEL.into());
        let rep = run(&mut a, &r);
        assert_eq!(rep.reopened, 0);
        assert!(!by_key(&a, K).open);
    }

    #[test]
    fn new_issue_cap_prefers_high_severity_and_reports_skipped() {
        let mut a = Fake::default();
        let mut r = result(vec![f("low", "a.java", 1, Severity::Low), f("crit", "a.java", 2, Severity::Critical)]);
        r.publish.max_new_issues = 1;
        let rep = run(&mut a, &r);
        assert_eq!(rep.skipped, 1);
        assert!(a.issues.iter().any(|i| parse_state(&i.body).is_some_and(|s| s.key == "rule:crit:a.java")));
        assert!(!a.issues.iter().any(|i| parse_state(&i.body).is_some_and(|s| s.key == "rule:low:a.java")));
        assert!(by_key(&a, SUMMARY_KEY).body.contains("1개"));
    }

    #[test]
    fn change_comment_counts_new_and_gone() {
        let mut a = Fake::default();
        run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        run(&mut a, &result(vec![f("r", "a.java", 2, Severity::High), f("r", "a.java", 3, Severity::High)]));
        let i = by_key(&a, K);
        assert!(a.comments.iter().any(|(n, c)| *n == i.number && c.contains("신규 2건") && c.contains("해결 1건")));
    }

    #[test]
    fn human_issues_are_ignored() {
        let mut a = Fake::default();
        a.issues.push(Issue { number: 900, title: "x".into(), body: "사람이 쓴 이슈".into(), open: true, labels: vec!["qualgate".into()] });
        a.next = 900;
        run(&mut a, &result(vec![]));
        assert!(a.issues.iter().find(|i| i.number == 900).unwrap().open);
    }
}
