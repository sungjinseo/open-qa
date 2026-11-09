# Gitea 기반 룰셋과 프로젝트 품질 게이트 설계

## 1. 목표

- 룰셋과 저장소(프로젝트)별 품질 게이트를 Gitea 설정 저장소의 파일로 관리한다.
- qualgate는 Tekton 파이프라인의 Task 하나로 동작하며, 설정에 따라 실패한 항목을 Gitea 이슈로 등록한다.
- 화면은 만들지 않는다. 설정은 Gitea에서 파일을 직접 편집하고, 결과는 Gitea 이슈로 확인한다. 나중에 다른 프로그램이 화면을 만들 수 있도록 설정 파일 형식과 검증 규칙을 문서로 고정하고, 검증만 하는 명령(`validate-config`)을 제공한다.

이 문서는 `2026-11-06-qualgate-design.md`를 확장한다. 둘이 충돌하면 이 문서가 우선한다.

### 1차 범위에서 제외

설정 편집 화면, 스캔 결과·대시보드 화면, 지표 조건 연산자 `gt`·`lt` 외의 연산자, Gitea Actions 지원.

### 대상 환경

- Gitea: `https://gitea.180-210-89-135.nip.io` (1.26.4). 인증서를 사내 CA(`NND Internal CA`)가 발급하므로 엔진은 추가 CA 인증서를 신뢰할 수 있어야 한다(5.4).
- 테스트 조직: `jenny`. 파이프라인 실행 계정: `cicdbot`.
- Tekton: 공통 Task는 `tekton-catalog` 네임스페이스, cluster resolver로 참조. 이미지는 `{{image_registry}}/nnd/tekton-cicd/` 아래에 둔다.

## 2. 구성

```text
사람 ── Gitea에서 직접 편집 ──▶ Gitea 설정 저장소 (예: qa/rulesets)
                                   ▲ 읽기 토큰
┌──────────────────── Tekton Task: qualgate ────────────────────┐
│ fetch-config  →  scan (토큰 없음)  →  publish (쓰기 토큰)       │
└───────────────────────────────────────────────────────────────┘
                                         │ 이슈 동기화, 게이트 판정
                                         ▼
                                  대상 저장소의 Gitea 이슈
```

설정 편집 실수는 `qualgate validate-config`로 미리 잡는다. 설정 저장소 자체의 CI(Tekton 또는 사람이 실행)에서 돌리는 것을 권장한다.

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
- 카탈로그 버전은 `qualgate` 패키지 버전이다. `qualgate catalog --json`으로 카탈로그를 출력해, 설정을 편집하는 사람이나 나중에 만들 화면이 규칙 목록을 얻을 수 있게 한다.

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

기존 공통 빌드 파이프라인(`build-common-image-gitops`)의 `code-analysis` 자리(현재 `sonarqube-scanner-env`)를 대신하는 Task로 만든다. 다른 공통 Task처럼 `tekton-catalog` 네임스페이스에 등록하고 cluster resolver로 참조한다.

```yaml
apiVersion: tekton.dev/v1
kind: Task
metadata:
  name: qualgate
  namespace: tekton-catalog
spec:
  params:
    - name: IMAGE                 # {{image_registry}}/nnd/tekton-cicd/qualgate:<ver>
    - name: CONTEXT_DIR           # $(params.working-pr-dir)/$(params.code-repo-name)
    - name: GIT_HOST_URL          # $(params.git-host-url)
    - name: REPO_ORG              # $(params.code-repo-org)
    - name: REPO_NAME             # $(params.code-repo-name)
    - name: REVISION              # $(params.code-revision) - 스캔한 브랜치
    - name: CONFIG_REPO           # 설정 저장소, 예: jenny/qualgate-config
    - name: CONFIG_REF
      default: main
    - name: CRT_FILENAME
      default: ca-certificates.crt
  workspaces:
    - name: source                # shared-data
    - name: basic-auth            # gitconfig (cicdbot 자격 증명)
    - name: ssl-ca-directory      # 사내 CA
  results:
    - name: gate                  # passed | failed
  volumes:
    - name: work
      emptyDir: {}
  steps:
    - name: fetch-config
      image: $(params.IMAGE)
      args: [fetch-config, --repo, $(params.REPO_ORG)/$(params.REPO_NAME), --config-repo, $(params.CONFIG_REPO), --ref, $(params.CONFIG_REF), --out, /work/config]
    - name: scan
      image: $(params.IMAGE)
      args: [scan, --src, $(workspaces.source.path)/$(params.CONTEXT_DIR), --config, /work/config, --out, /work/out]
    - name: publish
      image: $(params.IMAGE)
      args: [publish, --repo, $(params.REPO_ORG)/$(params.REPO_NAME), --out, /work/out, --gate-result, $(results.gate.path)]
```

- 실제 YAML은 `deploy/tekton/task-qualgate.yaml`로 제공한다. 이미지와 주소는 기존 템플릿처럼 `{{image_registry}}`, `{{gitea_domain}}` 자리표시자를 쓴다.
- `fetch-config`와 `publish`는 `GITEA_URL=$(params.GIT_HOST_URL)`, `QUALGATE_CA_FILE=$(workspaces.ssl-ca-directory.path)/$(params.CRT_FILENAME)`, `QUALGATE_GIT_CREDENTIALS=$(workspaces.basic-auth.path)/.git-credentials`를 받는다. `scan`에는 이 변수들을 주지 않고 `QUALGATE_BRANCH=$(params.REVISION)`만 준다.
- 한계: Tekton에서 workspace는 Task의 모든 단계에 마운트되므로 `scan` 단계도 자격 증명 파일 경로에 접근할 수는 있다. 환경 변수로 넘기지 않는 것까지만 분리한다. 같은 Pod라 네트워크도 공유된다. README에 명시한다.
- 소스 workspace에 쓰지 않는다. 결과와 설정은 `emptyDir`(`/work`)에만 쓴다.

### 5.0 파이프라인 연결

`deploy/tekton/pipeline-patch.md`에 기존 파이프라인을 바꾸는 방법을 적는다.

1. `code-analysis`의 `taskRef` 이름을 `qualgate`로 바꾸고 위 파라미터·workspace(`shared-data`, `gitconfig`, `ssl-ca-directory`)를 연결한다. `runAfter: [code-test]`는 유지해 커버리지 리포트를 읽는다.
2. `failure-issue-skip-tasks`에 `code-analysis`를 넣는다. qualgate가 자세한 이슈를 올리므로 실패 이슈 Task가 같은 실패를 중복으로 올리지 않게 한다.
3. `code-fetch`(git-clone)에 `depth` 파라미터를 준다. 기본값 1(얕은 클론)이면 신규 코드 지표를 측정할 수 없어 게이트가 건너뛰어진다. 권장값은 `0`(전체 이력) 또는 신규 코드 기간을 덮는 충분한 값.
4. 품질 게이트 실패로 파이프라인을 멈출지는 기존 `sonar-qualitygate-wait`처럼 파이프라인 파라미터 `qualgate-fail-on-gate`(기본 `true`)로 정한다. `false`면 `publish`가 게이트 실패에도 종료 코드 0을 내고 결과(`gate`)에만 `failed`를 남긴다.

### 5.0.1 Gitea 인증

- `QUALGATE_GIT_CREDENTIALS`(git-clone의 `basic-auth` workspace에 있는 `.git-credentials`)에서 `GITEA_URL`과 호스트가 같은 줄을 찾아 사용자·비밀번호로 Basic 인증한다.
- `GITEA_TOKEN`이 있으면 그것을 우선한다(로컬 실행과 테스트용).
- 둘 다 없으면 종료 코드 2. 자격 증명 값은 로그에 쓰지 않는다.
- 필요한 권한: 설정 저장소 읽기, 대상 저장소의 이슈·라벨 쓰기와 저장소 읽기. 현재 `cicdbot` 계정을 쓴다.

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
- `--fail-on-gate false`면 게이트 실패에도 종료 코드 0을 내고 결과 파일에만 `failed`를 쓴다(5.0의 4).
- 대상 저장소의 qualgate 이슈를 읽어 핫스팟 검토 상태를 계산한다.
  - 핫스팟이 `qualgate:safe` 라벨이 붙은 이슈의 항목과 지문으로 일치하면 검토됨.
- 조건 목록으로 게이트를 판정하고 `--gate-result` 파일에 `passed` 또는 `failed`를 쓴다. 종료 코드는 통과 0, 실패 1, 오류 2.
- 기본 브랜치일 때만 이슈를 동기화한다. 기본 브랜치가 아니면 이슈는 읽기만 하고 판정 결과만 낸다.
- 이슈 동기화 규칙은 기존과 같고, 다음을 더한다.
  - 핫스팟 이슈 제목은 `[qualgate] 핫스팟 <규칙>: <파일>`, 본문에 검토 방법(`qualgate:safe` 라벨)을 적는다.
  - `qualgate:safe` 이슈는 다시 열지 않고, 본문 갱신과 종료 코멘트도 하지 않는다. 코드가 사라지면 닫는다.
  - 실행하지 않은 규칙의 이슈는 "룰셋에서 제외되어 닫습니다" 코멘트와 함께 닫는다(4.1).
  - 요약 이슈에 조건별 결과, 사용한 룰셋과 설정 커밋 sha를 적는다.

### 5.4 사내 CA 인증서

- `QUALGATE_CA_FILE` 환경 변수로 PEM 파일(여러 인증서 가능)을 받아, 시스템 인증서에 더해 신뢰한다. Gitea에 접속하는 `fetch-config`와 `publish`에 적용한다.
- Tekton에서는 기존 파이프라인의 `ssl-ca-directory` workspace에 있는 `ca-certificates.crt`를 쓴다.
- 인증서 검증을 끄는 옵션은 만들지 않는다.
- 파일을 읽지 못하거나 PEM이 아니면 종료 코드 2.

## 6. 설정 도구 명령

| 명령 | 용도 |
|---|---|
| `qualgate validate-config --config-dir <설정 저장소 체크아웃>` | 설정 저장소 전체(전역, 모든 룰셋, 모든 프로젝트)를 검증. 오류는 파일과 위치를 함께 출력, 종료 코드 0 또는 2 |
| `qualgate catalog --json` | 규칙 카탈로그 출력 (4장 항목) |
| `qualgate init-config --out <디렉터리>` | 설정 저장소 초기 파일 생성: `qualgate.toml`, 기본 룰셋(모든 디렉터리 켬), README |

`docs/config-format.md`에 3장의 파일 형식과 검증 규칙을 사람이 읽을 수 있게 정리한다. 나중에 만들 화면은 이 문서와 `validate-config`, `catalog --json`만으로 구현할 수 있어야 한다.

## 7. 기존 기능 변경 요약

| 항목 | 변경 |
|---|---|
| `.qualgate.toml` | 무시하고 경고. 설정의 기준은 설정 저장소 |
| `scan` 종료 코드 | 게이트를 판정하지 않으므로 0 또는 2 |
| `publish` | 게이트 판정과 종료 코드 0/1/2, `--gate-result` |
| `run` 명령 | 제거 (Tekton 단계 분리로 대체) |
| 새 명령 | `fetch-config`, `validate-config`, `catalog`, `init-config` |
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
| Gitea TLS 인증서 검증 실패 | 종료 코드 2, 추가 CA 설정 방법 안내 |

## 9. 테스트

- **단위:** 설정 파일 파싱·검증(정상, 모르는 키, 잘못된 이름, 없는 규칙, 중복 지표, 범위), 실행 디렉터리 계산, 오버라이드 적용, 지표별 조건 판정, 핫스팟 검토율, 카탈로그 생성.
- **스캐너 통합:** 설정 디렉터리를 쓰는 `scan`, 룰셋에서 끈 규칙이 실행되지 않음, 오버라이드된 등급·종류가 결과에 반영됨, `.qualgate.toml` 무시.
- **publish:** 가짜 API로 게이트 판정, 핫스팟 `qualgate:safe` 처리, 끈 규칙의 이슈가 "룰셋에서 제외" 코멘트로 닫힘, 비기본 브랜치에서 판정만 함.
- **Gitea 통합:** 기존 `tests/gitea_it.sh`(로컬 Gitea 컨테이너) 확장. 설정 저장소를 만들고 `fetch-config → scan → publish`를 실행해 이슈 생성, 핫스팟 검토 후 게이트 통과, 룰셋 변경 반영을 확인.
- **대상 Gitea 확인:** `jenny` 조직에 테스트용 설정 저장소와 대상 저장소를 만들어 `fetch-config → scan → publish`를 로컬 컨테이너에서 한 번 실행한다. 사내 CA 인증서 파일을 받은 뒤에 실행하며, 인증서 검증은 끄지 않는다. 자격 증명은 저장소나 문서에 쓰지 않는다.
- **Tekton:** `deploy/tekton/*.yaml`의 YAML 구조 검사. 클러스터 등록과 실제 PipelineRun은 클러스터 연결 후 진행한다.
- **설정 도구:** `validate-config`가 각 오류 종류를 파일·위치와 함께 보고, `init-config` 결과가 그대로 검증을 통과함, `catalog --json` 형식.
- **Tekton:** `deploy/tekton/task.yaml`이 스키마상 유효한지 확인(kind/Tekton이 없으면 YAML 구조 검사). 실제 클러스터 실행은 사용자 환경에서 확인할 항목으로 README에 적는다.

## 10. 구현 전 확인할 사항

1. 대상 Gitea(1.26.4)의 파일 API로 설정 파일 읽기, 이슈·라벨 API 동작.
2. 클러스터 연결 후: `tekton-catalog`에 Task 등록, `ssl-ca-directory`·`gitconfig` workspace 형식(`.git-credentials` 파일 이름과 줄 형식) 확인.
3. `code-fetch`의 클론 깊이 변경 가능 여부(5.0의 3).
