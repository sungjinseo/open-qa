# Gitea 기반 룰셋과 프로젝트 품질 게이트 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 룰셋과 프로젝트별 품질 게이트를 Gitea 설정 저장소 파일로 관리하고, Tekton Task(`fetch-config → scan → publish`)로 실행해 실패 항목을 Gitea 이슈로 등록한다.

**Architecture:** 설정 형식·검증·규칙 카탈로그를 라이브러리 모듈(`settings`, `catalog`)로 만들고 CLI 명령들이 공유한다. `fetch-config`가 설정과(필요하면) 신규 코드 변경 줄을 Gitea API로 받아 작업 디렉터리에 쓰고, `scan`은 그 설정으로 규칙을 골라 지표를 계산하며, `publish`가 핫스팟 검토 상태를 반영해 게이트를 판정하고 이슈를 동기화한다.

**Tech Stack:** Rust 2021, clap, serde, toml, serde_json, serde_yaml(규칙 메타데이터 읽기), ureq 3(rustls), base64, Opengrep v1.33.1, Gitea 1.26.4, Tekton `tekton.dev/v1`

**Spec:** `docs/superpowers/specs/2026-11-09-gitea-rulesets-design.md` (기반 설계 `docs/superpowers/specs/2026-11-06-qualgate-design.md`)

## Global Constraints

- 브랜치 `feat/ruleset-console`에서 작업한다. 원격 push는 컨트롤러가 한다.
- 빌드와 테스트는 항상 `bash scripts/cargo.sh ...`(Docker)로 실행한다. 로컬 Rust 없음.
- 종료 코드: `scan` 0 성공 / 2 오류. `publish` 0 통과 / 1 게이트 실패 / 2 오류. `fetch-config`, `validate-config`, `init-config`, `catalog` 0 / 2.
- 측정하지 못한 값은 `None`(측정 안 됨)이며 그 조건은 건너뛴다. 분석 실패 파일의 이슈는 닫지 않는다. 기존 안전 규칙은 약하게 만들지 않는다.
- 자격 증명(토큰, 비밀번호, `.git-credentials` 내용)은 로그·오류·파일·커밋 어디에도 쓰지 않는다.
- 설정 파일은 모두 `schema = 1`, 모르는 키는 오류. 결과 파일은 `schema = 2`.
- 규칙 디렉터리: `java`, `kotlin`, `python`, `go`, `ts`, `react`, `vue`. 대상 파일 매핑은 스펙 4.1 표를 따른다.
- 게이트 지표 7개와 연산자 `gt`/`lt`만 지원한다(스펙 3.5). 조건은 "참이면 실패".
- `QUALGATE_SSL_VERIFY=false`일 때만 TLS 검증을 끄고 매 실행 경고를 낸다. 기본은 검증 켬. `QUALGATE_CA_FILE`은 시스템 인증서에 더해 신뢰한다.
- 커밋 메시지 끝에는 하네스가 알려 주는 Co-Authored-By 줄을 붙인다.
- 스캐너 이미지 150MB 이하 유지.

## 파일 구조

```text
src/settings/mod.rs        설정 디렉터리 로드: load_dir(), Settings
src/settings/global.rs     qualgate.toml
src/settings/ruleset.rs    rulesets/<name>.toml
src/settings/project.rs    projects/<owner>/<repo>.toml, GateCondition, Metric, Op
src/settings/validate.rs   교차 검증(룰셋 존재, 규칙 ID, 중복 지표, 값 범위), ConfigError
src/catalog.rs             rules/ 읽기, RuleInfo, Kind, catalog JSON
src/select.rs              실행 디렉터리 계산, 활성 규칙 임시 디렉터리, 오버라이드 적용
src/gatecalc.rs            지표 계산 묶음과 조건 판정(gate.rs의 계산 함수 재사용)
src/httpcfg.rs             ureq Agent 생성(CA 파일, SSL 검증), 자격 증명 해석
src/gitea.rs               (수정) 파일 API, 커밋 목록, diff, 이슈 API
src/fetch.rs               fetch-config 구현
src/newcode.rs             (수정) newcode.json 읽기·쓰기, diff 파싱 재사용
src/scan.rs                (수정) 설정 디렉터리 기반 스캔, 게이트 판정 제거
src/sync.rs, render.rs     (수정) 핫스팟, safe 라벨, 룰셋 제외 종료, 요약
src/main.rs                (수정) 명령: fetch-config, scan, publish, validate-config, init-config, catalog
src/config.rs              (수정) .qualgate.toml 로드 제거, 섹션 타입(ScanCfg 등)만 남김
deploy/tekton/task-qualgate.yaml
deploy/tekton/pipeline-patch.md
docs/config-format.md
docs/verification-gitea.md
tests/cli.rs, tests/gitea_it.sh, tests/e2e/run.sh   (수정)
tests/fixtures/config/...  설정 저장소 픽스처
```

---

### Task 1: 대상 Gitea 1.26.4 API 확인

**Files:**
- Create: `docs/verification-gitea.md`, `tests/gitea_probe.sh`

**Interfaces:**
- Produces: 이후 태스크가 쓰는 실측값(파일 API 응답 형식, 커밋 목록 `until`, diff 엔드포인트, 이슈 라벨 응답).

로컬 `gitea/gitea:1.26.4` 컨테이너로 확인한다(대상과 같은 버전). 실제 대상 Gitea 확인은 Task 12에서 컨트롤러가 한다.

- [ ] **Step 1: 프로브 스크립트 작성**

`tests/gitea_probe.sh`: 기존 `tests/gitea_it.sh`의 컨테이너 기동·계정·토큰 생성 부분을 그대로 써서 `gitea/gitea:1.26.4`를 띄우고, 저장소 `qg/cfg`에 `qualgate.toml`, `rulesets/a.toml`, `projects/qg/app.toml` 파일을 API로 만든 뒤 다음을 호출해 결과를 출력한다.

```bash
api GET "/repos/qg/cfg/contents/qualgate.toml?ref=main"          # content(base64), sha, encoding
api GET "/repos/qg/cfg/contents/projects/qg/app.toml?ref=main"
api GET "/repos/qg/cfg/contents/rulesets/none.toml?ref=main"     # 404 확인
api GET "/repos/qg/cfg/commits?sha=main&limit=1"                  # 설정 커밋 sha
# 대상 저장소 qg/app 에 날짜를 지정한 커밋 3개를 만든 뒤(git push, GIT_COMMITTER_DATE 사용)
api GET "/repos/qg/app/commits?sha=main&until=<RFC3339>&limit=1"  # 기준 커밋
api GET "/repos/qg/app/compare/<base>...<head>"                   # files 목록, patch 포함 여부
curl "/api/v1/repos/qg/app/git/commits/<head>.diff"               # 단일 커밋 diff
curl "<root>/qg/app/compare/<base>...<head>.diff"                 # 웹 경로 diff(토큰 인증 동작 여부)
```

- [ ] **Step 2: 실행하고 결과 기록**

Run: `bash tests/gitea_probe.sh`

`docs/verification-gitea.md`에 표로 적는다: 각 엔드포인트의 상태 코드, 응답 핵심 필드, `until`이 커밋 날짜 기준으로 동작하는지, 범위 diff를 한 번에 받는 방법(받을 수 있으면 그 경로와 형식, 없으면 대안), 대용량 diff 제한 여부. Task 8은 이 결과를 따른다. 범위 diff를 받을 방법이 전혀 없으면 멈추고 컨트롤러에 보고한다.

- [ ] **Step 3: 커밋**

```bash
git add tests/gitea_probe.sh docs/verification-gitea.md
git commit -m "test: probe Gitea 1.26.4 APIs needed for config and new code"
```

---

### Task 2: 설정 형식과 검증 (`settings`)

**Files:**
- Create: `src/settings/mod.rs`, `global.rs`, `ruleset.rs`, `project.rs`, `validate.rs`, `tests/fixtures/config/good/**`, `tests/fixtures/config/bad/**`
- Modify: `src/lib.rs`, `src/config.rs`

**Interfaces:**
- Consumes: `config::{ScanCfg, CoverageCfg, DupCfg, PublishCfg}`(기존 섹션 타입과 기본값을 그대로 재사용)
- Produces:

```rust
pub enum Metric { NewIssues, NewHotspotsReviewed, NewCoverage, NewDuplicatedLines, Issues, Coverage, DuplicatedLines }
// serde 이름: new_issues, new_hotspots_reviewed, new_coverage, new_duplicated_lines, issues, coverage, duplicated_lines
pub enum Op { Gt, Lt }                       // "gt", "lt"
pub struct GateCondition { pub metric: Metric, pub op: Op, pub value: f64 }
pub struct Global { pub schema: u32, pub default_ruleset: String, pub default_gate: Vec<GateCondition> }
pub struct LanguageSel { pub enabled: bool, pub disabled_rules: Vec<String> }
pub struct RuleOverride { pub severity: Option<Severity>, pub kind: Option<Kind> }
pub struct Ruleset { pub schema: u32, pub name: String, pub description: String,
                     pub languages: BTreeMap<String, LanguageSel>, pub overrides: BTreeMap<String, RuleOverride> }
pub struct Project { pub schema: u32, pub repo: String, pub ruleset: String, pub languages: Option<Vec<String>>,
                     pub new_code_days: u32 /*기본 30*/, pub gate: Vec<GateCondition>,
                     pub scan: ScanCfg, pub coverage: CoverageCfg, pub duplication: DupCfg, pub publish: PublishCfg }
pub struct ConfigError { pub file: String, pub message: String }   // Display: "<file>: <message>"
pub struct Resolved { pub global: Global, pub ruleset: Ruleset, pub project: Project, pub registered: bool }
pub fn load_dir_all(dir: &Path, catalog: &Catalog) -> Result<(), Vec<ConfigError>>;          // 전체 검증
pub fn resolve(dir: &Path, repo: &str, catalog: &Catalog) -> Result<Resolved, Vec<ConfigError>>; // 한 저장소용
pub const DEFAULT_GATE: [(Metric, Op, f64); 4];  // new_issues gt 0, new_hotspots_reviewed lt 100, new_coverage lt 80, new_duplicated_lines gt 3
```

`Kind`는 Task 3에서 `catalog.rs`에 정의한다. 이 태스크는 Task 3보다 먼저 하므로 `Kind`와 `Catalog`를 이 태스크에서 최소 형태로 먼저 만든다: `src/catalog.rs`에 `pub enum Kind { Issue, Hotspot }`(serde "issue"/"hotspot", Default=Issue)와 `pub struct Catalog { pub rules: BTreeMap<String, RuleInfo> }`, `pub struct RuleInfo { pub id: String, pub dir: String, pub severity: Severity, pub kind: Kind, pub category: String, pub mois: Option<String>, pub cwe: Option<String>, pub title: String }`, `impl Catalog { pub fn from_rules(v: Vec<RuleInfo>) -> Catalog }`. 디렉터리 읽기는 Task 3.

미등록 저장소는 `projects/<repo>.toml`이 없을 때: `ruleset = global.default_ruleset`, `gate = global.default_gate`(비어 있으면 `DEFAULT_GATE`), 나머지 기본값, `registered = false`. `default_ruleset`이 빈 문자열이면 `ConfigError`.

- [ ] **Step 1: 실패하는 테스트 작성** (`src/settings/validate.rs`의 `#[cfg(test)]`, 픽스처 사용)

테스트 케이스(각각 하나의 `#[test]`):
1. `good` 픽스처 전체 검증 통과.
2. 모르는 키(`[languages.java] bogus = 1`) → 오류 파일 `rulesets/x.toml`.
3. 룰셋 파일 이름과 `name` 불일치 → 오류.
4. 룰셋 이름 규칙 위반(`Bad_Name`) → 오류.
5. 프로젝트 경로와 `repo` 불일치 → 오류.
6. 프로젝트가 없는 룰셋 참조 → 오류.
7. `disabled_rules`에 카탈로그에 없는 ID → 오류. 다른 디렉터리 소속 ID(`[languages.java] disabled_rules = ["python.eval"]`) → 오류.
8. `overrides`의 없는 ID → 오류.
9. 같은 지표 두 번 → 오류. 비율 지표 값 101 → 오류. 건수 지표 값 1.5 → 오류. 음수 → 오류.
10. `languages`에 모르는 디렉터리(`rust`) → 오류(룰셋 키와 프로젝트 목록 모두).
11. `schema = 2` → 오류.
12. `resolve` 미등록 저장소 → 기본 룰셋·기본 게이트, `registered == false`. `default_ruleset = ""`이면 오류.
13. `resolve` 등록 저장소 → 프로젝트 값 그대로, 생략 섹션은 기본값(`duplication.block_lines == 10`).
14. 오류는 여러 개를 한 번에 모두 돌려준다(픽스처 `bad/multi`에 오류 3개 → `Vec` 길이 3).

검증용 카탈로그는 테스트에서 `Catalog::from_rules`로 `java.system-out`, `java.catch-throwable`, `python.eval` 세 항목만 만들어 쓴다.

- [ ] **Step 2: 실패 확인**

Run: `bash scripts/cargo.sh test --lib settings`
Expected: 컴파일 오류 또는 FAIL

- [ ] **Step 3: 구현**

serde `deny_unknown_fields`, 섹션은 `#[serde(default)]`. 비율 지표: `new_hotspots_reviewed`, `new_coverage`, `new_duplicated_lines`, `coverage`, `duplicated_lines`(0~100). 건수 지표: `new_issues`, `issues`(0 이상 정수). TOML 파싱 오류 메시지는 기존 `config.rs`처럼 줄·열만 쓰고 원문을 쓰지 않는다. 심볼릭 링크 설정 파일은 오류(기존 규칙). `config.rs`에서는 `Config::load`와 `.qualgate.toml` 관련 코드를 지우지 말고 이 태스크에서는 그대로 둔다(Task 5에서 제거).

- [ ] **Step 4: 통과 확인**

Run: `bash scripts/cargo.sh test --lib settings`
Expected: PASS (14개 이상)

- [ ] **Step 5: 커밋**

```bash
git commit -am "feat(settings): config repository format and validation"   # 새 파일은 git add로 먼저 추가
```

---

### Task 3: 규칙 카탈로그와 이슈 종류

**Files:**
- Modify: `src/catalog.rs`, `rules/**/*.yaml`(일부에 `qualgate_kind: hotspot`), `tests/rules_pack.rs`, `docs/mois49.md`, `Cargo.toml`(serde_yaml)

**Interfaces:**
- Consumes: Task 2의 `Kind`, `RuleInfo`, `Catalog`
- Produces: `pub fn Catalog::load(rules_root: &Path) -> anyhow::Result<Catalog>`, `pub fn Catalog::to_json(&self) -> serde_json::Value`(배열, 항목 필드는 `RuleInfo` 필드명 그대로), `pub fn Catalog::dir_of(&self, id: &str) -> Option<&str>`

- [ ] **Step 1: 실패하는 테스트**

`src/catalog.rs` 테스트: 임시 디렉터리에 규칙 YAML 2개(하나는 `qualgate_kind: hotspot`, 하나는 없음)를 쓰고 `Catalog::load` → 종류가 `Hotspot`, `Issue`. 규칙 ID가 `<dir>.<file>`가 아니면 오류. `rules/tests/`는 읽지 않음. 실제 `rules/`를 로드했을 때 규칙 수가 0보다 크고 모든 항목의 `dir`이 7개 디렉터리 중 하나.

- [ ] **Step 2: 실패 확인**

Run: `bash scripts/cargo.sh test --lib catalog`

- [ ] **Step 3: 구현**

`serde_yaml`로 `rules[0]`의 `id`, `message`(첫 줄 → `title`), `metadata.qualgate_severity`, `qualgate_category`, `qualgate_mois`, `cwe`, `qualgate_kind`를 읽는다.

- [ ] **Step 4: 핫스팟 기본값 지정**

다음 규칙 YAML의 `metadata`에 `qualgate_kind: hotspot`을 추가한다(파일이 있는 것만): 모든 디렉터리의 `hardcoded-secret`, `hardcoded-password`, `comment-secret`, `insecure-random`, `random-for-security`, `math-rand`, `cookie-flags`, `cookie-insecure`, `document-cookie-insecure`, `cors-wildcard`, `iframe-no-sandbox`, `dns-lookup-decision`, `dns-based-decision`, `debug-leftover`, `ts.prototype-pollution`, `java.catch-throwable`, `java.throws-generic-exception`. `ls rules/*/ | grep` 로 실제 파일을 확인해 적용하고, 적용 목록을 `docs/mois49.md`에 "핫스팟 기본" 열 또는 표시로 남긴다.

`tests/rules_pack.rs`에 검사 추가: `qualgate_kind`가 있으면 값이 `issue` 또는 `hotspot`.

- [ ] **Step 5: 통과 확인과 커밋**

Run: `bash scripts/cargo.sh test --lib catalog` (rules_pack 전체는 Task 11에서)

```bash
git add -A src/catalog.rs rules docs/mois49.md tests/rules_pack.rs Cargo.toml Cargo.lock
git commit -m "feat(catalog): rule catalog with issue/hotspot kind"
```

---

### Task 4: `validate-config`, `init-config`, `catalog` 명령과 형식 문서

**Files:**
- Modify: `src/main.rs`
- Create: `src/initcfg.rs`, `docs/config-format.md`
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `settings::load_dir_all`, `Catalog::load`, `Catalog::to_json`
- Produces: CLI `qualgate validate-config --config-dir <d>`, `qualgate init-config --out <d>`, `qualgate catalog --json`. 규칙 경로는 기존처럼 `QUALGATE_RULES`(기본 `/rules`).

- [ ] **Step 1: 실패하는 CLI 테스트** (`tests/cli.rs`)

1. `init-config --out <tmp>` 후 `validate-config --config-dir <tmp>` 종료 코드 0.
2. 생성된 기본 룰셋이 7개 디렉터리를 모두 켬, `qualgate.toml`의 `default_ruleset`이 그 룰셋, `default_gate`가 `DEFAULT_GATE` 4개.
3. `init-config`는 비어 있지 않은 디렉터리에는 쓰지 않고 종료 코드 2.
4. 오류 픽스처로 `validate-config` → 종료 코드 2, stderr에 각 오류의 파일 경로가 나옴.
5. `catalog --json` → JSON 배열, 항목에 `id`, `dir`, `kind` 필드, `QUALGATE_RULES`의 규칙 수와 같음.

- [ ] **Step 2: 실패 확인** `bash scripts/cargo.sh test --test cli`

- [ ] **Step 3: 구현** 기존 명령은 이 태스크에서 건드리지 않는다. `init-config`는 README(설정 저장소용, 파일 구조와 편집 방법 요약)도 쓴다.

- [ ] **Step 4: `docs/config-format.md` 작성**

스펙 3장(파일 구조, 세 파일 형식, 지표 표, 검증 규칙 전부)과 4.1(디렉터리·대상 파일), 핫스팟·`qualgate:safe` 의미, 예시 파일을 사람이 읽을 수 있게 정리한다. "나중에 만들 화면은 이 문서, `validate-config`, `catalog --json`으로 구현한다"를 명시한다.

- [ ] **Step 5: 통과 확인과 커밋**

```bash
git commit -m "feat(cli): validate-config, init-config and catalog commands"
```

---

### Task 5: 설정 디렉터리 기반 스캔과 규칙 선택

**Files:**
- Create: `src/select.rs`
- Modify: `src/scan.rs`, `src/model.rs`, `src/rules.rs`, `src/config.rs`, `src/main.rs`, `src/gate.rs`, `src/newcode.rs`, `tests/cli.rs`

**Interfaces:**
- Consumes: `settings::{resolve, Resolved}`, `Catalog`, 기존 `rules::run`, `newcode::changed`
- Produces:

```rust
// select.rs
pub const DIRS: [&str; 7] = ["java","kotlin","python","go","ts","react","vue"];
pub fn dirs_for_file(path: &str) -> &'static [&'static str];        // 스펙 4.1, 예: "a.tsx" -> ["ts","react"], "a.vue" -> ["ts","vue"]
pub fn active_dirs(rs: &Ruleset, project_langs: Option<&[String]>, files: &[String]) -> Vec<String>;
pub struct Selection { pub dirs: Vec<String>, pub rule_ids: Vec<String>, pub rules_dir: tempfile::TempDir }
pub fn select(rules_root: &Path, catalog: &Catalog, rs: &Ruleset, dirs: &[String]) -> anyhow::Result<Selection>;
pub fn apply_overrides(findings: &mut [Finding], catalog: &Catalog, rs: &Ruleset);
// model.rs
pub struct Finding { ...기존 필드..., pub kind: Kind }
pub struct ScanResult {
    pub schema: u32,                 // 2
    pub commit: String, pub branch: Option<String>,
    pub findings: Vec<Finding>, pub clones: Vec<CloneGroup>,
    pub metrics: Metrics,            // Task 6에서 필드 확장
    pub coverage_status: CoverageStatus, pub unanalyzed: Vec<String>,
    pub publish: PublishConfig,
    pub active_dirs: Vec<String>, pub active_rules: Vec<String>,
    pub gate_conditions: Vec<GateCondition>,
    pub config_source: serde_json::Value,     // source.json 내용
    pub new_hotspots: Vec<Finding>,           // 신규 코드 줄의 핫스팟
    pub new_code_measured: bool,
}
// scan.rs
pub fn scan(src: &Path, config_dir: &Path, repo: &str, rules_root: &Path, opengrep: &str) -> anyhow::Result<ScanResult>;
// newcode.rs
pub struct NewCodeFile { pub base: String, pub head: String, pub files: BTreeMap<String, Vec<u32>> }
pub fn read_newcode_json(path: &Path) -> anyhow::Result<Option<NewCodeFile>>;   // 파일 없으면 Ok(None)
```

`ScanResult`에서 `gate: GateResult`는 제거한다. `scan`은 게이트를 판정하지 않는다. `repo`는 `config_dir/source.json`의 `repo` 값을 쓴다(`scan` CLI에 `--repo` 인자를 따로 받지 않는다; `source.json`이 없으면 오류).

신규 코드: `newcode::changed(src, project.new_code_days)`가 `Some`이면 그것, `None`이면 `config_dir/newcode.json`이 있고 그 `head`가 소스 HEAD와 같을 때만 그것, 아니면 측정 안 됨.

- [ ] **Step 1: `select.rs` 실패 테스트**

1. `dirs_for_file`: `.java`→java, `.kts`→kotlin, `.py`→python, `.go`→go, `.mjs`/`.ts`→ts, `.jsx`/`.tsx`→ts,react, `.vue`→ts,vue, `.md`→빈 목록.
2. `active_dirs`: 룰셋 java·ts·react 켬, 파일 `a.java`,`b.tsx` → `[java, react, ts]`(정렬). 프로젝트 언어 `["java"]` → `[java]`. 룰셋에서 꺼진 디렉터리는 프로젝트가 지정해도 안 들어감.
3. `select`: 실제 `rules/`와 룰셋(java 켬, `disabled_rules=["java.system-out"]`) → `rule_ids`에 `java.system-out` 없음, `rules_dir`에 그 파일 없음, 다른 디렉터리 파일 없음.
4. `apply_overrides`: `java.catch-throwable` 오버라이드 `severity=low, kind=hotspot` → 결과 위반의 등급·종류 변경. 오버라이드 없는 규칙은 카탈로그 기본 종류.

- [ ] **Step 2: 실패 확인** `bash scripts/cargo.sh test --lib select`

- [ ] **Step 3: `select.rs` 구현 후 통과 확인**

`rules_dir`에는 `<dir>/<file>.yaml`을 복사한다. `rules::run`의 `rule_paths`에 이 디렉터리 하나만 넘긴다. 실행할 디렉터리가 하나도 없으면 Opengrep을 실행하지 않고 위반 0건(경고 출력).

- [ ] **Step 4: `scan` 실패 테스트** (`tests/cli.rs`, 기존 scan 테스트를 새 형식으로 교체)

픽스처 설정 디렉터리 `tests/fixtures/config/scan/`(source.json, qualgate.toml, rulesets/std.toml, projects/o/r.toml)와 임시 git 저장소로:
1. `scan --src <repo> --config <cfg> --out <out>` 종료 코드 0, `result.json`의 `schema == 2`, `active_rules`에 켠 규칙만, `findings[*].kind` 존재.
2. 룰셋에서 끈 규칙(`python.eval`을 `disabled_rules`)은 위반이 나오지 않음.
3. 오버라이드로 `kind = "hotspot"`이 된 규칙의 위반이 `new_hotspots`에 들어감(신규 줄일 때).
4. 저장소에 `.qualgate.toml`이 있어도 무시되고 stderr에 경고.
5. `source.json` 없으면 종료 코드 2.
6. 얕은 저장소(`git clone --depth 1`)에서 `newcode.json`(head가 HEAD와 같음)이 있으면 `new_code_measured == true`, head가 다르면 `false`.

- [ ] **Step 5: 구현**

`main.rs`: `Scan { src, config, out }`. `Run` 명령 제거. `config.rs`: `Config::load`, `GateCfg`, `blocking_severities` 제거. 기존 `gate::evaluate`와 `blocking_on_changed`는 제거(Task 6에서 대체). 컴파일이 깨지는 `publish` 쪽은 이 태스크에서 `result.gate` 참조를 지우고 "게이트 판정은 Task 6/9" 상태로 컴파일만 되게 둔다: `publish`는 동기화만 하고 종료 코드 0. 결과를 읽을 때 `schema != 2`면 오류.

- [ ] **Step 6: 통과 확인**

Run: `bash scripts/cargo.sh test --lib` 와 `bash scripts/cargo.sh test --test cli`
기존 테스트 중 제거된 기능(`run`, `blocking_severities`, `.qualgate.toml` 로드)을 검증하던 것은 새 동작 테스트로 바꾸거나 지운다. 지운 테스트 이름을 보고서에 적는다.

- [ ] **Step 7: 커밋**

```bash
git commit -m "feat(scan): scan from config directory with ruleset-based rule selection"
```

---

### Task 6: 게이트 지표 계산과 조건 판정

**Files:**
- Create: `src/gatecalc.rs`
- Modify: `src/model.rs`(Metrics), `src/scan.rs`, `src/gate.rs`

**Interfaces:**
- Consumes: Task 5 `ScanResult`, 기존 `gate::{coverage_pct, duplication}`
- Produces:

```rust
pub struct Metrics {
    pub new_issues: Option<u64>, pub issues: u64,
    pub new_hotspots: Option<u64>,
    pub new_coverage: Option<f64>, pub coverage: Option<f64>,
    pub new_duplicated_lines: Option<f64>, pub duplicated_lines: Option<f64>,
}
pub struct Condition { pub metric: Metric, pub op: Op, pub value: f64, pub actual: Option<f64>, pub status: ConditionStatus }
pub struct GateResult { pub passed: bool, pub conditions: Vec<Condition> }
pub fn hotspots_reviewed_pct(new_hotspots: u64, reviewed: u64) -> f64;   // new_hotspots == 0 -> 100.0
pub fn evaluate(conds: &[GateCondition], m: &Metrics, reviewed_pct: Option<f64>, coverage_required_missing: bool) -> GateResult;
```

`new_issues`는 신규 코드 줄의 `kind == Issue` 위반 수, `issues`는 전체 `Issue` 위반 수, `new_hotspots`는 신규 줄의 `Hotspot` 수. 신규 코드를 측정하지 못하면 `new_*`는 `None`.

- [ ] **Step 1: 실패 테스트** (`gatecalc.rs`)

1. `gt`: actual 1 > 0 → Failed, 0 → Passed. `lt`: 79.9 < 80 → Failed, 80 → Passed(경계는 통과).
2. actual `None` → Skipped, 전체 `passed == true`.
3. `coverage_required_missing == true`면 `new_coverage`·`coverage` 조건은 Failed.
4. `hotspots_reviewed_pct(0,0) == 100.0`, `(4,1) == 25.0`.
5. `new_hotspots_reviewed` 조건은 `reviewed_pct` 인자를 쓴다(None이면 Skipped).
6. 조건 목록이 비어 있으면 `passed == true`, `conditions` 빈 목록.

- [ ] **Step 2: 실패 확인** `bash scripts/cargo.sh test --lib gatecalc`

- [ ] **Step 3: 구현** `scan.rs`가 새 `Metrics`를 채운다. 이전 `Metrics` 필드 사용처는 모두 새 이름으로 바꾼다.

- [ ] **Step 4: 통과 확인과 커밋**

Run: `bash scripts/cargo.sh test --lib` 와 `--test cli`

```bash
git commit -m "feat(gate): metric catalog and condition evaluation"
```

---

### Task 7: HTTP 설정과 Gitea 클라이언트 확장

**Files:**
- Create: `src/httpcfg.rs`
- Modify: `src/gitea.rs`, `Cargo.toml`

**Interfaces:**
- Produces:

```rust
pub enum Auth { Token(String), Basic { user: String, pass: String } }   // Debug는 값을 가린다
pub fn resolve_auth(gitea_url: &str) -> anyhow::Result<Auth>;
// GITEA_TOKEN 우선, 없으면 QUALGATE_GIT_CREDENTIALS 파일에서 GITEA_URL과 호스트(포트 포함)가 같은 줄 "https://user:pass@host" (user/pass는 퍼센트 디코딩)
pub fn agent() -> anyhow::Result<ureq::Agent>;
// QUALGATE_CA_FILE: 시스템 루트 + PEM 인증서들. QUALGATE_SSL_VERIFY=false: 검증 끔 + stderr 경고. 잘못된 값("no")은 오류.
// gitea.rs
impl Client { pub fn new(base: &str, repo: &str, auth: Auth, agent: ureq::Agent) -> Client }
pub fn get_file(&mut self, repo: &str, path: &str, git_ref: &str) -> Result<Option<(Vec<u8>, String /*sha*/)>>;  // 404 -> None
pub fn latest_commit(&mut self, repo: &str, git_ref: &str) -> Result<String>;
pub fn commit_before(&mut self, repo: &str, branch: &str, until_rfc3339: &str) -> Result<Option<String>>;
pub fn diff(&mut self, repo: &str, base: &str, head: &str) -> Result<String>;   // unified diff, Task 1 결과의 경로 사용
```

기존 `IssueApi` 구현과 호출부(`main.rs`)는 새 생성자로 바꾼다. 오류 메시지·Debug 출력에 자격 증명이 나오지 않아야 한다.

- [ ] **Step 1: 실패 테스트** (`httpcfg.rs`)

1. `.git-credentials`에 다른 호스트 줄과 맞는 호스트 줄이 있을 때 맞는 줄의 사용자·비밀번호를 고름. `p%40ss` → `p@ss`.
2. `GITEA_TOKEN`이 있으면 `Token`.
3. 둘 다 없으면 오류이고, 오류 문자열에 파일 내용이 들어가지 않음.
4. `format!("{:?}", auth)`에 비밀번호가 없음.
5. `QUALGATE_SSL_VERIFY`: `true`/`false`/없음 허용, `no` → 오류.
6. `QUALGATE_CA_FILE`이 PEM이 아니면 오류. 자체 서명 CA로 만든 인증서를 쓰는 로컬 TLS 서버(테스트에서 `rcgen`으로 생성, dev-dependency)에 CA 파일 지정 시 연결 성공, 미지정 시 실패, `SSL_VERIFY=false`면 성공.

환경 변수를 바꾸는 테스트는 직렬로 실행되게 한다(공용 Mutex).

- [ ] **Step 2~4: 실패 확인, 구현, 통과**

Run: `bash scripts/cargo.sh test --lib httpcfg` 와 `--lib gitea`
`gitea.rs`의 새 함수는 로컬 Gitea 통합 테스트(Task 11)에서 검증한다. 여기서는 URL 조립과 응답 파싱을 순수 함수로 분리해 단위 테스트한다(파일 API의 base64 디코딩, 404 → None).

- [ ] **Step 5: 커밋**

```bash
git commit -m "feat(gitea): CA file, optional TLS skip, git-credentials auth and file/diff APIs"
```

---

### Task 8: `fetch-config`

**Files:**
- Create: `src/fetch.rs`
- Modify: `src/main.rs`, `src/newcode.rs`, `tests/cli.rs`

**Interfaces:**
- Consumes: `httpcfg::{resolve_auth, agent}`, `gitea::Client::{get_file, latest_commit, commit_before, diff}`, `settings::resolve`, `Catalog::load`, `newcode::parse_diff`
- Produces: CLI `qualgate fetch-config --repo <o/r> --branch <b> [--commit <sha>] --config-repo <o/r> [--ref main] --out <dir>`. 출력: `qualgate.toml`, `rulesets/<name>.toml`, `projects/<o>/<r>.toml`(등록된 경우), `source.json`(`{"config_repo","ref","config_commit","repo","branch","registered"}`), `newcode.json`(`--commit`이 있고 기준 커밋을 찾은 경우).

```rust
pub trait ConfigSource { fn get(&mut self, path: &str) -> Result<Option<Vec<u8>>>; fn commit(&mut self) -> Result<String>; }
pub trait HistorySource { fn commit_before(&mut self, until: &str) -> Result<Option<String>>; fn diff(&mut self, base: &str, head: &str) -> Result<String>; }
pub fn fetch(cfg: &mut dyn ConfigSource, hist: Option<&mut dyn HistorySource>, req: &FetchRequest, catalog: &Catalog, out: &Path) -> Result<()>;
pub struct FetchRequest { pub repo: String, pub branch: String, pub commit: Option<String>, pub config_repo: String, pub git_ref: String, pub now: std::time::SystemTime }
```

시간은 `std::time::SystemTime`으로 받고 RFC3339 변환은 직접 구현하거나 `time` 크레이트를 쓴다(하나만 추가). 기준 시각 = `now - new_code_days`.

- [ ] **Step 1: 실패 테스트** (`fetch.rs`, 가짜 `ConfigSource`/`HistorySource`)

1. 등록 저장소: 세 파일과 `source.json`(`registered: true`, `config_commit`) 생성.
2. 미등록 저장소: `qualgate.toml`과 기본 룰셋만, `registered: false`.
3. 검증 실패(없는 규칙 ID) → 오류, `out`에 아무것도 남기지 않음.
4. 미등록이고 `default_ruleset` 빈 문자열 → 오류.
5. `--commit` 있음: 가짜 히스토리가 기준 커밋과 diff를 주면 `newcode.json`의 `files`가 diff의 추가 줄과 같음. `commit_before`가 `None`이면 `newcode.json` 없음(경고).
6. 히스토리 API 오류는 경고만 하고 `newcode.json` 없이 성공(신규 코드는 측정 안 됨이 됨).
7. `out`이 비어 있지 않으면 오류.

- [ ] **Step 2~4: 실패 확인, 구현(`Client`에 두 트레이트 구현), 통과**

Run: `bash scripts/cargo.sh test --lib fetch`

- [ ] **Step 5: 커밋**

```bash
git commit -m "feat(fetch): fetch-config with API-based new code for shallow clones"
```

---

### Task 9: `publish` 게이트 판정, 핫스팟, 룰셋 제외 처리

**Files:**
- Modify: `src/sync.rs`, `src/render.rs`, `src/main.rs`, `tests/cli.rs`

**Interfaces:**
- Consumes: `gatecalc::{evaluate, hotspots_reviewed_pct}`, `ScanResult`(schema 2), `IssueApi`
- Produces:

```rust
pub const SAFE_LABEL: &str = "qualgate:safe";
pub struct PublishOutcome { pub gate: GateResult, pub report: Option<SyncReport> }  // 비기본 브랜치면 report None
pub fn publish(api: &mut dyn IssueApi, r: &ScanResult, base_url: &str, repo: &str, sync_issues: bool) -> Result<PublishOutcome>;
```

CLI: `qualgate publish --repo <o/r> --out <dir> [--gate-result <file>] [--fail-on-gate true|false]`(기본 true). 게이트 결과 파일에 `passed` 또는 `failed`(줄바꿈 없음). 종료 코드: 통과 0, 실패 1(`--fail-on-gate false`면 0), 오류 2.

- [ ] **Step 1: 실패 테스트** (`sync.rs`, 기존 가짜 API 확장)

1. 핫스팟 위반의 이슈 제목은 `[qualgate] 핫스팟 <규칙>: <파일> (N건)`, 본문에 `qualgate:safe` 안내 문구.
2. 신규 핫스팟 2건 중 1건이 `qualgate:safe` 이슈의 항목과 지문(트래커)으로 일치 → 검토율 50%, `new_hotspots_reviewed lt 100` → 실패.
3. 둘 다 safe → 100% → 통과. 신규 핫스팟 0건 → 100%.
4. `qualgate:safe` 이슈는 다시 열지 않고 본문도 바꾸지 않음. 코드가 사라지면 닫음.
5. `active_rules`에 없는 규칙의 기존 이슈 → "룰셋에서 제외되어 닫습니다" 코멘트 후 닫음(고쳐짐 코멘트가 아님). 다시 켜고 같은 위반 → 다시 열림.
6. `active_rules`에 있고 위반이 사라진 이슈 → 기존 "고쳐짐" 종료. 분석 실패 파일은 닫지 않음(기존 테스트 유지).
7. `sync_issues == false`: API 쓰기 호출 0회, 게이트는 판정.
8. 요약 이슈 본문에 조건별 실제값·기준·결과, `config_source`의 설정 저장소와 커밋 sha, 사용한 룰셋 이름.
9. 게이트 조건이 모두 Skipped면 stdout 경고(기존 문구 유지).

- [ ] **Step 2~4: 실패 확인, 구현, 통과**

`main.rs`의 `do_publish`: 기본 브랜치 확인 후 `sync_issues` 결정, 결과 파일 쓰기, 종료 코드. `render::RENDER_VERSION`을 올려 기존 이슈 본문을 한 번 다시 쓰게 한다.

Run: `bash scripts/cargo.sh test --lib sync` 와 `--test cli`

- [ ] **Step 5: CLI 테스트**

기존 스텁 서버 기반 CLI 테스트를 고친다: `--gate-result` 파일 내용, `--fail-on-gate false`면 실패여도 0, 결과 `schema` 1이면 2.

- [ ] **Step 6: 커밋**

```bash
git commit -m "feat(publish): gate evaluation with hotspot review and ruleset exclusion"
```

---

### Task 10: Tekton Task, 파이프라인 연결 문서, README

**Files:**
- Create: `deploy/tekton/task-qualgate.yaml`, `deploy/tekton/pipeline-patch.md`
- Delete: `docs/gitea-actions-example.yaml`
- Modify: `README.md`, `tests/cli.rs`(YAML 구조 테스트) 또는 `tests/tekton_yaml.rs`

**Interfaces:**
- Consumes: 스펙 5장의 Task 정의, 파라미터 이름.

- [ ] **Step 1: 실패 테스트** `tests/tekton_yaml.rs`

`serde_yaml`로 `deploy/tekton/task-qualgate.yaml`을 읽어: `apiVersion == tekton.dev/v1`, `kind == Task`, `metadata.namespace == tekton-catalog`, 파라미터 이름 집합이 스펙과 같음(`IMAGE, CONTEXT_DIR, GIT_HOST_URL, REPO_ORG, REPO_NAME, REVISION, COMMIT, CONFIG_REPO, CONFIG_REF, CRT_FILENAME, SSL_VERIFY, FAIL_ON_GATE`), workspace `source`, `basic-auth`, `ssl-ca-directory`, 결과 `gate`, 단계 이름 순서 `fetch-config, scan, publish`, `scan` 단계 env에 `GITEA`·`QUALGATE_GIT_CREDENTIALS`·`QUALGATE_CA_FILE`이 없음.

- [ ] **Step 2: Task YAML 작성**

이미지 기본값 `{{image_registry}}/nnd/tekton-cicd/qualgate:<패키지 버전>`, `GIT_HOST_URL` 기본값 `{{gitea_domain}}`. `fetch-config`·`publish` env: `GITEA_URL`, `QUALGATE_CA_FILE=$(workspaces.ssl-ca-directory.path)/$(params.CRT_FILENAME)`, `QUALGATE_GIT_CREDENTIALS=$(workspaces.basic-auth.path)/.git-credentials`, `QUALGATE_SSL_VERIFY=$(params.SSL_VERIFY)`. `ssl-ca-directory` workspace는 `optional: true`이고 바인딩이 없으면 CA 변수를 비운다(스크립트 없이 하려면 단계를 `script`로 작성해 `$(workspaces.ssl-ca-directory.bound)`를 확인). `publish`에 `--fail-on-gate $(params.FAIL_ON_GATE)`(기본 `"true"`). `/work`는 `emptyDir` 볼륨.

- [ ] **Step 3: `pipeline-patch.md`**

`build-common-image-gitops`의 `code-analysis` 태스크를 바꾼 YAML 조각(전체 태스크 블록), 새 파이프라인 파라미터 `qualgate-config-repo`, `qualgate-fail-on-gate`, `failure-issue-skip-tasks`에 `code-analysis` 추가, `SSL_VERIFY`는 `code-fetch`의 `sslVerify`와 같은 값, `COMMIT`은 `$(tasks.code-fetch.results.commit)`. 등록 명령 `kubectl apply -n tekton-catalog -f deploy/tekton/task-qualgate.yaml`.

- [ ] **Step 4: README 갱신**

빠른 시작을 Tekton 기준으로, 설정 저장소 만들기(`init-config` → Gitea에 push → `validate-config`), 명령 목록, 환경 변수, 종료 코드, Tekton 한계(workspace 공유, 같은 Pod 네트워크), 핫스팟과 `qualgate:safe`, 룰셋에서 끈 규칙의 이슈 종료 동작, `.qualgate.toml` 무시. Gitea Actions 언급 제거.

- [ ] **Step 5: 통과 확인과 커밋**

Run: `bash scripts/cargo.sh test --test tekton_yaml`

```bash
git commit -m "feat(tekton): qualgate Task, pipeline patch guide and README"
```

---

### Task 11: 로컬 통합·E2E 검증

**Files:**
- Modify: `tests/gitea_it.sh`, `tests/e2e/run.sh`, `Dockerfile`(필요 시)

- [ ] **Step 1: E2E 갱신**

`tests/e2e/run.sh`: 각 언어 샘플을 `init-config`로 만든 설정 디렉터리 + 직접 쓴 `source.json`으로 `scan --config` 실행. 기대 규칙 ID와 커버리지 검사는 유지.

- [ ] **Step 2: Gitea 통합 확장** (`gitea/gitea:1.26.4`)

설정 저장소 `qg/cfg`(init-config 결과 push)와 대상 저장소 `qg/demo`를 만들고, 토큰 대신 `.git-credentials` 파일(Basic 인증)로 다음을 확인한다.
1. `fetch-config → scan → publish` 첫 실행: 이슈와 요약 생성, 게이트 결과 파일.
2. 핫스팟 규칙 위반을 넣고 실행 → 게이트 실패(`new_hotspots_reviewed`). 그 이슈에 `qualgate:safe` 라벨을 API로 붙이고 재실행 → 통과, 이슈 무변화.
3. 설정 저장소에서 그 규칙을 `disabled_rules`에 넣고 push → 재실행 시 이슈가 "룰셋에서 제외" 코멘트로 닫힘.
4. 얕은 클론(`--depth 1`)으로 스캔 + `fetch-config --commit` → `new_code_measured == true`.
5. 비기본 브랜치: 이슈 변화 없음, 게이트 결과만.
기존 검사(재실행 무변화, 재오픈, 무시 라벨)도 유지한다.

- [ ] **Step 3: 전체 검증**

```bash
bash scripts/cargo.sh test                       # rules_pack 포함, run_in_background로 실행(약 20분)
docker build -t qualgate:dev . && docker image ls qualgate:dev --format '{{.Size}}'
bash tests/e2e/run.sh
bash tests/gitea_it.sh
```

Expected: 모두 통과, 이미지 150MB 이하.

- [ ] **Step 4: 커밋**

```bash
git commit -m "test: config repository flow in e2e and Gitea integration"
```

---

### Task 12: 대상 Gitea(jenny 조직) 확인 — 컨트롤러가 직접 수행

자격 증명이 필요하므로 서브에이전트에 맡기지 않는다. 컨트롤러가 사용자 확인 후 실행한다.

- [ ] **Step 1:** 사용자에게 실행 직전 확인: `jenny` 조직에 저장소 `qualgate-config`, `qualgate-demo` 생성, 이슈·라벨 생성.
- [ ] **Step 2:** 자격 증명은 실행 셸의 환경 변수로만 넘긴다(파일·커밋·로그 금지). `QUALGATE_SSL_VERIFY=false`(사용자 결정) 또는 CA 파일.
- [ ] **Step 3:** `qualgate:dev` 컨테이너로 `init-config` → 설정 저장소 push → 대상 저장소에 샘플 push → `fetch-config → scan → publish` 실행. Task 11 Step 2의 1~3을 실제 Gitea에서 확인.
- [ ] **Step 4:** 결과(이슈 링크, 게이트 결과, Gitea 1.26.4에서 다른 점)를 `docs/verification-gitea.md`에 추가하고 커밋. 테스트 저장소를 남길지 사용자에게 묻는다.
