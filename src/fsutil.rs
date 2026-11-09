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
                let skip = err.path().and_then(rel).is_some_and(|r| dir_excluded(&r, exclude) || exclude.is_match(r.as_str()));
                if skip { continue; }
                return Err(err).context("파일 탐색 실패");
            }
        };
        if !e.file_type().is_file() { continue; }
        if let Some(r) = rel(e.path()) { out.push(r); }
    }
    out.sort();
    Ok(out)
}

/// 디렉터리 자체가 걸리거나, 그 아래 임의의 깊이(자식, 손자)가 모두 걸리면 true. `**/x/**`는 `x`
/// 자체에는 걸리지 않으므로 자식 이름을 붙여 확인한다. `a/*`처럼 직계 자식만 걸리는 패턴은
/// 손자 확인에서 떨어지므로 하위 디렉터리를 잘못 건너뛰지 않는다.
pub fn dir_excluded(rel: &str, exclude: &GlobSet) -> bool {
    exclude.is_match(rel)
        || (exclude.is_match(format!("{rel}/x").as_str()) && exclude.is_match(format!("{rel}/x/y/z").as_str()))
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
    fn excluded_dir_predicate_and_excluded_files_still_listed() {
        let g = globset(&["**/pgdata/**".into(), "vendor/**".into(), "docs/*.py".into()]).unwrap();
        assert!(dir_excluded("pgdata", &g));
        assert!(dir_excluded("a/b/pgdata", &g));
        assert!(dir_excluded("vendor", &g));
        assert!(dir_excluded("vendor/x", &g));
        assert!(!dir_excluded("src", &g));
        assert!(!dir_excluded("docs", &g));
        let direct = globset(&["a/*".into()]).unwrap();
        assert!(!dir_excluded("a", &direct));
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("a/pgdata")).unwrap();
        std::fs::create_dir_all(d.path().join("a/ok")).unwrap();
        std::fs::write(d.path().join("a/pgdata/f"), "").unwrap();
        std::fs::write(d.path().join("a/ok/f.go"), "").unwrap();
        // 읽을 수 있는 제외 디렉터리는 그대로 나열된다(커버리지 리포트 탐색용).
        assert_eq!(walk(d.path(), &g).unwrap(), vec!["a/ok/f.go", "a/pgdata/f"]);
    }

    #[test]
    fn glob_matches_nested_and_root_only() {
        let g = globset(&["**/node_modules/**".into(), "coverage.xml".into()]).unwrap();
        assert!(g.is_match("web/node_modules/x/y.js"));
        assert!(g.is_match("coverage.xml"));
        assert!(!g.is_match("sub/coverage.xml"));
    }
}
