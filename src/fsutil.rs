use anyhow::Result;
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use std::path::Path;
use walkdir::WalkDir;

/// root 아래 파일을 `/` 구분 상대 경로로 정렬해 반환한다. `.git`은 제외한다.
pub fn walk(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.path().strip_prefix(root).ok().map(|p| p.to_string_lossy().replace('\\', "/")))
        .collect();
    out.sort();
    out
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
        assert_eq!(walk(d.path()), vec!["a/b.go"]);
    }

    #[test]
    fn glob_matches_nested_and_root_only() {
        let g = globset(&["**/node_modules/**".into(), "coverage.xml".into()]).unwrap();
        assert!(g.is_match("web/node_modules/x/y.js"));
        assert!(g.is_match("coverage.xml"));
        assert!(!g.is_match("sub/coverage.xml"));
    }
}
