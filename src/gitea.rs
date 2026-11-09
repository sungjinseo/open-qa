use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use base64::Engine;
use std::time::Duration;

use crate::httpcfg::Auth;
pub use crate::httpcfg::git_env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue { pub number: u64, pub title: String, pub body: String, pub open: bool, pub labels: Vec<String> }

pub trait IssueApi {
    fn default_branch(&mut self) -> Result<String>;
    fn ensure_label(&mut self, name: &str) -> Result<()>;
    fn list_issues(&mut self, label: &str) -> Result<Vec<Issue>>;
    fn create_issue(&mut self, title: &str, body: &str, labels: &[String]) -> Result<u64>;
    fn edit_issue(&mut self, number: u64, title: &str, body: &str, open: bool) -> Result<()>;
    fn comment(&mut self, number: u64, body: &str) -> Result<()>;
}

/// `auth_header`는 비밀 값이므로 Debug를 구현하지 않는다.
pub struct Client { agent: ureq::Agent, base: String, repo: String, auth_header: String }

#[derive(Deserialize)]
struct LabelDto { id: u64, name: String }
#[derive(Deserialize)]
struct IssueDto { number: u64, title: String, #[serde(default)] body: Option<String>, state: String, #[serde(default)] labels: Vec<LabelDto> }

pub(crate) enum Outcome { Transport, Status(u16) }

/// 재시도 정책: 멱등한 GET/PATCH는 전송 오류와 5xx를 재시도하고, POST는 429만 재시도한다.
/// (POST를 재시도하면 이슈와 댓글이 중복 생성될 수 있다. 다음 실행이 상태를 맞춘다.)
pub(crate) fn should_retry(method: &str, o: Outcome) -> bool {
    match o {
        Outcome::Status(429) => true,
        Outcome::Status(500..=599) | Outcome::Transport => method != "POST",
        Outcome::Status(_) => false,
    }
}

/// 재시도 전 대기 시간. 429의 `Retry-After`(초)가 있으면 60초까지 따르고, 없으면 지수 백오프(2, 4, 8초).
pub(crate) fn retry_delay(attempt: u32, retry_after: Option<u64>) -> Duration {
    match retry_after {
        Some(s) => Duration::from_secs(s.min(60)),
        None => Duration::from_secs(1 << attempt),
    }
}

/// 요청한 이름이 모두 존재해야 한다. 하나라도 없으면 라벨 없는 이슈가 생기므로 오류다.
fn resolve_labels(all: &[LabelDto], names: &[String]) -> Result<Vec<u64>> {
    names.iter().map(|n| {
        all.iter().find(|l| &l.name == n).map(|l| l.id).with_context(|| format!("라벨 '{n}'을(를) 찾을 수 없습니다"))
    }).collect()
}

/// 쿼리 값에서 예약 문자를 퍼센트 인코딩한다.
fn encode_query(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn auth_header(auth: &Auth) -> String {
    match auth {
        Auth::Token(t) => format!("token {t}"),
        Auth::Basic { user, pass } => {
            format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}")))
        }
    }
}

fn encode_path(s: &str) -> String {
    s.split('/').map(encode_query).collect::<Vec<_>>().join("/")
}

/// 경로 조각이 모두 비어 있지 않고 `.`/`..`이 아닌지.
fn safe_segments(s: &str) -> bool {
    s.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

/// 저장소 이름은 `owner/name` 두 조각이어야 하고 `.`, `..`, 빈 조각을 허용하지 않는다.
pub(crate) fn check_repo(repo: &str) -> Result<()> {
    if repo.split('/').count() != 2 || !safe_segments(repo) { bail!("저장소 이름은 owner/repo 형식이어야 합니다: {repo}"); }
    Ok(())
}

pub(crate) fn contents_url(base: &str, repo: &str, path: &str, git_ref: &str) -> Result<String> {
    check_repo(repo)?;
    if !safe_segments(path) { bail!("파일 경로에 빈 조각이나 '.', '..'을 쓸 수 없습니다: {path}"); }
    Ok(format!("{}/api/v1/repos/{}/contents/{}?ref={}", base.trim_end_matches('/'), repo, encode_path(path), encode_query(git_ref)))
}

pub(crate) fn latest_commit_url(base: &str, repo: &str, git_ref: &str) -> Result<String> {
    check_repo(repo)?;
    Ok(format!("{}/api/v1/repos/{}/commits?sha={}&limit=1", base.trim_end_matches('/'), repo, encode_query(git_ref)))
}

/// contents API의 파일 응답(JSON 객체, base64 `content`)에서 내용과 sha를 꺼낸다.
pub(crate) fn parse_file(v: &Value) -> Result<(Vec<u8>, String)> {
    if v["type"].as_str() != Some("file") { bail!("파일이 아닙니다(디렉터리이거나 알 수 없는 형식)"); }
    if v["encoding"].as_str() != Some("base64") { bail!("지원하지 않는 인코딩입니다"); }
    let raw: String = v["content"].as_str().context("content 없음")?.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD.decode(raw).context("base64 디코드 실패")?;
    let sha = v["sha"].as_str().context("sha 없음")?.to_string();
    Ok((bytes, sha))
}

pub(crate) fn parse_latest_commit(v: &Value) -> Result<String> {
    v.as_array().and_then(|a| a.first()).and_then(|c| c["sha"].as_str()).map(str::to_string).context("커밋 목록이 비어 있습니다")
}

impl Client {
    pub fn new(base: &str, repo: &str, auth: Auth, agent: ureq::Agent) -> Client {
        Client { agent, base: base.trim_end_matches('/').to_string(), repo: repo.to_string(), auth_header: auth_header(&auth) }
    }

    /// 파일 내용과 blob sha. 파일이 없으면(404) None.
    pub fn get_file(&mut self, repo: &str, path: &str, git_ref: &str) -> Result<Option<(Vec<u8>, String)>> {
        let url = contents_url(&self.base, repo, path, git_ref)?;
        match self.call_opt("GET", &url, None, true)? {
            None => Ok(None),
            Some(v) => parse_file(&v).map(Some).with_context(|| format!("파일 응답 해석 실패: {repo}/{path}")),
        }
    }

    /// 브랜치(또는 ref)의 최신 커밋 sha.
    pub fn latest_commit(&mut self, repo: &str, git_ref: &str) -> Result<String> {
        let url = latest_commit_url(&self.base, repo, git_ref)?;
        let v = self.call("GET", &url, None)?;
        parse_latest_commit(&v)
    }

    fn url(&self, path: &str) -> String { format!("{}/api/v1/repos/{}{}", self.base, self.repo, path) }

    fn call(&self, method: &str, url: &str, body: Option<&Value>) -> Result<Value> {
        Ok(self.call_opt(method, url, body, false)?.unwrap_or(Value::Null))
    }

    // 오류 메시지에 자격 증명을 넣지 않는다(url과 서버 응답 본문만 사용).
    // `allow_404`면 404를 None으로 돌려준다. GET의 409(푸시 직후 빈 저장소)는 재시도한다.
    fn call_opt(&self, method: &str, url: &str, body: Option<&Value>, allow_404: bool) -> Result<Option<Value>> {
        let mut last = String::new();
        let mut retry_after: Option<u64> = None;
        for attempt in 0..4 {
            if attempt > 0 { std::thread::sleep(retry_delay(attempt, retry_after.take())); }
            let auth = self.auth_header.as_str();
            let resp = match (method, body) {
                ("GET", _) => self.agent.get(url).header("Authorization", auth).call(),
                ("POST", Some(b)) => self.agent.post(url).header("Authorization", auth).send_json(b),
                ("PATCH", Some(b)) => self.agent.patch(url).header("Authorization", auth).send_json(b),
                _ => bail!("지원하지 않는 호출 {method}"),
            };
            let mut resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    last = e.to_string();
                    if should_retry(method, Outcome::Transport) { continue; }
                    bail!("Gitea 호출 실패: {method} {url}: {last}");
                }
            };
            let code = resp.status().as_u16();
            if code == 429 {
                retry_after = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse().ok());
            }
            let text = resp.body_mut().read_to_string().unwrap_or_default();
            match code {
                200..=299 => return if text.trim().is_empty() { Ok(Some(Value::Null)) } else { serde_json::from_str(&text).map(Some).context("Gitea 응답 파싱") },
                404 if allow_404 => return Ok(None),
                401 | 403 => bail!("Gitea 권한 오류 ({code}). 계정에 필요한 권한(이슈·라벨 쓰기, 저장소 읽기)이 있는지 확인하세요."),
                409 if method == "GET" => { last = "HTTP 409".to_string(); continue; }
                _ if should_retry(method, Outcome::Status(code)) => { last = format!("HTTP {code}"); continue; }
                _ => bail!("Gitea 오류 {code}: {method} {url}: {text}"),
            }
        }
        bail!("Gitea 호출 재시도 실패: {method} {url}: {last}")
    }

    fn all_labels(&mut self) -> Result<Vec<LabelDto>> {
        let mut out = vec![];
        for page in 1.. {
            let rows: Vec<LabelDto> = serde_json::from_value(self.call("GET", &self.url(&format!("/labels?limit=50&page={page}")), None)?)?;
            if rows.is_empty() { break; }
            out.extend(rows);
        }
        Ok(out)
    }
}

impl IssueApi for Client {
    fn default_branch(&mut self) -> Result<String> {
        let v = self.call("GET", &format!("{}/api/v1/repos/{}", self.base, self.repo), None)?;
        v["default_branch"].as_str().map(str::to_string).context("default_branch 없음")
    }

    fn ensure_label(&mut self, name: &str) -> Result<()> {
        if !self.all_labels()?.iter().any(|l| l.name == name) {
            self.call("POST", &self.url("/labels"), Some(&json!({"name": name, "color": "#ee8800"})))?;
        }
        Ok(())
    }

    fn list_issues(&mut self, label: &str) -> Result<Vec<Issue>> {
        let mut out = vec![];
        let label = encode_query(label);
        for page in 1.. {
            let url = self.url(&format!("/issues?state=all&type=issues&labels={label}&limit=50&page={page}"));
            let rows: Vec<IssueDto> = serde_json::from_value(self.call("GET", &url, None)?)?;
            if rows.is_empty() { break; }
            out.extend(rows.into_iter().map(|d| Issue {
                number: d.number, title: d.title, body: d.body.unwrap_or_default(),
                open: d.state == "open", labels: d.labels.into_iter().map(|l| l.name).collect(),
            }));
        }
        Ok(out)
    }

    fn create_issue(&mut self, title: &str, body: &str, labels: &[String]) -> Result<u64> {
        let ids = resolve_labels(&self.all_labels()?, labels)?;
        let v = self.call("POST", &self.url("/issues"), Some(&json!({"title": title, "body": body, "labels": ids})))?;
        v["number"].as_u64().context("생성된 이슈 번호 없음")
    }

    fn edit_issue(&mut self, number: u64, title: &str, body: &str, open: bool) -> Result<()> {
        let state = if open { "open" } else { "closed" };
        self.call("PATCH", &self.url(&format!("/issues/{number}")), Some(&json!({"title": title, "body": body, "state": state})))?;
        Ok(())
    }

    fn comment(&mut self, number: u64, body: &str) -> Result<()> {
        self.call("POST", &self.url(&format!("/issues/{number}/comments")), Some(&json!({"body": body})))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{auth_header, contents_url, encode_query, latest_commit_url, parse_file, parse_latest_commit, resolve_labels, retry_delay, should_retry, LabelDto, Outcome};
    use std::time::Duration;

    #[test]
    fn retry_policy_by_method() {
        assert!(should_retry("GET", Outcome::Transport));
        assert!(should_retry("PATCH", Outcome::Transport));
        assert!(!should_retry("POST", Outcome::Transport));
        for m in ["GET", "PATCH", "POST"] { assert!(should_retry(m, Outcome::Status(429))); }
        assert!(should_retry("GET", Outcome::Status(503)));
        assert!(should_retry("PATCH", Outcome::Status(500)));
        assert!(!should_retry("POST", Outcome::Status(502)));
        assert!(!should_retry("GET", Outcome::Status(404)));
    }

    #[test]
    fn retry_delay_honours_retry_after_capped_at_sixty() {
        assert_eq!(retry_delay(1, Some(5)), Duration::from_secs(5));
        assert_eq!(retry_delay(1, Some(0)), Duration::from_secs(0));
        assert_eq!(retry_delay(1, Some(3600)), Duration::from_secs(60));
        assert_eq!(retry_delay(1, None), Duration::from_secs(2));
        assert_eq!(retry_delay(3, None), Duration::from_secs(8));
    }

    #[test]
    fn unresolved_label_is_an_error() {
        let all = vec![LabelDto { id: 1, name: "a".into() }, LabelDto { id: 2, name: "b".into() }];
        assert_eq!(resolve_labels(&all, &["b".to_string(), "a".to_string()]).unwrap(), vec![2, 1]);
        assert!(resolve_labels(&all, &["a".to_string(), "c".to_string()]).is_err());
    }

    #[test]
    fn label_value_is_percent_encoded() {
        assert_eq!(encode_query("qualgate"), "qualgate");
        assert_eq!(encode_query("qualgate:ignored a"), "qualgate%3Aignored%20a");
    }

    #[test]
    fn contents_and_commit_urls_are_assembled_and_encoded() {
        assert_eq!(
            contents_url("https://g.example/", "qg/cfg", "projects/a b/app.toml", "feature/x").unwrap(),
            "https://g.example/api/v1/repos/qg/cfg/contents/projects/a%20b/app.toml?ref=feature%2Fx"
        );
        assert_eq!(latest_commit_url("https://g.example", "qg/cfg", "main").unwrap(), "https://g.example/api/v1/repos/qg/cfg/commits?sha=main&limit=1");
    }

    #[test]
    fn dot_dot_and_empty_segments_are_rejected() {
        for repo in ["qg/..", "../cfg", "qg//cfg", "qg", "qg/cfg/x", "/cfg", "qg/."] {
            assert!(contents_url("https://g.example", repo, "qualgate.toml", "main").is_err(), "{repo}");
            assert!(latest_commit_url("https://g.example", repo, "main").is_err(), "{repo}");
        }
        for path in ["../x.toml", "projects/../x.toml", "projects//x.toml", "", "a/", "./a"] {
            assert!(contents_url("https://g.example", "qg/cfg", path, "main").is_err(), "{path:?}");
        }
    }

    #[test]
    fn file_response_is_base64_decoded_with_sha() {
        let v = serde_json::json!({"type": "file", "encoding": "base64", "content": "aGVsbG8g\nd29ybGQ=\n", "sha": "abc123"});
        assert_eq!(parse_file(&v).unwrap(), (b"hello world".to_vec(), "abc123".to_string()));
        assert!(parse_file(&serde_json::json!([{"type": "file"}])).is_err(), "directory listing");
        assert!(parse_file(&serde_json::json!({"type": "file", "encoding": "base64", "content": "!!", "sha": "s"})).is_err());
        assert!(parse_file(&serde_json::json!({"type": "file", "encoding": "none", "content": "", "sha": "s"})).is_err());
    }

    #[test]
    fn latest_commit_is_first_sha() {
        assert_eq!(parse_latest_commit(&serde_json::json!([{"sha": "c2"}, {"sha": "c1"}])).unwrap(), "c2");
        assert!(parse_latest_commit(&serde_json::json!([])).is_err());
    }

    #[test]
    fn auth_header_formats() {
        use crate::httpcfg::Auth;
        assert_eq!(auth_header(&Auth::Token("t1".into())), "token t1");
        assert_eq!(auth_header(&Auth::Basic { user: "u".into(), pass: "p".into() }), "Basic dTpw");
    }
}
