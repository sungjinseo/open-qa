//! `fetch-config`: 설정 저장소에서 필요한 설정 파일만 받아 검증하고, 얕은 클론에 대비해 신규 코드
//! 범위(`newcode.json`)를 git으로 계산한다(스펙 5.1).
use crate::catalog::Catalog;
use crate::gitea::Client;
use crate::httpcfg::GitEnv;
use crate::newcode::{self, NewCodeFile};
use crate::settings;
use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DAY: Duration = Duration::from_secs(86_400);

/// 설정 저장소의 한 커밋에서 파일을 읽는다.
pub trait ConfigSource {
    /// 파일 내용. 없으면 `None`.
    fn get(&mut self, path: &str) -> Result<Option<Vec<u8>>>;
    /// 읽는 설정의 커밋 sha.
    fn commit(&mut self) -> Result<String>;
}

/// 대상 저장소의 이력.
pub trait HistorySource {
    /// `until`(RFC3339) 이전의 마지막 커밋. 받은 이력에 없으면 `None`.
    fn commit_before(&mut self, until: &str) -> Result<Option<String>>;
    /// `git diff -U0` 원문.
    fn diff(&mut self, base: &str, head: &str) -> Result<String>;
}

pub struct FetchRequest {
    pub repo: String,
    pub branch: String,
    pub commit: Option<String>,
    pub config_repo: String,
    pub git_ref: String,
    pub now: SystemTime,
}

/// 경고를 stderr에 출력한다.
pub fn warn_stderr(m: String) { eprintln!("경고: {m}"); }

/// 설정을 받아 검증하고 `out`에 쓴다. 검증이 끝나기 전에는 `out`에 아무것도 쓰지 않는다.
/// 신규 코드 계산 실패는 경고만 하고 `newcode.json`을 쓰지 않는다(신규 코드 측정 안 됨).
pub fn fetch(cfg: &mut dyn ConfigSource, hist: Option<&mut dyn HistorySource>, req: &FetchRequest, catalog: &Catalog, out: &Path) -> Result<()> {
    fetch_with(cfg, hist, req, catalog, out, &mut warn_stderr)
}

/// `fetch`와 같고, 경고를 `warn`으로 보낸다.
pub fn fetch_with(cfg: &mut dyn ConfigSource, hist: Option<&mut dyn HistorySource>, req: &FetchRequest, catalog: &Catalog, out: &Path,
                  warn: &mut dyn FnMut(String)) -> Result<()> {
    ensure_empty(out)?;
    if !settings::valid_repo(&req.repo) { bail!("저장소 이름은 owner/repo 형식이어야 합니다: {}", req.repo); }
    if let Some(c) = &req.commit {
        // scan은 newcode.json의 head를 `git rev-parse HEAD`(소문자 전체 sha)와 비교한다.
        // 짧은 sha, 대문자, 옵션처럼 보이는 값은 받지 않는다(CLI는 소문자로 바꿔 넘긴다).
        if !matches!(c.len(), 40 | 64) || !c.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            bail!("--commit은 전체 커밋 sha(소문자 16진수 40자 또는 64자)여야 합니다: {c}");
        }
    }

    // 모든 파일을 같은 커밋에서 읽도록 커밋을 먼저 정한다.
    let config_commit = cfg.commit()?;
    let global = cfg.get("qualgate.toml")?.ok_or_else(|| anyhow!(
        "설정 저장소 {}({}, 커밋 {config_commit})에 qualgate.toml이 없습니다. CONFIG_REPO와 ref, 계정의 읽기 권한을 확인하세요",
        req.config_repo, req.git_ref
    ))?;
    let project_file = format!("projects/{}.toml", req.repo);
    let project = cfg.get(&project_file)?;
    let mut files = vec![("qualgate.toml".to_string(), global)];
    // 이름을 읽지 못하거나 이름 규칙에 어긋나면 받지 않는다. 오류는 아래 검증이 파일 위치와 함께 낸다.
    if let Some(name) = ruleset_name(&files[0].1, project.as_deref()).filter(|n| settings::valid_ruleset_name(n)) {
        let file = format!("rulesets/{name}.toml");
        if let Some(b) = cfg.get(&file)? { files.push((file, b)); }
    }
    if let Some(p) = project { files.push((project_file, p)); }

    // `out`과 같은 상위 디렉터리에 모두 쓴 뒤 이름을 바꿔 한 번에 내놓는다. 도중에 실패하면 임시
    // 디렉터리는 drop될 때 지워지고 `out`은 그대로다.
    let parent = match out.parent() { Some(p) if !p.as_os_str().is_empty() => p, _ => Path::new(".") };
    std::fs::create_dir_all(parent).with_context(|| format!("{} 생성 실패", parent.display()))?;
    let stage = tempfile::Builder::new().prefix(".qualgate-fetch-").tempdir_in(parent)
        .with_context(|| format!("{}에 임시 디렉터리를 만들지 못했습니다", parent.display()))?;
    write_files(stage.path(), &files)?;
    let res = settings::resolve(stage.path(), &req.repo, catalog).map_err(|errs| {
        anyhow!("설정 오류 {}건 (설정 저장소 {}, 커밋 {config_commit}):\n{}", errs.len(), req.config_repo,
            errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))
    })?;

    let newcode = match (&req.commit, hist) {
        (Some(c), Some(h)) => {
            let until = rfc3339(req.now.checked_sub(DAY * res.project.new_code_days).unwrap_or(UNIX_EPOCH));
            new_code(h, c, &until).map_err(|m| warn(format!("{m}. 신규 코드는 측정하지 않습니다(scan이 git 이력으로 계산할 수 있으면 그 값을 씁니다)"))).ok()
        }
        _ => None,
    };

    let source = serde_json::json!({
        "config_repo": req.config_repo, "ref": req.git_ref, "config_commit": config_commit,
        "repo": req.repo, "branch": req.branch, "registered": res.registered,
    });
    let mut extra = vec![("source.json".to_string(), serde_json::to_vec_pretty(&source)?)];
    if let Some(n) = newcode { extra.push(("newcode.json".into(), serde_json::to_vec_pretty(&n)?)); }
    write_files(stage.path(), &extra)?;
    publish_dir(stage, out)
}

fn ensure_empty(out: &Path) -> Result<()> {
    match std::fs::read_dir(out) {
        Ok(mut it) => if it.next().is_some() { bail!("출력 디렉터리가 비어 있지 않습니다: {}", out.display()) },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("출력 디렉터리를 읽을 수 없습니다: {}", out.display())),
    }
    Ok(())
}

/// 완성된 임시 디렉터리를 `out` 이름으로 바꾼다. 빈 `out`이 있으면 먼저 지운다(비어 있지 않으면 실패).
fn publish_dir(stage: tempfile::TempDir, out: &Path) -> Result<()> {
    match std::fs::remove_dir(out) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("출력 디렉터리를 바꿀 수 없습니다(비어 있어야 합니다): {}", out.display())),
    }
    std::fs::rename(stage.path(), out).with_context(|| format!("{} 쓰기 실패", out.display()))?;
    // 이름을 바꿨으므로 drop이 지울 것이 없다.
    let _ = stage.keep();
    Ok(())
}

fn write_files(dir: &Path, files: &[(String, Vec<u8>)]) -> Result<()> {
    for (rel, bytes) in files {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() { std::fs::create_dir_all(parent).with_context(|| format!("{} 생성 실패", parent.display()))?; }
        std::fs::write(&p, bytes).with_context(|| format!("{} 쓰기 실패", p.display()))?;
    }
    Ok(())
}

/// 사용할 룰셋 이름: 프로젝트 파일이 있으면 그 `ruleset`, 없으면 전역 `default_ruleset`.
fn ruleset_name(global: &[u8], project: Option<&[u8]>) -> Option<String> {
    let key = |b: &[u8], k: &str| -> Option<String> {
        let t: toml::Table = toml::from_str(std::str::from_utf8(b).ok()?).ok()?;
        t.get(k)?.as_str().map(str::to_string)
    };
    match project { Some(p) => key(p, "ruleset"), None => key(global, "default_ruleset") }
}

/// 신규 코드 범위. 계산할 수 없으면 경고 문구.
fn new_code(h: &mut dyn HistorySource, commit: &str, until: &str) -> std::result::Result<NewCodeFile, String> {
    let base = match h.commit_before(until) {
        Ok(Some(b)) => b,
        Ok(None) => return Err(format!("기준 시각 {until} 이전 커밋을 받은 이력에서 찾지 못했습니다")),
        Err(e) => return Err(format!("신규 코드 기준 커밋을 찾지 못했습니다: {e:#}")),
    };
    let diff = h.diff(&base, commit).map_err(|e| format!("{base}..{commit} diff를 계산하지 못했습니다: {e:#}"))?;
    let (lines, quoted) = newcode::parse_diff_ex(&diff);
    if quoted { return Err("따옴표로 감싼 파일 경로가 있어 변경 줄을 파일에 연결할 수 없습니다".into()); }
    Ok(NewCodeFile { base, head: commit.to_string(), files: lines.into_iter().map(|(f, s)| (f, s.into_iter().collect())).collect() })
}

// --- RFC3339 (UTC, `YYYY-MM-DDTHH:MM:SSZ`) ---

/// 1970-01-01부터의 일수 -> (년, 월, 일). (H. Hinnant의 civil_from_days)
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let (m, d) = (i64::from(m), i64::from(d));
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

/// UTC RFC3339. 1970년 이전은 1970-01-01T00:00:00Z로 둔다.
pub fn rfc3339(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / 3600, s % 3600 / 60, s % 60)
}

/// `rfc3339`가 만드는 형식만 읽는다.
pub fn parse_rfc3339(s: &str) -> Option<SystemTime> {
    let b = s.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'Z' { return None; }
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let p = s.get(r)?;
        if p.bytes().all(|c| c.is_ascii_digit()) { p.parse().ok() } else { None }
    };
    let (y, mo, d, h, mi, se) = (num(0..4)?, num(5..7)?, num(8..10)?, num(11..13)?, num(14..16)?, num(17..19)?);
    if !(1..=12).contains(&mo) || h > 23 || mi > 59 || se > 59 { return None; }
    let days = days_from_civil(i64::from(y), mo, d);
    if civil_from_days(days) != (i64::from(y), mo, d) || days < 0 { return None; }
    let secs = days as u64 * 86_400 + u64::from(h * 3600 + mi * 60 + se);
    Some(UNIX_EPOCH + Duration::from_secs(secs))
}

/// Gitea 파일 API로 읽는 설정 저장소. 처음 정한 커밋에서 모든 파일을 읽는다.
pub struct GiteaConfig { client: Client, repo: String, git_ref: String, sha: Option<String> }

impl GiteaConfig {
    pub fn new(client: Client, repo: &str, git_ref: &str) -> GiteaConfig {
        GiteaConfig { client, repo: repo.to_string(), git_ref: git_ref.to_string(), sha: None }
    }
}

impl ConfigSource for GiteaConfig {
    fn commit(&mut self) -> Result<String> {
        if let Some(s) = &self.sha { return Ok(s.clone()); }
        let s = self.client.latest_commit(&self.repo, &self.git_ref).with_context(|| format!(
            "설정 저장소 {}의 {} 커밋을 읽지 못했습니다. CONFIG_REPO와 ref, 계정의 읽기 권한을 확인하세요", self.repo, self.git_ref
        ))?;
        self.sha = Some(s.clone());
        Ok(s)
    }

    fn get(&mut self, path: &str) -> Result<Option<Vec<u8>>> {
        let sha = self.commit()?;
        Ok(self.client.get_file(&self.repo, path, &sha)?.map(|(b, _)| b))
    }
}

/// `git clone --bare --single-branch --shallow-since`로 받은 대상 저장소 이력.
/// 인증 환경(`GitEnv`, credential 임시 파일)을 소유하므로 이 값이 살아 있는 동안 git이 인증할 수 있고,
/// drop되면 임시 파일이 지워진다.
pub struct GitHistory { url: String, branch: String, commit: String, dir: PathBuf, env: GitEnv, cloned: bool }

impl GitHistory {
    /// `url`은 자격 증명이 없는 클론 주소(`httpcfg::clone_url`), `dir`은 비어 있거나 없는 디렉터리.
    pub fn new(url: String, branch: String, commit: String, dir: PathBuf, env: GitEnv) -> GitHistory {
        GitHistory { url, branch, commit, dir, env, cloned: false }
    }

    /// 원격과 통신하는 git. 인증 환경(`GitEnv`)을 적용하고 `GITEA_TOKEN`은 넘기지 않는다.
    /// 실패하면 git의 stderr(자격 증명이 없는 URL만 담긴다).
    fn remote_git<I, S>(&self, args: I) -> std::result::Result<(), String>
    where I: IntoIterator<Item = S>, S: AsRef<std::ffi::OsStr> {
        let mut cmd = Command::new("git");
        cmd.env_remove("GITEA_TOKEN").stdin(Stdio::null());
        self.env.apply(&mut cmd);
        let o = cmd.args(args).output().map_err(|e| format!("git 실행 실패: {e}"))?;
        if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).trim().to_string()) }
    }

    fn clone(&self, since: &str) -> Result<()> {
        if let Some(p) = self.dir.parent() { std::fs::create_dir_all(p).with_context(|| format!("{} 생성 실패", p.display()))?; }
        let dir = self.dir.as_os_str();
        let clone = |limit: &str| {
            let mut args = ["clone", "--quiet", "--bare", "--single-branch", "--no-tags", "--branch", &self.branch, limit, "--", &self.url]
                .map(std::ffi::OsStr::new).to_vec();
            args.push(dir);
            self.remote_git(args)
        };
        let fail = |m: &str| anyhow!("{} (브랜치 {}) 이력을 받지 못했습니다: {m}", self.url, self.branch);
        if let Err(first) = clone(&format!("--shallow-since={since}")) {
            // 기간 안에 커밋이 하나도 없으면 git이 클론을 거부한다. 그때만 최신 커밋 하나를 받는다
            // (그 커밋이 기준 시각 이전이면 기준 커밋 = HEAD, 신규 코드 없음).
            if !first.contains("no commits selected for shallow requests") { return Err(fail(&first)); }
            clone("--depth=1").map_err(|second| fail(&format!("{first}; --depth=1 재시도도 실패했습니다: {second}")))?;
        }
        if self.dir.join("shallow").exists() {
            // 경계 커밋의 부모(기간 밖에 있는 실제 기준 커밋)까지 받는다.
            let args = [std::ffi::OsStr::new("-C"), dir].into_iter()
                .chain(["fetch", "--quiet", "--no-tags", "--deepen=1", "origin", &self.branch].map(std::ffi::OsStr::new));
            self.remote_git(args).map_err(|e| anyhow!("{} (브랜치 {}) 이력을 한 단계 더 받지 못했습니다: {e}", self.url, self.branch))?;
        }
        Ok(())
    }
}

impl HistorySource for GitHistory {
    fn commit_before(&mut self, until: &str) -> Result<Option<String>> {
        let t = parse_rfc3339(until).with_context(|| format!("기준 시각 형식 오류: {until}"))?;
        if !self.cloned {
            self.clone(&rfc3339(t.checked_sub(DAY).unwrap_or(UNIX_EPOCH)))?;
            self.cloned = true;
        }
        if newcode::git(&self.dir, &["cat-file", "-e", &format!("{}^{{commit}}", self.commit)]).is_err() {
            bail!("커밋 {}이(가) 받은 브랜치 {} 이력에 없습니다", self.commit, self.branch);
        }
        // 병합된 곁가지 커밋을 기준으로 고르지 않도록 첫 부모만 따라간다(newcode::changed와 같다).
        let base = newcode::git(&self.dir, &["rev-list", "-1", "--first-parent", &format!("--before={until}"), &self.commit, "--"])?;
        let base = base.trim();
        Ok(if base.is_empty() { None } else { Some(base.to_string()) })
    }

    fn diff(&mut self, base: &str, head: &str) -> Result<String> {
        newcode::git(&self.dir, &["-c", "core.quotepath=false", "diff", "-U0", "--no-color", "--no-ext-diff", "--find-renames",
            "--src-prefix=a/", "--dst-prefix=b/", base, head, "--"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Kind, RuleInfo};
    use crate::model::Severity;
    use std::collections::BTreeMap;
    use std::process::Command;
    use std::time::{Duration, UNIX_EPOCH};

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
    const GLOBAL: &str = "schema = 1\ndefault_ruleset = \"std\"\n";
    const STD: &str = "schema = 1\nname = \"std\"\n\n[languages.python]\nenabled = true\ndisabled_rules = [\"python.eval\"]\n";
    const TEAM: &str = "schema = 1\nname = \"team\"\n";
    const PROJECT: &str = "schema = 1\nrepo = \"o/r\"\nruleset = \"team\"\nnew_code_days = 10\n";

    fn catalog() -> Catalog {
        Catalog::from_rules(vec![RuleInfo {
            id: "python.eval".into(), dir: "python".into(), severity: Severity::Medium, kind: Kind::Issue,
            category: "c".into(), mois: None, cwe: None, title: "t".into(), owasp_api: vec![],
        }])
    }

    struct FakeConfig { files: BTreeMap<String, String>, requested: Vec<String> }
    impl ConfigSource for FakeConfig {
        fn get(&mut self, path: &str) -> Result<Option<Vec<u8>>> {
            self.requested.push(path.to_string());
            Ok(self.files.get(path).map(|s| s.as_bytes().to_vec()))
        }
        fn commit(&mut self) -> Result<String> { Ok("cfgsha".into()) }
    }

    fn config(files: &[(&str, &str)]) -> FakeConfig {
        FakeConfig { files: files.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), requested: vec![] }
    }

    fn standard() -> FakeConfig {
        config(&[("qualgate.toml", GLOBAL), ("rulesets/std.toml", STD), ("rulesets/team.toml", TEAM), ("projects/o/r.toml", PROJECT)])
    }

    enum Base { Found(&'static str), Missing, Fails }
    struct FakeHistory { base: Base, diff: Result<String, String>, untils: Vec<String> }
    impl HistorySource for FakeHistory {
        fn commit_before(&mut self, until: &str) -> Result<Option<String>> {
            self.untils.push(until.to_string());
            match self.base { Base::Found(b) => Ok(Some(b.into())), Base::Missing => Ok(None), Base::Fails => bail!("history api down") }
        }
        fn diff(&mut self, base: &str, head: &str) -> Result<String> {
            assert_eq!((base, head), ("b0", COMMIT));
            self.diff.clone().map_err(anyhow::Error::msg)
        }
    }

    fn history(base: Base, diff: &str) -> FakeHistory { FakeHistory { base, diff: Ok(diff.into()), untils: vec![] } }

    /// 2023-11-14T22:13:20Z
    fn now() -> SystemTime { UNIX_EPOCH + Duration::from_secs(1_700_000_000) }

    fn req(repo: &str, commit: Option<&str>) -> FetchRequest {
        FetchRequest {
            repo: repo.into(), branch: "main".into(), commit: commit.map(str::to_string),
            config_repo: "qg/cfg".into(), git_ref: "main".into(), now: now(),
        }
    }

    fn run(cfg: &mut FakeConfig, hist: Option<&mut FakeHistory>, r: &FetchRequest, out: &Path) -> Result<()> {
        fetch(cfg, hist.map(|h| h as &mut dyn HistorySource), r, &catalog(), out)
    }

    /// 성공해야 하는 실행. 경고 목록을 돌려준다.
    fn run_ok(cfg: &mut FakeConfig, hist: &mut FakeHistory, r: &FetchRequest, out: &Path) -> Vec<String> {
        let mut warnings = vec![];
        fetch_with(cfg, Some(hist), r, &catalog(), out, &mut |m| warnings.push(m)).unwrap();
        warnings
    }

    /// 상위 디렉터리에 임시 디렉터리(.qualgate-fetch-*)가 남지 않았는지.
    fn no_stage_left(parent: &Path) -> bool {
        std::fs::read_dir(parent).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with(".qualgate-fetch-"))
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = walkdir::WalkDir::new(dir).into_iter().map(|e| e.unwrap()).filter(|e| e.file_type().is_file())
            .map(|e| e.path().strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/")).collect();
        v.sort();
        v
    }

    fn json(p: &Path) -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap() }

    #[test]
    fn registered_repo_gets_its_files_and_source() {
        let t = tempfile::tempdir().unwrap();
        let out = t.path().join("out");
        let mut cfg = standard();
        run(&mut cfg, None, &req("o/r", None), &out).unwrap();
        assert_eq!(files_in(&out), vec!["projects/o/r.toml", "qualgate.toml", "rulesets/team.toml", "source.json"]);
        assert_eq!(std::fs::read_to_string(out.join("projects/o/r.toml")).unwrap(), PROJECT);
        assert_eq!(json(&out.join("source.json")), serde_json::json!({
            "config_repo": "qg/cfg", "ref": "main", "config_commit": "cfgsha", "repo": "o/r", "branch": "main", "registered": true
        }));
        assert!(!cfg.requested.contains(&"rulesets/std.toml".to_string()), "only the used ruleset is fetched");
        // 받은 설정은 scan과 같은 방법으로 읽힌다.
        assert!(crate::settings::resolve(&out, "o/r", &catalog()).unwrap().registered);
        assert!(no_stage_left(t.path()));
    }

    #[test]
    fn unregistered_repo_uses_default_ruleset() {
        let t = tempfile::tempdir().unwrap();
        let out = t.path().join("out");
        std::fs::create_dir(&out).unwrap();
        run(&mut standard(), None, &req("o/other", None), &out).unwrap();
        assert_eq!(files_in(&out), vec!["qualgate.toml", "rulesets/std.toml", "source.json"]);
        assert_eq!(json(&out.join("source.json"))["registered"], false);
    }

    #[test]
    fn invalid_config_leaves_nothing() {
        let t = tempfile::tempdir().unwrap();
        let mut cfg = standard();
        cfg.files.insert("rulesets/team.toml".into(), "schema = 1\nname = \"team\"\n[languages.python]\nenabled = true\ndisabled_rules = [\"python.nope\"]\n".into());
        let out = t.path().join("out");
        let e = format!("{:#}", run(&mut cfg, None, &req("o/r", None), &out).unwrap_err());
        assert!(e.contains("rulesets/team.toml") && e.contains("python.nope"), "{e}");
        assert!(!out.exists());
        let out = t.path().join("empty");
        std::fs::create_dir(&out).unwrap();
        assert!(run(&mut cfg, None, &req("o/r", None), &out).is_err());
        assert_eq!(files_in(&out), Vec::<String>::new());
        assert!(no_stage_left(t.path()));
    }

    #[test]
    fn unregistered_without_default_ruleset_is_an_error() {
        let t = tempfile::tempdir().unwrap();
        let mut cfg = config(&[("qualgate.toml", "schema = 1\ndefault_ruleset = \"\"\n"), ("rulesets/std.toml", STD)]);
        let e = format!("{:#}", run(&mut cfg, None, &req("o/r", None), &t.path().join("out")).unwrap_err());
        assert!(e.contains("default_ruleset"), "{e}");
        assert!(!t.path().join("out").exists());
    }

    #[test]
    fn missing_global_is_an_error_naming_the_config_repo() {
        let t = tempfile::tempdir().unwrap();
        let mut cfg = config(&[("projects/o/r.toml", PROJECT)]);
        let e = format!("{:#}", run(&mut cfg, None, &req("o/r", None), &t.path().join("out")).unwrap_err());
        assert!(e.contains("qualgate.toml") && e.contains("qg/cfg"), "{e}");
    }

    #[test]
    fn unsafe_ruleset_name_is_not_fetched() {
        let t = tempfile::tempdir().unwrap();
        let mut cfg = standard();
        cfg.files.insert("projects/o/r.toml".into(), "schema = 1\nrepo = \"o/r\"\nruleset = \"../x\"\n".into());
        assert!(run(&mut cfg, None, &req("o/r", None), &t.path().join("out")).is_err());
        assert!(cfg.requested.iter().all(|p| !p.contains("..")), "{:?}", cfg.requested);
    }

    #[test]
    fn invalid_repo_or_commit_is_an_error() {
        let t = tempfile::tempdir().unwrap();
        assert!(run(&mut standard(), None, &req("../r", None), &t.path().join("a")).is_err());
        assert!(run(&mut standard(), None, &req("o/r", Some("--upload-pack=x")), &t.path().join("b")).is_err());
        // newcode.json의 head는 git rev-parse 출력(소문자)과 비교되므로 대문자는 받지 않는다.
        assert!(run(&mut standard(), None, &req("o/r", Some(&COMMIT.to_ascii_uppercase())), &t.path().join("c")).is_err());
        assert!(run(&mut standard(), None, &req("o/r", Some(&COMMIT[..12])), &t.path().join("d")).is_err());
    }

    #[test]
    fn newcode_json_from_history() {
        let t = tempfile::tempdir().unwrap();
        let out = t.path().join("out");
        let mut h = history(Base::Found("b0"), "diff --git a/a.py b/a.py\n--- a/a.py\n+++ b/a.py\n@@ -1,0 +2,2 @@\n+x\n+y\n@@ -5 +8 @@\n-q\n+w\n");
        assert_eq!(run_ok(&mut standard(), &mut h, &req("o/r", Some(COMMIT)), &out), Vec::<String>::new());
        // 기준 시각 = now - new_code_days(10)
        assert_eq!(h.untils, vec!["2023-11-04T22:13:20Z".to_string()]);
        assert_eq!(json(&out.join("newcode.json")), serde_json::json!({"base": "b0", "head": COMMIT, "files": {"a.py": [2, 3, 8]}}));
        let n = crate::newcode::read_newcode_json(&out.join("newcode.json")).unwrap().unwrap();
        assert_eq!(n.head, COMMIT);
    }

    #[test]
    fn no_base_commit_means_no_newcode_json() {
        let t = tempfile::tempdir().unwrap();
        let out = t.path().join("out");
        let w = run_ok(&mut standard(), &mut history(Base::Missing, ""), &req("o/r", Some(COMMIT)), &out);
        assert!(out.join("source.json").exists());
        assert!(!out.join("newcode.json").exists());
        assert!(w.len() == 1 && w[0].contains("2023-11-04T22:13:20Z") && w[0].contains("측정하지 않습니다"), "{w:?}");
    }

    #[test]
    fn history_errors_only_warn() {
        let t = tempfile::tempdir().unwrap();
        let out = t.path().join("a");
        let w = run_ok(&mut standard(), &mut history(Base::Fails, ""), &req("o/r", Some(COMMIT)), &out);
        assert!(out.join("source.json").exists() && !out.join("newcode.json").exists());
        assert!(w.len() == 1 && w[0].contains("history api down") && w[0].contains("측정하지 않습니다"), "{w:?}");
        let out = t.path().join("b");
        let mut h = FakeHistory { base: Base::Found("b0"), diff: Err("diff failed".into()), untils: vec![] };
        let w = run_ok(&mut standard(), &mut h, &req("o/r", Some(COMMIT)), &out);
        assert!(out.join("source.json").exists() && !out.join("newcode.json").exists());
        assert!(w.len() == 1 && w[0].contains("diff failed"), "{w:?}");
        // 따옴표 경로는 줄을 파일에 연결할 수 없어 측정하지 않는다.
        let out = t.path().join("c");
        let mut h = history(Base::Found("b0"), "--- a/a\n+++ \"b/q\\\"x\"\n@@ -0,0 +1 @@\n+z\n");
        let w = run_ok(&mut standard(), &mut h, &req("o/r", Some(COMMIT)), &out);
        assert!(!out.join("newcode.json").exists());
        assert!(w.len() == 1 && w[0].contains("따옴표"), "{w:?}");
    }

    #[test]
    fn non_empty_out_is_an_error() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("old.txt"), "x").unwrap();
        let e = run(&mut standard(), None, &req("o/r", None), t.path()).unwrap_err().to_string();
        assert!(e.contains("비어 있지"), "{e}");
        assert_eq!(files_in(t.path()), vec!["old.txt"]);
    }

    #[test]
    fn rfc3339_round_trip() {
        assert_eq!(rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(now()), "2023-11-14T22:13:20Z");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400 + 3_661);
        assert_eq!(rfc3339(leap), "2000-02-29T01:01:01Z");
        assert_eq!(rfc3339(UNIX_EPOCH + Duration::from_secs(4_102_444_799)), "2099-12-31T23:59:59Z");
        for t in [UNIX_EPOCH, now(), leap] { assert_eq!(parse_rfc3339(&rfc3339(t)), Some(t)); }
        for bad in ["2023-11-14", "2023-11-14T22:13:20", "2023-13-01T00:00:00Z", "2023-11-14T22:13:20+09:00"] {
            assert_eq!(parse_rfc3339(bad), None, "{bad}");
        }
    }

    // --- GitHistory: file:// 로컬 저장소(네트워크 없음) ---

    fn git(dir: &Path, args: &[&str], date: &str) -> String {
        let o = Command::new("git").current_dir(dir).args(args)
            .env("GIT_AUTHOR_DATE", date).env("GIT_COMMITTER_DATE", date).output().unwrap();
        assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    }

    /// 2025-12-01(줄 1), 2026-01-01(줄 2), 2026-01-09T18:00Z(줄 3), 2026-01-11(줄 4, HEAD) 커밋의 sha.
    fn source_repo() -> (tempfile::TempDir, [String; 4]) {
        let d = tempfile::tempdir().unwrap();
        git(d.path(), &["init", "-qb", "main"], "2025-12-01T00:00:00Z");
        let mut shas = vec![];
        for (i, date) in ["2025-12-01T00:00:00Z", "2026-01-01T00:00:00Z", "2026-01-09T18:00:00Z", "2026-01-11T00:00:00Z"].iter().enumerate() {
            let body: String = (1..=i + 1).map(|n| format!("{n}\n")).collect();
            std::fs::write(d.path().join("a.txt"), body).unwrap();
            git(d.path(), &["add", "-A"], date);
            git(d.path(), &["commit", "-qm", "c"], date);
            shas.push(git(d.path(), &["rev-parse", "HEAD"], date));
        }
        (d, shas.try_into().unwrap())
    }

    fn git_history(src: &Path, commit: &str, work: &Path) -> GitHistory {
        GitHistory::new(format!("file://{}", src.display()), "main".into(), commit.into(), work.join("history"), GitEnv::empty())
    }

    fn has_commit(repo: &Path, c: &str) -> bool {
        Command::new("git").arg("-C").arg(repo).args(["cat-file", "-e", &format!("{c}^{{commit}}")])
            .stderr(std::process::Stdio::null()).status().unwrap().success()
    }

    #[test]
    fn git_history_clones_shallow_and_diffs() {
        let (src, [c0, c1, c2, c3]) = source_repo();
        let work = tempfile::tempdir().unwrap();
        let mut h = git_history(src.path(), &c3, work.path());
        assert_eq!(h.commit_before("2026-01-10T12:00:00Z").unwrap(), Some(c2.clone()));
        // --shallow-since=기준 시각 - 1일, 그리고 경계 커밋의 부모 한 단계(--deepen=1)까지만 받는다.
        let repo = work.path().join("history");
        assert!(repo.join("shallow").exists());
        assert!(has_commit(&repo, &c1) && has_commit(&repo, &c2) && has_commit(&repo, &c3) && !has_commit(&repo, &c0));
        let c = crate::newcode::parse_diff(&h.diff(&c2, &c3).unwrap());
        assert_eq!(c["a.txt"].iter().copied().collect::<Vec<_>>(), vec![4]);
    }

    #[test]
    fn git_history_base_outside_window_is_reached_by_deepening() {
        let (src, [_, c1, _, c3]) = source_repo();
        let work = tempfile::tempdir().unwrap();
        let mut h = git_history(src.path(), &c3, work.path());
        // 기준 시각 2026-01-09T12:00Z 이전 커밋(2026-01-01)은 클론 범위(-1일) 밖이지만 경계 커밋의 부모다.
        assert_eq!(h.commit_before("2026-01-09T12:00:00Z").unwrap(), Some(c1.clone()));
        let c = crate::newcode::parse_diff(&h.diff(&c1, &c3).unwrap());
        assert_eq!(c["a.txt"].iter().copied().collect::<Vec<_>>(), vec![3, 4]);
    }

    #[test]
    fn git_history_without_older_commit_is_none() {
        let (src, [_, _, _, c3]) = source_repo();
        let work = tempfile::tempdir().unwrap();
        let mut h = git_history(src.path(), &c3, work.path());
        // 모든 커밋이 기준 시각(2025-11-01) 이후다: 기준 커밋이 없다.
        assert_eq!(h.commit_before("2025-11-01T00:00:00Z").unwrap(), None);
    }

    #[test]
    fn git_history_without_commits_in_window_uses_head_as_base() {
        let (src, [_, _, _, c3]) = source_repo();
        let work = tempfile::tempdir().unwrap();
        let mut h = git_history(src.path(), &c3, work.path());
        // 기간(2026-01-31 이후)에 커밋이 없으면 --shallow-since 클론이 거부되어 최신 커밋만 받는다.
        assert_eq!(h.commit_before("2026-02-01T00:00:00Z").unwrap(), Some(c3.clone()));
        assert_eq!(crate::newcode::parse_diff(&h.diff(&c3, &c3).unwrap()), crate::newcode::ChangedLines::new());
    }

    #[test]
    fn git_history_merge_in_range_uses_first_parent_base() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        git(p, &["init", "-qb", "main"], "2026-01-01T00:00:00Z");
        std::fs::write(p.join("main.txt"), "1\n").unwrap();
        git(p, &["add", "-A"], "2026-01-01T00:00:00Z");
        git(p, &["commit", "-qm", "c1"], "2026-01-01T00:00:00Z");
        git(p, &["checkout", "-qb", "side"], "2026-01-01T00:00:00Z");
        std::fs::write(p.join("side.txt"), "s\n").unwrap();
        git(p, &["add", "-A"], "2026-01-09T20:00:00Z");
        git(p, &["commit", "-qm", "s1"], "2026-01-09T20:00:00Z");
        git(p, &["checkout", "-q", "main"], "2026-01-08T00:00:00Z");
        std::fs::write(p.join("main.txt"), "1\n2\n").unwrap();
        git(p, &["add", "-A"], "2026-01-08T00:00:00Z");
        git(p, &["commit", "-qm", "m0"], "2026-01-08T00:00:00Z");
        let m0 = git(p, &["rev-parse", "HEAD"], "2026-01-08T00:00:00Z");
        git(p, &["merge", "-q", "--no-ff", "-m", "merge", "side"], "2026-01-11T00:00:00Z");
        let head = git(p, &["rev-parse", "HEAD"], "2026-01-11T00:00:00Z");
        let work = tempfile::tempdir().unwrap();
        let mut h = git_history(p, &head, work.path());
        // 곁가지 커밋 s1(2026-01-09T20:00)이 더 최근이지만 기준은 첫 부모 쪽 m0이다.
        assert_eq!(h.commit_before("2026-01-10T12:00:00Z").unwrap(), Some(m0.clone()));
        let c = crate::newcode::parse_diff(&h.diff(&m0, &head).unwrap());
        assert_eq!(c.keys().collect::<Vec<_>>(), vec!["side.txt"]);
    }

    #[test]
    fn git_history_missing_commit_or_clone_failure_is_an_error() {
        let (src, _) = source_repo();
        let work = tempfile::tempdir().unwrap();
        let mut h = git_history(src.path(), COMMIT, work.path());
        let e = format!("{:#}", h.commit_before("2026-01-10T12:00:00Z").unwrap_err());
        assert!(e.contains(COMMIT), "{e}");
        let work = tempfile::tempdir().unwrap();
        let mut h = git_history(&src.path().join("nope"), COMMIT, work.path());
        // 기간 거부가 아닌 오류는 --depth=1로 다시 시도하지 않고 git의 오류를 그대로 알린다.
        let e = format!("{:#}", h.commit_before("2026-01-10T12:00:00Z").unwrap_err());
        assert!(e.contains("does not appear to be a git repository") && !e.contains("--depth=1"), "{e}");
        let mut h = git_history(src.path(), COMMIT, work.path());
        assert!(h.commit_before("not a time").is_err());
    }
}
