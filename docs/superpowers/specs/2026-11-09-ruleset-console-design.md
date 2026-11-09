# 룰셋 콘솔과 프로젝트 품질 게이트 설계

## 1. 목표

- 웹 화면에서 룰셋과 저장소(프로젝트)별 품질 게이트를 설정한다.
- qualgate는 Tekton 파이프라인의 Task 하나로 동작하며, 설정에 따라 실패한 항목을 Gitea 이슈로 등록한다.
- 스캔 결과를 보는 화면은 만들지 않는다. 결과는 Gitea 이슈로만 확인한다.

이 문서는 `2026-11-06-qualgate-design.md`를 확장한다. 둘이 충돌하면 이 문서가 우선한다.

### 1차 범위에서 제외

스캔 결과·대시보드 화면, 화면에서 스캔 실행, 권한 세분화(룰셋 편집과 프로젝트 게이트 편집을 다른 팀에 나누기), 지표 조건 연산자 `gt`·`lt` 외의 연산자, Gitea Actions 지원.

## 2. 구성

```text
┌──────────── 사용자 브라우저 ────────────┐
│  qualgate-web (Rust, 서버 렌더링 HTML)  │── Gitea OAuth2 로그인
└────────────────┬───────────────────────┘
                 │ 사용자 토큰으로 파일 읽기·커밋
                 ▼
        Gitea 설정 저장소 (예: qa/rulesets)
                 ▲ 읽기 토큰
┌────────────────┴──── Tekton Task: qualgate ────────────────────┐
│ fetch-config  →  scan (토큰 없음)  →  publish (쓰기 토큰)       │
└────────────────────────────────────────────────────────────────┘
                                         │ 이슈 동기화, 게이트 판정
                                         ▼
                                  대상 저장소의 Gitea 이슈
```

| 구성 요소 | 형태 | 역할 |
|---|---|---|
| `qualgate` | 기존 바이너리, 기존 이미지 | `fetch-config`, `scan`, `publish` |
| `qualgate-web` | 같은 저장소의 새 바이너리, 별도 이미지 | 룰셋·프로젝트·전역 설정 편집 |
| 공용 코드 | 같은 크레이트의 라이브러리 모듈 | 설정 파일 형식, 검증, 규칙 카탈로그 |

화면과 스캐너는 같은 설정 파싱·검증 코드를 쓴다. 화면에서 저장한 설정을 스캐너가 다르게 해석하는 일이 없어야 한다.

## 3. 설정 저장소

### 3.1 파일 구조

```text
qualgate.toml                   전역 설정
rulesets/<name>.toml            룰셋 하나 = 파일 하나
projects/<owner>/<repo>.toml    프로젝트 하나 = 파일 하나
```

- 룰셋 이름은 `^[a-z0-9][a-z0-9-]{0,62}$`이고 파일 이름과 같아야 한다.
- 프로젝트 파일 경로는 Gitea 저장소 전체 이름과 같아야 하며, 파일 안의 `repo` 값과도 같아야 한다.
- 모든 파일에 `schema = 1`이 있어야 한다. 모르는 키는 오류다.

### 3.2 전역 설정 `qualgate.toml`

```toml
schema = 1
default_ruleset = "backend-standard"   # 비어 있으면 미등록 저장소는 오류

[[default_gate]]
metric = "new_issues"
op = "gt"
value = 0
# ... 프로젝트 게이트와 같은 형식
```

미등록 저장소는 `default_ruleset`과 `default_gate`, 스캔 옵션 기본값으로 스캔한다. `default_ruleset`이 비어 있으면 `fetch-config`가 종료 코드 2로 끝난다.

### 3.3 룰셋 `rulesets/<name>.toml`

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

- `languages`의 키는 규칙 디렉터리 이름이다: `java`, `kotlin`, `python`, `go`, `ts`, `react`, `vue`. 없는 키는 꺼진 것으로 본다.
- 켜진 디렉터리의 규칙 중 `disabled_rules`에 없는 규칙이 실행된다.
- `overrides`는 규칙별로 `severity`(info|low|medium|high|critical)와 `kind`(issue|hotspot)를 바꾼다. 둘 다 선택이다.
- `disabled_rules`와 `overrides`의 규칙 ID는 카탈로그에 있어야 하고, 해당 디렉터리에 속해야 한다.

### 3.4 프로젝트 `projects/<owner>/<repo>.toml`

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

- `languages`는 규칙 디렉터리 이름의 목록이다. 지정하면 룰셋에서 켜진 것 중 이 목록에 있는 디렉터리만 실행한다.
- `[scan]`, `[coverage]`, `[duplication]`, `[publish]`는 기존 `.qualgate.toml`과 같은 키와 기본값을 쓴다. 생략하면 기본값이다.
- `gate`가 비어 있으면 게이트는 항상 통과하고 경고를 남긴다.

### 3.5 게이트 지표

조건은 "참이면 실패"로 읽는다. `gt`는 초과, `lt`는 미만이다.

| 지표 | 의미 | 단위 |
|---|---|---|
| `new_issues` | 신규 코드의 이슈 종류 위반 수 | 건 |
| `new_hotspots_reviewed` | 신규 코드 핫스팟 중 검토된 비율 | % |
| `new_coverage` | 신규 코드 커버리지 | % |
| `new_duplicated_lines` | 신규 코드 중복 줄 비율 | % |
| `issues` | 전체 이슈 종류 위반 수 | 건 |
| `coverage` | 전체 커버리지 | % |
| `duplicated_lines` | 전체 중복 줄 비율 | % |

- 비율 값은 0~100, 건수 값은 0 이상 정수다. 같은 지표를 두 번 쓰면 오류다.
- 측정하지 못한 지표의 조건은 건너뛴다(기존 규칙 유지). `coverage.required = true`이고 커버리지를 측정하지 못하면 커버리지 조건은 실패다.
- 신규 코드 핫스팟이 0건이면 `new_hotspots_reviewed`는 100%로 본다(SonarQube와 같음).
- 기존 게이트의 "변경 줄의 차단 등급 위반 0건"은 `new_issues`로 대체된다. `blocking_severities` 설정은 없어진다.

## 4. 규칙 카탈로그와 이슈 종류

- 규칙 YAML 메타데이터에 `qualgate_kind: issue | hotspot`을 추가한다. 없으면 `issue`다.
- 확신도가 낮은 휴리스틱 규칙은 기본 종류를 `hotspot`으로 둔다. 대상: 이름 기반 비밀값 탐지, 약한 난수, 쿠키·CORS 설정, 주석 속 비밀값, iframe sandbox, 그 밖에 오탐이 확인된 규칙. 목록은 구현 시 `docs/mois49.md`에 표시한다.
- 카탈로그는 `rules/` 디렉터리를 읽어 만든다. 항목: 규칙 ID, 디렉터리, 기본 등급, 기본 종류, 분류, 행안부 항목, CWE, 메시지 첫 줄.
- 카탈로그 버전은 `qualgate` 패키지 버전이다. 화면과 스캐너는 같은 버전으로 배포한다. 화면은 카탈로그 버전을 표시한다.

### 4.1 규칙 디렉터리와 대상 파일

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
- 대상 파일이 있지만 해당 디렉터리가 꺼져 있으면 그 파일은 규칙 검사를 하지 않는다. 이것은 분석 실패가 아니다(설정에 따른 제외).
- 실행하지 않은 규칙(꺼진 디렉터리의 규칙, `disabled_rules`의 규칙)의 기존 이슈는 "고쳐짐"이 아니라 "룰셋에서 제외되어 닫습니다" 코멘트와 함께 닫는다. 결과 파일에 실행한 규칙 ID 목록을 남겨 이 둘을 구분한다. 규칙을 다시 켜고 같은 위반이 나오면 기존 규칙대로 다시 연다.
- 중복도와 커버리지는 규칙 디렉터리와 무관하게 지원하는 모든 언어 파일에서 측정한다.

## 5. Tekton Task

```yaml
apiVersion: tekton.dev/v1
kind: Task
metadata:
  name: qualgate
spec:
  params:
    - name: repo            # owner/name
    - name: branch          # 스캔한 브랜치
    - name: gitea-url
    - name: config-repo
      default: qa/rulesets
    - name: config-ref
      default: main
  workspaces:
    - name: source
      readOnly: true
  results:
    - name: gate            # passed | failed
  volumes:
    - name: work
      emptyDir: {}
  steps:
    - name: fetch-config
      image: qualgate:<ver>
      env: GITEA_URL, GITEA_TOKEN(읽기 Secret)
      args: [fetch-config, --repo, $(params.repo), --config-repo, $(params.config-repo), --ref, $(params.config-ref), --out, /work/config]
    - name: scan
      image: qualgate:<ver>
      env: QUALGATE_BRANCH=$(params.branch)
      args: [scan, --src, $(workspaces.source.path), --config, /work/config, --out, /work/out]
    - name: publish
      image: qualgate:<ver>
      env: GITEA_URL, GITEA_TOKEN(쓰기 Secret)
      args: [publish, --repo, $(params.repo), --out, /work/out, --gate-result, $(results.gate.path)]
```

- 실제 YAML은 `deploy/tekton/task.yaml`로 제공하고, Secret 이름은 파라미터로 받는다.
- `scan` 단계에는 토큰 환경 변수를 주지 않는다. 같은 Pod라 네트워크는 공유되므로, 네트워크 격리가 필요하면 클러스터 NetworkPolicy로 건다. README에 명시한다.
- 소스 workspace는 읽기 전용이어도 동작해야 한다.
- 신규 코드 계산에는 git 이력이 필요하다. 앞 Task가 얕은 클론이면 신규 코드 지표는 측정 안 됨이 된다. README에 깊이 권장값을 적는다.

### 5.1 `fetch-config`

- 설정 저장소에서 `qualgate.toml`, `projects/<repo>.toml`(없으면 미등록), 사용할 룰셋 파일만 Gitea 파일 API로 받는다.
- 받은 파일을 같은 검증 코드로 검사하고, 실패하면 종료 코드 2.
- `--out`에 세 파일과 `source.json`(설정 저장소, ref, 커밋 sha, 미등록 여부)을 쓴다.

### 5.2 `scan`

- `--config` 디렉터리의 설정만 쓴다. 대상 저장소의 `.qualgate.toml`은 무시하고, 있으면 경고만 낸다.
- 실행할 규칙 파일만 임시 디렉터리에 모아 Opengrep에 넘긴다. `overrides`는 결과를 읽은 뒤 적용한다.
- 위반마다 `kind`를 기록한다.
- 지표를 계산하되 게이트는 판정하지 않는다. 종료 코드는 성공 0, 오류 2.
- 결과 파일 `schema = 2`: 기존 필드 + 위반의 `kind`, 실행한 디렉터리와 규칙 ID 목록, 게이트 조건 목록, `source.json` 내용, 신규 코드 핫스팟 목록.

### 5.3 `publish`

- `schema = 2`가 아니면 종료 코드 2.
- 대상 저장소의 qualgate 이슈를 읽어 핫스팟 검토 상태를 계산한다.
  - 핫스팟이 `qualgate:safe` 라벨이 붙은 이슈의 항목과 지문으로 일치하면 검토됨.
- 조건 목록으로 게이트를 판정하고 `--gate-result` 파일에 `passed` 또는 `failed`를 쓴다. 종료 코드는 통과 0, 실패 1, 오류 2.
- 기본 브랜치일 때만 이슈를 동기화한다. 기본 브랜치가 아니면 이슈는 읽기만 하고 판정 결과만 낸다.
- 이슈 동기화 규칙은 기존과 같고, 다음을 더한다.
  - 핫스팟 이슈 제목은 `[qualgate] 핫스팟 <규칙>: <파일>`, 본문에 검토 방법(`qualgate:safe` 라벨)을 적는다.
  - `qualgate:safe` 이슈는 다시 열지 않고, 본문 갱신과 종료 코멘트도 하지 않는다. 코드가 사라지면 닫는다.
  - 실행하지 않은 규칙의 이슈는 "룰셋에서 제외되어 닫습니다" 코멘트와 함께 닫는다(4.1).
  - 요약 이슈에 조건별 결과, 사용한 룰셋과 설정 커밋 sha를 적는다.

## 6. 웹 화면 `qualgate-web`

### 6.1 기술

- Rust, axum, 서버 렌더링 템플릿, htmx. 자바스크립트 빌드 도구는 쓰지 않는다.
- 설정 형식, 검증, 카탈로그는 스캐너와 같은 라이브러리 모듈을 쓴다.

### 6.2 로그인과 권한

- Gitea OAuth2 Authorization Code 흐름. 사용자 토큰은 서버 메모리 세션에만 두고 디스크에 쓰지 않는다. 세션 쿠키는 HttpOnly, Secure(설정으로 끌 수 있음), SameSite=Lax.
- 모든 저장은 로그인한 사용자 토큰으로 설정 저장소에 커밋한다. 쓰기 권한이 없으면 Gitea가 거부하고 화면은 그 사실을 보여 준다.
- 상태를 바꾸는 요청에는 CSRF 토큰을 요구한다.
- 세션 만료 기본 8시간.

### 6.3 화면

| 화면 | 기능 |
|---|---|
| 프로젝트 목록 | 등록된 프로젝트, 룰셋, 마지막 변경 커밋. 프로젝트 추가(사용자가 볼 수 있는 Gitea 저장소에서 선택) |
| 프로젝트 설정 | 룰셋, 언어(자동 감지 또는 직접 지정), 신규 코드 기간, 게이트 조건 표(추가·삭제·값 수정), 스캔 옵션 |
| 룰셋 목록 | 목록, 새로 만들기, 복제, 삭제, 사용 중인 프로젝트 |
| 룰셋 편집 | 디렉터리 탭별 규칙 표: 사용 여부, 규칙 ID, 행안부 항목, 등급, 종류. 필터(행안부 항목, 분류, 종류, 검색), 탭 전체 켜기·끄기 |
| 전역 설정 | 기본 룰셋, 기본 게이트 |

- 프로젝트 설정의 언어 영역에는 Gitea 저장소 언어 통계(`/repos/{owner}/{repo}/languages`)를 참고로 보여 준다.
- 새 프로젝트와 새 룰셋의 게이트 기본값은 전역 기본 게이트, 없으면 3.4의 네 조건이다.
- react나 vue는 켰는데 ts가 꺼진 룰셋이면 경고한다.
- 변경 이력은 설정 저장소의 Gitea 커밋 기록 링크로 대신한다.

### 6.4 저장

- 저장 전에 스캐너와 같은 검증을 한다. 실패하면 커밋하지 않고 오류를 표시한다.
- Gitea 파일 API로 커밋하며, 불러올 때의 sha를 함께 보낸다. sha가 다르면 덮어쓰지 않고 "다른 사용자가 먼저 변경했습니다"를 표시하고 다시 불러오게 한다.
- 커밋 메시지는 사용자가 입력하며, 비어 있으면 `qualgate-web: <파일> 변경`을 쓴다.
- 사용 중인 룰셋은 삭제할 수 없다.

### 6.5 배포

- 이미지: `qualgate-web` 바이너리, 템플릿, 규칙 카탈로그. Opengrep은 넣지 않는다.
- 환경 변수: `GITEA_URL`, `OAUTH_CLIENT_ID`, `OAUTH_CLIENT_SECRET`, `CONFIG_REPO`, `CONFIG_REF`(기본 main), `SESSION_KEY`, `PUBLIC_URL`, `COOKIE_SECURE`(기본 true).
- `deploy/k8s/web.yaml`에 Deployment, Service, Secret 예시를 제공한다. 세션이 메모리에 있으므로 1차는 복제본 1개다.

## 7. 기존 기능 변경 요약

| 항목 | 변경 |
|---|---|
| `.qualgate.toml` | 무시하고 경고. 설정의 기준은 설정 저장소 |
| `scan` 종료 코드 | 게이트를 판정하지 않으므로 0 또는 2 |
| `publish` | 게이트 판정과 종료 코드 0/1/2, `--gate-result` |
| `run` 명령 | 제거 (Tekton 단계 분리로 대체) |
| 결과 파일 | `schema = 2` |
| `blocking_severities` | 제거, `new_issues` 조건으로 대체 |
| Gitea Actions 예시 | 제거, Tekton Task 예시로 대체 |
| 규칙 메타데이터 | `qualgate_kind` 추가 |

## 8. 오류 처리

| 상황 | 동작 |
|---|---|
| 설정 저장소 접근 실패, 파일 형식 오류, 없는 룰셋, 모르는 규칙 ID | `fetch-config` 종료 코드 2 |
| 미등록 저장소이고 `default_ruleset` 비어 있음 | `fetch-config` 종료 코드 2 |
| 실행할 규칙 디렉터리가 하나도 없음 | 스캔은 진행(중복도·커버리지), 경고. `new_issues`는 0으로 측정 |
| 결과 파일 schema 불일치 | `publish` 종료 코드 2 |
| 화면 저장 시 검증 실패 | 커밋하지 않고 오류 표시 |
| 화면 저장 시 sha 불일치 | 덮어쓰지 않고 다시 불러오기 안내 |
| 화면에서 Gitea 권한 없음 | Gitea 응답을 그대로 안내 |

## 9. 테스트

- **단위:** 설정 파일 파싱·검증(정상, 모르는 키, 잘못된 이름, 없는 규칙, 중복 지표, 범위), 실행 디렉터리 계산, 오버라이드 적용, 지표별 조건 판정, 핫스팟 검토율, 카탈로그 생성.
- **스캐너 통합:** 설정 디렉터리를 쓰는 `scan`, 룰셋에서 끈 규칙이 실행되지 않음, 오버라이드된 등급·종류가 결과에 반영됨, `.qualgate.toml` 무시.
- **publish:** 가짜 API로 게이트 판정, 핫스팟 `qualgate:safe` 처리, 끈 규칙의 이슈가 "룰셋에서 제외" 코멘트로 닫힘, 비기본 브랜치에서 판정만 함.
- **Gitea 통합:** 기존 `tests/gitea_it.sh` 확장. 설정 저장소를 만들고 `fetch-config → scan → publish`를 실행해 이슈 생성, 핫스팟 검토 후 게이트 통과, 룰셋 변경 반영을 확인.
- **웹:** axum 핸들러 테스트(가짜 Gitea 서버): 로그인 콜백, 저장 검증 실패, sha 충돌, 권한 없음, CSRF 누락. 실제 Gitea에 OAuth 앱을 만들어 로그인과 저장까지 확인하는 통합 스크립트 1개.
- **Tekton:** `deploy/tekton/task.yaml`이 스키마상 유효한지 확인(kind/Tekton이 없으면 YAML 구조 검사). 실제 클러스터 실행은 사용자 환경에서 확인할 항목으로 README에 적는다.

## 10. 구현 전 확인할 사항

1. 사내 Gitea 버전에서 OAuth2 애플리케이션 생성과 Authorization Code 흐름, 파일 API의 sha 기반 갱신이 동작하는가.
2. 사내 Tekton 버전(`tekton.dev/v1` 지원 여부, step `results` 경로).
3. 앞 Task(git clone)의 클론 깊이.
