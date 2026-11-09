# 설정 저장소 형식

qualgate의 룰셋과 프로젝트별 품질 게이트는 Gitea 설정 저장소의 파일로 관리한다. 이 문서는 파일 형식과 검증 규칙의 기준이다. 설정 화면 같은 도구는 만들어져 있지 않으며, 나중에 만들 화면은 이 문서, `qualgate validate-config`, `qualgate catalog --json`만으로 구현한다. 문서와 검증 코드가 다르면 `validate-config`의 동작이 정답이고 이 문서를 고친다.

## 1. 명령

| 명령 | 용도 |
|---|---|
| `qualgate init-config --out <디렉터리>` | 초기 파일 생성: `qualgate.toml`, 기본 룰셋 `rulesets/standard.toml`(규칙 디렉터리 7개 모두 켬), README. 존재하지만 비어 있지 않은 디렉터리에는 아무것도 쓰지 않고 종료 코드 2. |
| `qualgate validate-config --config-dir <설정 저장소 체크아웃>` | 전역, 모든 룰셋, 모든 프로젝트를 검증한다. 오류를 모두 모아 `오류: <파일>: <내용>` 형식으로 stderr에 출력한다. 종료 코드 0(정상) 또는 2(오류). |
| `qualgate catalog --json` | 규칙 카탈로그를 JSON 배열로 stdout에 출력한다(4장). 종료 코드 0 또는 2. |

세 명령은 규칙 경로를 환경 변수 `QUALGATE_RULES`(기본 `/rules`)에서 읽는다. 이 경로에서 규칙 파일(`<디렉터리>/*.yaml`)을 하나도 찾지 못하면 잘못된 경로로 보고 종료 코드 2로 끝난다. 규칙이 없는 카탈로그로 검증하면 모든 규칙 ID가 "없는 규칙"이 되거나, 반대로 규칙이 조용히 꺼지는 것을 막기 위해서다.

## 2. 파일 구조

```text
qualgate.toml                   전역 설정
rulesets/<name>.toml            룰셋 하나 = 파일 하나
projects/<owner>/<repo>.toml    프로젝트 하나 = 파일 하나
```

- 룰셋 이름은 `^[a-z0-9][a-z0-9-]{0,62}$`이고 파일 이름(확장자 제외)과 같아야 한다.
- 프로젝트 파일 경로는 Gitea 저장소 전체 이름(`owner/repo`)과 같아야 하며, 파일 안의 `repo` 값과도 같아야 한다.
- 모든 파일에 `schema = 1`이 있어야 한다. 모르는 키는 오류다.
- `rulesets/`, `projects/`, 그리고 `projects/<owner>/` 안에서 점으로 시작하는 파일(`.gitkeep` 등)과 `.md` 파일(README)은 설정이 아니므로 무시한다. 그 밖의 예상하지 못한 항목과 심볼릭 링크는 오류다.
- `default_ruleset`이나 프로젝트의 `ruleset`이 가리키는 룰셋 파일이 없으면 오류다.

## 3. 전역 설정 `qualgate.toml`

```toml
schema = 1
default_ruleset = "backend-standard"   # 비어 있으면 미등록 저장소는 오류

[[default_gate]]
metric = "new_issues"
op = "gt"
value = 0
# ... 프로젝트 게이트와 같은 형식
```

- 미등록 저장소(프로젝트 파일이 없는 저장소)는 `default_ruleset`, `default_gate`, 스캔 옵션 기본값으로 스캔한다.
- `default_ruleset`이 비어 있으면 미등록 저장소는 오류이며 `fetch-config`가 종료 코드 2로 끝난다.
- `default_gate`가 비어 있으면 내장 기본 게이트를 쓴다: `new_issues gt 0`, `new_hotspots_reviewed lt 100`, `new_coverage lt 80`, `new_duplicated_lines gt 3`.

## 4. 규칙 카탈로그와 이슈 종류

규칙은 `rules/<디렉터리>/<이름>.yaml`이고 규칙 ID는 `<디렉터리>.<이름>`이다. 카탈로그는 이 파일들의 메타데이터를 읽어 만들며, 버전은 `qualgate` 패키지 버전이다. `qualgate catalog --json`의 각 항목은 다음 필드를 가진다.

| 필드 | 의미 |
|---|---|
| `id` | 규칙 ID. 예: `java.catch-throwable` |
| `dir` | 규칙 디렉터리 이름 |
| `severity` | 기본 등급: `info`, `low`, `medium`, `high`, `critical` |
| `kind` | 기본 종류: `issue` 또는 `hotspot` |
| `category` | 분류 |
| `mois` | 행안부 항목(없으면 `null`) |
| `cwe` | CWE(없으면 `null`) |
| `title` | 메시지 첫 줄 |

### 이슈와 핫스팟

- `issue`: 확실한 위반이다. 신규 코드의 `issue`는 `new_issues` 지표에 센다.
- `hotspot`: 확신도가 낮은 휴리스틱 규칙(이름 기반 비밀값 탐지, 약한 난수, 쿠키·CORS 설정 등)의 결과로, 사람이 검토해야 한다. 핫스팟 이슈 제목은 `[qualgate] 핫스팟 <규칙>: <파일>`이다.
- 핫스팟을 검토해 문제가 없다고 판단하면 Gitea 이슈에 `qualgate:safe` 라벨을 붙인다. 파일과 지문이 일치하는 핫스팟은 "검토됨"이 된다. `qualgate:safe` 이슈는 다시 열지 않고 본문 갱신과 종료 코멘트도 하지 않으며, 코드가 사라지면 닫는다.
- `new_hotspots_reviewed`는 신규 코드 핫스팟 중 검토된 비율이다. 신규 코드 핫스팟이 0건이면 100%로 본다.
- 룰셋의 `overrides`로 규칙별 종류와 등급을 바꿀 수 있다(5장).

### 규칙 디렉터리와 대상 파일

| 디렉터리 | 대상 파일 |
|---|---|
| `java` | `.java` |
| `kotlin` | `.kt`, `.kts` |
| `python` | `.py` |
| `go` | `.go` |
| `ts` | `.js`, `.mjs`, `.cjs`, `.ts`, `.jsx`, `.tsx`, Vue script |
| `react` | `.jsx`, `.tsx` |
| `vue` | `.vue` |

- 자동 감지: 스캔 대상(제외 경로를 뺀 파일)에 대상 파일이 하나라도 있는 디렉터리만 실행한다.
- 실행할 디렉터리 = 룰셋에서 켜짐 ∩ (프로젝트 `languages` 또는 자동 감지).
- 대상 파일이 있지만 해당 디렉터리가 꺼져 있으면 그 파일은 규칙 검사를 하지 않는다. 설정에 따른 제외이며 분석 실패가 아니다.
- 실행하지 않은 규칙(꺼진 디렉터리의 규칙, `disabled_rules`의 규칙)의 기존 이슈는 "고쳐짐"이 아니라 "룰셋에서 제외되어 닫습니다" 코멘트와 함께 닫는다. 규칙을 다시 켜고 같은 위반이 나오면 다시 연다.
- 중복도와 커버리지는 규칙 디렉터리와 무관하게 지원하는 모든 언어 파일에서 측정한다.

## 5. 룰셋 `rulesets/<name>.toml`

```toml
schema = 1
name = "backend-standard"
description = "백엔드 공통 기준"

[languages.java]
enabled = true
disabled_rules = ["java.system-out"]

[languages.ts]
enabled = true

[overrides."java.catch-throwable"]
severity = "low"
kind = "hotspot"
```

- `languages`의 키는 규칙 디렉터리 이름이다: `java`, `kotlin`, `python`, `go`, `ts`, `react`, `vue`. 없는 키는 꺼진 것으로 본다. 다른 키는 오류다.
- `enabled`의 기본값은 `true`다. 켜진 디렉터리의 규칙 중 `disabled_rules`에 없는 규칙이 실행된다.
- `overrides`는 규칙별로 `severity`(`info|low|medium|high|critical`)와 `kind`(`issue|hotspot`)를 바꾼다. 둘 다 선택이다.
- `disabled_rules`와 `overrides`의 규칙 ID는 카탈로그에 있어야 하고, `disabled_rules`는 해당 디렉터리에 속해야 한다.
- `description`은 선택이다.

## 6. 프로젝트 `projects/<owner>/<repo>.toml`

```toml
schema = 1
repo = "team-a/order-service"
ruleset = "backend-standard"
languages = ["java", "react"]   # 선택. 없으면 자동 감지
new_code_days = 30

[[gate]]
metric = "new_issues"
op = "gt"
value = 0

[[gate]]
metric = "new_hotspots_reviewed"
op = "lt"
value = 100

[[gate]]
metric = "new_coverage"
op = "lt"
value = 80.0

[[gate]]
metric = "new_duplicated_lines"
op = "gt"
value = 3.0

[scan]
exclude = ["**/generated/**"]
test_paths = ["**/src/test/**"]

[coverage]
jacoco = ["**/build/reports/jacoco/**/*.xml"]
required = false

[duplication]
block_lines = 10
min_tokens = 100

[publish]
max_new_issues = 20
labels = []
```

- 필수 키는 `schema`, `repo`, `ruleset`이다. `new_code_days`의 기본값은 30이다.
- `languages`는 규칙 디렉터리 이름의 목록이다. 지정하면 룰셋에서 켜진 것 중 이 목록에 있는 디렉터리만 실행한다.
- `[scan]`, `[coverage]`, `[duplication]`, `[publish]`는 기존 `.qualgate.toml`과 같은 키와 기본값을 쓴다. 생략하면 기본값이다. 대상 저장소의 `.qualgate.toml`은 더 이상 쓰지 않는다.
- `gate`가 비어 있으면 게이트는 항상 통과하고 경고를 남긴다.

## 7. 게이트 지표

조건은 "참이면 실패"로 읽는다. `gt`는 초과, `lt`는 미만이다. `op`는 이 둘뿐이다.

| 지표 | 의미 | 단위 |
|---|---|---|
| `new_issues` | 신규 코드의 이슈 종류 위반 수 | 건 |
| `new_hotspots_reviewed` | 신규 코드 핫스팟 중 검토된 비율 | % |
| `new_coverage` | 신규 코드 커버리지 | % |
| `new_duplicated_lines` | 신규 코드 중복 줄 비율 | % |
| `issues` | 전체 이슈 종류 위반 수 | 건 |
| `coverage` | 전체 커버리지 | % |
| `duplicated_lines` | 전체 중복 줄 비율 | % |

- 비율 지표의 값은 0~100, 건수 지표의 값은 0 이상 정수다. 같은 지표를 한 게이트(전역 `default_gate` 또는 한 프로젝트의 `gate`) 안에서 두 번 쓰면 오류다.
- 측정하지 못한 지표의 조건은 건너뛴다. `coverage.required = true`이고 커버리지를 측정하지 못하면 커버리지 조건은 실패다.
- 기존의 "변경 줄의 차단 등급 위반 0건" 조건은 `new_issues`로 대체되었고 `blocking_severities` 설정은 없다.

## 8. 검증 규칙 요약

`validate-config`가 검사하는 항목이다. 오류는 파일 경로와 함께 모두 출력한다.

1. 모든 파일의 `schema`가 1이고 모르는 키가 없다. TOML 형식이 맞다.
2. 룰셋 이름이 `^[a-z0-9][a-z0-9-]{0,62}$`이고 파일 이름·`name` 값과 같다.
3. 프로젝트의 `repo`가 `owner/repo` 형식이고 파일 경로·`repo` 값과 같다.
4. `default_ruleset`과 프로젝트의 `ruleset`이 존재하는 룰셋을 가리킨다.
5. `languages`의 키(룰셋)와 항목(프로젝트)이 규칙 디렉터리 이름이다.
6. `disabled_rules`와 `overrides`의 규칙 ID가 카탈로그에 있고, `disabled_rules`는 해당 디렉터리에 속한다.
7. `severity`, `kind`, `metric`, `op` 값이 허용 목록 안에 있다.
8. 게이트 값의 범위(비율 0~100, 건수 0 이상 정수)와 지표 중복.
9. `rulesets/`, `projects/` 안의 항목이 규칙에 맞는 일반 파일·디렉터리다(심볼릭 링크 불가).

## 9. 예시 파일

`qualgate init-config`가 만드는 기본 파일과 `tests/fixtures/config/good/`을 예시로 쓴다. 가장 작은 정상 설정은 다음과 같다.

```toml
# qualgate.toml
schema = 1
default_ruleset = "standard"
```

```toml
# rulesets/standard.toml
schema = 1
name = "standard"

[languages.java]
enabled = true
```

```toml
# projects/team-a/order-service.toml
schema = 1
repo = "team-a/order-service"
ruleset = "standard"
```
