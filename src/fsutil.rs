use anyhow::{Context, Result};
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use std::path::Path;
use walkdir::WalkDir;

/// root 아래 파일을 `/` 구분 상대 경로로 정렬해 반환한다. `.git`은 제외한다.
/// 읽지 못한 디렉터리를 건너뛰면 그 안의 이슈가 "고쳐짐"으로 보이므로 탐색 오류는 치명적이다.
/// 단, 오류가 난 경로가 `exclude`에 걸린 디렉터리(또는 파일)면 분석 대상이 아니므로 무시한다
/// (root 소유 0700 `pgdata/`를 제외한 경우). 읽을 수 있는 제외 디렉터리는 그대로 탐색한다.
/// 커버리지 리포트가 `build/` 같은 제외 폴더에 있기 때문이다.
pub fn walk(root: &Path, exclude: &GlobSet) -> Result<Vec<String>> {
    let rel = |p: &Path| p.strip_prefix(root).ok().map(|r| r.to_string_lossy().replace('\\', "/"));
    let mut out = vec![];
    for e in WalkDir::new(root).into_iter().filter_entry(|e| e.file_name() != ".git") {
        let e = match e {
            Ok(e) => e,
            Err(err) => {
                let r = err.path().and_then(rel);
                let is_dir = err.path().is_none_or(|p| std::fs::metadata(p).map(|m| m.is_dir()).unwrap_or(true));
                if r.as_deref().is_some_and(|r| error_skippable(r, is_dir, exclude)) {
                    eprintln!("경고: 제외된 경로를 읽지 못해 건너뜁니다 ({}): {err}. 이 아래의 커버리지 리포트는 읽지 않습니다.", r.unwrap_or_default());
                    continue;
                }
                return Err(err).context("파일 탐색 실패");
            }
        };
        if !e.file_type().is_file() { continue; }
        if let Some(r) = rel(e.path()) { out.push(r); }
    }
    out.sort();
    Ok(out)
}

/// 탐색 오류를 무시해도 되는지. 디렉터리는 그 안의 모든 것이 제외될 때만(`contents_excluded`),
/// 파일은 경로 자체가 걸릴 때만 무시한다. 디렉터리 경로 자체가 걸린다고 내용까지 제외된 것은
/// 아니다(`a/*`, `**/build`는 `a/priv`에 걸리지만 `a/priv/p.py`에는 걸리지 않는다).
pub fn error_skippable(rel: &str, is_dir: bool, exclude: &GlobSet) -> bool {
    // 상대 경로가 비면 탐색 루트 자체다. 루트를 읽지 못하면 무엇도 분석되지 않았으므로 건너뛰지 않는다.
    if rel.is_empty() { return false; }
    if is_dir { contents_excluded(rel, exclude) } else { exclude.is_match(rel) }
}

/// 디렉터리 아래 임의의 이름·깊이가 모두 `exclude`에 걸리면 true. 흔치 않은 이름의 자식과 손자를
/// 여러 개 확인하므로 `**/x`와 `**/z`처럼 서로 다른 패턴 둘이 우연히 걸리는 경우에 속지 않는다.
/// 디렉터리 자체의 일치(`is_match(rel)`)는 판단에 쓰지 않는다.
pub fn contents_excluded(rel: &str, exclude: &GlobSet) -> bool {
    const NAMES: [&str; 6] = ["qg-probe-a", "qg_probe.b", "Qg-Probe-3", "qg-probe-d.py", "0qg-probe", ".qg-probe"];
    NAMES.iter().all(|n| {
        exclude.is_match(format!("{rel}/{n}").as_str())
            && exclude.is_match(format!("{rel}/{n}/{}/{}", NAMES[1], NAMES[2]).as_str())
    })
}

pub fn globset(patterns: &[String]) -> Result<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        let g: Glob = GlobBuilder::new(p).literal_separator(true).build()?;
        b.add(g);
    }
    Ok(b.build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_skips_git_and_is_relative() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join(".git")).unwrap();
        std::fs::create_dir_all(d.path().join("a")).unwrap();
        std::fs::write(d.path().join(".git/x"), "").unwrap();
        std::fs::write(d.path().join("a/b.go"), "").unwrap();
        assert_eq!(walk(d.path(), &globset(&[]).unwrap()).unwrap(), vec!["a/b.go"]);
    }

    #[test]
    fn walk_error_is_returned() {
        let d = tempfile::tempdir().unwrap();
        assert!(walk(&d.path().join("missing"), &globset(&[]).unwrap()).is_err());
    }

    #[test]
    fn error_skip_needs_contents_excluded_not_just_the_dir_path() {
        let g = |p: &[&str]| globset(&p.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        assert!(error_skippable("build", true, &g(&["**/build/**"])));
        assert!(error_skippable("a/b/pgdata", true, &g(&["**/pgdata/**"])));
        assert!(error_skippable("vendor", true, &g(&["vendor/**"])));
        // 디렉터리 경로만 걸리는 패턴이면 내용은 제외된 것이 아니므로 오류가 치명적이다.
        assert!(!error_skippable("a/priv", true, &g(&["a/*"])));
        assert!(!error_skippable("a/priv", true, &g(&["**/priv"])));
        assert!(!error_skippable("a/build", true, &g(&["**/build"])));
        // 서로 무관한 패턴 둘이 우연히 걸려도 속지 않는다.
        assert!(!error_skippable("d", true, &g(&["**/x", "**/z"])));
        assert!(!error_skippable("d", true, &g(&["**/{x,z}"])));
        assert!(!error_skippable("src", true, &g(&["docs/*.py"])));
        // 파일은 경로 자체가 걸리면 된다.
        assert!(error_skippable("a/f.py", false, &g(&["a/*"])));
        assert!(!error_skippable("a/f.py", false, &g(&["b/*"])));
    }

    #[test]
    fn probes_cover_digit_and_dot_names_and_root_is_never_skipped() {
        let g = |p: &[&str]| globset(&p.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        // 문자 클래스 쌍은 영문자로 시작하는 이름만 덮으므로 디렉터리 전체가 제외된 것이 아니다.
        assert!(!error_skippable("d", true, &g(&["d/[A-Za-z]*", "d/[A-Za-z]*/**"])));
        assert!(error_skippable("d", true, &g(&["d/**"])));
        // 빈 상대 경로는 스캔 루트다.
        assert!(!error_skippable("", true, &g(&["**"])));
        assert!(!error_skippable("", false, &g(&["**"])));
    }

    #[test]
    fn readable_excluded_files_are_still_listed() {
        let g = globset(&["**/pgdata/**".into()]).unwrap();
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("a/pgdata")).unwrap();
        std::fs::write(d.path().join("a/pgdata/f"), "").unwrap();
        std::fs::write(d.path().join("a/f.go"), "").unwrap();
        assert_eq!(walk(d.path(), &g).unwrap(), vec!["a/f.go", "a/pgdata/f"]);
    }

    #[test]
    fn glob_matches_nested_and_root_only() {
        let g = globset(&["**/node_modules/**".into(), "coverage.xml".into()]).unwrap();
        assert!(g.is_match("web/node_modules/x/y.js"));
        assert!(g.is_match("coverage.xml"));
        assert!(!g.is_match("sub/coverage.xml"));
    }
}
