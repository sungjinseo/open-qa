use super::{ConfigError, GateCondition, Global, Metric, Project, Resolved, Ruleset, DEFAULT_GATE};
use crate::catalog::Catalog;
use crate::config::{position, validate_dup, validate_publish};
use serde::de::DeserializeOwned;
use std::collections::BTreeSet;
use std::path::Path;

/// 규칙 디렉터리 이름.
const LANG_DIRS: [&str; 7] = ["java", "kotlin", "python", "go", "ts", "react", "vue"];

/// 건수 지표 값의 상한.
const MAX_COUNT: f64 = 1_000_000.0;

type Errs = Vec<ConfigError>;

fn err(errs: &mut Errs, file: &str, message: impl Into<String>) {
    errs.push(ConfigError { file: file.to_string(), message: message.into() });
}

/// 설정 파일 하나를 읽는다. 저장소는 신뢰하지 않으므로 심볼릭 링크는 따라가지 않고,
/// 파서 오류는 원문 줄을 포함하므로 위치만 알린다.
fn read<T: DeserializeOwned>(dir: &Path, rel: &str, errs: &mut Errs) -> Option<T> {
    let p = dir.join(rel);
    // 상위 디렉터리(rulesets, projects, <owner>)가 링크면 파일 검사만으로는 막을 수 없다.
    let mut prefix = String::new();
    for part in rel.split('/').rev().skip(1).collect::<Vec<_>>().into_iter().rev() {
        if !prefix.is_empty() { prefix.push('/'); }
        prefix.push_str(part);
        if std::fs::symlink_metadata(dir.join(&prefix)).is_ok_and(|m| m.file_type().is_symlink()) {
            err(errs, &prefix, "일반 디렉터리여야 합니다 (심볼릭 링크 불가)");
            return None;
        }
    }
    let meta = match std::fs::symlink_metadata(&p) {
        Ok(m) => m,
        Err(e) => {
            err(errs, rel, format!("읽기 실패 ({})", e.kind()));
            return None;
        }
    };
    if !meta.is_file() {
        err(errs, rel, "일반 파일이어야 합니다 (심볼릭 링크 불가)");
        return None;
    }
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) => {
            err(errs, rel, format!("읽기 실패 ({})", e.kind()));
            return None;
        }
    };
    match toml::from_str(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            match e.span() {
                Some(r) => {
                    let (line, col) = position(&text, r.start);
                    err(errs, rel, format!("형식 오류 (줄 {line}, 열 {col})"));
                }
                None => err(errs, rel, "형식 오류"),
            }
            None
        }
    }
}

fn check_schema(file: &str, schema: u32, errs: &mut Errs) {
    if schema != 1 { err(errs, file, format!("schema는 1이어야 합니다 (현재 {schema})")); }
}

fn is_ratio(m: Metric) -> bool {
    !matches!(m, Metric::NewIssues | Metric::Issues)
}

fn validate_gate(file: &str, gate: &[GateCondition], errs: &mut Errs) {
    let mut seen = BTreeSet::new();
    for c in gate {
        let m = c.metric.as_str();
        if !seen.insert(c.metric) {
            err(errs, file, format!("지표 {m}이(가) 두 번 쓰였습니다"));
        }
        if !c.value.is_finite() || c.value < 0.0 {
            err(errs, file, format!("지표 {m}의 값은 0 이상의 수여야 합니다"));
        } else if is_ratio(c.metric) {
            if c.value > 100.0 { err(errs, file, format!("지표 {m}의 값은 0~100이어야 합니다")); }
        } else if c.value.fract() != 0.0 {
            err(errs, file, format!("지표 {m}의 값은 정수여야 합니다"));
        } else if c.value > MAX_COUNT {
            err(errs, file, format!("지표 {m}의 값은 {MAX_COUNT} 이하여야 합니다"));
        }
    }
}
fn valid_ruleset_name(n: &str) -> bool {
    let b = n.as_bytes();
    !b.is_empty()
        && b.len() <= 63
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn validate_global(file: &str, g: &Global, errs: &mut Errs) {
    check_schema(file, g.schema, errs);
    validate_gate(file, &g.default_gate, errs);
}

fn validate_ruleset(file: &str, stem: &str, r: &Ruleset, catalog: &Catalog, errs: &mut Errs) {
    check_schema(file, r.schema, errs);
    if !valid_ruleset_name(&r.name) { err(errs, file, format!("룰셋 이름 규칙 위반: {}", r.name)); }
    if r.name != stem { err(errs, file, format!("name '{}'이(가) 파일 이름 '{stem}'과 다릅니다", r.name)); }
    for (dir, sel) in &r.languages {
        if !LANG_DIRS.contains(&dir.as_str()) {
            err(errs, file, format!("languages.{dir}: 알 수 없는 규칙 디렉터리"));
        }
        for id in &sel.disabled_rules {
            match catalog.rules.get(id) {
                None => err(errs, file, format!("languages.{dir}.disabled_rules: 카탈로그에 없는 규칙 {id}")),
                Some(info) if info.dir != *dir => {
                    err(errs, file, format!("languages.{dir}.disabled_rules: {id}은(는) {} 디렉터리의 규칙입니다", info.dir));
                }
                Some(_) => {}
            }
        }
    }
    for id in r.overrides.keys() {
        if !catalog.rules.contains_key(id) { err(errs, file, format!("overrides: 카탈로그에 없는 규칙 {id}")); }
    }
}

fn validate_project(file: &str, expected_repo: &str, p: &mut Project, errs: &mut Errs) {
    check_schema(file, p.schema, errs);
    if p.repo != expected_repo { err(errs, file, format!("repo '{}'이(가) 파일 경로 '{expected_repo}'와 다릅니다", p.repo)); }
    if let Some(langs) = &p.languages {
        for l in langs {
            if !LANG_DIRS.contains(&l.as_str()) { err(errs, file, format!("languages: 알 수 없는 규칙 디렉터리 {l}")); }
        }
    }
    if !(1..=3650).contains(&p.new_code_days) {
        err(errs, file, format!("new_code_days는 1~3650이어야 합니다 (현재 {})", p.new_code_days));
    }
    validate_gate(file, &p.gate, errs);
    if let Err(e) = validate_dup(&p.duplication) { err(errs, file, e.to_string()); }
    if let Err(e) = validate_publish(&mut p.publish) { err(errs, file, e.to_string()); }
}

/// 설정 디렉터리 하나의 항목을 읽는다. 디렉터리가 없으면 비어 있는 것으로 본다.
/// 심볼릭 링크는 따라가지 않는다. 반환: (이름, 디렉터리 여부).
fn list(dir: &Path, rel: &str, errs: &mut Errs) -> Vec<(String, bool)> {
    let p = dir.join(rel);
    match std::fs::symlink_metadata(&p) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => {
            err(errs, rel, "일반 디렉터리여야 합니다 (심볼릭 링크 불가)");
            return Vec::new();
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            err(errs, rel, format!("읽기 실패 ({})", e.kind()));
            return Vec::new();
        }
    }
    let rd = match std::fs::read_dir(&p) {
        Ok(r) => r,
        Err(e) => {
            err(errs, rel, format!("읽기 실패 ({})", e.kind()));
            return Vec::new();
        }
    };
    let mut v = Vec::new();
    for e in rd.flatten() {
        let Some(name) = e.file_name().to_str().map(str::to_string) else {
            err(errs, rel, "UTF-8이 아닌 파일 이름");
            continue;
        };
        match e.file_type() {
            Ok(t) if t.is_dir() => v.push((name, true)),
            Ok(t) if t.is_file() => v.push((name, false)),
            _ => err(errs, &format!("{rel}/{name}"), "일반 파일 또는 디렉터리여야 합니다 (심볼릭 링크 불가)"),
        }
    }
    v.sort();
    v
}

fn ruleset_stems(dir: &Path, errs: &mut Errs) -> Vec<String> {
    let mut v = Vec::new();
    for (n, is_dir) in list(dir, "rulesets", errs) {
        match n.strip_suffix(".toml") {
            Some(s) if !is_dir => v.push(s.to_string()),
            _ => err(errs, &format!("rulesets/{n}"), "예상하지 못한 항목 (룰셋은 <name>.toml 파일이어야 합니다)"),
        }
    }
    v
}

fn project_repos(dir: &Path, errs: &mut Errs) -> Vec<String> {
    let mut v = Vec::new();
    for (o, is_dir) in list(dir, "projects", errs) {
        if !is_dir {
            err(errs, &format!("projects/{o}"), "예상하지 못한 항목 (projects/<owner>/<repo>.toml 형식이어야 합니다)");
            continue;
        }
        for (n, is_dir) in list(dir, &format!("projects/{o}"), errs) {
            match n.strip_suffix(".toml") {
                Some(r) if !is_dir => v.push(format!("{o}/{r}")),
                _ => err(errs, &format!("projects/{o}/{n}"), "예상하지 못한 항목 (<repo>.toml 파일이어야 합니다)"),
            }
        }
    }
    v
}

fn load_ruleset(dir: &Path, name: &str, catalog: &Catalog, errs: &mut Errs) -> Option<Ruleset> {
    let file = format!("rulesets/{name}.toml");
    // 이름이 경로가 되므로 읽기 전에 검사한다. 프로젝트 파일이 `../`를 넣을 수 있다.
    if !valid_ruleset_name(name) {
        err(errs, &file, format!("룰셋 이름 규칙 위반: {name}"));
        return None;
    }
    let r: Ruleset = read(dir, &file, errs)?;
    validate_ruleset(&file, name, &r, catalog, errs);
    Some(r)
}

fn load_project(dir: &Path, repo: &str, errs: &mut Errs) -> Option<Project> {
    let file = format!("projects/{repo}.toml");
    let mut p: Project = read(dir, &file, errs)?;
    validate_project(&file, repo, &mut p, errs);
    Some(p)
}

fn load_global(dir: &Path, errs: &mut Errs) -> Option<Global> {
    let g: Global = read(dir, "qualgate.toml", errs)?;
    validate_global("qualgate.toml", &g, errs);
    Some(g)
}

/// 설정 저장소 전체를 검증한다. 오류는 모두 모아서 돌려준다.
pub fn load_dir_all(dir: &Path, catalog: &Catalog) -> Result<(), Vec<ConfigError>> {
    let mut errs = Vec::new();
    let global = load_global(dir, &mut errs);
    let names = ruleset_stems(dir, &mut errs);
    for n in &names {
        load_ruleset(dir, n, catalog, &mut errs);
    }
    if let Some(g) = &global {
        if !g.default_ruleset.is_empty() && !names.contains(&g.default_ruleset) {
            err(&mut errs, "qualgate.toml", format!("default_ruleset '{}' 룰셋이 없습니다", g.default_ruleset));
        }
    }
    for repo in project_repos(dir, &mut errs) {
        if let Some(p) = load_project(dir, &repo, &mut errs) {
            if !names.contains(&p.ruleset) {
                err(&mut errs, &format!("projects/{repo}.toml"), format!("ruleset '{}' 룰셋이 없습니다", p.ruleset));
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn valid_repo(repo: &str) -> bool {
    let ok = |s: &str| {
        !s.is_empty() && s != "." && s != ".." && s.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    };
    matches!(repo.split_once('/'), Some((o, r)) if ok(o) && ok(r))
}

/// 저장소 하나에 적용할 설정을 모은다. 프로젝트 파일이 없으면 전역 기본값을 쓴다.
pub fn resolve(dir: &Path, repo: &str, catalog: &Catalog) -> Result<Resolved, Vec<ConfigError>> {
    let mut errs = Vec::new();
    if !valid_repo(repo) {
        err(&mut errs, "repo", format!("저장소 이름은 owner/repo 형식이어야 합니다: {repo}"));
        return Err(errs);
    }
    let Some(global) = load_global(dir, &mut errs) else { return Err(errs) };
    let registered = dir.join("projects").join(format!("{repo}.toml")).symlink_metadata().is_ok();
    let project = if registered {
        load_project(dir, repo, &mut errs)
    } else if global.default_ruleset.is_empty() {
        err(&mut errs, "qualgate.toml", "default_ruleset이 비어 있어 미등록 저장소를 스캔할 수 없습니다");
        None
    } else {
        let gate = if global.default_gate.is_empty() {
            DEFAULT_GATE.iter().map(|&(metric, op, value)| GateCondition { metric, op, value }).collect()
        } else {
            global.default_gate.clone()
        };
        Some(Project::unregistered(repo, &global.default_ruleset, gate))
    };
    let ruleset = project.as_ref().and_then(|p| load_ruleset(dir, &p.ruleset, catalog, &mut errs));
    match (project, ruleset) {
        (Some(project), Some(ruleset)) if errs.is_empty() => Ok(Resolved { global, ruleset, project, registered }),
        _ => Err(errs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Kind, RuleInfo};
    use crate::model::Severity;

    fn info(id: &str, dir: &str) -> RuleInfo {
        RuleInfo {
            id: id.into(), dir: dir.into(), severity: Severity::Medium, kind: Kind::Issue,
            category: "c".into(), mois: None, cwe: None, title: "t".into(),
        }
    }

    fn catalog() -> Catalog {
        Catalog::from_rules(vec![
            info("java.system-out", "java"),
            info("java.catch-throwable", "java"),
            info("python.eval", "python"),
        ])
    }

    fn fixtures(kind: &str, name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/config").join(kind).join(name)
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            let t = to.join(e.file_name());
            if e.file_type().unwrap().is_dir() { copy_dir(&e.path(), &t) } else { std::fs::copy(e.path(), t).unwrap(); }
        }
    }

    /// `good` 픽스처의 복사본. 테스트가 파일을 덮어써서 오류를 만든다.
    fn good_copy() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        copy_dir(&fixtures("good", ""), d.path());
        d
    }

    fn put(d: &Path, rel: &str, text: &str) {
        let p = d.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn errors(d: &Path) -> Vec<ConfigError> { load_dir_all(d, &catalog()).unwrap_err() }

    fn has(errs: &[ConfigError], file: &str, part: &str) -> bool {
        errs.iter().any(|e| e.file == file && e.message.contains(part))
    }

    const X_OK: &str = "schema = 1\nname = \"x\"\n";

    #[test]
    fn good_fixture_passes() {
        load_dir_all(&fixtures("good", ""), &catalog()).unwrap();
    }

    #[test]
    fn unknown_key_is_error_in_that_file() {
        let d = good_copy();
        put(d.path(), "rulesets/x.toml", &format!("{X_OK}[languages.java]\nbogus = 1\n"));
        let e = errors(d.path());
        assert!(has(&e, "rulesets/x.toml", "형식 오류"), "{e:?}");
    }

    #[test]
    fn ruleset_name_must_match_file_name() {
        let d = good_copy();
        put(d.path(), "rulesets/x.toml", "schema = 1\nname = \"y\"\n");
        assert!(has(&errors(d.path()), "rulesets/x.toml", "파일 이름"));
    }

    #[test]
    fn ruleset_name_pattern_is_enforced() {
        let d = good_copy();
        put(d.path(), "rulesets/Bad_Name.toml", "schema = 1\nname = \"Bad_Name\"\n");
        assert!(has(&errors(d.path()), "rulesets/Bad_Name.toml", "이름 규칙"));
    }

    #[test]
    fn project_repo_must_match_path() {
        let d = good_copy();
        put(d.path(), "projects/team-a/other.toml", "schema = 1\nrepo = \"team-a/nope\"\nruleset = \"backend-standard\"\n");
        assert!(has(&errors(d.path()), "projects/team-a/other.toml", "repo"));
    }

    #[test]
    fn project_must_reference_existing_ruleset() {
        let d = good_copy();
        put(d.path(), "projects/team-a/other.toml", "schema = 1\nrepo = \"team-a/other\"\nruleset = \"missing\"\n");
        assert!(has(&errors(d.path()), "projects/team-a/other.toml", "룰셋이 없습니다"));
    }

    #[test]
    fn disabled_rules_must_exist_in_catalog_and_directory() {
        let d = good_copy();
        put(d.path(), "rulesets/x.toml", &format!("{X_OK}[languages.java]\ndisabled_rules = [\"java.nope\"]\n"));
        assert!(has(&errors(d.path()), "rulesets/x.toml", "카탈로그에 없는 규칙 java.nope"));
        put(d.path(), "rulesets/x.toml", &format!("{X_OK}[languages.java]\ndisabled_rules = [\"python.eval\"]\n"));
        assert!(has(&errors(d.path()), "rulesets/x.toml", "python 디렉터리"));
    }

    #[test]
    fn overrides_must_exist_in_catalog() {
        let d = good_copy();
        put(d.path(), "rulesets/x.toml", &format!("{X_OK}[overrides.\"java.nope\"]\nseverity = \"low\"\n"));
        assert!(has(&errors(d.path()), "rulesets/x.toml", "overrides"));
    }

    fn gate_errors(gate: &str) -> Vec<ConfigError> {
        let d = good_copy();
        put(d.path(), "qualgate.toml", &format!("schema = 1\ndefault_ruleset = \"backend-standard\"\n{gate}"));
        errors(d.path())
    }

    fn cond(metric: &str, value: &str) -> String {
        format!("[[default_gate]]\nmetric = \"{metric}\"\nop = \"gt\"\nvalue = {value}\n")
    }

    #[test]
    fn gate_values_are_checked() {
        let twice = format!("{}{}", cond("new_issues", "0"), cond("new_issues", "1"));
        assert!(has(&gate_errors(&twice), "qualgate.toml", "두 번"));
        assert!(has(&gate_errors(&cond("new_coverage", "101")), "qualgate.toml", "0~100"));
        assert!(has(&gate_errors(&cond("issues", "1.5")), "qualgate.toml", "정수"));
        assert!(has(&gate_errors(&cond("issues", "-1")), "qualgate.toml", "0 이상"));
        assert!(has(&gate_errors(&cond("coverage", "-0.5")), "qualgate.toml", "0 이상"));
    }

    #[test]
    fn unknown_language_dir_is_error_in_ruleset_and_project() {
        let d = good_copy();
        put(d.path(), "rulesets/x.toml", &format!("{X_OK}[languages.rust]\nenabled = true\n"));
        assert!(has(&errors(d.path()), "rulesets/x.toml", "rust"));
        put(d.path(), "rulesets/x.toml", X_OK);
        put(d.path(), "projects/team-a/other.toml", "schema = 1\nrepo = \"team-a/other\"\nruleset = \"x\"\nlanguages = [\"rust\"]\n");
        assert!(has(&errors(d.path()), "projects/team-a/other.toml", "rust"));
    }

    #[test]
    fn schema_must_be_one() {
        let d = good_copy();
        put(d.path(), "rulesets/x.toml", "schema = 2\nname = \"x\"\n");
        assert!(has(&errors(d.path()), "rulesets/x.toml", "schema"));
    }

    #[test]
    fn resolve_unregistered_uses_defaults() {
        let r = resolve(&fixtures("good", ""), "team-z/new-repo", &catalog()).unwrap();
        assert!(!r.registered);
        assert_eq!(r.project.ruleset, "backend-standard");
        assert_eq!(r.project.repo, "team-z/new-repo");
        assert_eq!(r.project.gate.len(), 1); // 전역 default_gate
        assert_eq!(r.project.new_code_days, 30);
    }

    #[test]
    fn resolve_unregistered_with_empty_default_gate_uses_builtin() {
        let d = good_copy();
        put(d.path(), "qualgate.toml", "schema = 1\ndefault_ruleset = \"backend-standard\"\n");
        let r = resolve(d.path(), "team-z/new-repo", &catalog()).unwrap();
        assert_eq!(r.project.gate.len(), 4);
        assert_eq!(r.project.gate[2].metric, Metric::NewCoverage);
        assert_eq!(r.project.gate[2].value, 80.0);
    }

    #[test]
    fn resolve_unregistered_without_default_ruleset_is_error() {
        let d = good_copy();
        put(d.path(), "qualgate.toml", "schema = 1\ndefault_ruleset = \"\"\n");
        let e = resolve(d.path(), "team-z/new-repo", &catalog()).unwrap_err();
        assert!(has(&e, "qualgate.toml", "default_ruleset"), "{e:?}");
    }

    #[test]
    fn resolve_registered_keeps_project_values_and_defaults() {
        let r = resolve(&fixtures("good", ""), "team-a/order-service", &catalog()).unwrap();
        assert!(r.registered);
        assert_eq!(r.project.languages, Some(vec!["java".to_string()]));
        assert_eq!(r.project.gate.len(), 2);
        assert_eq!(r.project.duplication.block_lines, 10);
        assert_eq!(r.ruleset.name, "backend-standard");
    }

    #[test]
    fn resolve_rejects_malformed_repo_names() {
        assert!(resolve(&fixtures("good", ""), "../x/y", &catalog()).is_err());
        assert!(resolve(&fixtures("good", ""), "a/b/c", &catalog()).is_err());
    }

    #[test]
    fn all_errors_are_returned_at_once() {
        let e = errors(&fixtures("bad", "multi"));
        assert_eq!(e.len(), 3, "{e:?}");
    }

    #[test]
    fn error_display_is_file_colon_message() {
        let e = ConfigError { file: "a.toml".into(), message: "m".into() };
        assert_eq!(e.to_string(), "a.toml: m");
    }

    #[test]
    fn toml_syntax_error_reports_position_without_source() {
        let d = good_copy();
        put(d.path(), "rulesets/x.toml", "schema = 1\nsecret-value-xyz = = 1\n");
        let e = errors(d.path());
        assert!(has(&e, "rulesets/x.toml", "줄 2"), "{e:?}");
        assert!(!e.iter().any(|x| x.message.contains("secret-value-xyz")));
    }

    #[test]
    fn ruleset_name_is_checked_before_reading() {
        let outer = tempfile::tempdir().unwrap();
        let cfg = outer.path().join("cfg");
        copy_dir(&fixtures("good", ""), &cfg);
        put(outer.path(), "x/y.toml", "schema = 1\nname = \"y\"\n");
        put(&cfg, "projects/team-a/evil.toml", "schema = 1\nrepo = \"team-a/evil\"\nruleset = \"../../x/y\"\n");
        let e = resolve(&cfg, "team-a/evil", &catalog()).unwrap_err();
        assert!(has(&e, "rulesets/../../x/y.toml", "이름 규칙"), "{e:?}");
        assert!(!e.iter().any(|x| x.message.contains("파일 이름")), "file was read: {e:?}");
        put(&cfg, "qualgate.toml", "schema = 1\ndefault_ruleset = \"../../x/y\"\n");
        let e = resolve(&cfg, "team-z/new", &catalog()).unwrap_err();
        assert!(!e.iter().any(|x| x.message.contains("파일 이름")), "file was read: {e:?}");
    }

    #[test]
    fn unknown_keys_in_project_sections_are_errors() {
        for sec in ["scan", "coverage", "duplication", "publish"] {
            let d = good_copy();
            put(d.path(), "projects/team-a/other.toml",
                &format!("schema = 1\nrepo = \"team-a/other\"\nruleset = \"backend-standard\"\n[{sec}]\nbogus = 1\n"));
            assert!(has(&errors(d.path()), "projects/team-a/other.toml", "형식 오류"), "{sec}");
        }
    }

    #[test]
    fn severity_accepts_only_known_names() {
        let d = good_copy();
        put(d.path(), "rulesets/x.toml", &format!("{X_OK}[overrides.\"java.system-out\"]\nseverity = \"blocker\"\n"));
        assert!(has(&errors(d.path()), "rulesets/x.toml", "형식 오류"));
        put(d.path(), "rulesets/x.toml", &format!("{X_OK}[overrides.\"java.system-out\"]\nseverity = \"critical\"\nkind = \"hotspot\"\n"));
        load_dir_all(d.path(), &catalog()).unwrap();
    }

    #[test]
    fn empty_default_ruleset_is_ok_for_full_validation_but_missing_target_is_not() {
        let d = good_copy();
        put(d.path(), "qualgate.toml", "schema = 1\ndefault_ruleset = \"\"\n");
        load_dir_all(d.path(), &catalog()).unwrap();
        put(d.path(), "qualgate.toml", "schema = 1\ndefault_ruleset = \"ghost\"\n");
        assert!(has(&errors(d.path()), "qualgate.toml", "ghost"));
    }

    #[test]
    fn count_metric_has_upper_bound() {
        assert!(has(&gate_errors(&cond("issues", "1e12")), "qualgate.toml", "이하"));
    }

    #[test]
    fn new_code_days_range() {
        for (v, bad) in [("0", true), ("3651", true), ("1", false), ("3650", false)] {
            let d = good_copy();
            put(d.path(), "projects/team-a/other.toml",
                &format!("schema = 1\nrepo = \"team-a/other\"\nruleset = \"backend-standard\"\nnew_code_days = {v}\n"));
            assert_eq!(load_dir_all(d.path(), &catalog()).is_err(), bad, "{v}");
        }
    }

    #[test]
    fn error_messages_use_config_spelling_and_omit_missing_position() {
        let e = gate_errors(&format!("{}{}", cond("new_issues", "0"), cond("new_issues", "1")));
        assert!(e.iter().any(|x| x.message.contains("new_issues")), "{e:?}");
        assert!(!e.iter().any(|x| x.message.contains("NewIssues")), "{e:?}");
    }

    #[test]
    fn unexpected_entries_are_errors() {
        let d = good_copy();
        put(d.path(), "projects/stray.toml", "schema = 1\n");
        put(d.path(), "projects/team-a/notes.txt", "x");
        put(d.path(), "rulesets/readme.md", "x");
        let e = errors(d.path());
        assert!(has(&e, "projects/stray.toml", "예상하지 못한"), "{e:?}");
        assert!(has(&e, "projects/team-a/notes.txt", "예상하지 못한"), "{e:?}");
        assert!(has(&e, "rulesets/readme.md", "예상하지 못한"), "{e:?}");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_rejected() {
        use std::os::unix::fs::symlink;
        let d = good_copy();
        let outside = tempfile::tempdir().unwrap();
        put(outside.path(), "o/r.toml", "schema = 1\n");
        std::fs::remove_dir_all(d.path().join("projects")).unwrap();
        symlink(outside.path(), d.path().join("projects")).unwrap();
        assert!(has(&errors(d.path()), "projects", "심볼릭"));

        let d = good_copy();
        symlink(outside.path().join("o"), d.path().join("projects/linked-owner")).unwrap();
        assert!(has(&errors(d.path()), "projects/linked-owner", "심볼릭"));

        let d = good_copy();
        std::fs::remove_file(d.path().join("qualgate.toml")).unwrap();
        put(outside.path(), "g.toml", "schema = 1\n");
        symlink(outside.path().join("g.toml"), d.path().join("qualgate.toml")).unwrap();
        assert!(has(&errors(d.path()), "qualgate.toml", "심볼릭"));
    }
}
