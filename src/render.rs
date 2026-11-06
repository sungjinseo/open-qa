use crate::model::*;
use crate::track::Item;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_STATE_BYTES: usize = 40_000;
pub const SUMMARY_KEY: &str = "summary";
const MARK: &str = "<!-- qualgate:v1 ";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueState { pub key: String, pub part: usize, pub items: Vec<Item> }

#[derive(Debug, Clone, PartialEq)]
pub struct Part { pub title: String, pub body: String, pub items: Vec<Item> }

#[derive(Debug, Clone, PartialEq)]
pub struct Desired { pub key: String, pub title: String, pub severity: Severity, pub parts: Vec<Part> }

fn state_comment(st: &IssueState) -> String {
    let json = serde_json::to_string(st).expect("상태 직렬화");
    // JSON 안의 `-->`가 HTML 주석을 닫지 않도록 이스케이프한다.
    // `<`도 이스케이프해 메시지 속 마커가 `rfind`를 속이지 못하게 한다.
    format!("{MARK}{} -->", json.replace("-->", "--\\u003e").replace('<', "\\u003c"))
}

pub fn parse_state(body: &str) -> Option<IssueState> {
    let start = body.rfind(MARK)? + MARK.len();
    let end = start + body[start..].find(" -->")?;
    // 상태 주석은 본문 끝에 있어야 한다. 뒤에 글이 이어지면 인용된 것이므로 무시한다.
    if !body[end + " -->".len()..].trim().is_empty() { return None; }
    serde_json::from_str(&body[start..end]).ok()
}

fn cell(s: &str) -> String {
    let t: String = s.chars().take(200).collect();
    t.replace('|', "\\|").replace('\n', " ")
}

fn chunk_items(items: Vec<Item>) -> Vec<Vec<Item>> {
    let mut out = vec![];
    let mut cur = vec![];
    let mut size = 0;
    for it in items {
        let n = serde_json::to_string(&it).expect("항목 직렬화").len();
        if size + n > MAX_STATE_BYTES && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            size = 0;
        }
        size += n;
        cur.push(it);
    }
    if !cur.is_empty() || out.is_empty() { out.push(cur); }
    out
}

fn finish_parts(key: &str, title: &str, chunks: Vec<Vec<Item>>, table: impl Fn(&[Item]) -> String) -> Vec<Part> {
    let n = chunks.len();
    chunks.into_iter().enumerate().map(|(i, items)| {
        let t = if n > 1 { format!("{title} ({}/{n})", i + 1) } else { title.to_string() };
        let st = IssueState { key: key.to_string(), part: i, items: items.clone() };
        Part { title: t, body: format!("{}\n\n{}\n", table(&items), state_comment(&st)), items }
    }).collect()
}

pub fn desired(r: &ScanResult, base_url: &str, repo: &str) -> Vec<Desired> {
    let link = |file: &str, line: u32| format!("{base_url}/{repo}/src/commit/{}/{file}#L{line}", r.commit);
    let mut out = vec![];
    let mut groups: BTreeMap<(String, String), Vec<&Finding>> = BTreeMap::new();
    for f in &r.findings { groups.entry((f.rule.clone(), f.file.clone())).or_default().push(f); }
    for ((rule, file), mut fs) in groups {
        fs.sort_by_key(|f| f.line);
        let key = format!("rule:{rule}:{file}");
        let title = format!("[qualgate] {rule}: {file} ({}건)", fs.len());
        let severity = fs.iter().map(|f| f.severity).max().unwrap_or(Severity::Info);
        let message = fs[0].message.clone();
        let category = fs[0].category.clone();
        let items: Vec<Item> = fs.iter().map(|f| Item {
            rule: f.rule.clone(), file: f.file.clone(), line: f.line,
            line_hash: f.line_hash.clone(), message: f.message.clone(),
        }).collect();
        let table = |items: &[Item]| {
            let mut b = format!("**규칙:** `{rule}`  \n**심각도:** {severity:?}  \n**분류:** {category}\n\n{}\n\n| 위치 | 내용 |\n|---|---|\n", cell(&message));
            for it in items.iter().take(50) {
                b.push_str(&format!("| [{}:{}]({}) | {} |\n", it.file, it.line, link(&it.file, it.line), cell(&it.message)));
            }
            if items.len() > 50 { b.push_str(&format!("\n외 {}건\n", items.len() - 50)); }
            b
        };
        let parts = finish_parts(&key, &title, chunk_items(items), table);
        out.push(Desired { key, title, severity, parts });
    }
    for g in &r.clones {
        let key = format!("clone:{}", g.key);
        let files: std::collections::BTreeSet<&str> = g.parts.iter().map(|p| p.file.as_str()).collect();
        let first = files.iter().next().copied().unwrap_or("");
        let title = format!("[qualgate] 중복 코드: {first} 외 {}개 파일 ({}줄)", files.len().saturating_sub(1), g.lines);
        let items: Vec<Item> = g.parts.iter().map(|p| Item {
            rule: "duplication".into(), file: p.file.clone(), line: p.start_line,
            line_hash: g.key.clone(), message: format!("{}-{}", p.start_line, p.end_line),
        }).collect();
        let table = |items: &[Item]| {
            let mut b = String::from("같은 코드가 여러 위치에 있습니다. 공통 코드로 추출하세요.\n\n| 위치 | 줄 |\n|---|---|\n");
            for it in items.iter().take(50) {
                b.push_str(&format!("| [{}:{}]({}) | {} |\n", it.file, it.line, link(&it.file, it.line), it.message));
            }
            if items.len() > 50 { b.push_str(&format!("\n외 {}건\n", items.len() - 50)); }
            b
        };
        let parts = finish_parts(&key, &title, chunk_items(items), table);
        out.push(Desired { key, title, severity: Severity::Medium, parts });
    }
    out
}

fn pct(v: Option<f64>) -> String { v.map_or("측정 안 됨".to_string(), |x| format!("{x:.1}%")) }

pub fn summary(r: &ScanResult, skipped: usize) -> (String, String) {
    let mut b = format!("**게이트:** {}  \n**커밋:** `{}`\n\n", if r.gate.passed { "통과" } else { "실패" }, r.commit);
    b.push_str(&format!(
        "| 지표 | 값 |\n|---|---|\n| 전체 커버리지 | {} |\n| 신규 코드 커버리지 | {} |\n| 전체 중복률 | {} |\n| 신규 코드 중복률 | {} |\n| 변경 줄 차단 등급 위반 | {} |\n\n",
        pct(r.metrics.coverage), pct(r.metrics.new_coverage), pct(r.metrics.duplication), pct(r.metrics.new_duplication),
        r.metrics.new_blocking.map_or("측정 안 됨".to_string(), |v| format!("{v}건"))));
    b.push_str("| 조건 | 실제 | 기준 | 결과 |\n|---|---|---|---|\n");
    for c in &r.gate.conditions {
        b.push_str(&format!("| {} | {} | {} | {:?} |\n", c.name, c.actual.map_or("측정 안 됨".to_string(), |v| format!("{v:.1}")), c.limit, c.status));
    }
    b.push('\n');
    let list = |title: &str, v: &[String], b: &mut String| {
        if v.is_empty() { return; }
        b.push_str(&format!("**{title} ({}개):** ", v.len()));
        b.push_str(&v.iter().take(20).map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", "));
        if v.len() > 20 { b.push_str(&format!(" 외 {}개", v.len() - 20)); }
        b.push_str("\n\n");
    };
    list("분석 실패 파일", &r.unanalyzed, &mut b);
    list("커버리지 리포트 미포함", &r.coverage_status.not_in_report, &mut b);
    list("리포트 경로 매칭 실패", &r.coverage_status.unresolved, &mut b);
    if skipped > 0 { b.push_str(&format!("생성 제한으로 이번에 등록하지 못한 이슈: {skipped}개\n\n")); }
    let st = IssueState { key: SUMMARY_KEY.to_string(), part: 0, items: vec![] };
    b.push_str(&state_comment(&st));
    b.push('\n');
    ("[qualgate] 품질 요약".to_string(), b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(rule: &str, file: &str, line: u32, msg: &str, sev: Severity) -> Finding {
        Finding { rule: rule.into(), file: file.into(), line, line_hash: "h".into(), message: msg.into(), severity: sev, category: "c".into() }
    }

    fn result() -> ScanResult {
        ScanResult {
            schema: 1, commit: "abc".into(), branch: Some("main".into()),
            findings: vec![
                f("r1", "a.java", 3, "m --> x", Severity::Low),
                f("r1", "a.java", 1, "m", Severity::High),
                f("r2", "a.java", 2, "m", Severity::Medium),
            ],
            clones: vec![CloneGroup { key: "ck".into(), lines: 12, parts: vec![
                ClonePart { file: "a.kt".into(), start_line: 1, end_line: 12 },
                ClonePart { file: "b.kt".into(), start_line: 5, end_line: 16 },
            ]}],
            metrics: Metrics { coverage: Some(70.5), ..Default::default() },
            coverage_status: CoverageStatus::default(),
            gate: GateResult { passed: true, conditions: vec![] },
            unanalyzed: vec![],
            publish: PublishConfig { max_new_issues: 20, labels: vec!["qualgate".into()] },
        }
    }

    #[test]
    fn groups_by_rule_and_file_with_highest_severity() {
        let d = desired(&result(), "http://g", "o/r");
        let g = d.iter().find(|x| x.key == "rule:r1:a.java").unwrap();
        assert_eq!(g.severity, Severity::High);
        assert_eq!(g.parts[0].items.len(), 2);
        assert_eq!(g.title, "[qualgate] r1: a.java (2건)");
        assert!(g.parts[0].body.contains("http://g/o/r/src/commit/abc/a.java#L1"));
        let c = d.iter().find(|x| x.key == "clone:ck").unwrap();
        assert_eq!(c.title, "[qualgate] 중복 코드: a.kt 외 1개 파일 (12줄)");
    }

    #[test]
    fn state_round_trips_even_with_comment_terminator() {
        let d = desired(&result(), "http://g", "o/r");
        let g = d.iter().find(|x| x.key == "rule:r1:a.java").unwrap();
        let st = parse_state(&g.parts[0].body).unwrap();
        assert_eq!(st.key, "rule:r1:a.java");
        assert_eq!(st.items, g.parts[0].items);
        assert!(st.items.iter().any(|i| i.message.contains("-->")));
    }

    #[test]
    fn human_issue_has_no_state() {
        assert_eq!(parse_state("그냥 이슈"), None);
        assert_eq!(parse_state("<!-- qualgate:v1 {broken -->"), None);
    }

    #[test]
    fn large_group_splits_into_parts() {
        let mut r = result();
        r.findings = (0..2000).map(|i| f("r", "a.java", i + 1, &"x".repeat(100), Severity::Low)).collect();
        r.clones.clear();
        let d = desired(&r, "http://g", "o/r");
        let g = &d[0];
        assert!(g.parts.len() > 1);
        assert!(g.parts[1].title.contains("(2/"));
        assert!(g.parts.iter().all(|p| p.body.len() < 60_000));
        assert_eq!(g.parts.iter().map(|p| p.items.len()).sum::<usize>(), 2000);
    }

    #[test]
    fn table_shows_at_most_fifty_rows() {
        let mut r = result();
        r.findings = (0..60).map(|i| f("r", "a.java", i + 1, "m", Severity::Low)).collect();
        r.clones.clear();
        let b = &desired(&r, "http://g", "o/r")[0].parts[0].body;
        assert!(b.contains("외 10건"));
    }

    #[test]
    fn summary_marks_unmeasured_and_skipped() {
        let (t, b) = summary(&result(), 3);
        assert_eq!(t, "[qualgate] 품질 요약");
        assert!(b.contains("70.5"));
        assert!(b.contains("측정 안 됨"));
        assert!(b.contains("3개"));
        assert_eq!(parse_state(&b).unwrap().key, SUMMARY_KEY);
    }

    #[test]
    fn marker_inside_message_round_trips() {
        let mut r = result();
        r.findings = vec![f("r1", "a.java", 1, "x <!-- qualgate:v1 {\"key\":\"evil\"} --> y", Severity::Low)];
        r.clones.clear();
        let d = desired(&r, "http://g", "o/r");
        let st = parse_state(&d[0].parts[0].body).unwrap();
        assert_eq!(st.key, "rule:r1:a.java");
        assert_eq!(st.items, d[0].parts[0].items);
    }

    #[test]
    fn quoted_marker_followed_by_prose_is_not_state() {
        let st = IssueState { key: "rule:r:a.java".into(), part: 0, items: vec![] };
        let quoted = format!("> {}\n\n이건 사람이 인용한 것입니다.", state_comment(&st));
        assert_eq!(parse_state(&quoted), None);
        assert!(parse_state(&format!("본문\n{}\n\n", state_comment(&st))).is_some());
    }
}
