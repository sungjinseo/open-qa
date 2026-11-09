use crate::config::CoverageCfg;
use crate::model::{CoverageMap, LineCov};
use anyhow::{Context, Result};
use globset::GlobSet;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::BTreeMap;
use std::path::Path;

pub type RawCov = BTreeMap<String, BTreeMap<u32, LineCov>>;

#[derive(Debug, Default, PartialEq)]
pub struct Loaded {
    pub map: CoverageMap,
    pub reports: Vec<String>,
    pub unresolved: Vec<String>,
    pub errors: Vec<String>,
}

fn attr(e: &BytesStart, name: &[u8]) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref().as_bytes() == name)
        .map(|a| a.value.into_owned())
}

fn merge_line(m: &mut BTreeMap<u32, LineCov>, line: u32, c: LineCov) {
    let e = m.entry(line).or_default();
    e.covered |= c.covered;
    e.conditions = e.conditions.max(c.conditions);
    e.covered_conditions = e.covered_conditions.max(c.covered_conditions);
}

pub fn parse_jacoco(xml: &str) -> Result<RawCov> {
    let mut r = Reader::from_str(xml);
    let mut out = RawCov::new();
    let (mut pkg, mut file) = (String::new(), None::<String>);
    let mut saw_root = false;
    loop {
        match r.read_event().context("JaCoCo XML 오류")? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref().as_bytes() {
                b"report" => saw_root = true,
                b"package" => pkg = attr(&e, b"name").unwrap_or_default(),
                b"sourcefile" => {
                    let n = attr(&e, b"name").unwrap_or_default();
                    file = Some(if pkg.is_empty() { n } else { format!("{pkg}/{n}") });
                }
                b"line" => if let Some(f) = &file {
                    let g = |k: &[u8]| attr(&e, k).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                    let (mb, cb) = (g(b"mb"), g(b"cb"));
                    merge_line(out.entry(f.clone()).or_default(), g(b"nr"),
                        LineCov { covered: g(b"ci") > 0, conditions: mb + cb, covered_conditions: cb });
                },
                _ => {}
            },
            Event::End(e) if e.name().as_ref().as_bytes() == b"sourcefile" => file = None,
            Event::Eof => break,
            _ => {}
        }
    }
    anyhow::ensure!(saw_root, "JaCoCo 루트 요소 없음");
    Ok(out)
}

pub fn parse_cobertura(xml: &str) -> Result<RawCov> {
    let mut r = Reader::from_str(xml);
    let mut out = RawCov::new();
    let mut file = None::<String>;
    let mut saw_root = false;
    loop {
        match r.read_event().context("Cobertura XML 오류")? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref().as_bytes() {
                b"coverage" => saw_root = true,
                b"class" => file = attr(&e, b"filename"),
                b"line" => if let Some(f) = &file {
                    let line = attr(&e, b"number").and_then(|v| v.parse().ok()).unwrap_or(0);
                    let hits: u64 = attr(&e, b"hits").and_then(|v| v.parse().ok()).unwrap_or(0);
                    let (cc, total) = attr(&e, b"condition-coverage").as_deref().and_then(parse_condition).unwrap_or((0, 0));
                    merge_line(out.entry(f.clone()).or_default(), line,
                        LineCov { covered: hits > 0, conditions: total, covered_conditions: cc });
                },
                _ => {}
            },
            Event::End(e) if e.name().as_ref().as_bytes() == b"class" => file = None,
            Event::Eof => break,
            _ => {}
        }
    }
    anyhow::ensure!(saw_root, "Cobertura 루트 요소 없음");
    Ok(out)
}

fn parse_condition(s: &str) -> Option<(u32, u32)> {
    let inner = s.split('(').nth(1)?.trim_end_matches(')');
    let (a, b) = inner.split_once('/')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

pub fn parse_lcov(text: &str) -> Result<RawCov> {
    let mut out = RawCov::new();
    let mut file = None::<String>;
    let mut branches: BTreeMap<u32, (u32, u32)> = BTreeMap::new();
    for line in text.lines() {
        if let Some(p) = line.strip_prefix("SF:") {
            file = Some(p.trim().replace('\\', "/"));
            branches.clear();
        } else if let Some(p) = line.strip_prefix("DA:") {
            let f = file.as_ref().context("LCOV: SF 없는 DA")?;
            let (n, c) = p.split_once(',').context("LCOV: DA 형식")?;
            let hits: u64 = c.split(',').next().unwrap_or("0").parse().unwrap_or(0);
            merge_line(out.entry(f.clone()).or_default(), n.parse().context("LCOV: 줄 번호")?,
                LineCov { covered: hits > 0, ..Default::default() });
        } else if let Some(p) = line.strip_prefix("BRDA:") {
            let parts: Vec<&str> = p.split(',').collect();
            anyhow::ensure!(parts.len() == 4, "LCOV: BRDA 형식");
            let n: u32 = parts[0].parse().context("LCOV: 줄 번호")?;
            let taken = parts[3] != "-" && parts[3] != "0";
            let e = branches.entry(n).or_default();
            e.0 += 1;
            if taken { e.1 += 1; }
        } else if line.trim() == "end_of_record" {
            if let Some(f) = &file {
                for (n, (t, c)) in &branches {
                    merge_line(out.entry(f.clone()).or_default(), *n,
                        LineCov { covered: false, conditions: *t, covered_conditions: *c });
                }
            }
            file = None;
            branches.clear();
        }
    }
    Ok(out)
}

pub fn parse_go(text: &str) -> Result<RawCov> {
    let mut out = RawCov::new();
    for line in text.lines() {
        if line.starts_with("mode:") || line.trim().is_empty() { continue; }
        let (file, rest) = line.rsplit_once(':').context("Go 커버리지 형식")?;
        let mut it = rest.split_whitespace();
        let range = it.next().context("Go: 범위 없음")?;
        let _stmts = it.next().context("Go: 문장 수 없음")?;
        let count: u64 = it.next().context("Go: 횟수 없음")?.parse().context("Go: 횟수")?;
        let (a, b) = range.split_once(',').context("Go: 범위 형식")?;
        let sl: u32 = a.split('.').next().unwrap_or("0").parse().context("Go: 시작 줄")?;
        let el: u32 = b.split('.').next().unwrap_or("0").parse().context("Go: 끝 줄")?;
        for n in sl..=el {
            merge_line(out.entry(file.to_string()).or_default(), n,
                LineCov { covered: count > 0, ..Default::default() });
        }
    }
    Ok(out)
}

/// 한쪽 경로 전체가 다른 쪽의 접미사일 때 일치한 구성 요소 수. 아니면 0.
/// 파일 이름만 같은 다른 파일(`a/util.py`와 `b/util.py`)에 커버리지가 붙지 않게 한다.
fn match_len(report_path: &str, file: &str) -> usize {
    let rp: Vec<&str> = report_path.split('/').filter(|s| !s.is_empty()).collect();
    let fp: Vec<&str> = file.split('/').collect();
    let n = fp.iter().rev().zip(rp.iter().rev()).take_while(|(a, b)| a == b).count();
    if n == rp.len().min(fp.len()) { n } else { 0 }
}

fn best_len(report_path: &str, files: &[String]) -> usize {
    files.iter().map(|f| match_len(report_path, f)).max().unwrap_or(0)
}

/// 리포트 경로와 저장소 경로를 뒤에서부터 비교해 가장 길게 일치하는 파일을 고른다.
pub fn resolve(report_path: &str, files: &[String]) -> Option<String> {
    let best = best_len(report_path, files);
    if best == 0 { return None; }
    let found: Vec<&String> = files.iter().filter(|f| match_len(report_path, f) == best).collect();
    if found.len() == 1 { Some(found[0].clone()) } else { None }
}

/// `code`는 분석 대상, `excluded`는 `exclude`에 걸린 소스 파일이다. 리포트 경로가 제외된 파일에 더
/// 길게 일치하면 그 파일의 기록이므로 버린다. 테스트 파일의 기록도 지표에 넣지 않는다.
pub fn load(src: &Path, all: &[String], code: &[String], excluded: &[String], tests: &GlobSet, cfg: &CoverageCfg) -> Loaded {
    let mut out = Loaded::default();
    let kinds: [(&Vec<String>, fn(&str) -> Result<RawCov>); 4] = [
        (&cfg.jacoco, parse_jacoco), (&cfg.cobertura, parse_cobertura),
        (&cfg.lcov, parse_lcov), (&cfg.go, parse_go),
    ];
    for (patterns, parser) in kinds {
        let Ok(gs) = crate::fsutil::globset(patterns) else { out.errors.extend(patterns.iter().cloned()); continue };
        for rel in all.iter().filter(|p| gs.is_match(p.as_str())) {
            let parsed = std::fs::read_to_string(src.join(rel)).map_err(anyhow::Error::from).and_then(|t| parser(&t));
            match parsed {
                Err(_) => out.errors.push(rel.clone()),
                Ok(raw) => {
                    out.reports.push(rel.clone());
                    for (path, lines) in raw {
                        if best_len(&path, excluded) > best_len(&path, code) { continue; }
                        match resolve(&path, code) {
                            None => out.unresolved.push(path),
                            Some(f) if tests.is_match(f.as_str()) => {}
                            Some(f) => {
                                let dst = out.map.entry(f).or_default();
                                for (n, c) in lines { merge_line(dst, n, c); }
                            }
                        }
                    }
                }
            }
        }
    }
    for v in [&mut out.reports, &mut out.unresolved, &mut out.errors] { v.sort(); v.dedup(); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    const JACOCO: &str = include_str!("../tests/fixtures/coverage/jacoco.xml");
    const COBERTURA: &str = include_str!("../tests/fixtures/coverage/cobertura.xml");
    const LCOV: &str = include_str!("../tests/fixtures/coverage/lcov.info");
    const GO: &str = include_str!("../tests/fixtures/coverage/go.out");

    fn lc(covered: bool, conditions: u32, covered_conditions: u32) -> LineCov {
        LineCov { covered, conditions, covered_conditions }
    }

    #[test]
    fn jacoco_lines_and_branches() {
        let m = parse_jacoco(JACOCO).unwrap();
        let f = &m["com/acme/A.java"];
        assert_eq!(f[&3], lc(true, 0, 0));
        assert_eq!(f[&4], lc(false, 2, 1));
    }

    #[test]
    fn cobertura_lines_and_conditions() {
        let m = parse_cobertura(COBERTURA).unwrap();
        let f = &m["pkg/a.py"];
        assert_eq!(f[&1], lc(true, 0, 0));
        assert_eq!(f[&2], lc(false, 2, 1));
    }

    #[test]
    fn lcov_lines_and_branches() {
        let m = parse_lcov(LCOV).unwrap();
        let f = &m["src/App.tsx"];
        assert_eq!(f[&1], lc(true, 0, 0));
        assert_eq!(f[&2], lc(false, 2, 1));
    }

    #[test]
    fn go_expands_blocks_and_covered_wins() {
        let m = parse_go(GO).unwrap();
        let f = &m["example.com/m/pkg/a.go"];
        assert!(f[&3].covered && f[&4].covered && f[&5].covered);
        assert!(!f[&6].covered);
    }

    #[test]
    fn resolve_by_longest_suffix_and_rejects_ambiguous() {
        let files = vec!["svc/src/main/java/com/acme/A.java".to_string(), "pkg/a.go".to_string(), "x/u.py".to_string(), "y/u.py".to_string()];
        assert_eq!(resolve("com/acme/A.java", &files).as_deref(), Some("svc/src/main/java/com/acme/A.java"));
        assert_eq!(resolve("example.com/m/pkg/a.go", &files).as_deref(), Some("pkg/a.go"));
        assert_eq!(resolve("u.py", &files), None);
        assert_eq!(resolve("none.py", &files), None);
    }

    #[test]
    fn empty_xml_is_an_error_not_an_empty_report() {
        assert!(parse_jacoco("").is_err());
        assert!(parse_jacoco("<?xml version=\"1.0\"?><other/>").is_err());
        assert!(parse_jacoco("<report name=\"x\"/>").unwrap().is_empty());
    }

    #[test]
    fn one_matching_component_is_not_enough() {
        let files = vec!["a/util.py".to_string()];
        assert_eq!(resolve("b/util.py", &files), None);
        assert_eq!(resolve("/ci/work/a/util.py", &files).as_deref(), Some("a/util.py"));
        assert_eq!(resolve("util.py", &files).as_deref(), Some("a/util.py"));
        let files = vec!["src/x/a.py".to_string()];
        assert_eq!(resolve("build/x/a.py", &files), None);
    }

    fn load_in(d: &Path, code: &[String], excluded: &[String], tests: &[&str]) -> Loaded {
        let all = crate::fsutil::walk(d).unwrap();
        let tests = crate::fsutil::globset(&tests.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        load(d, &all, code, excluded, &tests, &CoverageCfg::default())
    }

    #[test]
    fn test_files_are_left_out_and_not_unresolved() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("coverage")).unwrap();
        std::fs::write(d.path().join("coverage/lcov.info"), "SF:src/a.test.ts\nDA:1,1\nend_of_record\nSF:src/a.ts\nDA:1,0\nend_of_record\n").unwrap();
        let code = vec!["src/a.test.ts".to_string(), "src/a.ts".to_string()];
        let l = load_in(d.path(), &code, &[], &["**/*.test.*"]);
        assert_eq!(l.map.keys().collect::<Vec<_>>(), vec!["src/a.ts"]);
        assert_eq!(l.unresolved, Vec::<String>::new());
    }

    #[test]
    fn excluded_files_do_not_absorb_coverage() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("coverage")).unwrap();
        std::fs::write(d.path().join("coverage/lcov.info"), "SF:vendor/x/a.js\nDA:1,1\nend_of_record\nSF:a.js\nDA:1,0\nend_of_record\n").unwrap();
        let l = load_in(d.path(), &["a.js".to_string()], &["vendor/x/a.js".to_string()], &[]);
        assert!(!l.map["a.js"][&1].covered);
        assert_eq!(l.unresolved, Vec::<String>::new());
    }

    #[test]
    fn load_merges_finds_reports_and_records_errors() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("coverage")).unwrap();
        std::fs::write(d.path().join("coverage/lcov.info"), LCOV).unwrap();
        std::fs::write(d.path().join("coverage.xml"), "<not closed").unwrap();
        let files = vec!["src/App.tsx".to_string()];
        let l = load_in(d.path(), &files, &[], &[]);
        assert_eq!(l.reports, vec!["coverage/lcov.info".to_string()]);
        assert_eq!(l.errors, vec!["coverage.xml".to_string()]);
        assert!(l.map.contains_key("src/App.tsx"));
    }

    #[test]
    fn unresolved_paths_are_reported() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("coverage")).unwrap();
        std::fs::write(d.path().join("coverage/lcov.info"), LCOV).unwrap();
        let l = load_in(d.path(), &[], &[], &[]);
        assert_eq!(l.unresolved, vec!["src/App.tsx".to_string()]);
    }
}
