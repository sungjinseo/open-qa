# qualgate 설계

`qualgate`는 작업용 이름이며, 공개 전에 바꿀 수 있다.

## 1. 목표

저장소를 컨테이너에서 스캔해 규칙 위반, 중복도, 커버리지를 판정하고 결과를 Gitea 이슈로 등록한다. 개발자는 이슈를 보고 코드를 고치며, 고쳐진 항목의 이슈는 다음 스캔에서 자동으로 닫힌다.

- 서버, DB, 웹 UI가 없다. 상태는 Gitea 이슈에 저장한다.
- 지원 언어: Java, Kotlin, Python, React(JSX/TSX), Vue, Go
- 구현 언어: Rust
- 실행 형태: 단일 컨테이너 이미지. 어떤 CI에서든 실행할 수 있다.
- 공개 OSS로 만들 수 있게 구성한다.

### 1차 범위에서 제외

PR 코멘트, git blame 담당자 지정, 의존성 취약점 검사, SBOM, 아키텍처 규칙, 대시보드, PDF 리포트, AI 수정 제안, 저장소 간 중복 탐지, Vue `<template>` 분석, 파일 간 데이터 흐름 분석.

## 2. 코드 출처 원칙

| 출처 | 사용 방법 |
|---|---|
| `SonarSource/sonarqube` 공개 소스 (LGPL 3) | 이슈 추적과 중복 블록 알고리즘을 Rust로 포팅, 지표 공식 참고 |
| Opengrep 릴리스 (LGPL 2.1) | 수정 없이 외부 바이너리로 실행 |
| tree-sitter와 언어 문법 크레이트 (MIT) | 의존성으로 사용 |
| Sonar Enterprise, SSAL 분석기, 디컴파일 결과 | 사용하지 않음 |
| `semgrep-rules`, `opengrep-rules` | 저장소에 포함하지 않음. 사용자가 외부 경로로 불러올 수는 있음 |

프로젝트 라이선스는 LGPL 3.0으로 하고, 포팅한 파일에는 원본 저작권 고지를 유지한다.

## 3. 구조

```text
/src (읽기 전용)
  │
  ├─ Vue 전처리 ─ `.vue`의 script를 줄 번호를 유지한 임시 파일로 변환
  │
  ├─ rules    ─ Opengrep 실행, JSON 파싱
  ├─ dup      ─ tree-sitter 토큰, 블록 해시, 복제 그룹
  ├─ coverage ─ JaCoCo, Cobertura, LCOV, Go 리포트 파싱
  │
  └─ 결과 파일 (/out/result.json)
       │
       ├─ gate   ─ 품질 게이트 판정
       └─ publish ─ Gitea 이슈와 비교(track), 생성·갱신·종료
```

Rust 크레이트 하나에 모듈을 나눈다.

| 모듈 | 책임 | 의존 |
|---|---|---|
| `model` | `Finding`, `CloneGroup`, `CoverageData`, `ScanResult` 타입 | 없음 |
| `config` | `.qualgate.toml`과 환경 변수 읽기 | `model` |
| `vue` | SFC 블록 분리와 줄 번호 유지 변환 | 없음 |
| `rules` | Opengrep 실행, JSON 결과를 `Finding`으로 변환 | `model` |
| `dup` | 토큰화와 복제 탐지 | `model`, tree-sitter |
| `coverage` | 리포트 파서와 경로 매핑 | `model` |
| `newcode` | git으로 변경 줄 계산 | `model` |
| `gate` | 지표 계산과 통과 판정 | `model` |
| `track` | 이전 결과와 현재 결과 매칭 | `model` |
| `gitea` | 이슈 API 클라이언트와 본문 생성·파싱 | `model` |
| `main` | CLI 조립 | 전체 |

주요 크레이트: `clap`, `serde`, `toml`, `quick-xml`, `ureq`, `rayon`, `tree-sitter`, `tree-sitter-java`, `tree-sitter-kotlin-ng`, `tree-sitter-python`, `tree-sitter-javascript`, `tree-sitter-typescript`, `tree-sitter-go`.

## 4. CLI와 실행

```bash
docker run --rm --network none \
  -v "$PWD:/src:ro" -v "$PWD/.qualgate-out:/out" \
  qualgate scan

docker run --rm \
  -v "$PWD/.qualgate-out:/out:ro" \
  -e GITEA_URL -e GITEA_TOKEN \
  qualgate publish --repo owner/name
```

- `scan`: 네트워크가 필요 없다. 코드를 읽어 `/out/result.json`을 쓴다.
- `publish`: 코드를 읽지 않는다. 결과 파일을 Gitea에 반영한다.
- `run`: 두 단계를 한 컨테이너에서 실행한다. 신뢰할 수 없는 코드와 토큰이 한 프로세스에 있으므로 외부 기여 저장소에는 `scan`과 `publish`를 나눠 쓴다. `.qualgate.toml` 심볼릭 링크는 거부하고, 설정 오류는 줄과 열만 알리며, Opengrep과 git 자식 프로세스에서 `GITEA_TOKEN`을 지운다.
- `publish`는 `result.json`의 `schema`가 1이 아니면 종료 코드 `2`로 끝낸다.

스캔과 등록을 분리하면 코드를 다루는 단계를 네트워크에서 격리할 수 있고, 토큰은 코드에 접근하지 않는 단계에만 전달할 수 있다.

종료 코드: `0` 게이트 통과, `1` 게이트 실패, `2` 실행 오류.

### 이미지

- 런타임: Alpine + git + `opengrep_musllinux_*` 바이너리 + 기본 규칙 + `qualgate`
- Opengrep은 버전을 고정하고, 빌드할 때 릴리스 서명을 검증한다.
- root가 아닌 사용자로 실행한다.
- 1차는 amd64만 지원한다. 크기 목표는 150MB 이하다.

## 5. 설정

`.qualgate.toml`이 없으면 기본값으로 동작한다.

```toml
[scan]
exclude = ["**/node_modules/**", "**/build/**", "**/dist/**", "**/vendor/**"]
test_paths = ["**/src/test/**", "**/*_test.go", "**/test_*.py", "**/*.spec.*", "**/*.test.*"]

[rules]
extra_paths = []
disabled = []

[duplication]
block_lines = 10
min_tokens = 100

[coverage]
jacoco = ["**/build/reports/jacoco/**/*.xml", "**/target/site/jacoco/jacoco.xml"]
cobertura = ["coverage.xml"]
lcov = ["coverage/lcov.info"]
go = ["coverage.out"]
required = false

[new_code]
days = 30

[gate]
new_coverage_min = 80.0
new_duplication_max = 3.0
new_blocking_max = 0
blocking_severities = ["critical", "high"]

[publish]
max_new_issues = 20
labels = []
```

잘못된 `blocking_severities`, 0인 `block_lines`·`min_tokens`, `labels`의 `qualgate:ignored`, 저장소 밖의 `extra_paths`는 종료 코드 `2`다. `max_new_issues`는 1~100으로 조정한다.

## 6. 규칙 검사

```bash
opengrep scan --no-git-ignore --x-ignore-semgrepignore-files --max-target-bytes=0   --json-output=<파일> -f <규칙 경로> [--exclude=**/<폴더>/**] .
```

- `exclude`가 유일한 필터다. Opengrep의 `.gitignore`, `.semgrepignore`, 크기 제한, 기본 제외 폴더는 끈다. Opengrep의 `--exclude`는 의미가 다르므로 `**/<이름>/**` 형태만 성능을 위해 넘기고, 나머지는 qualgate가 결과를 거른다.
- `paths.scanned`가 없으면 오류다. 대상 파일 중 스캔되지 않은 파일은 분석 실패로 기록한다(기본 규칙이 대상으로 하지 않는 `.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`는 예외). 대상이 있는데 하나도 스캔되지 않으면 오류다.
- 경로 없는 `level: error` 오류나 비어 있지 않은 `skipped_rules`는 규칙 이름과 함께 종료 코드 `2`로 끝낸다.
- 테스트 파일도 규칙 검사와 `new_blocking`에 포함한다. 중복도와 커버리지에서만 제외한다.

- 기본 규칙은 이미지의 `/rules/<언어>/*.yaml`에 포함하며 프로젝트에서 직접 작성한다.
- 1차 목표는 언어별로 오탐이 적은 규칙 20개다. 규칙마다 위반 예제와 정상 예제를 테스트로 둔다.
- 규칙 메타데이터에 `qualgate_id`, `qualgate_severity`, `qualgate_category`를 둔다. 값이 없는 외부 규칙은 `check_id`와 규칙의 `severity`로 대체한다 (`ERROR`→`high`, `WARNING`→`medium`, `INFO`→`low`).
- JSON의 `errors`에 파일 경로가 있으면 해당 파일을 분석 실패로 기록한다.
- 네트워크 없는 컨테이너에서 실행해 외부 전송을 원천 차단한다.

### Vue

`vue` 모듈이 `.vue`의 최상위 `<script>`와 `<script setup>` 블록을 찾고(HTML 주석 안의 태그와 자체 닫힘 `<template/>`을 처리한다), 블록 밖의 내용을 빈 줄로 바꾼 임시 파일을 만든다. 줄 번호가 같으므로 결과를 별도 변환 없이 원본 `.vue` 경로에 대응시킨다. 확장자는 `lang` 속성에 따라 `.ts`, `.js`로 정한다. script가 없는 파일과 구조를 읽지 못한 파일(닫히지 않은 블록 등)을 구분하며, 후자는 분석 실패다.

`tree-sitter-vue` 크레이트는 2022년 이후 갱신되지 않아 사용하지 않는다. 블록 경계를 직접 찾는다.

## 7. 중복도

Sonar의 토큰 줄 방식을 포팅한다.

1. tree-sitter의 리프 노드로 파일을 토큰화한다. 주석은 제외한다.
2. import와 package 선언을 제외한다.
3. 줄 단위로 토큰을 합쳐 줄 해시를 만든다.
4. 같은 줄이 연속되면 처음과 마지막만 남긴다. (`PmdBlockChunker`와 동일)
5. `block_lines` 크기의 롤링 해시(`hash * 31 + line_hash`)로 블록을 만든다.
6. 같은 해시의 블록을 묶어 인접 블록을 복제 구간으로 확장한다.
7. `min_tokens` 미만인 구간은 버린다.

- 언어가 같은 파일끼리만 비교한다. React의 JS와 TS는 같은 그룹으로 본다.
- 테스트 경로와 `exclude`는 제외한다.
- 지표: `중복률 = 100 × 중복 줄 수 / 전체 줄 수`. 같은 줄은 한 번만 센다.

토크나이저가 다르므로 수치가 Sonar와 정확히 같지는 않다.

## 8. 커버리지

테스트는 실행하지 않고 리포트만 읽는다.

| 언어 | 형식 | 읽는 정보 |
|---|---|---|
| Java, Kotlin | JaCoCo XML | `line`의 `mi`, `ci`, `mb`, `cb` |
| Python | Cobertura XML | `line`의 `hits`, `condition-coverage` |
| React, Vue | LCOV | `DA`, `BRDA` |
| Go | coverprofile | 블록 범위를 줄로 전개, 같은 줄은 커버 우선 |

- 리포트의 파일 경로는 저장소 경로와 뒤에서부터 구성 요소 단위로 비교해, 한쪽 전체가 다른 쪽의 접미사일 때만 매칭한다. 후보가 여럿이거나 없으면 버리고 요약에 표시한다. 테스트 파일과 `exclude`된 파일의 기록은 지표에 넣지 않는다.
- `required = true`이면 리포트가 저장소 파일과 하나도 매칭되지 않을 때도 실패다.
- 지표: `커버리지 = 100 × (커버된 줄 + 커버된 조건) / (전체 측정 줄 + 전체 조건)`. 구현할 때 Sonar의 `LinesAndConditionsWithUncoveredCounter`와 대조한다.
- 리포트가 없으면 0%가 아니라 **측정 안 됨**이다. `required = true`일 때만 게이트 실패로 처리한다.
- 리포트에 없는 소스 파일은 지표에 넣지 않고, 요약에 `리포트 미포함`으로 표시한다.

## 9. 신규 코드와 게이트

- 기준 커밋: `git rev-list -1 --before="<days> days ago" HEAD`
- 변경 줄: 기준 커밋과 `HEAD`의 `git diff -U0 --find-renames` (이름만 바뀐 파일은 신규 코드가 아니다)
- 얕은 클론이거나 기준 커밋이 없으면 신규 코드 지표는 **측정 안 됨**이다. 게이트 조건은 건너뛰고 요약에 표시한다.

게이트는 다음 조건을 모두 만족해야 통과한다.

| 조건 | 기본값 |
|---|---|
| 신규 코드 커버리지 | 80% 이상 |
| 신규 코드 중복률 | 3% 이하 |
| 변경 줄의 차단 등급(`critical`, `high`) 위반 | 0건 |

측정되지 않은 조건은 건너뛴다. 모두 건너뛰면 경고를 출력한다. 저장소가 기준값을 바꿀 수 있으므로 게이트는 보안 경계가 아니다.

## 10. Gitea 이슈

### 이슈 단위

| 유형 | 단위 | 제목 예시 |
|---|---|---|
| 규칙 위반 | 규칙 + 파일 | `[qualgate] java.sql-injection: src/Foo.java (3건)` |
| 중복 | 복제 그룹 | `[qualgate] 중복 코드: src/A.kt 외 2개 파일 (42줄)` |
| 요약 | 저장소당 1개, 고정 | `[qualgate] 품질 요약` |

- 커버리지는 파일별로 이슈를 만들지 않고 요약 이슈에 넣는다.
- 요약 이슈에는 게이트 결과, 지표, 측정 안 된 항목, 생성 제한으로 등록하지 못한 건수를 적는다.
- 스캔한 커밋 기준의 파일 링크를 본문에 넣는다(경로는 퍼센트 인코딩). 저장소에서 온 글은 멘션·링크·이미지·HTML이 되지 않게 이스케이프한다.
- 라벨: 모든 이슈에 고정 라벨 `qualgate`를 붙이고 이슈 목록은 이 라벨로만 조회한다. 설정의 `labels`는 추가 라벨이다.

### 이슈 본문의 상태

본문 끝의 HTML 주석에 다음 JSON을 저장한다. 별도 DB는 쓰지 않는다.

```text
<!-- qualgate:v1 {"key":"...","findings":[{"rule":"...","file":"...","line":12,"line_hash":"...","message":"..."}]} -->
```

상태 주석은 본문 끝에 있어야 한다. 단, 제목이 `[qualgate]`로 시작하면 마지막 마커 뒤에 사람이 쓴 글이 있어도 받아들인다(다음 갱신 때 그 글은 지워진다). 사람이 읽는 목록은 최대 50건만 표시하고, 숨은 상태에는 전체를 저장한다. 본문이 Gitea 허용 크기를 넘으면 이슈를 `(2/n)`으로 나눈다.

### 이슈 추적 (`track`)

`Tracker.java`의 순서를 포팅한다. 같은 규칙 안에서 다음 순서로 매칭하며, 매칭된 항목은 후보에서 뺀다.

1. 줄 번호 + 줄 해시 + 메시지
2. 줄 번호 + 줄 해시
3. 줄 해시 + 메시지
4. 줄 번호 + 메시지
5. 줄 해시

원본의 블록 단위 코드 이동 감지 단계는 1차에서 제외한다. 이동한 코드는 5번에서 대부분 잡힌다. 줄 해시는 공백을 제거한 줄의 SHA-256이다.

복제 그룹의 키는 정렬한 블록 해시 묶음의 해시다.

### 동기화

| 상황 | 동작 |
|---|---|
| 새 그룹 | 이슈 생성, 심각도 순으로 실행당 `max_new_issues`개까지 |
| 기존 그룹의 항목이나 제목이 바뀜 | 본문 갱신 (커밋만 바뀌면 호출하지 않음) |
| 그룹의 항목이 모두 사라짐 | 코멘트 후 이슈 닫기 |
| 닫힌 이슈의 항목이 재발 | 다시 열고 코멘트 |
| `qualgate:ignored` 라벨이 있는 이슈 | 다시 열지 않고 건드리지 않음 |
| 그 외 사람이 수동으로 닫은 이슈 | 재발하면 다시 열기 |

### 안전장치

- **스캔 실패 시 닫지 않는다.** 엔진이 실패했거나 해당 파일을 분석하지 못했으면 그 범위의 이슈는 닫지 않는다.
- 커밋이 기본 브랜치일 때만 이슈를 동기화한다.
- 같은 내용이면 API를 호출하지 않아 재실행해도 알림이 늘지 않는다.
- 토큰은 환경 변수로만 받고 로그에 출력하지 않는다.
- 재시도: GET·PATCH는 전송 오류와 5xx, 모든 요청은 429를 최대 3회 재시도한다. POST는 중복 생성을 막기 위해 5xx와 전송 오류를 재시도하지 않는다. 재시도 후 실패하면 종료 코드 `2`로 끝낸다.
- 파일 탐색 오류(읽을 수 없는 폴더)는 종료 코드 `2`다.

## 11. 오류 처리

| 상황 | 동작 |
|---|---|
| Opengrep 실행 실패 | 종료 코드 `2`, 이슈 닫기 금지 |
| 일부 파일 파싱 실패 | 계속 진행, 실패 목록을 요약에 표시, 해당 파일의 이슈 닫기 금지 |
| 커버리지 리포트 형식 오류 | 해당 리포트는 측정 안 됨, 경고 |
| 설정 파일 오류 | 스캔 전에 종료 코드 `2` |
| Gitea 인증·권한 오류 | 종료 코드 `2`, 필요한 권한 안내 |

## 12. 테스트

- **단위:** 파서별 픽스처, 블록 해시, 트래커 단계별 매칭, Vue 줄 번호 유지, 게이트 경계값, 이슈 본문 왕복 변환
- **규칙:** 규칙마다 위반 예제와 정상 예제
- **통합:** 테스트 Gitea 컨테이너에 생성, 갱신, 닫기, 재오픈, 재실행 무변화를 검증
- **E2E:** 6개 언어 샘플 저장소로 `docker run` 실행, 네트워크 없는 `scan` 확인
- **비교:** 중복 탐지와 Sonar 공개판의 결과를 샘플로 비교해 큰 차이가 없는지 확인

## 13. 구현 첫 단계에서 검증할 사항

결과에 따라 해당 부분의 설계를 수정한다.

1. Opengrep musl 바이너리가 Alpine과 `--network none`에서 동작하는가
2. 문법 크레이트들이 `tree-sitter` 현재 버전과 함께 빌드되는가. 문법 크레이트 중 일부는 2024~2025년 이후 갱신이 없다.
3. Gitea에서 이슈 생성과 라벨 부여에 필요한 토큰 범위 (`write:issue`, 저장소 읽기 범위 확인)
4. Gitea 이슈 본문 크기 제한
5. 사내 저장소 1~2개에서 Opengrep과 자체 규칙의 실제 탐지 품질

## 14. 구현 순서

1. 프로젝트 골격, `model`, `config`, 이미지 빌드
2. 13장 검증
3. `rules`와 `vue`, 언어별 기본 규칙
4. `coverage`, `newcode`, `gate`
5. `dup`
6. `track`, `gitea`, `publish`
7. 통합·E2E 테스트와 문서
