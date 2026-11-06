use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use qualgate::gitea::{Client, IssueApi};
use qualgate::model::ScanResult;
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
    serde_json::from_str(&t).context("result.json 형식 오류")
}

fn run(cli: Cli) -> Result<ScanResult> {
    match cli.cmd {
        Cmd::Scan { src, out } => do_scan(&src, &out),
        Cmd::Publish { repo, out } => { let r = load_result(&out)?; do_publish(&r, &repo)?; Ok(r) }
        Cmd::Run { repo, src, out } => { let r = do_scan(&src, &out)?; do_publish(&r, &repo)?; Ok(r) }
    }
}

fn main() {
    match run(Cli::parse()) {
        Ok(r) => std::process::exit(if r.gate.passed { 0 } else { 1 }),
        Err(e) => { eprintln!("오류: {e:#}"); std::process::exit(2); }
    }
}
