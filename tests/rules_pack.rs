use std::path::{Path, PathBuf};
use std::process::Command;

fn yamls(root: &Path) -> Vec<PathBuf> {
    let mut v = vec![];
    for lang in ["java", "kotlin", "python", "react", "vue", "go"] {
        let dir = root.join(lang);
        let n_before = v.len();
        for e in std::fs::read_dir(&dir).unwrap_or_else(|_| panic!("{} 없음", dir.display())) {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "yaml") { v.push(p); }
        }
        assert!(v.len() - n_before >= 20, "{lang} 규칙이 20개 미만");
    }
    v
}

fn scan(rule: &Path, file: &Path) -> serde_json::Value {
    let out = tempfile::NamedTempFile::new().unwrap();
    let dir = file.parent().unwrap();
    let st = Command::new("opengrep")
        .current_dir(dir)
        .args(["scan", "--no-git-ignore"])
        .arg(format!("--json-output={}", out.path().display()))
        .arg("-f").arg(rule)
        .arg(file.file_name().unwrap())
        .output().unwrap();
    assert!(st.status.success(), "{}: {}", rule.display(), String::from_utf8_lossy(&st.stderr));
    serde_json::from_str(&std::fs::read_to_string(out.path()).unwrap()).unwrap()
}

#[test]
fn every_rule_detects_bad_and_accepts_good() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules");
    let mut failures = vec![];
    for rule in yamls(&root) {
        let lang = rule.parent().unwrap().file_name().unwrap().to_str().unwrap().to_string();
        let name = rule.file_stem().unwrap().to_str().unwrap().to_string();
        let ext = match lang.as_str() {
            "java" => "java", "kotlin" => "kt", "python" => "py",
            "react" => "tsx", "vue" => "vue.ts", _ => "go",
        };
        let t = root.join("tests").join(&lang);
        let bad = t.join(format!("{name}.bad.{ext}"));
        let good = t.join(format!("{name}.good.{ext}"));
        let yaml = std::fs::read_to_string(&rule).unwrap();
        for k in ["qualgate_id", "qualgate_severity", "qualgate_category"] {
            if !yaml.contains(k) { failures.push(format!("{name}: {k} 없음")); }
        }
        if !bad.exists() || !good.exists() { failures.push(format!("{name}: 예제 없음")); continue; }
        let b = scan(&rule, &bad);
        if b["results"].as_array().map_or(0, |a| a.len()) == 0 { failures.push(format!("{name}: bad 미탐지")); }
        let g = scan(&rule, &good);
        if g["results"].as_array().map_or(0, |a| a.len()) != 0 { failures.push(format!("{name}: good 오탐")); }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
