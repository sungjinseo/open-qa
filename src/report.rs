//! 언어별 분석 보고서(HTML, Markdown, XLSX).
//!
//! `result.json`, 규칙 카탈로그(규칙 예시는 `rules/tests/<dir>/<이름>.bad.*`, `.good.*`), 그리고
//! 있으면 소스(위치별 코드 조각)를 읽는다. 네트워크를 쓰지 않는다. 게이트는 `publish` 전에 계산하는
//! 예비 판정이다. 핫스팟 검토율은 Gitea 라벨을 봐야 하므로 신규 핫스팟이 있으면 판정을 미룬다.
use crate::catalog::{Catalog, Kind};
use crate::gatecalc::{self, ConditionStatus, GateResult};
use crate::model::*;
use crate::settings::{Metric, Op};
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Format { Html, Md, Xlsx }

impl Format {
    pub fn ext(self) -> &'static str {
        match self { Format::Html => "html", Format::Md => "md", Format::Xlsx => "xlsx" }
    }
}

/// `html,md,xlsx`, `[html, md, xlsx]`, 여러 인자 모두 받는다. 빈 목록은 보고서를 만들지 않는다는 뜻이다.
pub fn parse_formats(values: &[String]) -> Result<Vec<Format>> {
    let mut out = BTreeSet::new();
    for v in values {
        for t in v.split([',', ' ', '\t', '\n']) {
            let t = t.trim_matches(|c: char| c == '[' || c == ']' || c == '"' || c == '\'' || c.is_whitespace());
            if t.is_empty() { continue; }
            out.insert(match t.to_ascii_lowercase().as_str() {
                "html" | "htm" => Format::Html,
                "md" | "markdown" => Format::Md,
                "xlsx" | "excel" => Format::Xlsx,
                o => bail!("보고서 형식 {o:?}을(를) 지원하지 않습니다 (지원: html, md, xlsx)"),
            });
        }
    }
    Ok(out.into_iter().collect())
}

/// 보고서 언어 키(규칙 디렉터리)와 표시 이름. 이 순서로 보고서를 만든다.
pub const LANGS: [(&str, &str); 7] = [
    ("java", "Java"), ("kotlin", "Kotlin"), ("python", "Python"), ("go", "Go"),
    ("ts", "TypeScript/JavaScript"), ("react", "React (JSX/TSX)"), ("vue", "Vue"),
];

pub fn lang_name(key: &str) -> &str { LANGS.iter().find(|(k, _)| *k == key).map_or(key, |(_, n)| n) }

fn rule_lang(rule: &str) -> Option<&'static str> {
    let dir = rule.split_once('.')?.0;
    LANGS.iter().map(|(k, _)| *k).find(|k| *k == dir)
}

/// SonarQube 표기의 이슈 유형.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IssueType { Bug, Vulnerability, CodeSmell, Hotspot }

impl IssueType {
    pub const ALL: [IssueType; 4] = [IssueType::Bug, IssueType::Vulnerability, IssueType::CodeSmell, IssueType::Hotspot];
    pub fn of(f: &Finding) -> IssueType {
        if f.kind == Kind::Hotspot { return IssueType::Hotspot; }
        match f.category.as_str() {
            "security" => IssueType::Vulnerability,
            "bug" | "reliability" => IssueType::Bug,
            _ => IssueType::CodeSmell,
        }
    }
    pub fn label(self) -> &'static str {
        match self { IssueType::Bug => "BUG", IssueType::Vulnerability => "VULNERABILITY", IssueType::CodeSmell => "CODE_SMELL", IssueType::Hotspot => "SECURITY_HOTSPOT" }
    }
    pub fn ko(self) -> &'static str {
        match self { IssueType::Bug => "안정성", IssueType::Vulnerability => "보안", IssueType::CodeSmell => "유지보수성", IssueType::Hotspot => "보안 핫스팟" }
    }
    fn css(self) -> &'static str {
        match self { IssueType::Bug => "rel", IssueType::Vulnerability => "sec", IssueType::CodeSmell => "mnt", IssueType::Hotspot => "hot" }
    }
}

const SEVERITIES: [Severity; 5] = [Severity::Info, Severity::Low, Severity::Medium, Severity::High, Severity::Critical];

fn sev_label(s: Severity) -> &'static str {
    match s { Severity::Info => "INFO", Severity::Low => "LOW", Severity::Medium => "MEDIUM", Severity::High => "HIGH", Severity::Critical => "CRITICAL" }
}

/// 유형별 등급. 그 유형 이슈의 가장 높은 심각도로 정한다(없음·INFO A, LOW B, MEDIUM C, HIGH D, CRITICAL E).
fn grade(findings: &[&Finding], ty: IssueType) -> char {
    match findings.iter().filter(|f| IssueType::of(f) == ty).map(|f| f.severity).max() {
        None | Some(Severity::Info) => 'A',
        Some(Severity::Low) => 'B',
        Some(Severity::Medium) => 'C',
        Some(Severity::High) => 'D',
        Some(Severity::Critical) => 'E',
    }
}

/// 보고서 공통 입력.
pub struct Input<'a> {
    pub result: &'a ScanResult,
    pub catalog: &'a Catalog,
    pub rules_root: &'a Path,
    /// 위치별 코드 조각을 읽을 소스 디렉터리. 없으면 조각을 넣지 않는다.
    pub src: Option<&'a Path>,
    /// 파일 이름 앞부분(예: PipelineRun 이름).
    pub name: String,
    /// `YYYY-MM-DD`
    pub date: String,
}

/// 위치 주변 코드.
#[derive(Debug, Clone, PartialEq)]
pub struct Snippet { pub start: u32, pub hit: u32, pub lines: Vec<String> }

pub struct RuleGroup<'a> {
    pub id: String,
    pub title: String,
    pub description: String,
    pub ty: IssueType,
    pub severity: Severity,
    pub mois: Option<String>,
    pub cwe: Option<String>,
    pub owasp_api: Vec<String>,
    pub findings: Vec<&'a Finding>,
    pub bad: Option<String>,
    pub good: Option<String>,
}

/// 게이트 예비 판정.
pub struct Gate {
    pub result: GateResult,
    /// 신규 핫스팟이 있어 검토율을 `publish`에서 판정한다.
    pub hotspots_pending: bool,
}

impl Gate {
    pub fn label(&self) -> &'static str {
        if self.result.conditions.is_empty() { "통과(조건 없음)" }
        else if !self.result.passed { "실패" }
        else if self.hotspots_pending { "판정 대기" }
        else { "통과" }
    }
    fn css(&self) -> &'static str {
        if !self.result.passed { "fail" } else if self.hotspots_pending { "wait" } else { "ok" }
    }
}

pub struct LangReport<'a> {
    pub key: &'static str,
    pub name: &'static str,
    pub stats: Option<&'a LangStats>,
    /// 파일, 줄 순서.
    pub findings: Vec<&'a Finding>,
    /// 유형(버그, 취약점, 코드 스멜, 핫스팟), 심각도 내림차순, 건수 내림차순.
    pub rules: Vec<RuleGroup<'a>>,
    pub clones: Vec<&'a CloneGroup>,
}

impl LangReport<'_> {
    fn issues(&self) -> Vec<&Finding> { self.findings.iter().copied().filter(|f| f.kind == Kind::Issue).collect() }
    fn hotspots(&self) -> Vec<&Finding> { self.findings.iter().copied().filter(|f| f.kind == Kind::Hotspot).collect() }
    fn count(&self, ty: IssueType, sev: Option<Severity>) -> usize {
        self.findings.iter().filter(|f| IssueType::of(f) == ty && sev.is_none_or(|s| f.severity == s)).count()
    }
}

pub fn gate(r: &ScanResult) -> Gate {
    let reviewed = (r.new_code_measured && r.new_hotspots.is_empty()).then_some(100.0);
    let result = gatecalc::evaluate(&r.gate_conditions, &r.metrics, reviewed, r.coverage_required_missing);
    let hotspots_pending = reviewed.is_none() && r.new_code_measured
        && result.conditions.iter().any(|c| c.metric == Metric::NewHotspotsReviewed);
    Gate { result, hotspots_pending }
}

/// 보고서를 만들 언어. 코드 파일, 위반, 중복이 하나라도 있는 언어.
pub fn languages(r: &ScanResult) -> Vec<&'static str> {
    let mut set: BTreeSet<&str> = BTreeSet::new();
    for (k, st) in &r.languages { if st.files + st.test_files > 0 { set.insert(k.as_str()); } }
    for f in &r.findings { if let Some(k) = rule_lang(&f.rule) { set.insert(k); } }
    for g in &r.clones { for p in &g.parts { if let Some(k) = crate::select::report_lang(&p.file) { set.insert(k); } } }
    LANGS.iter().map(|(k, _)| *k).filter(|k| set.contains(k)).collect()
}

/// 규칙 예시 파일(`rules/tests/<dir>/<이름>.bad.*`). 너무 길면 자른다.
fn example(rules_root: &Path, rule: &str, which: &str) -> Option<String> {
    let (dir, name) = rule.split_once('.')?;
    let d = rules_root.join("tests").join(dir);
    let prefix = format!("{name}.{which}.");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&d).ok()?.filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix))).collect();
    files.sort();
    let text = std::fs::read_to_string(files.first()?).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    let mut out = lines.iter().take(60).copied().collect::<Vec<_>>().join("\n");
    if lines.len() > 60 { out.push_str("\n// …"); }
    Some(out)
}

pub fn build<'a>(inp: &Input<'a>, key: &'static str) -> LangReport<'a> {
    let r = inp.result;
    let mut findings: Vec<&Finding> = r.findings.iter().filter(|f| rule_lang(&f.rule) == Some(key)).collect();
    findings.sort_by(|a, b| (&a.file, a.line, &a.rule).cmp(&(&b.file, b.line, &b.rule)));
    let mut by_rule: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for f in &findings { by_rule.entry(f.rule.as_str()).or_default().push(f); }
    let mut rules: Vec<RuleGroup> = by_rule.into_iter().map(|(id, fs)| {
        let info = inp.catalog.rules.get(id);
        let first = fs[0];
        RuleGroup {
            id: id.to_string(),
            title: info.map(|i| i.title.clone()).filter(|t| !t.is_empty())
                .unwrap_or_else(|| first.message.lines().next().unwrap_or(id).to_string()),
            description: first.message.clone(),
            ty: IssueType::of(first),
            severity: fs.iter().map(|f| f.severity).max().unwrap_or(Severity::Info),
            mois: info.and_then(|i| i.mois.clone()),
            cwe: info.and_then(|i| i.cwe.clone()),
            owasp_api: info.map(|i| i.owasp_api.clone()).unwrap_or_default(),
            bad: example(inp.rules_root, id, "bad"),
            good: example(inp.rules_root, id, "good"),
            findings: fs,
        }
    }).collect();
    rules.sort_by(|a, b| a.ty.cmp(&b.ty).then(b.severity.cmp(&a.severity)).then(b.findings.len().cmp(&a.findings.len())).then(a.id.cmp(&b.id)));
    let clones = r.clones.iter()
        .filter(|g| g.parts.iter().any(|p| crate::select::report_lang(&p.file) == Some(key))).collect();
    let name = LANGS.iter().find(|(k, _)| *k == key).map_or("", |(_, n)| n);
    LangReport { key, name, stats: r.languages.get(key), findings, rules, clones }
}

/// 저장소 안의 상대 경로만 읽는다(절대 경로, `..`는 거부).
fn safe_join(src: &Path, rel: &str) -> Option<PathBuf> {
    let p = Path::new(rel);
    if p.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir)) { Some(src.join(p)) } else { None }
}

/// 파일별 줄 캐시로 위치 주변 코드를 꺼낸다.
pub struct Snippets<'a> { src: Option<&'a Path>, cache: BTreeMap<String, Option<Vec<String>>> }

pub const CONTEXT_LINES: u32 = 3;
const MAX_LINE_CHARS: usize = 300;

impl<'a> Snippets<'a> {
    pub fn new(src: Option<&'a Path>) -> Self { Snippets { src, cache: BTreeMap::new() } }
    pub fn get(&mut self, file: &str, line: u32) -> Option<Snippet> {
        let src = self.src?;
        let lines = self.cache.entry(file.to_string()).or_insert_with(|| {
            let p = safe_join(src, file)?;
            let bytes = std::fs::read(p).ok()?;
            Some(String::from_utf8_lossy(&bytes).lines().map(|l| {
                let l = l.trim_end_matches('\r');
                if l.chars().count() > MAX_LINE_CHARS { format!("{}…", l.chars().take(MAX_LINE_CHARS).collect::<String>()) } else { l.to_string() }
            }).collect())
        }).as_ref()?;
        if line == 0 || line as usize > lines.len() { return None; }
        let start = line.saturating_sub(CONTEXT_LINES).max(1);
        let end = (line + CONTEXT_LINES).min(lines.len() as u32);
        Some(Snippet { start, hit: line, lines: lines[start as usize - 1..end as usize].to_vec() })
    }
}

/// 상세 절에서 규칙마다 코드 조각을 붙이는 위치 수. 나머지는 전체 위치 표에만 있다.
pub const MAX_DETAILED: usize = 30;

/// OWASP API Security Top 10 항목별 집계 한 줄.
pub struct OwaspRow { pub id: &'static str, pub name: &'static str, pub active_rules: usize, pub findings: usize, pub rules: Vec<String> }

/// 이 언어에서 실행한 규칙과 위반을 OWASP API 항목별로 센다. API6·API9는 정적 분석 대상이 아니다.
pub fn owasp_rows(inp: &Input, rep: &LangReport) -> Vec<OwaspRow> {
    crate::catalog::OWASP_API.iter().map(|(id, name)| {
        let tagged = |rule: &str| inp.catalog.rules.get(rule).is_some_and(|r| r.owasp_api.iter().any(|x| x == id));
        let active_rules = inp.result.active_rules.iter().filter(|r| rule_lang(r) == Some(rep.key) && tagged(r)).count();
        let groups: Vec<&RuleGroup> = rep.rules.iter().filter(|g| g.owasp_api.iter().any(|x| x == id)).collect();
        OwaspRow { id, name, active_rules, findings: groups.iter().map(|g| g.findings.len()).sum(), rules: groups.iter().map(|g| g.id.clone()).collect() }
    }).collect()
}

fn owasp_note(id: &str) -> &'static str {
    match id { "API6:2023" | "API9:2023" => "정적 분석 대상 아님(설계·운영 점검)", _ => "-" }
}

fn pct(v: Option<f64>) -> String { v.map_or("측정 안 됨".to_string(), |x| format!("{x:.1}%")) }

fn metric_value(m: Metric, v: f64) -> String {
    if matches!(m, Metric::NewIssues | Metric::Issues) { format!("{v:.0}건") } else { format!("{v:.1}%") }
}

fn cond_row(c: &gatecalc::Condition, g: &Gate) -> [String; 4] {
    let op = match c.op { Op::Gt => "초과", Op::Lt => "미만" };
    let status = match c.status {
        ConditionStatus::Passed => "통과",
        ConditionStatus::Failed => "실패",
        ConditionStatus::Skipped if g.hotspots_pending && c.metric == Metric::NewHotspotsReviewed => "publish에서 판정",
        ConditionStatus::Skipped => "건너뜀",
    };
    [c.metric.as_str().to_string(), c.actual.map_or("측정 안 됨".into(), |v| metric_value(c.metric, v)),
     format!("{} {op}", metric_value(c.metric, c.value)), status.to_string()]
}

fn src_field(r: &ScanResult, key: &str) -> String {
    r.config_source.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or("알 수 없음").to_string()
}

fn short(sha: &str) -> &str { sha.get(..7).unwrap_or(sha) }

/// 파일 이름에 쓸 수 있는 글자만 남긴다.
pub fn sanitize_name(s: &str) -> String {
    let t: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '-' }).collect();
    let t = t.trim_matches(|c| c == '-' || c == '.').to_string();
    if t.is_empty() { "qualgate".into() } else { t }
}

pub fn file_name(name: &str, key: &str, fmt: Format) -> String {
    format!("{}-{key}-analysis-report.{}", sanitize_name(name), fmt.ext())
}

/// 언어마다 요청한 형식의 보고서를 `dir`에 쓴다. 쓴 파일 경로를 돌려준다.
pub fn write_all(inp: &Input, formats: &[Format], dir: &Path) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir).with_context(|| format!("{} 생성 실패", dir.display()))?;
    let g = gate(inp.result);
    let mut out = vec![];
    for key in languages(inp.result) {
        let rep = build(inp, key);
        let mut sn = Snippets::new(inp.src);
        for &fmt in formats {
            let path = dir.join(file_name(&inp.name, key, fmt));
            match fmt {
                Format::Html => write_text(&path, &html(inp, &rep, &g, &mut sn))?,
                Format::Md => write_text(&path, &markdown(inp, &rep, &g))?,
                Format::Xlsx => xlsx(inp, &rep, &g, &path)?,
            }
            out.push(path);
        }
    }
    Ok(out)
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    std::fs::write(path, text).with_context(|| format!("{} 쓰기 실패", path.display()))
}

// ---------------------------------------------------------------- HTML

fn h(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

fn anchor(rule: &str) -> String { format!("r-{}", rule.replace(|c: char| !c.is_ascii_alphanumeric(), "-")) }

const CSS: &str = r#"
:root{--bg:#f6f7fb;--card:#fff;--line:#e3e6ee;--text:#1f2430;--muted:#667085;--accent:#3b6fd8;--code:#f7f8fa}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--text);font:14px/1.6 -apple-system,"Segoe UI","Malgun Gothic","Apple SD Gothic Neo",sans-serif;padding:24px}
.wrap{max-width:1200px;margin:0 auto}
h1{font-size:24px;margin:0 0 4px}h2{font-size:19px;margin:32px 0 10px;border-bottom:2px solid var(--line);padding-bottom:4px}h3{font-size:16px;margin:18px 0 8px}
code{font-family:Consolas,"D2Coding",Menlo,monospace;font-size:12.5px;background:var(--code);border:1px solid var(--line);border-radius:4px;padding:0 4px}
.meta{color:var(--muted);font-size:13px}
table{border-collapse:collapse;width:100%;background:var(--card);margin:8px 0;font-size:13px}
th,td{border:1px solid var(--line);padding:6px 8px;text-align:left;vertical-align:top;overflow-wrap:anywhere}
th{background:#eef1f7}.num{text-align:right;white-space:nowrap}
.tiles{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:10px;margin:10px 0}
.tile{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:10px 12px}
.tile .label{color:var(--muted);font-size:12px}.tile .value{font-size:22px;font-weight:700}
.grade{display:inline-block;width:32px;text-align:center;border-radius:50%;color:#fff}
.grade-A{background:#1a7f37}.grade-B{background:#7fae21}.grade-C{background:#d9a400}.grade-D{background:#e46c0a}.grade-E{background:#c62828}
.gate{border-radius:4px;padding:0 10px;color:#fff;font-size:16px}.gate-ok{background:#1a7f37}.gate-fail{background:#c62828}.gate-wait{background:#8a6d00}
.cards{display:grid;grid-template-columns:repeat(auto-fill,minmax(330px,1fr));gap:10px}
.card{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:10px 12px}
.card .title{font-weight:600;margin-bottom:4px}.card a{color:var(--text);text-decoration:none}.card a:hover{text-decoration:underline}
.badge{display:inline-flex;border-radius:4px;overflow:hidden;font-size:12px;vertical-align:middle}.badge span{padding:1px 8px}
.q-rel{background:#fde7e1;color:#8a3b1c}.q-sec{background:#e8e6fb;color:#3e2f99}.q-mnt{background:#fdf3d0;color:#7a5b00}.q-hot{background:#e0f2f1;color:#00695c}
.sev{font-weight:600}.sev-CRITICAL,.sev-HIGH{background:#fbd5d5;color:#a1141c}.sev-MEDIUM{background:#fde1d3;color:#b04a10}.sev-LOW{background:#fdf0c2;color:#7a5b00}.sev-INFO{background:#e5eefc;color:#2c5d93}
.tag{display:inline-block;background:#eef0f5;border-radius:3px;padding:0 6px;margin-left:4px;color:#3b4252;font-size:12px}
.count{font-weight:700}
.rule{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:14px 18px;margin:16px 0}
.desc{white-space:pre-wrap}
.diff{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:10px;margin-top:8px}
.diff h5{margin:0 0 4px;font-size:12px}.bad h5{color:#a1141c}.good h5{color:#1a7f37}
pre.code{background:var(--code);border:1px solid var(--line);border-radius:6px;padding:10px 12px;margin:0;overflow-x:auto;font-family:Consolas,"D2Coding",Menlo,monospace;font-size:12.5px;line-height:1.55;tab-size:4;white-space:pre}
.where{border-top:1px dashed var(--line);margin-top:12px;padding-top:10px}
.loc{font-family:Consolas,"D2Coding",Menlo,monospace;font-size:13px;font-weight:600}
.ln{display:inline-block;width:3.2em;color:#9aa3b2;user-select:none;text-align:right;margin-right:10px}
.hit{background:#fff3c4;display:inline-block;min-width:100%}
.more{color:var(--muted)}
.footer{color:var(--muted);font-size:12px;margin-top:32px;border-top:1px solid var(--line);padding-top:8px}
@media (max-width:760px){.diff{grid-template-columns:1fr}body{padding:12px}}
"#;

fn badge(ty: IssueType, sev: Severity) -> String {
    format!(r#"<span class="badge"><span class="q-{}">{}</span><span class="sev sev-{s}">{s}</span></span>"#, ty.css(), ty.ko(), s = sev_label(sev))
}

fn code_block(sn: &Snippet) -> String {
    let mut b = String::from(r#"<pre class="code">"#);
    for (i, l) in sn.lines.iter().enumerate() {
        let n = sn.start + i as u32;
        let line = format!(r#"<span class="ln">{n}</span>{}"#, h(l));
        if n == sn.hit { let _ = write!(b, r#"<span class="hit">{line}</span>"#); } else { b.push_str(&line); }
        if i + 1 < sn.lines.len() { b.push('\n'); }
    }
    b.push_str("</pre>");
    b
}

fn tr(cells: &[String]) -> String {
    let mut b = String::from("<tr>");
    for c in cells { let _ = write!(b, "<td>{c}</td>"); }
    b.push_str("</tr>\n");
    b
}

pub fn html(inp: &Input, rep: &LangReport, g: &Gate, sn: &mut Snippets) -> String {
    let r = inp.result;
    let repo = src_field(r, "repo");
    let mut b = String::new();
    let title = format!("Static Analysis Report — {repo} · {}", rep.name);
    let _ = write!(b, "<!DOCTYPE html>\n<html lang=\"ko\"><head><meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n<title>{}</title>\n<style>{CSS}</style>\n</head><body><div class=\"wrap\">\n<h1>{}</h1>\n", h(&title), h(&title));
    let _ = writeln!(b, "<p class=\"meta\">저장소 <code>{}</code> · 언어 <b>{}</b> · 브랜치 <code>{}</code> · 커밋 <code>{}</code> · 분석일 {} · 실행 <code>{}</code></p>",
        h(&repo), h(rep.name), h(r.branch.as_deref().unwrap_or("알 수 없음")), h(short(&r.commit)), h(&inp.date), h(&inp.name));

    // 요약
    b.push_str("<h2 id=\"summary\">Summary</h2>\n<div class=\"tiles\">");
    let tile = |b: &mut String, label: &str, value: String| { let _ = write!(b, "<div class=\"tile\"><div class=\"label\">{}</div><div class=\"value\">{value}</div></div>", h(label)); };
    tile(&mut b, "Quality Gate (프로젝트, 예비)", format!("<span class=\"gate gate-{}\">{}</span>", g.css(), h(g.label())));
    for (label, ty) in [("Reliability", IssueType::Bug), ("Security", IssueType::Vulnerability), ("Maintainability", IssueType::CodeSmell)] {
        let gr = grade(&rep.findings, ty);
        tile(&mut b, label, format!("<span class=\"grade grade-{gr}\">{gr}</span>"));
    }
    for ty in IssueType::ALL { tile(&mut b, ty.label(), rep.count(ty, None).to_string()); }
    tile(&mut b, "Duplications (그룹)", rep.clones.len().to_string());
    b.push_str("</div>\n");

    b.push_str("<h3>지표</h3><table><tr><th>범위</th><th class=\"num\">파일</th><th class=\"num\">테스트 파일</th><th class=\"num\">줄 수</th><th class=\"num\">커버리지</th><th class=\"num\">중복률</th></tr>\n");
    if let Some(st) = rep.stats {
        let _ = writeln!(b, "<tr><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            h(rep.name), st.files, st.test_files, st.lines, pct(st.coverage), pct(st.duplicated_lines));
    }
    let _ = writeln!(b, "<tr><td>프로젝트 전체</td><td class=\"num\">-</td><td class=\"num\">-</td><td class=\"num\">-</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr></table>",
        pct(r.metrics.coverage), pct(r.metrics.duplicated_lines));

    b.push_str("<h3>유형 × 심각도</h3><table><tr><th>Type / Severity</th>");
    for s in SEVERITIES { let _ = write!(b, "<th class=\"num\">{}</th>", sev_label(s)); }
    b.push_str("<th class=\"num\">합계</th></tr>\n");
    for ty in IssueType::ALL {
        let _ = write!(b, "<tr><td>{}</td>", ty.label());
        for s in SEVERITIES { let _ = write!(b, "<td class=\"num\">{}</td>", rep.count(ty, Some(s))); }
        let _ = writeln!(b, "<td class=\"num\"><b>{}</b></td></tr>", rep.count(ty, None));
    }
    b.push_str("</table>\n");

    b.push_str("<h3>품질 게이트 조건 (프로젝트 전체)</h3>");
    if g.result.conditions.is_empty() {
        b.push_str("<p>게이트 조건이 없어 항상 통과합니다.</p>\n");
    } else {
        b.push_str("<table><tr><th>조건</th><th>실제</th><th>기준(실패)</th><th>결과</th></tr>\n");
        for c in &g.result.conditions { b.push_str(&tr(&cond_row(c, g).map(|s| h(&s)))); }
        b.push_str("</table>\n");
    }
    if g.hotspots_pending { b.push_str("<p class=\"meta\">신규 핫스팟이 있어 검토율 조건은 Gitea 이슈의 <code>qualgate:safe</code> 라벨을 확인하는 publish 단계에서 판정합니다.</p>\n"); }
    let _ = writeln!(b, "<p class=\"meta\">설정 저장소 <code>{}</code> (ref {}, 커밋 {}) · 룰셋 <b>{}</b></p>",
        h(&src_field(r, "config_repo")), h(&src_field(r, "ref")), h(&src_field(r, "config_commit")), h(&src_field(r, "ruleset")));

    b.push_str("<h3>OWASP API Security Top 10 (2023)</h3><table><tr><th>항목</th><th>이름</th><th class=\"num\">실행 규칙</th><th class=\"num\">위반</th><th>규칙</th></tr>\n");
    for o in owasp_rows(inp, rep) {
        let rules = if o.rules.is_empty() { h(owasp_note(o.id)) } else { o.rules.iter().map(|r| format!("<code>{}</code>", h(r))).collect::<Vec<_>>().join(" ") };
        let _ = writeln!(b, "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{rules}</td></tr>", o.id, h(o.name), o.active_rules, o.findings);
    }
    b.push_str("</table>\n<p class=\"meta\">실행 규칙은 이 언어에서 실행한 규칙 중 해당 항목에 연결된 규칙 수입니다. 위반 0건이 그 항목의 위험이 없다는 뜻은 아닙니다. 인가(API1·API5) 규칙은 핫스팟이라 사람 검토가 필요합니다.</p>\n");

    // 규칙별
    b.push_str("<h2 id=\"rules\">Issues by Rule</h2>\n");
    if rep.rules.is_empty() { b.push_str("<p>이 언어에서 발견된 위반이 없습니다.</p>\n"); }
    else {
        b.push_str("<div class=\"cards\">\n");
        for g in &rep.rules {
            let mut tags = String::new();
            if let Some(m) = &g.mois { let _ = write!(tags, "<span class=\"tag\">행안부 {}</span>", h(m)); }
            if let Some(c) = &g.cwe { let _ = write!(tags, "<span class=\"tag\">{}</span>", h(c)); }
            for o in &g.owasp_api { let _ = write!(tags, "<span class=\"tag\">OWASP {}</span>", h(o)); }
            let _ = writeln!(b, "<div class=\"card\"><div class=\"title\"><a href=\"#{}\">{}</a></div><div>{} <span class=\"count\">{}건</span> <code>{}</code>{tags}</div></div>",
                anchor(&g.id), h(&g.title), badge(g.ty, g.severity), g.findings.len(), h(&g.id));
        }
        b.push_str("</div>\n");
    }

    // 상세
    if !rep.rules.is_empty() { b.push_str("<h2 id=\"details\">Details &amp; Recommendations</h2>\n"); }
    for g in &rep.rules {
        let _ = writeln!(b, "<div class=\"rule\" id=\"{}\">\n<h3>{} <code>{}</code></h3>\n<div>{} <span class=\"count\">{}건</span></div>\n<p class=\"desc\">{}</p>",
            anchor(&g.id), h(&g.title), h(&g.id), badge(g.ty, g.severity), g.findings.len(), h(&g.description));
        if g.bad.is_some() || g.good.is_some() {
            b.push_str("<div class=\"diff\">");
            if let Some(x) = &g.bad { let _ = write!(b, "<div class=\"bad\"><h5>Noncompliant code example</h5><pre class=\"code\">{}</pre></div>", h(x)); }
            if let Some(x) = &g.good { let _ = write!(b, "<div class=\"good\"><h5>Compliant solution</h5><pre class=\"code\">{}</pre></div>", h(x)); }
            b.push_str("</div>\n");
        }
        for f in g.findings.iter().take(MAX_DETAILED) {
            let _ = write!(b, "<div class=\"where\"><div class=\"loc\">{}:{}</div>", h(&f.file), f.line);
            if f.message != g.description { let _ = write!(b, "<div class=\"desc\">{}</div>", h(&f.message)); }
            if let Some(s) = sn.get(&f.file, f.line) { b.push_str(&code_block(&s)); }
            b.push_str("</div>\n");
        }
        if g.findings.len() > MAX_DETAILED {
            let _ = writeln!(b, "<p class=\"more\">외 {}건은 아래 All Locations 표에 있습니다.</p>", g.findings.len() - MAX_DETAILED);
        }
        b.push_str("</div>\n");
    }

    // 핫스팟
    let hs = rep.hotspots();
    if !hs.is_empty() {
        b.push_str("<h2 id=\"hotspots\">Security Hotspots</h2>\n<p class=\"meta\">핫스팟은 검토가 필요한 코드입니다. 안전하다고 판단하면 Gitea 이슈에 <code>qualgate:safe</code> 라벨을 붙이세요.</p>\n<table><tr><th>#</th><th>파일</th><th class=\"num\">줄</th><th>규칙</th><th>심각도</th><th>메시지</th></tr>\n");
        for (i, f) in hs.iter().enumerate() {
            let _ = writeln!(b, "<tr><td class=\"num\">{}</td><td><code>{}</code></td><td class=\"num\">{}</td><td><code>{}</code></td><td>{}</td><td>{}</td></tr>",
                i + 1, h(&f.file), f.line, h(&f.rule), sev_label(f.severity), h(first_line(&f.message)));
        }
        b.push_str("</table>\n");
    }

    // 중복
    if !rep.clones.is_empty() {
        b.push_str("<h2 id=\"duplications\">Duplications</h2>\n<table><tr><th>#</th><th class=\"num\">줄 수</th><th>위치</th></tr>\n");
        for (i, c) in rep.clones.iter().enumerate() {
            let locs = c.parts.iter().map(|p| format!("<code>{}:{}-{}</code>", h(&p.file), p.start_line, p.end_line)).collect::<Vec<_>>().join("<br>");
            let _ = writeln!(b, "<tr><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{locs}</td></tr>", i + 1, c.lines);
        }
        b.push_str("</table>\n");
    }

    // 전체 위치
    let issues = rep.issues();
    if !issues.is_empty() {
        b.push_str("<h2 id=\"locations\">All Locations</h2><table><tr><th>#</th><th>파일</th><th class=\"num\">줄</th><th>규칙</th><th>유형</th><th>심각도</th><th>메시지</th></tr>\n");
        for (i, f) in issues.iter().enumerate() {
            let _ = writeln!(b, "<tr><td class=\"num\">{}</td><td><code>{}</code></td><td class=\"num\">{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>",
                i + 1, h(&f.file), f.line, h(&f.rule), IssueType::of(f).label(), sev_label(f.severity), h(first_line(&f.message)));
        }
        b.push_str("</table>\n");
    }
    unanalyzed_html(&mut b, r, rep.key);

    let _ = write!(b, "<div class=\"footer\">qualgate {} · 등급은 해당 유형 위반의 가장 높은 심각도로 정합니다(없음 A, LOW B, MEDIUM C, HIGH D, CRITICAL E). 게이트는 publish 전의 예비 판정이며 최종 결과는 Tekton result <code>gate</code>와 Gitea 요약 이슈에 있습니다. 규칙 예시는 규칙 저장소의 <code>rules/tests</code>에서, 위치 코드는 분석한 소스에서 옮겼습니다.</div>\n</div></body></html>\n", env!("CARGO_PKG_VERSION"));
    b
}

fn unanalyzed_html(b: &mut String, r: &ScanResult, key: &str) {
    let files: Vec<&String> = r.unanalyzed.iter().filter(|f| crate::select::report_lang(f) == Some(key)).collect();
    if files.is_empty() { return; }
    let _ = write!(b, "<h3>분석하지 못한 파일 ({}개)</h3><ul>", files.len());
    for f in files { let _ = write!(b, "<li><code>{}</code></li>", h(f)); }
    b.push_str("</ul>\n");
}

fn first_line(s: &str) -> &str { s.lines().next().unwrap_or("") }

// ---------------------------------------------------------------- Markdown

/// 표 칸과 본문에서 Markdown·HTML 문법이 되지 않게 한다.
fn m(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '|' => o.push_str("\\|"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '\\' | '`' | '*' | '_' | '[' | ']' | '#' => { o.push('\\'); o.push(c); }
            '\n' | '\r' => o.push(' '),
            _ => o.push(c),
        }
    }
    o
}

/// 인라인 코드. 내용 속 백틱보다 긴 구분자를 쓴다.
fn mc(s: &str) -> String {
    let s = s.replace(['\n', '\r'], " ");
    let longest = s.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    let pad = if s.starts_with('`') || s.ends_with('`') { " " } else { "" };
    format!("{fence}{pad}{s}{pad}{fence}")
}

fn fenced(code: &str, lang: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in code.chars() { if c == '`' { run += 1; longest = longest.max(run); } else { run = 0; } }
    let fence = "`".repeat((longest + 1).max(3));
    format!("{fence}{lang}\n{code}\n{fence}\n")
}

fn md_lang(key: &str) -> &'static str {
    match key { "java" => "java", "kotlin" => "kotlin", "python" => "python", "go" => "go", "ts" | "react" => "tsx", "vue" => "vue", _ => "" }
}

pub fn markdown(inp: &Input, rep: &LangReport, g: &Gate) -> String {
    let r = inp.result;
    let repo = src_field(r, "repo");
    let mut b = String::new();
    let _ = writeln!(b, "# Code analysis\n## {} — {}\n#### Branch {}\n#### Version {}\n\n**By: qualgate {}**\n\n*Date: {}*\n\n*Run: {}*\n",
        m(&repo), m(rep.name), m(r.branch.as_deref().unwrap_or("알 수 없음")), m(short(&r.commit)), env!("CARGO_PKG_VERSION"), m(&inp.date), m(&inp.name));
    let _ = writeln!(b, "## Introduction\n이 문서는 {}의 {} 코드 분석 결과입니다.\n", m(&repo), m(rep.name));
    let _ = writeln!(b, "## Configuration\n\n- 설정 저장소: {} (ref {}, 커밋 {})\n- 룰셋: {}\n- 실행 규칙: {}개 (이 언어 {}개)\n",
        m(&src_field(r, "config_repo")), m(&src_field(r, "ref")), m(&src_field(r, "config_commit")), m(&src_field(r, "ruleset")),
        r.active_rules.len(), r.active_rules.iter().filter(|x| rule_lang(x) == Some(rep.key)).count());

    b.push_str("## Synthesis\n\n### Analysis Status\n\nReliability | Security | Maintainability | Security Hotspots |\n:---:|:---:|:---:|:---:\n");
    let _ = writeln!(b, "{} | {} | {} | {} |\n", grade(&rep.findings, IssueType::Bug), grade(&rep.findings, IssueType::Vulnerability), grade(&rep.findings, IssueType::CodeSmell), rep.count(IssueType::Hotspot, None));

    let _ = writeln!(b, "### Quality gate status (프로젝트 전체, 예비 판정)\n\n| Quality Gate Status | {} |\n|-|-|\n", g.label());
    if !g.result.conditions.is_empty() {
        b.push_str("조건|실제|기준(실패)|결과\n---|---|---|---\n");
        for c in &g.result.conditions { let [a, x, y, z] = cond_row(c, g); let _ = writeln!(b, "{}|{}|{}|{}", m(&a), m(&x), m(&y), m(&z)); }
        b.push('\n');
    }
    if g.hotspots_pending { b.push_str("신규 핫스팟 검토율은 publish 단계에서 판정합니다.\n\n"); }

    b.push_str("### Metrics\n\n범위 | 파일 | 테스트 파일 | 줄 수 | Coverage | Duplications |\n:---|:---:|:---:|:---:|:---:|:---:\n");
    if let Some(st) = rep.stats {
        let _ = writeln!(b, "{} | {} | {} | {} | {} | {}", m(rep.name), st.files, st.test_files, st.lines, pct(st.coverage), pct(st.duplicated_lines));
    }
    let _ = writeln!(b, "프로젝트 전체 | - | - | - | {} | {}\n", pct(r.metrics.coverage), pct(r.metrics.duplicated_lines));

    b.push_str("## Issues\n\n### Issues count by severity and types\n\nType / Severity|");
    b.push_str(&SEVERITIES.map(sev_label).join("|"));
    b.push_str("|Total\n---|---|---|---|---|---|---\n");
    for ty in IssueType::ALL {
        let _ = writeln!(b, "{}|{}|{}", ty.label(), SEVERITIES.map(|s| rep.count(ty, Some(s)).to_string()).join("|"), rep.count(ty, None));
    }
    b.push_str("\n### Issues List\n\n");
    if rep.rules.is_empty() { b.push_str("이 언어에서 발견된 위반이 없습니다.\n\n"); } else {
        b.push_str("Rule|Name|Type|Severity|MOIS|CWE|OWASP API|Number\n---|---|---|---|---|---|---|---\n");
        for g in &rep.rules {
            let owasp = if g.owasp_api.is_empty() { "-".to_string() } else { g.owasp_api.join(", ") };
            let _ = writeln!(b, "{}|{}|{}|{}|{}|{}|{}|{}", mc(&g.id), m(&g.title), g.ty.label(), sev_label(g.severity),
                m(g.mois.as_deref().unwrap_or("-")), m(g.cwe.as_deref().unwrap_or("-")), m(&owasp), g.findings.len());
        }
        b.push('\n');
    }
    b.push_str("### OWASP API Security Top 10 (2023)\n\nItem|Name|Active rules|Issues|Rules\n---|---|---|---|---\n");
    for o in owasp_rows(inp, rep) {
        let rules = if o.rules.is_empty() { m(owasp_note(o.id)) } else { o.rules.iter().map(|r| mc(r)).collect::<Vec<_>>().join(", ") };
        let _ = writeln!(b, "{}|{}|{}|{}|{rules}", o.id, m(o.name), o.active_rules, o.findings);
    }
    b.push('\n');

    if !rep.rules.is_empty() {
        b.push_str("## Details & Recommendations\n\n");
        for g in &rep.rules {
            let _ = writeln!(b, "### {} {}\n\n{} · {} · {}건\n\n{}\n", m(&g.title), mc(&g.id), g.ty.label(), sev_label(g.severity), g.findings.len(), m(&g.description));
            if let Some(x) = &g.bad { let _ = writeln!(b, "**Noncompliant code example**\n\n{}", fenced(x, md_lang(rep.key))); }
            if let Some(x) = &g.good { let _ = writeln!(b, "**Compliant solution**\n\n{}", fenced(x, md_lang(rep.key))); }
            b.push_str("위치:\n\n");
            for f in &g.findings { let _ = writeln!(b, "- {}", mc(&format!("{}:{}", f.file, f.line))); }
            b.push('\n');
        }
    }

    b.push_str("## Security Hotspots\n\n");
    let hs = rep.hotspots();
    if hs.is_empty() { b.push_str("없음\n\n"); } else {
        b.push_str("### Security hotspots count by rule and severity\n\nRule / Severity|");
        b.push_str(&SEVERITIES.map(sev_label).join("|"));
        b.push_str("\n---|---|---|---|---|---\n");
        let hot_rules: BTreeSet<&str> = hs.iter().map(|f| f.rule.as_str()).collect();
        for rule in &hot_rules {
            let _ = writeln!(b, "{}|{}", mc(rule), SEVERITIES.map(|s| hs.iter().filter(|f| f.rule == *rule && f.severity == s).count().to_string()).join("|"));
        }
        b.push_str("\n### Security hotspots\n\n");
        b.push_str("File|Line|Rule|Severity|Message\n---|---|---|---|---\n");
        for f in &hs { let _ = writeln!(b, "{}|{}|{}|{}|{}", mc(&f.file), f.line, mc(&f.rule), sev_label(f.severity), m(first_line(&f.message))); }
        b.push('\n');
    }

    b.push_str("## Duplications\n\n");
    if rep.clones.is_empty() { b.push_str("없음\n\n"); } else {
        b.push_str("#|Lines|Locations\n---|---|---\n");
        for (i, c) in rep.clones.iter().enumerate() {
            let locs = c.parts.iter().map(|p| mc(&format!("{}:{}-{}", p.file, p.start_line, p.end_line))).collect::<Vec<_>>().join(", ");
            let _ = writeln!(b, "{}|{}|{locs}", i + 1, c.lines);
        }
        b.push('\n');
    }
    let un: Vec<&String> = r.unanalyzed.iter().filter(|f| crate::select::report_lang(f) == Some(rep.key)).collect();
    if !un.is_empty() {
        let _ = writeln!(b, "## Unanalyzed files ({})\n", un.len());
        for f in un { let _ = writeln!(b, "- {}", mc(f)); }
        b.push('\n');
    }
    b
}

// ---------------------------------------------------------------- XLSX

/// 셀 글자 수 한도(32767)보다 짧게 자른다.
fn xs(s: &str) -> String { if s.chars().count() > 32_000 { s.chars().take(32_000).collect() } else { s.to_string() } }

pub fn xlsx(inp: &Input, rep: &LangReport, g: &Gate, path: &Path) -> Result<()> {
    use rust_xlsxwriter::{Color, Format as XFormat, FormatAlign, Workbook, Worksheet, XlsxError};
    let r = inp.result;
    let head = XFormat::new().set_bold().set_background_color(Color::RGB(0xEEF1F7)).set_border(rust_xlsxwriter::FormatBorder::Thin);
    let bold = XFormat::new().set_bold();
    let wrap = XFormat::new().set_text_wrap().set_align(FormatAlign::Top);
    let mut wb = Workbook::new();

    fn header(ws: &mut Worksheet, cols: &[(&str, f64)], f: &XFormat) -> Result<(), XlsxError> {
        for (i, (name, w)) in cols.iter().enumerate() {
            ws.write_string_with_format(0, i as u16, *name, f)?;
            ws.set_column_width(i as u16, *w)?;
        }
        ws.set_freeze_panes(1, 0)?;
        Ok(())
    }
    fn finish(ws: &mut Worksheet, rows: u32, cols: u16) -> Result<(), XlsxError> {
        if rows > 0 { ws.autofilter(0, 0, rows, cols - 1)?; }
        Ok(())
    }

    let mut run = || -> Result<(), XlsxError> {
        // Summary
        let ws = wb.add_worksheet().set_name("Summary")?;
        ws.set_column_width(0, 34)?;
        ws.set_column_width(1, 60)?;
        let repo = src_field(r, "repo");
        let mut row = 0u32;
        let mut kv = |ws: &mut Worksheet, k: &str, v: String| -> Result<(), XlsxError> {
            ws.write_string_with_format(row, 0, k, &bold)?;
            ws.write_string(row, 1, xs(&v))?;
            row += 1;
            Ok(())
        };
        kv(ws, "Repository", repo)?;
        kv(ws, "Language", rep.name.to_string())?;
        kv(ws, "Branch", r.branch.clone().unwrap_or_else(|| "알 수 없음".into()))?;
        kv(ws, "Commit", r.commit.clone())?;
        kv(ws, "Date", inp.date.clone())?;
        kv(ws, "Run", inp.name.clone())?;
        kv(ws, "Config repository", format!("{} (ref {}, commit {})", src_field(r, "config_repo"), src_field(r, "ref"), src_field(r, "config_commit")))?;
        kv(ws, "Ruleset", src_field(r, "ruleset"))?;
        kv(ws, "Quality Gate (project, preliminary)", g.label().to_string())?;
        kv(ws, "Reliability", grade(&rep.findings, IssueType::Bug).to_string())?;
        kv(ws, "Security", grade(&rep.findings, IssueType::Vulnerability).to_string())?;
        kv(ws, "Maintainability", grade(&rep.findings, IssueType::CodeSmell).to_string())?;
        if let Some(st) = rep.stats {
            kv(ws, "Files", st.files.to_string())?;
            kv(ws, "Test files", st.test_files.to_string())?;
            kv(ws, "Lines", st.lines.to_string())?;
            kv(ws, "Coverage", pct(st.coverage))?;
            kv(ws, "Duplications", pct(st.duplicated_lines))?;
        }
        kv(ws, "Project coverage", pct(r.metrics.coverage))?;
        kv(ws, "Project duplications", pct(r.metrics.duplicated_lines))?;
        kv(ws, "Duplication groups", rep.clones.len().to_string())?;
        row += 1;
        ws.write_string_with_format(row, 0, "Type / Severity", &head)?;
        for (i, s) in SEVERITIES.iter().enumerate() { ws.write_string_with_format(row, i as u16 + 1, sev_label(*s), &head)?; }
        ws.write_string_with_format(row, 6, "Total", &head)?;
        row += 1;
        for ty in IssueType::ALL {
            ws.write_string(row, 0, ty.label())?;
            for (i, s) in SEVERITIES.iter().enumerate() { ws.write_number(row, i as u16 + 1, rep.count(ty, Some(*s)) as f64)?; }
            ws.write_number(row, 6, rep.count(ty, None) as f64)?;
            row += 1;
        }
        if !g.result.conditions.is_empty() {
            row += 1;
            for (i, t) in ["Condition", "Actual", "Threshold (fail)", "Status"].iter().enumerate() { ws.write_string_with_format(row, i as u16, *t, &head)?; }
            row += 1;
            for c in &g.result.conditions {
                for (i, v) in cond_row(c, g).iter().enumerate() { ws.write_string(row, i as u16, v)?; }
                row += 1;
            }
        }

        row += 1;
        for (i, t) in ["OWASP API (2023)", "Name", "Active rules", "Issues", "Rules"].iter().enumerate() { ws.write_string_with_format(row, i as u16, *t, &head)?; }
        row += 1;
        for o in owasp_rows(inp, rep) {
            ws.write_string(row, 0, o.id)?;
            ws.write_string(row, 1, o.name)?;
            ws.write_number(row, 2, o.active_rules as f64)?;
            ws.write_number(row, 3, o.findings as f64)?;
            ws.write_string(row, 4, if o.rules.is_empty() { owasp_note(o.id).to_string() } else { o.rules.join(", ") })?;
            row += 1;
        }

        // Issues
        let ws = wb.add_worksheet().set_name("Issues")?;
        let cols = [("Rule", 28.0), ("Message", 70.0), ("Type", 16.0), ("Severity", 10.0), ("Language", 14.0), ("File", 60.0), ("Line", 8.0), ("MOIS", 8.0), ("CWE", 10.0), ("OWASP API", 22.0)];
        header(ws, &cols, &head)?;
        let issues = rep.issues();
        for (i, f) in issues.iter().enumerate() {
            let row = i as u32 + 1;
            let info = inp.catalog.rules.get(&f.rule);
            ws.write_string(row, 0, &f.rule)?;
            ws.write_string_with_format(row, 1, xs(&f.message), &wrap)?;
            ws.write_string(row, 2, IssueType::of(f).label())?;
            ws.write_string(row, 3, sev_label(f.severity))?;
            ws.write_string(row, 4, rep.name)?;
            ws.write_string(row, 5, xs(&f.file))?;
            ws.write_number(row, 6, f64::from(f.line))?;
            ws.write_string(row, 7, info.and_then(|x| x.mois.as_deref()).unwrap_or(""))?;
            ws.write_string(row, 8, info.and_then(|x| x.cwe.as_deref()).unwrap_or(""))?;
            ws.write_string(row, 9, info.map(|x| x.owasp_api.join(", ")).unwrap_or_default())?;
        }
        finish(ws, issues.len() as u32, cols.len() as u16)?;

        // Rules
        let ws = wb.add_worksheet().set_name("Rules")?;
        let cols = [("Rule", 28.0), ("Name", 60.0), ("Type", 18.0), ("Severity", 10.0), ("MOIS", 8.0), ("CWE", 10.0), ("Number", 9.0), ("OWASP API", 22.0)];
        header(ws, &cols, &head)?;
        for (i, g) in rep.rules.iter().enumerate() {
            let row = i as u32 + 1;
            ws.write_string(row, 0, &g.id)?;
            ws.write_string(row, 1, xs(&g.title))?;
            ws.write_string(row, 2, g.ty.label())?;
            ws.write_string(row, 3, sev_label(g.severity))?;
            ws.write_string(row, 4, g.mois.as_deref().unwrap_or(""))?;
            ws.write_string(row, 5, g.cwe.as_deref().unwrap_or(""))?;
            ws.write_number(row, 6, g.findings.len() as f64)?;
            ws.write_string(row, 7, g.owasp_api.join(", "))?;
        }
        finish(ws, rep.rules.len() as u32, cols.len() as u16)?;

        // Security Hotspots
        let ws = wb.add_worksheet().set_name("Security Hotspots")?;
        let cols = [("Rule", 28.0), ("Message", 70.0), ("Category", 14.0), ("Severity", 10.0), ("Language", 14.0), ("File", 60.0), ("Line", 8.0), ("Status", 14.0)];
        header(ws, &cols, &head)?;
        let hs = rep.hotspots();
        let new_keys: BTreeSet<(&str, &str, u32)> = r.new_hotspots.iter().map(|f| (f.rule.as_str(), f.file.as_str(), f.line)).collect();
        for (i, f) in hs.iter().enumerate() {
            let row = i as u32 + 1;
            ws.write_string(row, 0, &f.rule)?;
            ws.write_string_with_format(row, 1, xs(&f.message), &wrap)?;
            ws.write_string(row, 2, &f.category)?;
            ws.write_string(row, 3, sev_label(f.severity))?;
            ws.write_string(row, 4, rep.name)?;
            ws.write_string(row, 5, xs(&f.file))?;
            ws.write_number(row, 6, f64::from(f.line))?;
            ws.write_string(row, 7, if new_keys.contains(&(f.rule.as_str(), f.file.as_str(), f.line)) { "TO_REVIEW (new)" } else { "TO_REVIEW" })?;
        }
        finish(ws, hs.len() as u32, cols.len() as u16)?;

        // Duplications
        let ws = wb.add_worksheet().set_name("Duplications")?;
        let cols = [("Group", 8.0), ("Lines", 8.0), ("File", 70.0), ("Start", 8.0), ("End", 8.0)];
        header(ws, &cols, &head)?;
        let mut row = 0u32;
        for (i, c) in rep.clones.iter().enumerate() {
            for p in &c.parts {
                row += 1;
                ws.write_number(row, 0, (i + 1) as f64)?;
                ws.write_number(row, 1, f64::from(c.lines))?;
                ws.write_string(row, 2, xs(&p.file))?;
                ws.write_number(row, 3, f64::from(p.start_line))?;
                ws.write_number(row, 4, f64::from(p.end_line))?;
            }
        }
        finish(ws, row, cols.len() as u16)?;
        Ok(())
    };
    run().context("XLSX 작성 실패")?;
    wb.save(path).with_context(|| format!("{} 쓰기 실패", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::RuleInfo;
    use crate::settings::GateCondition;

    fn f(rule: &str, file: &str, line: u32, sev: Severity, cat: &str, kind: Kind) -> Finding {
        Finding { rule: rule.into(), file: file.into(), line, line_hash: "h".into(), message: format!("{rule} 메시지 <b>|x|</b>"), severity: sev, category: cat.into(), kind }
    }

    fn result() -> ScanResult {
        let mut languages = BTreeMap::new();
        languages.insert("java".to_string(), LangStats { files: 2, test_files: 1, lines: 40, coverage: Some(50.0), duplicated_lines: Some(10.0) });
        languages.insert("python".to_string(), LangStats { files: 1, ..Default::default() });
        ScanResult {
            schema: SCHEMA, commit: "0123456789abcdef".into(), branch: Some("main".into()),
            findings: vec![
                f("java.sql-concat", "src/A.java", 3, Severity::High, "security", Kind::Issue),
                f("java.sql-concat", "src/A.java", 1, Severity::High, "security", Kind::Issue),
                f("java.empty-catch", "src/B.java", 2, Severity::Low, "maintainability", Kind::Issue),
                f("java.ssrf", "src/B.java", 4, Severity::Medium, "security", Kind::Hotspot),
                f("ts.code-eval", "web/a.ts", 1, Severity::Critical, "security", Kind::Issue),
            ],
            clones: vec![CloneGroup { key: "k".into(), lines: 12, parts: vec![
                ClonePart { file: "src/A.java".into(), start_line: 1, end_line: 12 },
                ClonePart { file: "src/B.java".into(), start_line: 1, end_line: 12 },
            ]}],
            metrics: Metrics { coverage: Some(40.0), new_issues: Some(1), ..Default::default() },
            coverage_status: CoverageStatus::default(),
            unanalyzed: vec!["src/Broken.java".into()],
            publish: PublishConfig { max_new_issues: 20, labels: vec![] },
            active_dirs: vec!["java".into()], active_rules: vec!["java.sql-concat".into()],
            gate_conditions: vec![GateCondition { metric: Metric::NewIssues, op: Op::Gt, value: 0.0 }],
            config_source: serde_json::json!({"repo": "jenny/spring-petclinic", "ruleset": "std"}),
            new_hotspots: vec![], new_code_measured: true, coverage_required_missing: false,
            languages,
        }
    }

    fn catalog() -> Catalog {
        Catalog::from_rules(vec![RuleInfo {
            id: "java.sql-concat".into(), dir: "java".into(), severity: Severity::High, kind: Kind::Issue,
            category: "security".into(), mois: Some("IV-01".into()), cwe: Some("CWE-89".into()), title: "SQL 삽입".into(),
            owasp_api: vec!["API8:2023".into()],
        }])
    }

    fn setup() -> (tempfile::TempDir, tempfile::TempDir) {
        let rules = tempfile::tempdir().unwrap();
        let t = rules.path().join("tests/java");
        std::fs::create_dir_all(&t).unwrap();
        std::fs::write(t.join("sql-concat.bad.java"), "s.executeQuery(\"a\" + x);").unwrap();
        std::fs::write(t.join("sql-concat.good.java"), "ps.setString(1, x);").unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(src.path().join("src")).unwrap();
        std::fs::write(src.path().join("src/A.java"), "l1\nl2 <x>\nl3\nl4\nl5\nl6\nl7\n").unwrap();
        (rules, src)
    }

    #[test]
    fn formats_accept_lists_brackets_and_reject_unknown() {
        assert_eq!(parse_formats(&["[html, md, xlsx]".into()]).unwrap(), vec![Format::Html, Format::Md, Format::Xlsx]);
        assert_eq!(parse_formats(&["xlsx".into(), "HTML".into(), "html".into()]).unwrap(), vec![Format::Html, Format::Xlsx]);
        assert!(parse_formats(&["".into()]).unwrap().is_empty());
        assert!(parse_formats(&["[]".into()]).unwrap().is_empty());
        assert!(parse_formats(&["pdf".into()]).is_err());
    }

    #[test]
    fn languages_and_grouping() {
        let r = result();
        assert_eq!(languages(&r), vec!["java", "python", "ts"]);
        let (rules, src) = setup();
        let cat = catalog();
        let inp = Input { result: &r, catalog: &cat, rules_root: rules.path(), src: Some(src.path()), name: "build-1".into(), date: "2026-11-18".into() };
        let rep = build(&inp, "java");
        assert_eq!(rep.findings.len(), 4);
        assert_eq!((rep.findings[0].file.as_str(), rep.findings[0].line), ("src/A.java", 1));
        assert_eq!(rep.rules.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), ["java.sql-concat", "java.empty-catch", "java.ssrf"]);
        assert_eq!(rep.rules[0].title, "SQL 삽입");
        assert_eq!(rep.rules[0].bad.as_deref(), Some("s.executeQuery(\"a\" + x);"));
        assert_eq!(rep.rules[1].ty, IssueType::CodeSmell);
        assert_eq!(rep.rules[2].ty, IssueType::Hotspot);
        assert_eq!(grade(&rep.findings, IssueType::Vulnerability), 'D');
        assert_eq!(grade(&rep.findings, IssueType::CodeSmell), 'B');
        assert_eq!(grade(&rep.findings, IssueType::Bug), 'A');
        assert_eq!(rep.clones.len(), 1);
        assert!(build(&inp, "python").findings.is_empty());
    }

    #[test]
    fn snippets_window_and_reject_escaping_paths() {
        let (_, src) = setup();
        let mut sn = Snippets::new(Some(src.path()));
        let s = sn.get("src/A.java", 2).unwrap();
        assert_eq!((s.start, s.hit, s.lines.len()), (1, 2, 5));
        let s = sn.get("src/A.java", 7).unwrap();
        assert_eq!((s.start, s.lines.len()), (4, 4));
        assert!(sn.get("src/A.java", 99).is_none());
        assert!(sn.get("../etc/passwd", 1).is_none());
        assert!(sn.get("/etc/passwd", 1).is_none());
        assert!(Snippets::new(None).get("src/A.java", 1).is_none());
    }

    #[test]
    fn gate_waits_for_publish_only_with_new_hotspots() {
        let mut r = result();
        r.gate_conditions.push(GateCondition { metric: Metric::NewHotspotsReviewed, op: Op::Lt, value: 100.0 });
        let g = gate(&r);
        assert!(!g.result.passed && !g.hotspots_pending);
        assert_eq!(g.label(), "실패");
        r.metrics.new_issues = Some(0);
        r.new_hotspots = vec![r.findings[3].clone()];
        let g = gate(&r);
        assert!(g.result.passed && g.hotspots_pending);
        assert_eq!(g.label(), "판정 대기");
        r.gate_conditions.clear();
        assert_eq!(gate(&r).label(), "통과(조건 없음)");
    }

    #[test]
    fn writes_every_format_per_language_and_escapes_repo_text() {
        let r = result();
        let (rules, src) = setup();
        let cat = catalog();
        let out = tempfile::tempdir().unwrap();
        let inp = Input { result: &r, catalog: &cat, rules_root: rules.path(), src: Some(src.path()), name: "build-jenny 1".into(), date: "2026-11-18".into() };
        let files = write_all(&inp, &[Format::Html, Format::Md, Format::Xlsx], out.path()).unwrap();
        let names: Vec<String> = files.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        for l in ["java", "python", "ts"] {
            for e in ["html", "md", "xlsx"] { assert!(names.contains(&format!("build-jenny-1-{l}-analysis-report.{e}")), "{names:?}"); }
        }
        let html = std::fs::read_to_string(out.path().join("build-jenny-1-java-analysis-report.html")).unwrap();
        assert!(html.contains("Static Analysis Report — jenny/spring-petclinic · Java"));
        assert!(html.contains("&lt;b&gt;&#124;x&#124;&lt;/b&gt;") || html.contains("&lt;b&gt;|x|&lt;/b&gt;"));
        assert!(!html.contains("<b>|x|</b>"));
        assert!(html.contains("<span class=\"hit\"><span class=\"ln\">1</span>l1</span>"), "{html}");
        assert!(html.contains("l2 &lt;x&gt;"));
        assert!(html.contains("Noncompliant code example") && html.contains("ps.setString(1, x);"));
        assert!(html.contains("grade-D") && html.contains("gate-fail"));
        assert!(html.contains("src/Broken.java"));
        let md = std::fs::read_to_string(out.path().join("build-jenny-1-java-analysis-report.md")).unwrap();
        assert!(md.contains("## Synthesis") && md.contains("VULNERABILITY|0|0|0|2|0|2"), "{md}");
        assert!(md.contains("&lt;b&gt;\\|x\\|&lt;/b&gt;"), "{md}");
        assert!(md.contains("`java.sql-concat`|SQL 삽입|VULNERABILITY|HIGH|IV-01|CWE-89|API8:2023|2"), "{md}");
        assert!(md.contains("API8:2023|Security Misconfiguration|1|2|`java.sql-concat`"), "{md}");
        assert!(md.contains("API6:2023|Unrestricted Access to Sensitive Business Flows|0|0|정적 분석 대상 아님"), "{md}");
        assert!(html.contains("<span class=\"tag\">OWASP API8:2023</span>"));
        assert!(html.contains("OWASP API Security Top 10 (2023)"));
        let ts = std::fs::read_to_string(out.path().join("build-jenny-1-ts-analysis-report.md")).unwrap();
        assert!(ts.contains("CRITICAL") && !ts.contains("java.sql-concat"));
        let x = std::fs::read(out.path().join("build-jenny-1-java-analysis-report.xlsx")).unwrap();
        assert_eq!(&x[..2], b"PK");
    }

    #[test]
    fn inline_code_survives_backticks() {
        assert_eq!(mc("a`b"), "``a`b``");
        assert_eq!(mc("`a"), "`` `a ``");
        assert!(fenced("x ``` y", "java").starts_with("````java\n"));
        assert_eq!(sanitize_name("../a b"), "a-b");
        assert_eq!(sanitize_name("///"), "qualgate");
    }
}
