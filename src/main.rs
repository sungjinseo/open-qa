use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use qualgate::gitea::{Client, IssueApi};
use qualgate::model::{ConditionStatus, ScanResult, SCHEMA};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(version, about = "코드 품질을 스캔해 Gitea 이슈로 동기화합니다")]
struct Cli { #[command(subcommand)] cmd: Cmd }

#[derive(Subcommand)]
enum Cmd {
    /// 코드를 스캔해 result.json을 씁니다. 네트워크가 필요 없습니다.
    Scan {
        #[arg(long, default_value = "/src")] src: PathBuf,
        #[arg(long, default_value = "/out")] out: PathBuf,
    },
    /// result.json을 Gitea 이슈에 반영합니다. 코드를 읽지 않습니다.
    Publish {
        #[arg(long)] repo: String,
        #[arg(long, default_value = "/out")] out: PathBuf,
    },
    /// scan과 publish를 차례로 실행합니다.
    Run {
        #[arg(long)] repo: String,
        #[arg(long, default_value = "/src")] src: PathBuf,
        #[arg(long, default_value = "/out")] out: PathBuf,
    },
    /// 설정 저장소 전체(전역, 룰셋, 프로젝트)를 검증합니다. 오류가 있으면 종료 코드 2.
    ValidateConfig { #[arg(long)] config_dir: PathBuf },
    /// 설정 저장소 초기 파일(qualgate.toml, 기본 룰셋, README)을 만듭니다.
    InitConfig { #[arg(long)] out: PathBuf },
    /// 규칙 카탈로그를 JSON 배열로 출력합니다.
    Catalog { #[arg(long, required = true)] json: bool },
}

fn do_scan(src: &Path, out: &Path) -> Result<ScanResult> {
    let file = out.join("result.json");
    if file.exists() { std::fs::remove_file(&file).context("이전 결과 삭제 실패")?; }
    let rules = std::env::var("QUALGATE_RULES").unwrap_or_else(|_| "/rules".into());
    let og = std::env::var("QUALGATE_OPENGREP").unwrap_or_else(|_| "opengrep".into());
    let r = qualgate::scan::scan(src, Path::new(&rules), &og)?;
    std::fs::create_dir_all(out)?;
    std::fs::write(&file, serde_json::to_vec_pretty(&r)?).context("결과 파일 쓰기 실패")?;
    println!("위반 {}건, 중복 그룹 {}개, 게이트 {}", r.findings.len(), r.clones.len(), if r.gate.passed { "통과" } else { "실패" });
    if r.gate.conditions.iter().all(|c| c.status == ConditionStatus::Skipped) {
        println!("경고: 게이트 조건을 하나도 측정하지 못해 모두 건너뛰었습니다. 이 통과는 품질을 판정한 결과가 아닙니다.");
    }
    Ok(r)
}

fn do_publish(r: &ScanResult, repo: &str) -> Result<()> {
    let base = std::env::var("GITEA_URL").context("GITEA_URL 환경 변수가 필요합니다")?;
    let token = std::env::var("GITEA_TOKEN").context("GITEA_TOKEN 환경 변수가 필요합니다")?;
    let mut api = Client::new(&base, repo, &token);
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
    let rules = std::env::var("QUALGATE_RULES").unwrap_or_else(|_| "/rules".into());
    qualgate::catalog::Catalog::load(Path::new(&rules))
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

fn gate_code(r: &ScanResult) -> i32 { if r.gate.passed { 0 } else { 1 } }

fn run(cli: Cli) -> Result<i32> {
    match cli.cmd {
        Cmd::Scan { src, out } => Ok(gate_code(&do_scan(&src, &out)?)),
        Cmd::Publish { repo, out } => { let r = load_result(&out)?; do_publish(&r, &repo)?; Ok(gate_code(&r)) }
        Cmd::Run { repo, src, out } => { let r = do_scan(&src, &out)?; do_publish(&r, &repo)?; Ok(gate_code(&r)) }
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
