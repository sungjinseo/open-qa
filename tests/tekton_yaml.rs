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
    assert_eq!(params, ["IMAGE", "CONTEXT_DIR", "GIT_HOST_URL", "REPO_ORG", "REPO_NAME", "REVISION", "COMMIT", "CONFIG_REPO", "CONFIG_REF", "CRT_FILENAME", "SSL_VERIFY", "FAIL_ON_GATE"].into_iter().map(str::to_string).collect());

    let workspaces: BTreeSet<String> = task["spec"]["workspaces"].as_sequence().unwrap().iter()
        .map(|w| w["name"].as_str().unwrap().to_string()).collect();
    assert_eq!(workspaces, ["source", "basic-auth", "ssl-ca-directory"].into_iter().map(str::to_string).collect());
    assert_eq!(task["spec"]["workspaces"].as_sequence().unwrap().iter().find(|w| w["name"] == "ssl-ca-directory").unwrap()["optional"].as_bool(), Some(true));
    assert_eq!(task["spec"]["results"][0]["name"].as_str(), Some("gate"));

    let steps = task["spec"]["steps"].as_sequence().unwrap();
    let names: Vec<_> = steps.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["fetch-config", "scan", "publish"]);
    let scan_env: BTreeSet<String> = steps[1]["env"].as_sequence().map(|v| strings(v)).unwrap_or_default();
    for forbidden in ["GITEA_URL", "QUALGATE_GIT_CREDENTIALS", "QUALGATE_CA_FILE"] {
        assert!(!scan_env.contains(forbidden), "scan must not receive {forbidden}");
    }
    for step in [&steps[0], &steps[2]] {
        let env: BTreeSet<String> = step["env"].as_sequence().map(|v| strings(v)).unwrap_or_default();
        for required in ["GITEA_URL", "QUALGATE_GIT_CREDENTIALS", "QUALGATE_CA_FILE", "QUALGATE_SSL_VERIFY"] {
            assert!(env.contains(required), "{} must receive {required}", step["name"].as_str().unwrap());
        }
    }
}
