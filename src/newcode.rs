use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

pub type ChangedLines = BTreeMap<String, BTreeSet<u32>>;

fn git(src: &Path, args: &[&str]) -> Result<String> {
    let o = Command::new("git").env_remove("GITEA_TOKEN").args(["-c", "safe.directory=*", "-C"]).arg(src).args(args).output().context("git 실행 실패")?;
    if !o.status.success() { bail!("git {args:?}: {}", String::from_utf8_lossy(&o.stderr)); }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// `@@ -a[,b] +c[,d] @@`에서 (옛 줄 수, 새 시작, 새 줄 수)
fn parse_hunk(line: &str) -> Option<(u32, u32, u32)> {
    let mut it = line.split_whitespace().skip(1);
    let span = |t: &str, sign: char| -> Option<(u32, u32)> {
        let spec = t.strip_prefix(sign)?;
        Some(match spec.split_once(',') {
            Some((a, b)) => (a.parse().ok()?, b.parse().ok()?),
            None => (spec.parse().ok()?, 1),
        })
    };
    let (_, old) = span(it.next()?, '-')?;
    let (start, count) = span(it.next()?, '+')?;
    Some((old, start, count))
}

/// 변경된 줄과, 따옴표로 감싼 파일 경로(줄 번호를 파일에 연결할 수 없음)를 만났는지.
/// 헤더(`+++ `)는 hunk 밖에서만 인정한다. 줄 수로 hunk 본문을 건너뛰므로 내용이 `++ b/x`인 추가 줄
/// (`+++ b/x`로 보임)이 파일 헤더로 오인되지 않는다.
fn parse_diff_ex(diff: &str) -> (ChangedLines, bool) {
    let mut out = ChangedLines::new();
    let mut quoted = false;
    let mut file: Option<String> = None;
    let (mut old_left, mut new_left) = (0u32, 0u32);
    for line in diff.lines() {
        if old_left > 0 || new_left > 0 {
            match line.as_bytes().first() {
                Some(b'+') => new_left = new_left.saturating_sub(1),
                Some(b'-') => old_left = old_left.saturating_sub(1),
                Some(b'\\') => {}
                _ => { old_left = old_left.saturating_sub(1); new_left = new_left.saturating_sub(1); }
            }
        } else if let Some(p) = line.strip_prefix("+++ ") {
            // git appends a tab after paths that contain a space.
            let p = p.strip_suffix('\t').unwrap_or(p);
            quoted |= p.starts_with('"');
            file = p.strip_prefix("b/").map(str::to_string);
        } else if line.starts_with("@@") {
            let Some((old, start, count)) = parse_hunk(line) else { continue };
            (old_left, new_left) = (old, count);
            let Some(f) = &file else { continue };
            let e = out.entry(f.clone()).or_default();
            for n in start..start.saturating_add(count) { e.insert(n); }
        }
    }
    (out, quoted)
}

pub fn parse_diff(diff: &str) -> ChangedLines { parse_diff_ex(diff).0 }

pub fn changed(src: &Path, days: u32) -> Result<Option<ChangedLines>> {
    if git(src, &["rev-parse", "--is-shallow-repository"])?.trim() == "true" { return Ok(None); }
    let before = format!("--before={days} days ago");
    let base = git(src, &["rev-list", "-1", &before, "HEAD"])?;
    let base = base.trim();
    if base.is_empty() { return Ok(None); }
    let diff = git(src, &["-c", "core.quotepath=false", "diff", "-U0", "--no-color", "--no-ext-diff", "--find-renames", "--src-prefix=a/", "--dst-prefix=b/", base, "HEAD"])?;
    // git still quotes paths with control characters, quotes or backslashes. Those hunks
    // cannot be mapped to a file name, so report unmeasured rather than undercount.
    let (c, quoted) = parse_diff_ex(&diff);
    if quoted { return Ok(None); }
    Ok(Some(c))
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

    #[test]
    fn added_line_that_looks_like_a_file_header_is_content() {
        // 추가된 줄 `++ b/x`는 diff에서 `+++ b/x`로 보인다. 파일 헤더가 아니다.
        let d = "diff --git a/a.go b/a.go\n--- a/a.go\n+++ b/a.go\n@@ -1,0 +2,2 @@\n+++ b/x\n+y\n@@ -9 +11 @@\n-q\n+w\n";
        let (c, quoted) = parse_diff_ex(d);
        assert!(!quoted);
        assert_eq!(c.keys().collect::<Vec<_>>(), vec!["a.go"]);
        assert_eq!(c["a.go"].iter().copied().collect::<Vec<_>>(), vec![2, 3, 11]);
        // 따옴표 경로처럼 보이는 추가 줄도 헤더가 아니다.
        let d = "--- a/a.go\n+++ b/a.go\n@@ -0,0 +1 @@\n+++ \"b/q\n";
        assert!(!parse_diff_ex(d).1);
        // 진짜 따옴표 헤더는 감지한다.
        assert!(parse_diff_ex("--- a/a\n+++ \"b/q\\\"x\"\n@@ -0,0 +1 @@\n+z\n").1);
    }

    #[test]
    fn hunk_bodies_with_context_lines_are_skipped_by_count() {
        let d = "--- a/a.go\n+++ b/a.go\n@@ -1,2 +1,3 @@\n ctx\n-old\n+new\n+++ b/evil\n@@ -9 +10 @@\n-q\n+w\n\\ No newline at end of file\n";
        let c = parse_diff(d);
        assert_eq!(c.keys().collect::<Vec<_>>(), vec!["a.go"]);
        assert_eq!(c["a.go"].iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 10]);
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

    fn commit_all(dir: &Path, date: &str) {
        for args in [vec!["add", "-A"], vec!["commit", "-qm", "x"]] {
            let st = Command::new("git").current_dir(dir).args(&args)
                .env("GIT_AUTHOR_DATE", date).env("GIT_COMMITTER_DATE", date).status().unwrap();
            assert!(st.success());
        }
    }

    #[test]
    fn moved_directory_is_not_new_code() {
        let d = tempfile::tempdir().unwrap();
        Command::new("git").current_dir(d.path()).args(["init", "-qb", "main"]).status().unwrap();
        // 저장소 설정이 이름 변경 감지를 꺼도 이동한 코드가 신규로 잡히면 안 된다.
        Command::new("git").current_dir(d.path()).args(["config", "diff.renames", "false"]).status().unwrap();
        std::fs::create_dir_all(d.path().join("old")).unwrap();
        let body: String = (0..20).map(|i| format!("line {i}\n")).collect();
        std::fs::write(d.path().join("old/a.py"), &body).unwrap();
        std::fs::write(d.path().join("old/b.py"), format!("{body}tail\n")).unwrap();
        commit_all(d.path(), "2000-01-01T00:00:00");
        std::fs::rename(d.path().join("old"), d.path().join("new")).unwrap();
        commit_all(d.path(), "2099-01-01T00:00:00");
        assert_eq!(changed(d.path(), 30).unwrap().unwrap(), ChangedLines::new());
    }

    #[test]
    fn renamed_and_edited_file_keys_on_new_path() {
        let d = tempfile::tempdir().unwrap();
        Command::new("git").current_dir(d.path()).args(["init", "-qb", "main"]).status().unwrap();
        let body: String = (0..20).map(|i| format!("line {i}\n")).collect();
        std::fs::write(d.path().join("a.py"), &body).unwrap();
        commit_all(d.path(), "2000-01-01T00:00:00");
        std::fs::remove_file(d.path().join("a.py")).unwrap();
        std::fs::write(d.path().join("b.py"), format!("{body}added\n")).unwrap();
        commit_all(d.path(), "2099-01-01T00:00:00");
        let c = changed(d.path(), 30).unwrap().unwrap();
        assert_eq!(c["b.py"].iter().copied().collect::<Vec<_>>(), vec![21]);
        assert_eq!(c.len(), 1);
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
