//! Gitea 접속 설정: 인증(토큰 또는 git-credentials), CA 파일, TLS 검증 옵션, git 하위 프로세스 환경.
//! 자격 증명은 오류 메시지, Debug 출력, 명령줄 인자에 넣지 않는다.
use anyhow::{anyhow, bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use ureq::tls::{parse_pem, Certificate, PemItem, RootCerts, TlsConfig};

#[derive(Clone, PartialEq, Eq)]
pub enum Auth { Token(String), Basic { user: String, pass: String } }

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Auth::Token(_) => f.write_str("Auth::Token(<redacted>)"),
            Auth::Basic { .. } => f.write_str("Auth::Basic { user: <redacted>, pass: <redacted> }"),
        }
    }
}

/// `scheme://host[:port]/...`를 (scheme, 소문자 host, 기본 포트를 채운 port)로 나눈다.
fn host_key(url: &str) -> Option<(String, String, u16)> {
    let (scheme, rest) = url.trim().split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    let authority = rest.split(['/', '?', '#']).next()?;
    let hostport = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let default = match scheme.as_str() { "https" => 443, "http" => 80, _ => return None };
    let (host, port) = if let Some(r) = hostport.strip_prefix('[') {
        let (h, tail) = r.split_once(']')?;
        (h, tail.strip_prefix(':').map(str::parse::<u16>).transpose().ok()?)
    } else if let Some((h, p)) = hostport.rsplit_once(':') {
        (h, Some(p.parse::<u16>().ok()?))
    } else {
        (hostport, None)
    };
    if host.is_empty() { return None; }
    Some((scheme, host.to_ascii_lowercase(), port.unwrap_or(default)))
}

/// 정규화한 origin(`scheme://host[:port]`, 소문자 host, 기본 포트 생략, IPv6는 대괄호, userinfo 제거)과
/// 끝 `/`를 뗀 경로. 클론 URL, 토큰 헤더 범위, credential store 줄이 모두 이 값을 쓴다.
fn normalized_base(url: &str) -> Option<(String, String)> {
    let (scheme, host, port) = host_key(url)?;
    let host = if host.contains(':') { format!("[{host}]") } else { host };
    let default = if scheme == "https" { 443 } else { 80 };
    let hostport = if port == default { host } else { format!("{host}:{port}") };
    let rest = url.trim().split_once("://")?.1;
    let path = rest.find(['/', '?', '#']).map_or("", |i| &rest[i..]);
    let path = path.split(['?', '#']).next().unwrap_or("").trim_end_matches('/');
    Some((format!("{scheme}://{hostport}"), path.to_string()))
}

/// `<GITEA_URL>/<owner>/<repo>.git`. credential store 줄과 같은 정규화 값으로 만든다.
pub fn clone_url(gitea_url: &str, repo: &str) -> Result<String> {
    crate::gitea::check_repo(repo)?;
    let (origin, base_path) = normalized_base(gitea_url).context("GITEA_URL 형식이 올바르지 않습니다(http 또는 https)")?;
    Ok(format!("{origin}{base_path}/{repo}.git"))
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn percent_encode_userinfo(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `.git-credentials` 내용에서 `gitea_url`과 scheme·호스트·포트가 같은 첫 줄의 사용자·비밀번호.
/// (https 줄의 비밀번호를 http 주소로 보내지 않는다.)
fn find_credentials(content: &str, gitea_url: &str) -> Option<Auth> {
    let key = host_key(gitea_url)?;
    for line in content.lines() {
        let line = line.trim();
        let Some(k) = host_key(line) else { continue };
        if k != key { continue; }
        let (_, rest) = line.split_once("://")?;
        let authority = rest.split(['/', '?', '#']).next()?;
        let Some((userinfo, _)) = authority.rsplit_once('@') else { continue };
        let Some((user, pass)) = userinfo.split_once(':') else { continue };
        if let (Some(user), Some(pass)) = (percent_decode(user), percent_decode(pass)) {
            return Some(Auth::Basic { user, pass });
        }
    }
    None
}

/// `GITEA_TOKEN` 우선, 없으면 `QUALGATE_GIT_CREDENTIALS` 파일에서 찾는다.
pub fn resolve_auth(gitea_url: &str) -> Result<Auth> {
    match std::env::var("GITEA_TOKEN") {
        Ok(t) if !t.is_empty() => return Ok(Auth::Token(t)),
        Err(std::env::VarError::NotUnicode(_)) => bail!("GITEA_TOKEN 값이 UTF-8 문자열이 아닙니다"),
        _ => {}
    }
    let none = || anyhow!("Gitea 자격 증명이 없습니다. GITEA_TOKEN 또는 QUALGATE_GIT_CREDENTIALS(GITEA_URL과 호스트가 같은 줄이 있는 파일)를 지정하세요");
    let Some(path) = std::env::var_os("QUALGATE_GIT_CREDENTIALS").filter(|p| !p.is_empty()) else { return Err(none()) };
    // 파일 내용과 OS 오류 원문은 오류 메시지에 넣지 않는다.
    let content = std::fs::read_to_string(&path)
        .map_err(|_| anyhow!("QUALGATE_GIT_CREDENTIALS 파일을 읽을 수 없습니다: {}", Path::new(&path).display()))?;
    find_credentials(&content, gitea_url).ok_or_else(|| {
        anyhow!("QUALGATE_GIT_CREDENTIALS 파일({})에 GITEA_URL과 호스트가 같은 자격 증명 줄이 없습니다", Path::new(&path).display())
    })
}

/// `QUALGATE_SSL_VERIFY`: 없음/`true` -> true, `false` -> false, 그 밖의 값은 오류.
pub fn ssl_verify() -> Result<bool> {
    match std::env::var("QUALGATE_SSL_VERIFY") {
        Err(std::env::VarError::NotPresent) => Ok(true),
        Err(_) => bail!("QUALGATE_SSL_VERIFY 값을 읽을 수 없습니다. true 또는 false여야 합니다"),
        Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => bail!("QUALGATE_SSL_VERIFY 값 '{v}'이(가) 올바르지 않습니다. true 또는 false여야 합니다"),
        },
    }
}

fn ca_file() -> Option<PathBuf> {
    std::env::var_os("QUALGATE_CA_FILE").filter(|p| !p.is_empty()).map(PathBuf::from)
}

fn read_ca_certs(path: &Path) -> Result<Vec<Certificate<'static>>> {
    let pem = std::fs::read(path).with_context(|| format!("QUALGATE_CA_FILE을 읽을 수 없습니다: {}", path.display()))?;
    let mut certs = vec![];
    for item in parse_pem(&pem) {
        match item {
            Ok(PemItem::Certificate(c)) => certs.push(c),
            Ok(_) => {}
            Err(_) => bail!("QUALGATE_CA_FILE이 올바른 PEM이 아닙니다: {}", path.display()),
        }
    }
    if certs.is_empty() { bail!("QUALGATE_CA_FILE에 PEM 인증서가 없습니다: {}", path.display()); }
    Ok(certs)
}

/// Gitea용 HTTP 에이전트. `QUALGATE_CA_FILE`이 있으면 시스템 루트에 그 인증서들을 더해 신뢰한다.
/// `QUALGATE_SSL_VERIFY=false`면 검증을 끄고 매번 stderr에 경고한다.
pub fn agent() -> Result<ureq::Agent> {
    let verify = ssl_verify()?;
    let mut tls = TlsConfig::builder();
    if !verify {
        eprintln!("경고: QUALGATE_SSL_VERIFY=false 이므로 Gitea TLS 인증서를 검증하지 않습니다. QUALGATE_CA_FILE로 CA를 신뢰하는 쪽을 권장합니다.");
        tls = tls.disable_verification(true);
    }
    tls = match ca_file() {
        Some(path) => {
            let extra = read_ca_certs(&path)?;
            let mut roots: Vec<Certificate<'static>> = rustls_native_certs::load_native_certs().certs.iter()
                .map(|der| Certificate::from_der(der.as_ref()).to_owned())
                .collect();
            roots.extend(extra);
            tls.root_certs(RootCerts::new_with_certs(&roots))
        }
        None => tls.root_certs(RootCerts::PlatformVerifier),
    };
    Ok(ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(60)))
        .tls_config(tls.build())
        .build()
        .into())
}

/// 값을 Debug 출력에서 가리는 문자열.
#[derive(Clone)]
pub struct Secret(String);
impl Secret { pub fn expose(&self) -> &str { &self.0 } }
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("<redacted>") }
}

/// git 하위 프로세스용 환경. credential store 임시 파일(0600)은 이 값이 drop될 때 삭제된다.
pub struct GitEnv {
    vars: Vec<(String, Secret)>,
    _store: Option<tempfile::TempPath>,
}

impl GitEnv {
    pub fn vars(&self) -> impl Iterator<Item = (&str, &str)> {
        self.vars.iter().map(|(k, v)| (k.as_str(), v.expose()))
    }
    pub fn apply(&self, cmd: &mut std::process::Command) {
        for (k, v) in self.vars() { cmd.env(k, v); }
    }

    /// 인증·TLS 설정이 없는 환경(로컬 file:// 저장소 테스트용).
    #[cfg(test)]
    pub(crate) fn empty() -> GitEnv {
        GitEnv { vars: vec![("GIT_TERMINAL_PROMPT".to_string(), Secret("0".into()))], _store: None }
    }
}

impl std::fmt::Debug for GitEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitEnv").field("vars", &self.vars).finish_non_exhaustive()
    }
}

fn shell_quote(path: &str) -> Result<String> {
    if path.contains('\'') { bail!("임시 파일 경로에 작은따옴표가 있어 credential helper를 설정할 수 없습니다"); }
    Ok(format!("'{path}'"))
}

/// git 프로세스에 넘길 환경(`GIT_CONFIG_COUNT/KEY/VALUE`). 자격 증명은 인자에 넣지 않는다.
/// Basic 인증이면 `workdir`에 0600 credential store 파일을 만든다.
pub fn git_env(auth: &Auth, gitea_url: &str, workdir: &Path) -> Result<GitEnv> {
    let mut cfg: Vec<(String, Secret)> = vec![];
    let mut add = |k: &str, v: String| cfg.push((k.to_string(), Secret(v)));
    if let Some(ca) = ca_file() { add("http.sslCAInfo", ca.to_string_lossy().into_owned()); }
    if !ssl_verify()? { add("http.sslVerify", "false".into()); }
    let mut store = None;
    let (origin, base_path) = normalized_base(gitea_url).context("GITEA_URL 형식이 올바르지 않습니다(http 또는 https)")?;
    // 사용자·시스템에 설정된 credential helper를 비운다(우리가 준 인증만 쓰게).
    add("credential.helper", String::new());
    match auth {
        Auth::Token(t) => {
            // Gitea 주소에만 보낸다(리다이렉트 등 다른 호스트로 새지 않게).
            add(&format!("http.{origin}{base_path}/.extraHeader"), format!("Authorization: token {t}"));
        }
        Auth::Basic { user, pass } => {
            let mut f = tempfile::Builder::new().prefix(".qualgate-cred-").tempfile_in(workdir).context("credential 임시 파일 생성 실패")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                f.as_file().set_permissions(std::fs::Permissions::from_mode(0o600)).context("credential 임시 파일 권한 설정 실패")?;
            }
            // 클론 URL(`clone_url`)과 같은 정규화 authority여야 git이 이 줄을 고른다.
            let userinfo = format!("://{}:{}@", percent_encode_userinfo(user), percent_encode_userinfo(pass));
            writeln!(f, "{}", origin.replacen("://", &userinfo, 1)).context("credential 임시 파일 쓰기 실패")?;
            f.flush().context("credential 임시 파일 쓰기 실패")?;
            let path = f.into_temp_path();
            add("credential.helper", format!("store --file={}", shell_quote(&path.to_string_lossy())?));
            store = Some(path);
        }
    }
    let mut vars = vec![("GIT_TERMINAL_PROMPT".to_string(), Secret("0".into())), ("GIT_CONFIG_COUNT".to_string(), Secret(cfg.len().to_string()))];
    for (i, (k, v)) in cfg.into_iter().enumerate() {
        vars.push((format!("GIT_CONFIG_KEY_{i}"), Secret(k)));
        vars.push((format!("GIT_CONFIG_VALUE_{i}"), v));
    }
    Ok(GitEnv { vars, _store: store })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, MutexGuard};

    static ENV: Mutex<()> = Mutex::new(());

    /// 환경 변수를 바꾸는 테스트를 직렬화하고 관련 변수를 비운다.
    fn env_lock() -> MutexGuard<'static, ()> {
        let g = ENV.lock().unwrap_or_else(|e| e.into_inner());
        for k in ["GITEA_TOKEN", "QUALGATE_GIT_CREDENTIALS", "QUALGATE_CA_FILE", "QUALGATE_SSL_VERIFY"] { std::env::remove_var(k); }
        g
    }

    fn dummy(tag: &str) -> String {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        format!("{tag}{n:x}")
    }

    fn creds_file(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn picks_the_credentials_line_for_the_gitea_host() {
        let _g = env_lock();
        let (u, p) = (dummy("u"), dummy("p"));
        let f = creds_file(&format!("https://other:x@elsewhere.example\nhttps://{u}:{p}%40ss@gitea.example:8443\nhttps://z:z@gitea.example\n"));
        std::env::set_var("QUALGATE_GIT_CREDENTIALS", f.path());
        let auth = resolve_auth("https://gitea.example:8443/").unwrap();
        assert_eq!(auth, Auth::Basic { user: u, pass: format!("{p}@ss") });
        // 포트가 다르면 다른 호스트다
        let other = resolve_auth("https://gitea.example").unwrap();
        assert!(matches!(other, Auth::Basic { ref user, .. } if user == "z"));
    }

    #[test]
    fn token_wins_over_credentials_file() {
        let _g = env_lock();
        let f = creds_file("https://a:b@gitea.example\n");
        std::env::set_var("QUALGATE_GIT_CREDENTIALS", f.path());
        let t = dummy("t");
        std::env::set_var("GITEA_TOKEN", &t);
        assert_eq!(resolve_auth("https://gitea.example").unwrap(), Auth::Token(t));
    }

    #[test]
    fn missing_credentials_is_an_error_without_file_content() {
        let _g = env_lock();
        let e = resolve_auth("https://gitea.example").unwrap_err().to_string();
        assert!(e.contains("GITEA_TOKEN"));
        let secret = dummy("s");
        let f = creds_file(&format!("https://someone:{secret}@not-gitea.example\n"));
        std::env::set_var("QUALGATE_GIT_CREDENTIALS", f.path());
        let e = format!("{:#}", resolve_auth("https://gitea.example").unwrap_err());
        assert!(!e.contains(&secret) && !e.contains("someone"), "{e}");
        std::env::set_var("QUALGATE_GIT_CREDENTIALS", f.path().with_extension("nope"));
        assert!(resolve_auth("https://gitea.example").is_err());
    }

    #[test]
    fn debug_hides_secrets() {
        let (p, t) = (dummy("p"), dummy("t"));
        let s = format!("{:?} {:?}", Auth::Basic { user: "alice".into(), pass: p.clone() }, Auth::Token(t.clone()));
        assert!(!s.contains(&p) && !s.contains(&t) && !s.contains("alice"), "{s}");
    }

    #[test]
    fn ssl_verify_values() {
        let _g = env_lock();
        assert!(ssl_verify().unwrap());
        std::env::set_var("QUALGATE_SSL_VERIFY", "true");
        assert!(ssl_verify().unwrap());
        std::env::set_var("QUALGATE_SSL_VERIFY", "false");
        assert!(!ssl_verify().unwrap());
        std::env::set_var("QUALGATE_SSL_VERIFY", "no");
        assert!(ssl_verify().is_err());
        assert!(agent().is_err());
    }

    #[test]
    fn ca_file_must_be_pem() {
        let _g = env_lock();
        let f = creds_file("this is not a certificate\n");
        std::env::set_var("QUALGATE_CA_FILE", f.path());
        assert!(agent().is_err());
        std::env::set_var("QUALGATE_CA_FILE", f.path().with_extension("nope"));
        assert!(agent().is_err());
    }

    /// 자체 서명 CA가 서명한 서버 인증서로 응답하는 로컬 TLS 서버(연결 수만큼 "ok" 응답).
    fn tls_server(conns: usize) -> (u16, String) {
        use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair, KeyUsagePurpose};
        let ca_key = KeyPair::generate().unwrap();
        let mut ca_params = CertificateParams::new(vec![]).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let key = KeyPair::generate().unwrap();
        let cert = CertificateParams::new(vec!["localhost".to_string()]).unwrap().signed_by(&key, &ca, &ca_key).unwrap();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions().unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert.der().clone()], rustls::pki_types::PrivateKeyDer::Pkcs8(key.serialize_der().into()))
            .unwrap();
        let cfg = Arc::new(cfg);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for _ in 0..conns {
                let Ok((tcp, _)) = listener.accept() else { return };
                let Ok(conn) = rustls::ServerConnection::new(cfg.clone()) else { continue };
                let mut tls = rustls::StreamOwned::new(conn, tcp);
                let mut buf = [0u8; 4096];
                let mut got = vec![];
                // 핸드셰이크 실패(클라이언트가 인증서를 거부)면 읽기가 오류로 끝난다.
                while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                    match std::io::Read::read(&mut tls, &mut buf) { Ok(0) | Err(_) => break, Ok(n) => got.extend_from_slice(&buf[..n]) }
                }
                if got.is_empty() { continue; }
                let _ = tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
                let _ = tls.flush();
                tls.conn.send_close_notify();
                let _ = tls.flush();
            }
        });
        (port, ca.pem())
    }

    fn get_status(port: u16) -> Result<u16> {
        let a = agent()?;
        let r = a.get(&format!("https://localhost:{port}/")).call()?;
        Ok(r.status().as_u16())
    }

    #[test]
    fn tls_ca_file_and_verify_switch() {
        let _g = env_lock();
        let (port, ca_pem) = tls_server(3);
        // CA 미지정: 실패
        assert!(get_status(port).is_err());
        // CA 지정: 성공
        let ca = creds_file(&ca_pem);
        std::env::set_var("QUALGATE_CA_FILE", ca.path());
        assert_eq!(get_status(port).unwrap(), 200);
        // 검증 끄기: 성공
        std::env::remove_var("QUALGATE_CA_FILE");
        std::env::set_var("QUALGATE_SSL_VERIFY", "false");
        assert_eq!(get_status(port).unwrap(), 200);
    }

    fn cfg_pairs(env: &GitEnv) -> Vec<(String, String)> {
        let m: std::collections::HashMap<_, _> = env.vars().collect();
        let n: usize = m["GIT_CONFIG_COUNT"].parse().unwrap();
        (0..n).map(|i| (m[format!("GIT_CONFIG_KEY_{i}").as_str()].to_string(), m[format!("GIT_CONFIG_VALUE_{i}").as_str()].to_string())).collect()
    }

    #[test]
    fn git_env_basic_writes_private_store_and_removes_it() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let (u, p) = (dummy("u"), dummy("p"));
        let auth = Auth::Basic { user: u.clone(), pass: format!("{p}@x") };
        std::env::set_var("QUALGATE_CA_FILE", "/tmp/ca.pem");
        std::env::set_var("QUALGATE_SSL_VERIFY", "false");
        let env = git_env(&auth, "https://gitea.example", dir.path()).unwrap();
        let pairs = cfg_pairs(&env);
        assert!(pairs.contains(&("http.sslCAInfo".into(), "/tmp/ca.pem".into())));
        assert!(pairs.contains(&("http.sslVerify".into(), "false".into())));
        let helper = pairs.iter().filter(|(k, _)| k == "credential.helper").map(|(_, v)| v.clone()).collect::<Vec<_>>();
        assert_eq!(helper[0], "");
        let file = helper[1].strip_prefix("store --file='").unwrap().strip_suffix('\'').unwrap().to_string();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), format!("https://{u}:{p}%40x@gitea.example\n"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        }
        assert_eq!(env.vars().find(|(k, _)| *k == "GIT_TERMINAL_PROMPT").unwrap().1, "0");
        let dbg = format!("{env:?} {auth:?}");
        assert!(!dbg.contains(&p) && !dbg.contains(&u), "{dbg}");
        assert!(!pairs.iter().any(|(_, v)| v.contains(&p)));
        drop(env);
        assert!(!Path::new(&file).exists());
    }

    #[test]
    fn git_env_token_uses_scoped_extra_header_without_store() {
        let _g = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let t = dummy("t");
        let env = git_env(&Auth::Token(t.clone()), "https://gitea.example/", dir.path()).unwrap();
        let pairs = cfg_pairs(&env);
        assert!(pairs.contains(&("http.https://gitea.example/.extraHeader".into(), format!("Authorization: token {t}"))));
        // 사용자·시스템 credential helper가 끼어들지 않도록 비운다(값 없는 항목 하나뿐).
        let helpers: Vec<_> = pairs.iter().filter(|(k, _)| k.starts_with("credential")).collect();
        assert_eq!(helpers, vec![&("credential.helper".to_string(), String::new())]);
        assert!(!pairs.iter().any(|(k, _)| k == "http.sslVerify" || k == "http.sslCAInfo"));
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert!(!format!("{env:?}").contains(&t));
    }

    #[test]
    fn credentials_line_must_match_scheme() {
        let _g = env_lock();
        // 같은 호스트·포트라도 https 줄의 비밀번호를 http 주소에 보내지 않는다.
        let f = creds_file("https://a:b@gitea.example:80\nhttp://c:d@gitea.example:443\n");
        std::env::set_var("QUALGATE_GIT_CREDENTIALS", f.path());
        assert!(resolve_auth("http://gitea.example").is_err());
        assert!(resolve_auth("https://gitea.example").is_err());
        let f = creds_file("https://a:b@gitea.example\nhttp://c:d@gitea.example\n");
        std::env::set_var("QUALGATE_GIT_CREDENTIALS", f.path());
        assert_eq!(resolve_auth("http://gitea.example").unwrap(), Auth::Basic { user: "c".into(), pass: "d".into() });
        assert_eq!(resolve_auth("HTTPS://gitea.example").unwrap(), Auth::Basic { user: "a".into(), pass: "b".into() });
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_token_is_an_error() {
        use std::os::unix::ffi::OsStrExt;
        let _g = env_lock();
        let f = creds_file("https://a:b@gitea.example\n");
        std::env::set_var("QUALGATE_GIT_CREDENTIALS", f.path());
        std::env::set_var("GITEA_TOKEN", std::ffi::OsStr::from_bytes(&[b't', 0xff]));
        let e = resolve_auth("https://gitea.example").unwrap_err().to_string();
        assert!(e.contains("GITEA_TOKEN"), "{e}");
    }

    #[test]
    fn clone_url_and_store_line_share_the_normalized_authority() {
        let _g = env_lock();
        for (base, origin, clone) in [
            ("HTTPS://Gitea.Example:443/", "https://gitea.example", "https://gitea.example/o/r.git"),
            ("https://u@gitea.example:8443/sub/", "https://gitea.example:8443", "https://gitea.example:8443/sub/o/r.git"),
            ("http://[::1]:80", "http://[::1]", "http://[::1]/o/r.git"),
        ] {
            assert_eq!(clone_url(base, "o/r").unwrap(), clone);
            let dir = tempfile::tempdir().unwrap();
            let env = git_env(&Auth::Basic { user: "u".into(), pass: "p".into() }, base, dir.path()).unwrap();
            let pairs = cfg_pairs(&env);
            let file = pairs.iter().rfind(|(k, _)| k == "credential.helper").unwrap().1
                .strip_prefix("store --file='").unwrap().strip_suffix('\'').unwrap().to_string();
            assert_eq!(std::fs::read_to_string(&file).unwrap(), format!("{}\n", origin.replacen("://", "://u:p@", 1)));
            assert!(clone.starts_with(&format!("{origin}/")));
        }
        for bad in ["../r", "o/..", "o//r", "o", "o/r/x", "o/./r"] {
            assert!(clone_url("https://gitea.example", bad).is_err(), "{bad}");
        }
        assert!(clone_url("ftp://gitea.example", "o/r").is_err());
    }
}
