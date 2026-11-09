use std::path::{Path, PathBuf};
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

fn git_out(d: &Path, args: &[&str]) -> String {
    let o = Command::new("git").current_dir(d).args(args).output().unwrap();
    assert!(o.status.success());
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

/// 2099년 커밋이 2~4번 줄을 추가한다: eval(룰셋에서 끔), print(오버라이드로 핫스팟), exec(이슈).
/// A.java는 룰셋에서 켜졌지만 프로젝트 `languages`에 없어 실행하지 않는 디렉터리의 파일이다.
fn repo() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-qb", "main"], "2000-01-01T00:00:00");
    std::fs::write(d.path().join("a.py"), "x = 1\n").unwrap();
    std::fs::write(d.path().join("A.java"), "class A { void f() { System.out.println(1); } }\n").unwrap();
    git(d.path(), &["add", "-A"], "2000-01-01T00:00:00");
    git(d.path(), &["commit", "-qm", "base"], "2000-01-01T00:00:00");
    std::fs::write(d.path().join("a.py"), "x = 1\neval(input())\nprint(x)\nexec(input())\n").unwrap();
    git(d.path(), &["add", "-A"], "2099-01-01T00:00:00");
    git(d.path(), &["commit", "-qm", "new"], "2099-01-01T00:00:00");
    d
}

fn rules_dir() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("rules") }

fn fixture_cfg(kind: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/config").join(kind)
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() { copy_dir(&e.path(), &t) } else { std::fs::copy(e.path(), t).unwrap(); }
    }
}

/// 픽스처 설정 디렉터리를 고칠 수 있게 복사한다.
fn config_copy() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    copy_dir(&fixture_cfg("scan"), t.path());
    t
}

fn scan_cmd(src: &Path, config: &Path, out: &Path) -> Command {
    let mut c = bin();
    c.args(["scan", "--src"]).arg(src).arg("--config").arg(config).arg("--out").arg(out).env("QUALGATE_RULES", rules_dir());
    c
}

fn read_result(out: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(out.join("result.json")).unwrap()).unwrap()
}

fn rules_of(v: &serde_json::Value, key: &str) -> Vec<String> {
    v[key].as_array().unwrap().iter().map(|f| f["rule"].as_str().unwrap().to_string()).collect()
}

#[test]
fn scan_uses_config_dir_ruleset_and_overrides() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let out = tempfile::tempdir().unwrap();
    let o = scan_cmd(src.path(), &fixture_cfg("scan"), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let v = read_result(out.path());
    assert_eq!(v["schema"], 2);
    assert_eq!(v["branch"], "main");
    assert!(v.get("gate").is_none());
    assert_eq!(v["config_source"]["repo"], "o/r");
    assert_eq!(v["config_source"]["config_commit"], "c0ffee");
    assert_eq!(v["gate_conditions"].as_array().unwrap().len(), 2);
    assert_eq!(v["gate_conditions"][0]["metric"], "new_issues");
    // 실행한 규칙: 프로젝트 languages(python)의 규칙 중 룰셋에서 끈 python.eval을 뺀 것.
    assert_eq!(v["active_dirs"], serde_json::json!(["python"]));
    let active: Vec<&str> = v["active_rules"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
    assert!(active.contains(&"python.exec") && active.contains(&"python.print-call"));
    assert!(!active.contains(&"python.eval"));
    assert!(active.iter().all(|r| r.starts_with("python.")), "{active:?}");
    // 룰셋에서 끈 규칙은 위반을 내지 않는다.
    let rules = rules_of(&v, "findings");
    assert!(!rules.contains(&"python.eval".to_string()), "{rules:?}");
    let f = |rule: &str| v["findings"].as_array().unwrap().iter().find(|f| f["rule"] == rule).unwrap().clone();
    assert_eq!((f("python.exec")["line"].clone(), f("python.exec")["kind"].clone()), (4.into(), "issue".into()));
    assert_eq!(f("python.print-call")["kind"], "hotspot");
    assert!(v["findings"].as_array().unwrap().iter().all(|f| f["kind"].is_string()));
    // 오버라이드로 핫스팟이 된 규칙의 신규 줄 위반은 new_hotspots에 들어간다.
    assert_eq!(v["new_code_measured"], true);
    assert_eq!(rules_of(&v, "new_hotspots"), vec!["python.print-call".to_string()]);
    // 실행하지 않는 디렉터리(java)의 파일은 분석 실패가 아니다.
    assert_eq!(v["unanalyzed"], serde_json::json!([]));
    assert!(v["metrics"]["coverage"].is_null());
    assert!(v["metrics"].get("new_blocking").is_none());
    // 이슈는 exec 한 건(신규 줄), 핫스팟은 print 한 건(신규 줄).
    assert_eq!((v["metrics"]["new_issues"].clone(), v["metrics"]["issues"].clone(), v["metrics"]["new_hotspots"].clone()), (1.into(), 1.into(), 1.into()));
}

#[test]
fn repo_config_file_is_ignored_with_warning() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    // 예전에는 형식 오류로 종료 코드 2였다. 이제 읽지 않으므로 내용도 출력되지 않는다.
    std::fs::write(src.path().join(".qualgate.toml"), "GITEA_TOKEN=supersecret123 =\n[gate]\nnew_coverage_min = 0\n").unwrap();
    let out = tempfile::tempdir().unwrap();
    let o = scan_cmd(src.path(), &fixture_cfg("scan"), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("경고") && err.contains(".qualgate.toml"), "{err}");
    let all = format!("{}{err}", String::from_utf8_lossy(&o.stdout));
    assert!(!all.contains("supersecret123"), "{all}");
    assert_eq!(read_result(out.path())["schema"], 2);
}

#[test]
fn required_coverage_without_a_report_is_flagged_in_the_result() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let run = |cfg: &Path| {
        let out = tempfile::tempdir().unwrap();
        let o = scan_cmd(src.path(), cfg, out.path()).output().unwrap();
        assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
        read_result(out.path())
    };
    assert_eq!(run(&fixture_cfg("scan"))["coverage_required_missing"], false);
    let cfg = config_copy();
    let p = cfg.path().join("projects/o/r.toml");
    let mut text = std::fs::read_to_string(&p).unwrap();
    text.push_str("\n[coverage]\nrequired = true\n");
    std::fs::write(&p, text).unwrap();
    let v = run(cfg.path());
    assert!(v["metrics"]["coverage"].is_null());
    assert_eq!(v["coverage_required_missing"], true);
}

#[test]
fn missing_source_json_exits_2_and_removes_old_result() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let cfg = config_copy();
    std::fs::remove_file(cfg.path().join("source.json")).unwrap();
    let out = tempfile::tempdir().unwrap();
    std::fs::write(out.path().join("result.json"), "old").unwrap();
    let o = scan_cmd(src.path(), cfg.path(), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("source.json"));
    assert!(!out.path().join("result.json").exists());
}

#[test]
fn invalid_config_dir_exits_2() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let cfg = config_copy();
    std::fs::write(cfg.path().join("rulesets/std.toml"), "schema = 1\nname = \"std\"\nbogus = 1\n").unwrap();
    let out = tempfile::tempdir().unwrap();
    let o = scan_cmd(src.path(), cfg.path(), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("rulesets/std.toml"), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!out.path().join("result.json").exists());
}

/// 얕은 클론은 git으로 신규 코드를 계산하지 못한다. fetch-config가 쓴 newcode.json은 그 head가
/// 스캔하는 커밋과 같을 때만 쓴다.
#[test]
fn shallow_clone_uses_newcode_json_only_for_the_same_head() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let full = repo();
    let t = tempfile::tempdir().unwrap();
    let shallow = t.path().join("s");
    let url = format!("file://{}", full.path().display());
    let st = Command::new("git").args(["clone", "-q", "--depth", "1", &url]).arg(&shallow).status().unwrap();
    assert!(st.success());
    assert_eq!(git_out(&shallow, &["rev-parse", "--is-shallow-repository"]), "true");
    let head = git_out(&shallow, &["rev-parse", "HEAD"]);
    let cfg = config_copy();
    let run = |head: &str| {
        std::fs::write(cfg.path().join("newcode.json"), format!(r#"{{"base":"b","head":"{head}","files":{{"a.py":[3]}}}}"#)).unwrap();
        let out = tempfile::tempdir().unwrap();
        let o = scan_cmd(&shallow, cfg.path(), out.path()).output().unwrap();
        assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
        read_result(out.path())
    };
    let v = run(&head);
    assert_eq!(v["new_code_measured"], true);
    assert_eq!(rules_of(&v, "new_hotspots"), vec!["python.print-call".to_string()]);
    let v = run("0000000000000000000000000000000000000000");
    assert_eq!(v["new_code_measured"], false);
    assert_eq!(v["new_hotspots"], serde_json::json!([]));
    assert!(v["metrics"]["new_duplicated_lines"].is_null());
}

#[test]
fn opengrep_failure_exits_2() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let out = tempfile::tempdir().unwrap();
    let st = scan_cmd(src.path(), &fixture_cfg("scan"), out.path()).env("QUALGATE_OPENGREP", "/nonexistent").status().unwrap();
    assert_eq!(st.code(), Some(2));
}

fn scanned_out() -> (tempfile::TempDir, tempfile::TempDir) {
    let src = repo();
    let out = tempfile::tempdir().unwrap();
    let st = scan_cmd(src.path(), &fixture_cfg("scan"), out.path()).status().unwrap();
    assert_eq!(st.code(), Some(0));
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

/// scan 단계에는 Gitea 자격 증명을 주지 않지만, 주어지더라도 Opengrep에 넘기지 않는다.
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
    let st = scan_cmd(src.path(), &fixture_cfg("scan"), out.path()).env("QUALGATE_OPENGREP", &fake)
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
    v["schema"] = 1.into();
    std::fs::write(&p, v.to_string()).unwrap();
    let o = bin().args(["publish", "--repo", "o/r", "--out"]).arg(out.path())
        .env("GITEA_TOKEN", "t").env("GITEA_URL", "http://127.0.0.1:9").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("스키마"));
}

#[test]
fn run_command_is_removed() {
    let o = bin().args(["run", "--repo", "o/r"]).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
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
fn publish_on_non_default_branch_skips_sync() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (_src, out) = scanned_out();
    let (url, h) = stub_gitea("release");
    let o = bin().args(["publish", "--repo", "o/r", "--out"]).arg(out.path())
        .env("GITEA_TOKEN", "t").env("GITEA_URL", &url).output().unwrap();
    // Gate evaluation applies even when issue sync is skipped for a non-default branch.
    assert_eq!(o.status.code(), Some(1), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stdout).contains("issue sync skipped"));
    assert_eq!(h.join().unwrap(), vec!["GET /api/v1/repos/o/r HTTP/1.1".to_string()]);
}

#[test]
fn publish_writes_gate_result_and_fail_on_gate_false_returns_zero() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let (_src, out) = scanned_out();
    let result_path = out.path().join("result.json");
    let mut value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&result_path).unwrap()).unwrap();
    value["branch"] = "feature".into();
    value["metrics"]["new_issues"] = 1.into();
    value["gate_conditions"] = serde_json::json!([{"metric":"new_issues","op":"gt","value":0.0}]);
    std::fs::write(&result_path, value.to_string()).unwrap();
    let gate_path = out.path().join("gate-result.json");
    let (url, h) = stub_gitea("main");
    let output = bin().args(["publish", "--repo", "o/r", "--out"]).arg(out.path())
        .args(["--gate-result"]).arg(&gate_path).args(["--fail-on-gate", "false"])
        .env("GITEA_TOKEN", "t").env("GITEA_URL", &url).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(std::fs::read_to_string(gate_path).unwrap(), "failed");
    assert_eq!(h.join().unwrap(), vec!["GET /api/v1/repos/o/r HTTP/1.1".to_string()]);
}

fn rule_count() -> usize {
    ["java", "kotlin", "python", "react", "vue", "go", "ts"].iter()
        .map(|d| std::fs::read_dir(rules_dir().join(d)).unwrap().flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "yaml")).count())
        .sum()
}

#[test]
fn init_config_output_passes_validate_config() {
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("cfg");
    let st = bin().args(["init-config", "--out"]).arg(&out).env("QUALGATE_RULES", rules_dir()).status().unwrap();
    assert_eq!(st.code(), Some(0));
    assert!(out.join("README.md").is_file());
    let st = bin().args(["validate-config", "--config-dir"]).arg(&out).env("QUALGATE_RULES", rules_dir()).status().unwrap();
    assert_eq!(st.code(), Some(0));
}

#[test]
fn init_config_enables_all_dirs_and_default_gate() {
    let t = tempfile::tempdir().unwrap();
    let st = bin().args(["init-config", "--out"]).arg(t.path()).env("QUALGATE_RULES", rules_dir()).status().unwrap();
    assert_eq!(st.code(), Some(0)); // 비어 있는 기존 디렉터리는 허용
    let g: toml::Value = toml::from_str(&std::fs::read_to_string(t.path().join("qualgate.toml")).unwrap()).unwrap();
    let name = g["default_ruleset"].as_str().unwrap();
    assert!(!name.is_empty());
    let metrics: Vec<(&str, &str, f64)> = g["default_gate"].as_array().unwrap().iter()
        .map(|c| (c["metric"].as_str().unwrap(), c["op"].as_str().unwrap(),
                  c["value"].as_float().or(c["value"].as_integer().map(|i| i as f64)).unwrap()))
        .collect();
    assert_eq!(metrics, [("new_issues", "gt", 0.0), ("new_hotspots_reviewed", "lt", 100.0), ("new_coverage", "lt", 80.0), ("new_duplicated_lines", "gt", 3.0)]);
    let r: toml::Value = toml::from_str(&std::fs::read_to_string(t.path().join(format!("rulesets/{name}.toml"))).unwrap()).unwrap();
    assert_eq!(r["name"].as_str(), Some(name));
    let langs = r["languages"].as_table().unwrap();
    assert_eq!(langs.len(), 7);
    for d in ["java", "kotlin", "python", "react", "vue", "go", "ts"] {
        assert_eq!(langs[d]["enabled"].as_bool(), Some(true), "{d}");
    }
}

#[test]
fn init_config_refuses_non_empty_dir() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("keep.txt"), "x").unwrap();
    let o = bin().args(["init-config", "--out"]).arg(t.path()).env("QUALGATE_RULES", rules_dir()).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(!t.path().join("qualgate.toml").exists());
    assert_eq!(std::fs::read_to_string(t.path().join("keep.txt")).unwrap(), "x");
}

#[test]
fn validate_config_reports_every_error_with_file_path() {
    let o = bin().args(["validate-config", "--config-dir"]).arg(fixture_cfg("bad/multi")).env("QUALGATE_RULES", rules_dir()).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let e = String::from_utf8_lossy(&o.stderr);
    assert!(e.contains("rulesets/backend-standard.toml"), "{e}");
    assert!(e.contains("projects/team-a/order-service.toml"), "{e}");
    assert!(e.lines().filter(|l| l.contains(".toml")).count() >= 3, "{e}");
}

#[test]
fn validate_config_accepts_good_fixture() {
    let st = bin().args(["validate-config", "--config-dir"]).arg(fixture_cfg("good")).env("QUALGATE_RULES", rules_dir()).status().unwrap();
    assert_eq!(st.code(), Some(0));
}

#[test]
fn catalog_json_lists_all_rules() {
    let o = bin().args(["catalog", "--json"]).env("QUALGATE_RULES", rules_dir()).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let a = v.as_array().unwrap();
    assert_eq!(a.len(), rule_count());
    for it in a {
        for k in ["id", "dir", "kind"] { assert!(it[k].is_string(), "{it}"); }
    }
}

#[test]
fn wrong_rules_path_exits_2_for_catalog_and_validate() {
    let t = tempfile::tempdir().unwrap();
    let o = bin().args(["catalog", "--json"]).env("QUALGATE_RULES", t.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("QUALGATE_RULES"));
    let st = bin().args(["validate-config", "--config-dir"]).arg(fixture_cfg("good")).env("QUALGATE_RULES", t.path()).status().unwrap();
    assert_eq!(st.code(), Some(2));
}

/// 활성 디렉터리의 파일을 Opengrep이 분석하지 못하면(문법 오류) `unanalyzed`에 기록한다.
/// 룰셋에서 꺼진 디렉터리의 파일(A.java)은 기록하지 않는다.
#[test]
fn file_of_an_active_dir_that_opengrep_cannot_parse_is_unanalyzed() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    std::fs::write(src.path().join("broken.py"), "def f(:\n    ))) = = =\n").unwrap();
    let out = tempfile::tempdir().unwrap();
    let o = scan_cmd(src.path(), &fixture_cfg("scan"), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(read_result(out.path())["unanalyzed"], serde_json::json!(["broken.py"]));
}

#[test]
fn scan_without_active_rules_warns_and_reports_empty_findings() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let src = repo();
    let cfg = config_copy();
    std::fs::write(cfg.path().join("rulesets/std.toml"), "schema = 1\nname = \"std\"\n[languages.python]\nenabled = false\n").unwrap();
    let out = tempfile::tempdir().unwrap();
    let o = scan_cmd(src.path(), cfg.path(), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stderr).contains("실행할 규칙이 없어"), "{}", String::from_utf8_lossy(&o.stderr));
    let v = read_result(out.path());
    assert_eq!(v["findings"], serde_json::json!([]));
    assert_eq!(v["active_rules"], serde_json::json!([]));
    assert_eq!(v["active_dirs"], serde_json::json!([]));
    assert_eq!(v["unanalyzed"], serde_json::json!([]));
    // 신규 코드를 측정했으므로 이슈는 0건으로 측정된다.
    assert_eq!((v["metrics"]["new_issues"].clone(), v["metrics"]["issues"].clone()), (0.into(), 0.into()));
}

#[test]
fn malformed_newcode_json_exits_2() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let full = repo();
    let t = tempfile::tempdir().unwrap();
    let shallow = t.path().join("s");
    let url = format!("file://{}", full.path().display());
    assert!(Command::new("git").args(["clone", "-q", "--depth", "1", &url]).arg(&shallow).status().unwrap().success());
    let cfg = config_copy();
    std::fs::write(cfg.path().join("newcode.json"), "{ not json").unwrap();
    let out = tempfile::tempdir().unwrap();
    let o = scan_cmd(&shallow, cfg.path(), out.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("newcode.json"), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!out.path().join("result.json").exists());
}

const CFG_SHA: &str = "c0ffee00c0ffee00c0ffee00c0ffee00c0ffee00";

/// 설정 저장소 qg/cfg(커밋 CFG_SHA)의 `files`만 아는 가짜 Gitea. 그 밖의 요청(git 클론 포함)은 404.
/// 받은 요청마다 "요청 줄 | Authorization 헤더"를 기록한다.
fn fake_config_gitea(files: Vec<(&'static str, String)>) -> (String, std::sync::Arc<Mutex<Vec<String>>>) {
    use base64::Engine;
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let log = std::sync::Arc::new(Mutex::new(vec![]));
    let seen = log.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { return };
            let mut buf = vec![];
            let mut b = [0u8; 4096];
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut b) { Ok(0) | Err(_) => break, Ok(n) => buf.extend_from_slice(&b[..n]) }
            }
            let text = String::from_utf8_lossy(&buf).to_string();
            let line = text.lines().next().unwrap_or("").to_string();
            let auth = text.lines().find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("authorization")).map(|(_, v)| v.trim().to_string())).unwrap_or_default();
            seen.lock().unwrap().push(format!("{line} | {auth}"));
            let path = line.split_whitespace().nth(1).unwrap_or("");
            let contents = "/api/v1/repos/qg/cfg/contents/";
            let body = if path == "/api/v1/repos/qg/cfg/commits?sha=main&limit=1" {
                Some(format!(r#"[{{"sha":"{CFG_SHA}"}}]"#))
            } else if let Some(rest) = path.strip_prefix(contents).and_then(|r| r.strip_suffix(&format!("?ref={CFG_SHA}"))) {
                files.iter().find(|(p, _)| *p == rest).map(|(_, c)| {
                    let enc = base64::engine::general_purpose::STANDARD.encode(c);
                    format!(r#"{{"type":"file","encoding":"base64","content":"{enc}","sha":"blob"}}"#)
                })
            } else { None };
            let (status, body) = match body { Some(b) => ("200 OK", b), None => ("404 Not Found", r#"{"message":"not found"}"#.to_string()) };
            let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        }
    });
    (url, log)
}

fn scan_fixture_files() -> Vec<(&'static str, String)> {
    ["qualgate.toml", "rulesets/std.toml", "projects/o/r.toml"].into_iter()
        .map(|p| (p, std::fs::read_to_string(fixture_cfg("scan").join(p)).unwrap())).collect()
}

fn fetch_cmd(url: &str, token: &str, out: &Path) -> Command {
    let mut c = bin();
    c.args(["fetch-config", "--repo", "o/r", "--branch", "main", "--config-repo", "qg/cfg", "--out"]).arg(out)
        .env("GITEA_URL", url).env("GITEA_TOKEN", token).env("QUALGATE_RULES", rules_dir())
        .env_remove("QUALGATE_GIT_CREDENTIALS").env_remove("QUALGATE_CA_FILE").env_remove("QUALGATE_SSL_VERIFY");
    c
}

#[test]
fn fetch_config_writes_config_from_gitea_and_warns_when_history_is_unavailable() {
    let (url, log) = fake_config_gitea(scan_fixture_files());
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("config");
    let token = "tok3n-not-printed";
    let o = fetch_cmd(&url, token, &out).args(["--commit", "0123456789ABCDEF0123456789abcdef01234567"]).output().unwrap();
    let (stdout, stderr) = (String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert_eq!(o.status.code(), Some(0), "{stderr}");
    assert!(!stdout.contains(token) && !stderr.contains(token), "{stdout}{stderr}");
    // 대문자 sha도 받아 소문자로 쓴다. 클론이 실패하면 이유와 함께 경고한다.
    assert!(stderr.contains("경고") && stderr.contains("/o/r.git") && stderr.contains("측정하지 않습니다"), "{stderr}");
    for f in ["qualgate.toml", "rulesets/std.toml", "projects/o/r.toml", "source.json"] { assert!(out.join(f).is_file(), "{f}"); }
    assert!(!out.join("newcode.json").exists());
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out.join("source.json")).unwrap()).unwrap();
    assert_eq!(v, serde_json::json!({"config_repo": "qg/cfg", "ref": "main", "config_commit": CFG_SHA, "repo": "o/r", "branch": "main", "registered": true}));
    let log = log.lock().unwrap().clone();
    // 모든 API 호출은 토큰으로 인증하고 같은 설정 커밋에서 파일을 읽는다.
    let api: Vec<_> = log.iter().filter(|l| l.contains("/api/v1/")).collect();
    assert!(api.iter().all(|l| l.ends_with(&format!("| token {token}"))), "{log:?}");
    assert!(api.iter().filter(|l| l.contains("/contents/")).all(|l| l.contains(&format!("?ref={CFG_SHA} "))), "{log:?}");
    // git 클론은 GITEA_URL 아래 대상 저장소로 가고, 토큰은 인자가 아닌 헤더로 전달된다.
    assert!(log.iter().any(|l| l.starts_with("GET /o/r.git/info/refs") && l.ends_with(&format!("| token {token}"))), "{log:?}");
}

#[test]
fn fetch_config_without_global_config_exits_2() {
    let (url, _log) = fake_config_gitea(vec![]);
    let t = tempfile::tempdir().unwrap();
    let out = t.path().join("config");
    let o = fetch_cmd(&url, "t", &out).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let e = String::from_utf8_lossy(&o.stderr);
    assert!(e.contains("qualgate.toml") && e.contains("qg/cfg"), "{e}");
    assert!(!out.exists());
}

#[test]
fn fetch_config_refuses_non_empty_out() {
    let (url, _log) = fake_config_gitea(scan_fixture_files());
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("keep.txt"), "x").unwrap();
    let o = fetch_cmd(&url, "t", t.path()).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("비어 있지"));
    assert!(!t.path().join("qualgate.toml").exists());
}

/// 이력 준비(credential 파일 위치 등)에 실패해도 설정은 받고 경고만 한다. credential 임시 파일은 남지 않는다.
#[test]
fn fetch_config_history_setup_failure_only_warns_and_cleans_credentials() {
    let (url, _log) = fake_config_gitea(scan_fixture_files());
    let t = tempfile::tempdir().unwrap();
    let creds = t.path().join("creds");
    std::fs::write(&creds, format!("{}\n", url.replacen("://", "://bot:pa55word@", 1))).unwrap();
    let commit = ["--commit", "0123456789abcdef0123456789abcdef01234567"];
    let basic = |out: &Path, history: &Path| {
        let mut c = fetch_cmd(&url, "", out);
        c.env_remove("GITEA_TOKEN").env("QUALGATE_GIT_CREDENTIALS", &creds).args(commit).arg("--history-dir").arg(history);
        c.output().unwrap()
    };
    // 이력 디렉터리의 상위가 파일이라 만들 수 없다.
    std::fs::write(t.path().join("file"), "x").unwrap();
    let o = basic(&t.path().join("a"), &t.path().join("file/history"));
    let e = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(0), "{e}");
    assert!(e.contains("이력을 받을 준비") && !e.contains("pa55word"), "{e}");
    assert!(t.path().join("a/source.json").exists() && !t.path().join("a/newcode.json").exists());
    // 정상 준비: credential 파일은 이력 디렉터리의 상위(work)에 만들어졌다가 종료 전에 지워진다.
    let work = t.path().join("work");
    let o = basic(&t.path().join("b"), &work.join("history"));
    let e = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(0), "{e}");
    assert!(!e.contains("pa55word"), "{e}");
    assert!(work.is_dir());
    let left: Vec<_> = std::fs::read_dir(&work).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert!(left.iter().all(|n| !n.starts_with(".qualgate-cred-")), "{left:?}");
}
