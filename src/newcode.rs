use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

pub type ChangedLines = BTreeMap<String, BTreeSet<u32>>;

fn git(src: &Path, args: &[&str]) -> Result<String> {
    let o = Command::new("git").args(["-c", "safe.directory=*", "-C"]).arg(src).args(args).output().context("git 실행 실패")?;
    if !o.status.success() { bail!("git {args:?}: {}", String::from_utf8_lossy(&o.stderr)); }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

pub fn parse_diff(diff: &str) -> ChangedLines {
    let mut out = ChangedLines::new();
    let mut file: Option<String> = None;
    for line in diff.lines() {
        if let Some(p) = line.strip_prefix("+++ ") {
            // git appends a tab after paths that contain a space.
            let p = p.strip_suffix('\t').unwrap_or(p);
            file = p.strip_prefix("b/").map(str::to_string);
        } else if line.starts_with("@@") {
            let Some(f) = &file else { continue };
            let Some(plus) = line.split_whitespace().find(|t| t.starts_with('+')) else { continue };
            let spec = plus.trim_start_matches('+');
            let (start, count) = match spec.split_once(',') {
                Some((a, b)) => (a.parse::<u32>().unwrap_or(0), b.parse::<u32>().unwrap_or(0)),
                None => (spec.parse::<u32>().unwrap_or(0), 1),
            };
            let e = out.entry(f.clone()).or_default();
            for n in start..start.saturating_add(count) { e.insert(n); }
        }
    }
    out
}

pub fn changed(src: &Path, days: u32) -> Result<Option<ChangedLines>> {
    if git(src, &["rev-parse", "--is-shallow-repository"])?.trim() == "true" { return Ok(None); }
    let before = format!("--before={days} days ago");
    let base = git(src, &["rev-list", "-1", &before, "HEAD"])?;
    let base = base.trim();
    if base.is_empty() { return Ok(None); }
    let diff = git(src, &["-c", "core.quotepath=false", "diff", "-U0", "--no-color", "--no-ext-diff", "--no-renames", "--src-prefix=a/", "--dst-prefix=b/", base, "HEAD"])?;
    // git still quotes paths with control characters, quotes or backslashes. Those hunks
    // cannot be mapped to a file name, so report unmeasured rather than undercount.
    if diff.lines().any(|l| l.starts_with("+++ \"")) { return Ok(None); }
    Ok(Some(parse_diff(&diff)))
}

pub fn head_info(src: &Path) -> Result<(String, Option<String>)> {
    let commit = git(src, &["rev-parse", "HEAD"])?.trim().to_string();
    if let Ok(b) = std::env::var("QUALGATE_BRANCH") {
        if !b.is_empty() { return Ok((commit, Some(b))); }
    }
    let branch = git(src, &["rev-parse", "--abbrev-ref", "HEAD"])?.trim().to_string();
    Ok((commit, if branch == "HEAD" { None } else { Some(branch) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_added_ranges_and_skips_deleted_files() {
        let d = "diff --git a/a.go b/a.go\n--- a/a.go\n+++ b/a.go\n@@ -1,0 +2,2 @@\n+x\n+y\n@@ -5 +7 @@\n-z\n+w\ndiff --git a/gone b/gone\n--- a/gone\n+++ /dev/null\n@@ -1 +0,0 @@\n-q\n";
        let c = parse_diff(d);
        assert_eq!(c["a.go"].iter().copied().collect::<Vec<_>>(), vec![2, 3, 7]);
        assert!(!c.contains_key("gone"));
    }

    fn commit(dir: &Path, file: &str, body: &str, date: &str) {
        std::fs::write(dir.join(file), body).unwrap();
        for args in [vec!["add", "-A"], vec!["commit", "-qm", "x"]] {
            let st = Command::new("git").current_dir(dir).args(&args)
                .env("GIT_AUTHOR_DATE", date).env("GIT_COMMITTER_DATE", date).status().unwrap();
            assert!(st.success());
        }
    }

    #[test]
    fn changed_uses_commit_before_window() {
        let d = tempfile::tempdir().unwrap();
        Command::new("git").current_dir(d.path()).args(["init", "-qb", "main"]).status().unwrap();
        commit(d.path(), "a.txt", "1\n2\n", "2000-01-01T00:00:00");
        commit(d.path(), "a.txt", "1\n2\n3\n", "2099-01-01T00:00:00");
        let c = changed(d.path(), 30).unwrap().unwrap();
        assert_eq!(c["a.txt"].iter().copied().collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn no_old_commit_means_unmeasured() {
        let d = tempfile::tempdir().unwrap();
        Command::new("git").current_dir(d.path()).args(["init", "-qb", "main"]).status().unwrap();
        commit(d.path(), "a.txt", "1\n", "2099-01-01T00:00:00");
        assert_eq!(changed(d.path(), 30).unwrap(), None);
    }

    #[test]
    fn parses_path_with_space_and_trailing_tab() {
        let d = "diff --git a/my file.txt b/my file.txt\n--- a/my file.txt\n+++ b/my file.txt\t\n@@ -0,0 +1 @@\n+x\n";
        let c = parse_diff(d);
        assert_eq!(c["my file.txt"].iter().copied().collect::<Vec<_>>(), vec![1]);
    }

    fn repo_with_changed_file(name: &str) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        Command::new("git").current_dir(d.path()).args(["init", "-qb", "main"]).status().unwrap();
        commit(d.path(), name, "1\n2\n", "2000-01-01T00:00:00");
        commit(d.path(), name, "1\n2\n3\n", "2099-01-01T00:00:00");
        d
    }

    #[test]
    fn changed_handles_non_ascii_path() {
        let d = repo_with_changed_file("한글.txt");
        let c = changed(d.path(), 30).unwrap().unwrap();
        assert_eq!(c["한글.txt"].iter().copied().collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn changed_handles_path_with_space() {
        let d = repo_with_changed_file("my file.txt");
        let c = changed(d.path(), 30).unwrap().unwrap();
        assert_eq!(c["my file.txt"].iter().copied().collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn quoted_path_is_unmeasured() {
        let d = repo_with_changed_file("q\"x.txt");
        assert_eq!(changed(d.path(), 30).unwrap(), None);
    }

    #[test]
    fn head_info_reports_branch() {
        let d = tempfile::tempdir().unwrap();
        Command::new("git").current_dir(d.path()).args(["init", "-qb", "main"]).status().unwrap();
        commit(d.path(), "a.txt", "1\n", "2000-01-01T00:00:00");
        let (c, b) = head_info(d.path()).unwrap();
        assert_eq!(c.len(), 40);
        assert_eq!(b.as_deref(), Some("main"));
    }
}
