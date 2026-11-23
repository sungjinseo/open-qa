use serde_yaml::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn task() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("deploy/tekton/task-qualgate.yaml");
    serde_yaml::from_slice(&fs::read(path).expect("Tekton Task file must exist")).expect("Task must be valid YAML")
}

fn strings(value: &[Value]) -> BTreeSet<String> {
    value.iter()
        .map(|v| v["name"].as_str().expect("expected an env name").to_string()).collect()
}

#[test]
fn qualgate_task_matches_the_pipeline_contract() {
    let task = task();
    assert_eq!(task["apiVersion"].as_str(), Some("tekton.dev/v1"));
    assert_eq!(task["kind"].as_str(), Some("Task"));
    assert_eq!(task["metadata"]["namespace"].as_str(), Some("tekton-catalog"));

    let params: BTreeSet<String> = task["spec"]["params"].as_sequence().unwrap().iter()
        .map(|p| p["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(params, ["IMAGE", "CONTEXT_DIR", "GIT_HOST_URL", "REPO_ORG", "REPO_NAME", "REVISION", "COMMIT", "CONFIG_REPO", "CONFIG_REF", "CRT_FILENAME", "SSL_VERIFY", "FAIL_ON_GATE", "RULES_REPO", "RULES_REF", "REPORT_FORMATS", "REPORT_NAME", "REPORT_DIR"].into_iter().map(str::to_string).collect());

    let workspaces: BTreeSet<String> = task["spec"]["workspaces"].as_sequence().unwrap().iter()
        .map(|w| w["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(workspaces, ["source", "basic-auth", "ssl-ca-directory"].into_iter().map(str::to_string).collect());
    assert_eq!(task["spec"]["workspaces"].as_sequence().unwrap().iter().find(|w| w["name"] == "ssl-ca-directory").unwrap()["optional"].as_bool(), Some(true));
    assert_eq!(task["spec"]["results"][0]["name"].as_str(), Some("gate"));

    let steps = task["spec"]["steps"].as_sequence().unwrap();
    let names: Vec<_> = steps.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["fetch-config", "fetch-rules", "scan", "report", "publish"]);
    let scan_env: BTreeSet<String> = steps[2]["env"].as_sequence().map(|v| strings(v)).unwrap_or_default();
    for forbidden in ["GITEA_URL", "QUALGATE_GIT_CREDENTIALS", "QUALGATE_CA_FILE"] {
        assert!(!scan_env.contains(forbidden), "scan must not receive {forbidden}");
    }
    let rules_env = steps[2]["env"].as_sequence().unwrap().iter().find(|e| e["name"] == "QUALGATE_RULES")
        .expect("scan must read the rules published by fetch-rules");
    assert_eq!(rules_env["value"].as_str(), Some("/work/rules"));
    let params = task["spec"]["params"].as_sequence().unwrap();
    let default = |n: &str| params.iter().find(|p| p["name"] == n).unwrap()["default"].as_str().map(str::to_string);
    assert_eq!(default("RULES_REPO").as_deref(), Some("platform/qualgate-rules"));
    assert_eq!(default("RULES_REF").as_deref(), Some("main"));
    // fetch-rules: TLS settings and credentials are explicit, the workspace .gitconfig is ignored,
    // the credential file is only looked up, and an empty RULES_REPO falls back to the bundled rules.
    let rules_script = steps[1]["script"].as_str().expect("fetch-rules must be a script step");
    for needle in ["GIT_CONFIG_GLOBAL=/dev/null", "http.sslVerify=\"${SSL_VERIFY}\"", "http.sslCAInfo=",
                   "credential.helper= ", "credential-store --file=${CRED} get", "ln -s /work/rules-src/rules /work/rules", "ln -s /rules /work/rules"] {
        assert!(rules_script.contains(needle), "fetch-rules script must contain {needle:?}");
    }
    assert!(!rules_script.contains("cat "), "fetch-rules must not print files");
    // git-clone in the shared catalog publishes `git rev-parse --short HEAD`; fetch-config needs the full sha.
    let fetch_script = steps[0]["script"].as_str().expect("fetch-config must be a script step");
    assert!(fetch_script.contains("rev-parse --verify"), "fetch-config must expand an abbreviated COMMIT from the source checkout");
    assert_eq!(default("REPORT_FORMATS").as_deref(), Some("html,md,xlsx"));
    assert_eq!(default("REPORT_DIR").as_deref(), Some("qualgate-report"));
    // report: no Gitea access, reads the fetched rules, and never fails the task before publish.
    let report_env: BTreeSet<String> = steps[3]["env"].as_sequence().map(|v| strings(v)).unwrap_or_default();
    for forbidden in ["GITEA_URL", "QUALGATE_GIT_CREDENTIALS", "QUALGATE_CA_FILE"] {
        assert!(!report_env.contains(forbidden), "report must not receive {forbidden}");
    }
    assert!(report_env.contains("QUALGATE_RULES"), "report must read the rules published by fetch-rules");
    let report_script = steps[3]["script"].as_str().expect("report must be a script step");
    for needle in ["qualgate report", "--format \"${REPORT_FORMATS}\"", "--report-dir \"${REPORT_DIR}\"", "${REPORT_NAME:-${TASKRUN_NAME}}", "|| echo"] {
        assert!(report_script.contains(needle), "report script must contain {needle:?}");
    }
    assert!(!report_script.contains("set -e"), "a report failure must not stop publish");
    for step in [&steps[0], &steps[4]] {
        let env: BTreeSet<String> = step["env"].as_sequence().map(|v| strings(v)).unwrap_or_default();
        for required in ["GITEA_URL", "QUALGATE_GIT_CREDENTIALS", "QUALGATE_CA_FILE", "QUALGATE_SSL_VERIFY"] {
            assert!(env.contains(required), "{} must receive {required}", step["name"].as_str().unwrap());
        }
    }
}
