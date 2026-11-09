use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use qualgate::gitea::{Client, IssueApi};
use qualgate::model::{ScanResult, SCHEMA};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version, about = "코드 품질을 스캔해 Gitea 이슈로 동기화합니다")]
struct Cli { #[command(subcommand)] cmd: Cmd }

#[derive(Subcommand)]
enum Cmd {
    /// 설정 저장소에서 대상 저장소에 쓸 설정을 받아 검증하고 --out에 씁니다. --commit이 있으면 신규 코드 범위(newcode.json)도 계산합니다.
    FetchConfig {
        #[arg(long)] repo: String,
        #[arg(long)] branch: String,
        /// 스캔할 커밋의 전체 sha. 얕은 클론의 신규 코드 계산에 씁니다.
        #[arg(long)] commit: Option<String>,
        #[arg(long)] config_repo: String,
        #[arg(long = "ref", default_value = "main")] git_ref: String,
        #[arg(long)] out: PathBuf,
        /// 대상 저장소 이력을 받을 디렉터리(기본: --out의 상위 디렉터리의 history).
        #[arg(long)] history_dir: Option<PathBuf>,
    },
    /// 설정 디렉터리(fetch-config 출력)의 설정으로 코드를 스캔해 result.json을 씁니다. 네트워크가 필요 없습니다.
    Scan {
        #[arg(long, default_value = "/src")] src: PathBuf,
        #[arg(long)] config: PathBuf,
        #[arg(long, default_value = "/out")] out: PathBuf,
    },
    /// result.json을 Gitea 이슈에 반영합니다. 코드를 읽지 않습니다.
    Publish {
        #[arg(long)] repo: String,
        #[arg(long, default_value = "/out")] out: PathBuf,
    },
    /// 설정 저장소 전체(전역, 룰셋, 프로젝트)를 검증합니다. 오류가 있으면 종료 코드 2.
    ValidateConfig { #[arg(long)] config_dir: PathBuf },
    /// 설정 저장소 초기 파일(qualgate.toml, 기본 룰셋, README)을 만듭니다.
    InitConfig { #[arg(long)] out: PathBuf },
    /// 규칙 카탈로그를 JSON 배열로 출력합니다.
    Catalog { #[arg(long, required = true)] json: bool },
}

fn rules_root() -> PathBuf { PathBuf::from(std::env::var("QUALGATE_RULES").unwrap_or_else(|_| "/rules".into())) }

fn do_scan(src: &Path, config: &Path, out: &Path) -> Result<()> {
    let file = out.join("result.json");
    if file.exists() { std::fs::remove_file(&file).context("이전 결과 삭제 실패")?; }
    let og = std::env::var("QUALGATE_OPENGREP").unwrap_or_else(|_| "opengrep".into());
    let repo = qualgate::scan::source_repo(&qualgate::scan::read_source(config)?)?;
    let r = qualgate::scan::scan(src, config, &repo, &rules_root(), &og)?;
    std::fs::create_dir_all(out)?;
    std::fs::write(&file, serde_json::to_vec_pretty(&r)?).context("결과 파일 쓰기 실패")?;
    println!("위반 {}건, 중복 그룹 {}개, 실행 규칙 {}개", r.findings.len(), r.clones.len(), r.active_rules.len());
    Ok(())
}

/// 게이트 판정은 아직 하지 않는다(Task 6/9). 동기화만 한다.
fn do_publish(r: &ScanResult, repo: &str) -> Result<()> {
    let base = std::env::var("GITEA_URL").context("GITEA_URL 환경 변수가 필요합니다")?;
    let auth = qualgate::httpcfg::resolve_auth(&base)?;
    let mut api = Client::new(&base, repo, auth, qualgate::httpcfg::agent()?);
    let default = api.default_branch()?;
    if r.branch.as_deref() != Some(default.as_str()) {
        println!("브랜치 {:?}는 기본 브랜치 {default}가 아니어서 이슈를 동기화하지 않습니다.", r.branch);
        return Ok(());
    }
    let rep = qualgate::sync::sync(&mut api, r, base.trim_end_matches('/'), repo)?;
    println!("이슈 생성 {}, 갱신 {}, 종료 {}, 재오픈 {}, 제한으로 보류 {}", rep.created, rep.updated, rep.closed, rep.reopened, rep.skipped);
    Ok(())
}

fn load_result(out: &Path) -> Result<ScanResult> {
    let t = std::fs::read_to_string(out.join("result.json")).context("result.json이 없습니다. scan이 성공했는지 확인하세요")?;
    let v: serde_json::Value = serde_json::from_str(&t).context("result.json 형식 오류")?;
    if v["schema"] != SCHEMA { anyhow::bail!("result.json 스키마 {}을(를) 지원하지 않습니다 (지원: {SCHEMA})", v["schema"]); }
    serde_json::from_value(v).context("result.json 형식 오류")
}

fn load_catalog() -> Result<qualgate::catalog::Catalog> {
    qualgate::catalog::Catalog::load(&rules_root())
}

fn validate_config(dir: &Path) -> Result<i32> {
    let catalog = load_catalog()?;
    match qualgate::settings::load_dir_all(dir, &catalog) {
        Ok(()) => { println!("설정이 올바릅니다: {}", dir.display()); Ok(0) }
        Err(errs) => {
            for e in &errs { eprintln!("오류: {e}"); }
            eprintln!("설정 오류 {}건", errs.len());
            Ok(2)
        }
    }
}

struct FetchArgs { repo: String, branch: String, commit: Option<String>, config_repo: String, git_ref: String, out: PathBuf, history_dir: Option<PathBuf> }

/// `GitHistory`(credential 임시 파일을 가진 `GitEnv` 포함)는 이 함수가 끝날 때 drop된다.
/// 프로세스 종료(`std::process::exit`)는 그 뒤 `main`에서만 한다.
fn do_fetch(a: FetchArgs) -> Result<()> {
    use qualgate::fetch::{self, FetchRequest, GitHistory, GiteaConfig, HistorySource};
    let base = std::env::var("GITEA_URL").context("GITEA_URL 환경 변수가 필요합니다")?;
    let auth = qualgate::httpcfg::resolve_auth(&base)?;
    let catalog = load_catalog()?;
    let client = Client::new(&base, &a.config_repo, auth.clone(), qualgate::httpcfg::agent()?);
    let mut cfg = GiteaConfig::new(client, &a.config_repo, &a.git_ref);
    let mut hist = match &a.commit {
        Some(c) => {
            let dir = a.history_dir.clone().unwrap_or_else(|| a.out.parent().unwrap_or(Path::new(".")).join("history"));
            let env = qualgate::httpcfg::git_env(&auth, &base, &std::env::temp_dir())?;
            Some(GitHistory::new(qualgate::httpcfg::clone_url(&base, &a.repo)?, a.branch.clone(), c.clone(), dir, env))
        }
        None => None,
    };
    let req = FetchRequest {
        repo: a.repo, branch: a.branch, commit: a.commit, config_repo: a.config_repo, git_ref: a.git_ref,
        now: std::time::SystemTime::now(),
    };
    fetch::fetch(&mut cfg, hist.as_mut().map(|h| h as &mut dyn HistorySource), &req, &catalog, &a.out)?;
    println!("설정을 {}에 썼습니다{}", a.out.display(),
        if a.out.join("newcode.json").exists() { " (newcode.json 포함)" } else { "" });
    Ok(())
}

fn run(cli: Cli) -> Result<i32> {
    match cli.cmd {
        Cmd::FetchConfig { repo, branch, commit, config_repo, git_ref, out, history_dir } => {
            do_fetch(FetchArgs { repo, branch, commit, config_repo, git_ref, out, history_dir })?;
            Ok(0)
        }
        Cmd::Scan { src, config, out } => { do_scan(&src, &config, &out)?; Ok(0) }
        Cmd::Publish { repo, out } => { let r = load_result(&out)?; do_publish(&r, &repo)?; Ok(0) }
        Cmd::ValidateConfig { config_dir } => validate_config(&config_dir),
        Cmd::InitConfig { out } => {
            qualgate::initcfg::init_config(&out)?;
            println!("설정 저장소 초기 파일을 만들었습니다: {}", out.display());
            Ok(0)
        }
        Cmd::Catalog { .. } => {
            println!("{}", serde_json::to_string_pretty(&load_catalog()?.to_json())?);
            Ok(0)
        }
    }
}

fn main() {
    match run(Cli::parse()) {
        Ok(code) => std::process::exit(code),
        Err(e) => { eprintln!("오류: {e:#}"); std::process::exit(2); }
    }
}
