use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

// opengrep은 컨테이너에서 동시 실행이 많으면 Bus error로 죽을 수 있어 테스트를 직렬화한다.
static SERIAL: Mutex<()> = Mutex::new(());

fn bin() -> Command { Command::new(env!("CARGO_BIN_EXE_qualgate")) }

fn git(d: &Path, args: &[&str], date: &str) {
    let st = Command::new("git").current_dir(d).args(args)
        .env("GIT_AUTHOR_DATE", date).env("GIT_COMMITTER_DATE", date).status().unwrap();
    assert!(st.success());
}

fn repo() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-qb", "main"], "2000-01-01T00:00:00");
    std::fs::write(d.path().join("a.py"), "x = 1\n").unwrap();
    git(d.path(), &["add", "-A"], "2000-01-01T00:00:00");
    git(d.path(), &["commit", "-qm", "base"], "2000-01-01T00:00:00");
    std::fs::write(d.path().join("a.py"), "x = 1\neval(input())\n").unwrap();
    git(d.path(), &["add", "-A"], "2099-01-01T00:00:00");
    git(d.path(), &["commit", "-qm", "new"], "2099-01-01T00:00:00");
    d
}

fn rules_dir() -> std::path::PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("rules") }

#[test]
fn scan_writes_result_and_gate_failure_exits_1() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let out = tempfile::tempdir().unwrap();
    let st = bin().args(["scan", "--src"]).arg(src.path()).arg("--out").arg(out.path())
        .env("QUALGATE_RULES", rules_dir()).status().unwrap();
    assert_eq!(st.code(), Some(1));
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out.path().join("result.json")).unwrap()).unwrap();
    assert_eq!(v["branch"], "main");
    assert_eq!(v["gate"]["passed"], false);
    assert!(v["findings"].as_array().unwrap().iter().any(|f| f["rule"] == "python.eval" && f["line"] == 2));
    assert_eq!(v["metrics"]["new_blocking"], 1);
    assert!(v["metrics"]["coverage"].is_null());
}

#[test]
fn bad_config_exits_2_and_removes_old_result() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let out = tempfile::tempdir().unwrap();
    std::fs::write(out.path().join("result.json"), "old").unwrap();
    std::fs::write(src.path().join(".qualgate.toml"), "[nope]\n").unwrap();
    let st = bin().args(["scan", "--src"]).arg(src.path()).arg("--out").arg(out.path())
        .env("QUALGATE_RULES", rules_dir()).status().unwrap();
    assert_eq!(st.code(), Some(2));
    assert!(!out.path().join("result.json").exists());
}

#[test]
fn opengrep_failure_exits_2() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let out = tempfile::tempdir().unwrap();
    let st = bin().args(["scan", "--src"]).arg(src.path()).arg("--out").arg(out.path())
        .env("QUALGATE_RULES", rules_dir()).env("QUALGATE_OPENGREP", "/nonexistent").status().unwrap();
    assert_eq!(st.code(), Some(2));
}

fn scanned_out() -> (tempfile::TempDir, tempfile::TempDir) {
    let src = repo();
    let out = tempfile::tempdir().unwrap();
    let st = bin().args(["scan", "--src"]).arg(src.path()).arg("--out").arg(out.path())
        .env("QUALGATE_RULES", rules_dir()).status().unwrap();
    assert_eq!(st.code(), Some(1));
    assert!(out.path().join("result.json").exists());
    (src, out)
}

#[test]
fn publish_without_token_exits_2() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (_src, out) = scanned_out();
    let o = bin().args(["publish", "--repo", "o/r", "--out"]).arg(out.path())
        .env_remove("GITEA_TOKEN").env("GITEA_URL", "http://127.0.0.1:9").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("GITEA_TOKEN"));
}

#[test]
fn publish_without_url_exits_2() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (_src, out) = scanned_out();
    let o = bin().args(["publish", "--repo", "o/r", "--out"]).arg(out.path())
        .env_remove("GITEA_URL").env("GITEA_TOKEN", "t").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("GITEA_URL"));
}

fn scan_cmd(src: &Path, out: &Path) -> Command {
    let mut c = bin();
    c.args(["scan", "--src"]).arg(src).arg("--out").arg(out).env("QUALGATE_RULES", rules_dir());
    c
}

fn clean_repo() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-qb", "main"], "2000-01-01T00:00:00");
    std::fs::write(d.path().join("a.py"), "x = 1\n").unwrap();
    git(d.path(), &["add", "-A"], "2000-01-01T00:00:00");
    git(d.path(), &["commit", "-qm", "base"], "2000-01-01T00:00:00");
    std::fs::write(d.path().join("a.py"), "x = 1\ny = 2\n").unwrap();
    git(d.path(), &["add", "-A"], "2099-01-01T00:00:00");
    git(d.path(), &["commit", "-qm", "new"], "2099-01-01T00:00:00");
    d
}

#[test]
fn passing_gate_exits_0() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = clean_repo();
    let out = tempfile::tempdir().unwrap();
    let o = scan_cmd(src.path(), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!String::from_utf8_lossy(&o.stdout).contains("경고"));
}

#[test]
fn all_skipped_gate_prints_warning() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-qb", "main"], "2099-01-01T00:00:00");
    std::fs::write(d.path().join("a.py"), "x = 1\n").unwrap();
    git(d.path(), &["add", "-A"], "2099-01-01T00:00:00");
    git(d.path(), &["commit", "-qm", "only"], "2099-01-01T00:00:00");
    let out = tempfile::tempdir().unwrap();
    let o = scan_cmd(d.path(), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout).contains("경고: 게이트 조건을 하나도 측정하지 못해"));
}

#[test]
fn required_coverage_matching_nothing_fails_gate() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = clean_repo();
    std::fs::write(src.path().join(".qualgate.toml"), "[coverage]\nrequired = true\n").unwrap();
    std::fs::write(src.path().join("coverage.out"), "mode: set\nother/x.go:1.1,2.2 1 1\n").unwrap();
    let out = tempfile::tempdir().unwrap();
    assert_eq!(scan_cmd(src.path(), out.path()).status().unwrap().code(), Some(1));
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out.path().join("result.json")).unwrap()).unwrap();
    assert_eq!(v["gate"]["conditions"][0]["status"], "failed");
}

#[test]
fn toml_error_does_not_print_config_text() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    std::fs::write(src.path().join(".qualgate.toml"), "GITEA_TOKEN=supersecret123 =\n").unwrap();
    let out = tempfile::tempdir().unwrap();
    let o = scan_cmd(src.path(), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let all = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(!all.contains("supersecret123"), "{all}");
    assert!(all.contains("줄 1"), "{all}");
}

#[cfg(unix)]
#[test]
fn symlinked_config_exits_2() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let secret = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(secret.path(), "GITEA_TOKEN=supersecret123\n").unwrap();
    std::os::unix::fs::symlink(secret.path(), src.path().join(".qualgate.toml")).unwrap();
    let out = tempfile::tempdir().unwrap();
    let o = scan_cmd(src.path(), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(!String::from_utf8_lossy(&o.stderr).contains("supersecret123"));
}

#[cfg(unix)]
#[test]
fn opengrep_child_does_not_receive_the_token() {
    use std::os::unix::fs::PermissionsExt;
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let tools = tempfile::tempdir().unwrap();
    let dump = tools.path().join("env.txt");
    let fake = tools.path().join("og.sh");
    std::fs::write(&fake, format!("#!/bin/sh\nenv > '{}'\nexit 3\n", dump.display())).unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = tempfile::tempdir().unwrap();
    let st = bin().args(["run", "--repo", "o/r", "--src"]).arg(src.path()).arg("--out").arg(out.path())
        .env("QUALGATE_RULES", rules_dir()).env("QUALGATE_OPENGREP", &fake)
        .env("GITEA_TOKEN", "supersecret123").env("GITEA_URL", "http://127.0.0.1:9").status().unwrap();
    assert_eq!(st.code(), Some(2));
    let env = std::fs::read_to_string(&dump).unwrap();
    assert!(env.contains("QUALGATE_RULES"));
    assert!(!env.contains("supersecret123"));
}

#[test]
fn publish_rejects_unknown_schema() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (_src, out) = scanned_out();
    let p = out.path().join("result.json");
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    v["schema"] = 2.into();
    std::fs::write(&p, v.to_string()).unwrap();
    let o = bin().args(["publish", "--repo", "o/r", "--out"]).arg(out.path())
        .env("GITEA_TOKEN", "t").env("GITEA_URL", "http://127.0.0.1:9").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("스키마"));
}

/// 기본 브랜치만 알려 주는 최소 HTTP 서버. 다른 요청이 오면 테스트가 실패한다.
fn stub_gitea(default_branch: &'static str) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let h = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut buf = vec![];
        let mut b = [0u8; 1024];
        while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = s.read(&mut b).unwrap();
            if n == 0 { break; }
            buf.extend_from_slice(&b[..n]);
        }
        let req = String::from_utf8_lossy(&buf).lines().next().unwrap_or("").to_string();
        let body = format!("{{\"default_branch\":\"{default_branch}\"}}");
        write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        vec![req]
    });
    (url, h)
}

#[test]
fn run_on_non_default_branch_skips_sync() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let out = tempfile::tempdir().unwrap();
    let (url, h) = stub_gitea("release");
    let o = bin().args(["run", "--repo", "o/r", "--src"]).arg(src.path()).arg("--out").arg(out.path())
        .env("QUALGATE_RULES", rules_dir()).env("GITEA_TOKEN", "t").env("GITEA_URL", &url).output().unwrap();
    // 게이트 실패(1)는 그대로 전달되고 이슈 API는 호출하지 않는다.
    assert_eq!(o.status.code(), Some(1), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("기본 브랜치 release가 아니어서"));
    assert_eq!(h.join().unwrap(), vec!["GET /api/v1/repos/o/r HTTP/1.1".to_string()]);
    assert!(out.path().join("result.json").exists());
}
