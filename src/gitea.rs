use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use ureq::tls::{RootCerts, TlsConfig};

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

pub struct Client { agent: ureq::Agent, base: String, repo: String, token: String }

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

impl Client {
    pub fn new(base: &str, repo: &str, token: &str) -> Client {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .tls_config(TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build())
            .build()
            .into();
        Client { agent, base: base.trim_end_matches('/').to_string(), repo: repo.to_string(), token: token.to_string() }
    }

    fn url(&self, path: &str) -> String { format!("{}/api/v1/repos/{}{}", self.base, self.repo, path) }

    // 오류 메시지에 토큰을 넣지 않는다(url과 서버 응답 본문만 사용).
    fn call(&self, method: &str, url: &str, body: Option<&Value>) -> Result<Value> {
        let mut last = String::new();
        let mut retry_after: Option<u64> = None;
        for attempt in 0..4 {
            if attempt > 0 { std::thread::sleep(retry_delay(attempt, retry_after.take())); }
            let auth = format!("token {}", self.token);
            let resp = match (method, body) {
                ("GET", _) => self.agent.get(url).header("Authorization", &auth).call(),
                ("POST", Some(b)) => self.agent.post(url).header("Authorization", &auth).send_json(b),
                ("PATCH", Some(b)) => self.agent.patch(url).header("Authorization", &auth).send_json(b),
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
                200..=299 => return if text.trim().is_empty() { Ok(Value::Null) } else { serde_json::from_str(&text).context("Gitea 응답 파싱") },
                401 | 403 => bail!("Gitea 권한 오류 ({code}). 토큰에 write:issue와 저장소 읽기 권한이 필요합니다."),
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
    use super::{encode_query, resolve_labels, retry_delay, should_retry, LabelDto, Outcome};
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
}
