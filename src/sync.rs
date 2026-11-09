use crate::gitea::{Issue, IssueApi};
use crate::model::ScanResult;
use crate::render::{self, parse_issue_state, IssueState, RENDER_VERSION, SUMMARY_KEY};
use crate::track::track;
use anyhow::Result;
use std::collections::BTreeMap;

pub const IGNORED_LABEL: &str = "qualgate:ignored";
/// 항상 붙이고 이슈를 찾을 때 유일하게 쓰는 라벨. 설정의 라벨을 바꿔도 이슈를 잃지 않는다.
pub const BASE_LABEL: &str = "qualgate";

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct SyncReport { pub created: usize, pub updated: usize, pub closed: usize, pub reopened: usize, pub skipped: usize }

struct Existing { issue: Issue, state: IssueState }

fn apply_part(api: &mut dyn IssueApi, e: &Existing, title: &str, body: &str, rep: &mut SyncReport, items: &[crate::track::Item]) -> Result<()> {
    let t = track(items, &e.state.items);
    if !e.issue.open {
        api.edit_issue(e.issue.number, title, body, true)?;
        api.comment(e.issue.number, "같은 문제가 다시 발견되어 이슈를 다시 열었습니다.")?;
        rep.reopened += 1;
    } else if e.issue.title != title || e.state.items != items || e.state.v != RENDER_VERSION {
        // 링크에 커밋이 들어가 본문은 커밋마다 달라지므로 본문이 아니라 항목과 렌더 버전을 비교한다.
        api.edit_issue(e.issue.number, title, body, true)?;
        if !t.new_items.is_empty() || !t.gone.is_empty() {
            api.comment(e.issue.number, &format!("변경 사항: 신규 {}건, 해결 {}건", t.new_items.len(), t.gone.len()))?;
        }
        rep.updated += 1;
    }
    Ok(())
}

fn is_ignored(e: &Existing) -> bool { e.issue.labels.iter().any(|l| l == IGNORED_LABEL) }

fn close(api: &mut dyn IssueApi, e: &Existing, why: &str, rep: &mut SyncReport) -> Result<()> {
    // 먼저 닫는다. 닫기가 실패했는데 댓글만 남으면 다음 실행에서 댓글이 반복된다.
    // 닫은 뒤 댓글이 실패하면 오류를 돌려주지만 이슈는 이미 닫혀 다음 실행이 반복하지 않는다.
    api.edit_issue(e.issue.number, &e.issue.title, &e.issue.body, false)?;
    rep.closed += 1;
    api.comment(e.issue.number, why)?;
    Ok(())
}

pub fn sync(api: &mut dyn IssueApi, r: &ScanResult, base_url: &str, repo: &str) -> Result<SyncReport> {
    let mut labels = vec![BASE_LABEL.to_string()];
    for l in &r.publish.labels {
        if !labels.contains(l) { labels.push(l.clone()); }
    }
    let labels = &labels;
    for l in labels { api.ensure_label(l)?; }
    api.ensure_label(IGNORED_LABEL)?;

    let mut existing: BTreeMap<String, Vec<Existing>> = BTreeMap::new();
    for issue in api.list_issues(BASE_LABEL)? {
        // 상태 주석이 없는 이슈는 사람이 만든 것이므로 건드리지 않는다.
        let Some(state) = parse_issue_state(&issue.title, &issue.body) else { continue };
        existing.entry(state.key.clone()).or_default().push(Existing { issue, state });
    }
    for v in existing.values_mut() { v.sort_by_key(|e| e.state.part); }

    let mut rep = SyncReport::default();
    let mut desired = render::desired(r, base_url, repo);
    desired.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.key.cmp(&b.key)));
    let mut created_groups = 0usize;

    for d in &desired {
        let cur = existing.remove(&d.key).unwrap_or_default();
        if cur.iter().any(is_ignored) { continue; }
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
        // 분석하지 못한 파일의 항목이 이 그룹에 있으면 남는 파트를 닫지 않는다.
        let blocked = cur.iter().flat_map(|e| e.state.items.iter())
            .chain(d.parts.iter().flat_map(|p| p.items.iter()))
            .any(|i| r.unanalyzed.contains(&i.file));
        if blocked { continue; }
        for e in cur.iter().skip(d.parts.len()) {
            if e.issue.open {
                close(api, e, &format!("커밋 `{}`에서 항목이 줄어 이 파트가 더 필요하지 않아 이슈를 닫습니다.", r.commit), &mut rep)?;
            }
        }
    }

    let summary_existing = existing.remove(SUMMARY_KEY);
    for group in existing.values() {
        let blocked = group.iter().flat_map(|e| e.state.items.iter()).any(|i| r.unanalyzed.contains(&i.file));
        if blocked || group.iter().any(is_ignored) { continue; }
        for e in group.iter().filter(|e| e.issue.open) {
            close(api, e, &format!("커밋 `{}`에서 더 이상 발견되지 않아 이슈를 닫습니다.", r.commit), &mut rep)?;
        }
    }

    let summary_ignored = summary_existing.as_ref().is_some_and(|v| v.iter().any(is_ignored));
    if !summary_ignored {
        let (title, body) = render::summary(r, rep.skipped);
        match summary_existing.as_ref().and_then(|v| v.first()) {
            Some(e) if e.issue.title == title && render::same_except_commit(&e.issue.body, &body) && e.issue.open => {}
            Some(e) => { api.edit_issue(e.issue.number, &title, &body, true)?; rep.updated += 1; }
            None => { api.create_issue(&title, &body, labels)?; rep.created += 1; }
        }
        // 요약 이슈는 하나만 관리한다. 더 있으면 닫는다.
        for e in summary_existing.iter().flatten().skip(1).filter(|e| e.issue.open) {
            close(api, e, "요약 이슈가 중복되어 이 이슈를 닫습니다.", &mut rep)?;
        }
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use crate::render::parse_state;

    #[derive(Default)]
    struct Fake { issues: Vec<Issue>, comments: Vec<(u64, String)>, calls: usize, labels: Vec<String>, listed: Vec<String>, next: u64, fail_edit: bool, fail_comment: bool }

    impl IssueApi for Fake {
        fn default_branch(&mut self) -> Result<String> { Ok("main".into()) }
        fn ensure_label(&mut self, n: &str) -> Result<()> { self.labels.push(n.into()); Ok(()) }
        fn list_issues(&mut self, l: &str) -> Result<Vec<Issue>> { self.listed.push(l.into()); Ok(self.issues.clone()) }
        fn create_issue(&mut self, t: &str, b: &str, l: &[String]) -> Result<u64> {
            self.calls += 1;
            self.next += 1;
            self.issues.push(Issue { number: self.next, title: t.into(), body: b.into(), open: true, labels: l.to_vec() });
            Ok(self.next)
        }
        fn edit_issue(&mut self, n: u64, t: &str, b: &str, open: bool) -> Result<()> {
            if self.fail_edit { anyhow::bail!("edit 실패"); }
            self.calls += 1;
            let i = self.issues.iter_mut().find(|i| i.number == n).unwrap();
            i.title = t.into(); i.body = b.into(); i.open = open;
            Ok(())
        }
        fn comment(&mut self, n: u64, b: &str) -> Result<()> { if self.fail_comment { anyhow::bail!("comment 실패"); } self.calls += 1; self.comments.push((n, b.into())); Ok(()) }
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

    fn many(n: u32) -> Vec<Finding> { (0..n).map(|i| f("r", "a.java", i + 1, Severity::High)).collect() }

    #[test]
    fn surplus_part_is_closed_with_comment() {
        let mut a = Fake::default();
        let mut big = many(2000);
        for x in &mut big { x.message = "x".repeat(100); }
        run(&mut a, &result(big));
        let parts = a.issues.iter().filter(|i| parse_state(&i.body).is_some_and(|s| s.key == K)).count();
        assert!(parts > 1);
        let rep = run(&mut a, &result(many(1)));
        assert_eq!(rep.closed, parts - 1);
        let closed: Vec<&Issue> = a.issues.iter().filter(|i| !i.open).collect();
        assert_eq!(closed.len(), parts - 1);
        for i in closed { assert!(a.comments.iter().any(|(n, _)| *n == i.number)); }
    }

    #[test]
    fn blocked_surplus_part_stays_open() {
        let mut a = Fake::default();
        let mut big = many(2000);
        for x in &mut big { x.message = "x".repeat(100); }
        run(&mut a, &result(big));
        let mut r = result(many(1));
        r.unanalyzed = vec!["a.java".into()];
        let rep = run(&mut a, &r);
        assert_eq!(rep.closed, 0);
        assert!(a.issues.iter().all(|i| i.open));
    }

    #[test]
    fn ignored_summary_is_left_alone() {
        let mut a = Fake::default();
        run(&mut a, &result(vec![]));
        let n = by_key(&a, SUMMARY_KEY).number;
        let before = by_key(&a, SUMMARY_KEY);
        let i = a.issues.iter_mut().find(|i| i.number == n).unwrap();
        i.labels.push(IGNORED_LABEL.into());
        a.calls = 0;
        let mut r = result(vec![]);
        r.commit = "other".into();
        let rep = run(&mut a, &r);
        assert_eq!(a.calls, 0);
        assert_eq!(rep.updated, 0);
        assert_eq!(by_key(&a, SUMMARY_KEY).body, before.body);
    }

    #[test]
    fn new_commit_with_same_findings_makes_no_calls() {
        let mut a = Fake::default();
        let r = result(vec![f("r", "a.java", 1, Severity::High)]);
        run(&mut a, &r);
        a.calls = 0;
        let mut next = r.clone();
        next.commit = "def".into();
        let rep = run(&mut a, &next);
        assert_eq!(a.calls, 0);
        assert_eq!(rep, SyncReport::default());
    }

    #[test]
    fn summary_updates_when_metrics_or_gate_change() {
        let mut a = Fake::default();
        let r = result(vec![]);
        run(&mut a, &r);
        a.calls = 0;
        let mut next = r.clone();
        next.metrics.coverage = Some(42.0);
        let rep = run(&mut a, &next);
        assert_eq!(rep.updated, 1);
        assert_eq!(a.calls, 1);
        assert!(by_key(&a, SUMMARY_KEY).body.contains("42.0"));
        let mut gate = next.clone();
        gate.gate.passed = false;
        assert_eq!(run(&mut a, &gate).updated, 1);
    }

    #[test]
    fn empty_config_labels_use_the_base_label() {
        let mut a = Fake::default();
        let mut r = result(vec![f("r", "a.java", 1, Severity::High)]);
        r.publish.labels.clear();
        run(&mut a, &r);
        assert_eq!(by_key(&a, K).labels, vec![BASE_LABEL.to_string()]);
        assert!(a.labels.contains(&BASE_LABEL.to_string()));
    }

    #[test]
    fn config_labels_are_additive_and_changing_them_keeps_issues() {
        let mut a = Fake::default();
        let mut r = result(vec![f("r", "a.java", 1, Severity::High)]);
        r.publish.labels = vec!["team-a".into(), BASE_LABEL.into()];
        run(&mut a, &r);
        assert_eq!(by_key(&a, K).labels, vec![BASE_LABEL.to_string(), "team-a".to_string()]);
        a.calls = 0;
        r.publish.labels = vec!["team-b".into()];
        assert_eq!(run(&mut a, &r), SyncReport::default());
        assert_eq!(a.calls, 0);
        assert_eq!(a.listed, vec![BASE_LABEL.to_string(); 2]);
    }

    #[test]
    fn human_text_below_state_does_not_orphan_the_issue() {
        let mut a = Fake::default();
        run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        let n = by_key(&a, K).number;
        a.issues.iter_mut().find(|i| i.number == n).unwrap().body.push_str("\n담당자 메모");
        // 항목이 그대로면 건드리지 않는다.
        let rep = run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        assert_eq!(rep, SyncReport::default());
        assert_eq!(a.issues.iter().filter(|i| i.title.contains("r: a.java")).count(), 1);
        // 고쳐지면 같은 이슈가 닫힌다.
        let rep = run(&mut a, &result(vec![]));
        assert_eq!(rep.closed, 1);
        assert!(!a.issues.iter().find(|i| i.number == n).unwrap().open);
    }

    #[test]
    fn human_issue_quoting_state_stays_untouched() {
        let mut a = Fake::default();
        run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        let state_body = by_key(&a, K).body;
        a.next = 900;
        a.issues.push(Issue { number: 900, title: "사람 이슈".into(), body: format!("> 인용\n{state_body}\n이어지는 글"), open: true, labels: vec![BASE_LABEL.into()] });
        run(&mut a, &result(vec![]));
        assert!(a.issues.iter().find(|i| i.number == 900).unwrap().open);
    }

    #[test]
    fn human_issue_quoting_state_at_the_end_stays_untouched() {
        let mut a = Fake::default();
        run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        let state_line = by_key(&a, K).body.lines().rev().find(|l| l.starts_with("<!--")).unwrap().to_string();
        a.next = 900;
        a.issues.push(Issue { number: 900, title: "사람 이슈".into(), body: format!("> {state_line}"), open: true, labels: vec![BASE_LABEL.into()] });
        a.calls = 0;
        let rep = run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        assert_eq!(rep, SyncReport::default());
        assert_eq!(a.calls, 0);
        run(&mut a, &result(vec![]));
        let h = a.issues.iter().find(|i| i.number == 900).unwrap();
        assert!(h.open);
        assert_eq!(h.title, "사람 이슈");
    }

    #[test]
    fn old_render_version_is_rewritten_once() {
        let mut a = Fake::default();
        let r = result(vec![f("r", "a.java", 1, Severity::High)]);
        run(&mut a, &r);
        let n = by_key(&a, K).number;
        let i = a.issues.iter_mut().find(|i| i.number == n).unwrap();
        i.body = i.body.replace(&format!(",\"v\":{RENDER_VERSION}"), "");
        assert_eq!(parse_state(&i.body).unwrap().v, 0);
        a.calls = 0;
        let rep = run(&mut a, &r);
        assert_eq!(rep.updated, 1);
        assert_eq!(a.calls, 1);
        assert_eq!(parse_state(&by_key(&a, K).body).unwrap().v, RENDER_VERSION);
        a.calls = 0;
        assert_eq!(run(&mut a, &r), SyncReport::default());
        assert_eq!(a.calls, 0);
    }

    #[test]
    fn failed_close_edit_posts_no_comment() {
        let mut a = Fake::default();
        run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        a.comments.clear();
        a.fail_edit = true;
        assert!(sync(&mut a, &result(vec![]), "http://g", "o/r").is_err());
        assert!(a.comments.is_empty());
        assert!(by_key(&a, K).open);
    }

    #[test]
    fn failed_close_comment_is_an_error_but_the_issue_is_closed() {
        let mut a = Fake::default();
        run(&mut a, &result(vec![f("r", "a.java", 1, Severity::High)]));
        a.fail_comment = true;
        assert!(sync(&mut a, &result(vec![]), "http://g", "o/r").is_err());
        assert!(!by_key(&a, K).open);
        a.fail_comment = false;
        a.comments.clear();
        let rep = run(&mut a, &result(vec![]));
        assert_eq!(rep.closed, 0);
        assert!(a.comments.is_empty());
    }

    #[test]
    fn failed_reopen_edit_posts_no_comment() {
        let mut a = Fake::default();
        let r = result(vec![f("r", "a.java", 1, Severity::High)]);
        run(&mut a, &r);
        run(&mut a, &result(vec![]));
        a.comments.clear();
        a.fail_edit = true;
        assert!(sync(&mut a, &r, "http://g", "o/r").is_err());
        assert!(a.comments.is_empty());
    }

    #[test]
    fn duplicate_summary_issues_are_closed_with_comment() {
        let mut a = Fake::default();
        run(&mut a, &result(vec![]));
        let first = by_key(&a, SUMMARY_KEY);
        a.next = 900;
        a.issues.push(Issue { number: 900, ..first.clone() });
        let rep = run(&mut a, &result(vec![]));
        assert_eq!(rep.closed, 1);
        assert!(a.issues.iter().find(|i| i.number == first.number).unwrap().open);
        assert!(!a.issues.iter().find(|i| i.number == 900).unwrap().open);
        assert!(a.comments.iter().any(|(n, _)| *n == 900));
        a.calls = 0;
        assert_eq!(run(&mut a, &result(vec![])), SyncReport::default());
        assert_eq!(a.calls, 0);
    }
}
