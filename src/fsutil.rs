use anyhow::{Context, Result};
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use std::path::Path;
use walkdir::WalkDir;

/// root 아래 파일을 `/` 구분 상대 경로로 정렬해 반환한다. `.git`은 제외한다.
/// 읽지 못한 디렉터리를 건너뛰면 그 안의 이슈가 "고쳐짐"으로 보이므로 탐색 오류는 치명적이다.
pub fn walk(root: &Path) -> Result<Vec<String>> {
    let mut out = vec![];
    for e in WalkDir::new(root).into_iter().filter_entry(|e| e.file_name() != ".git") {
        let e = e.context("파일 탐색 실패")?;
        if !e.file_type().is_file() { continue; }
        if let Ok(p) = e.path().strip_prefix(root) { out.push(p.to_string_lossy().replace('\\', "/")); }
    }
    out.sort();
    Ok(out)
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
        assert_eq!(walk(d.path()).unwrap(), vec!["a/b.go"]);
    }

    #[test]
    fn walk_error_is_returned() {
        let d = tempfile::tempdir().unwrap();
        assert!(walk(&d.path().join("missing")).is_err());
    }

    #[test]
    fn glob_matches_nested_and_root_only() {
        let g = globset(&["**/node_modules/**".into(), "coverage.xml".into()]).unwrap();
        assert!(g.is_match("web/node_modules/x/y.js"));
        assert!(g.is_match("coverage.xml"));
        assert!(!g.is_match("sub/coverage.xml"));
    }
}
