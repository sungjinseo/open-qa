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

#[test]
fn publish_without_token_exits_2() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let out = tempfile::tempdir().unwrap();
    let st = bin().args(["publish", "--repo", "o/r", "--out"]).arg(out.path())
        .env_remove("GITEA_TOKEN").env("GITEA_URL", "http://127.0.0.1:9").status().unwrap();
    assert_eq!(st.code(), Some(2));
}
