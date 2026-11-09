# qualgate

`qualgate`는 저장소를 컨테이너에서 스캔해 규칙 위반, 중복도, 커버리지를 판정하고, 결과를 Gitea 이슈로 등록하는 도구입니다. 서버, DB, 웹 UI가 없습니다. 상태는 Gitea 이슈 본문에 저장하며, 코드를 고치면 다음 스캔에서 해당 이슈가 자동으로 닫힙니다.

(`qualgate`는 작업용 이름이며 공개 전에 바뀔 수 있습니다.)

## 빠른 시작

```bash
docker run --rm --network none -v "$PWD:/src:ro" -v "$PWD/.qualgate-out:/out" qualgate scan
docker run --rm -v "$PWD/.qualgate-out:/out:ro" -e GITEA_URL -e GITEA_TOKEN qualgate publish --repo owner/name
```

`scan`은 네트워크가 필요 없고 `/out/result.json`을 씁니다. `publish`는 코드를 읽지 않고 결과 파일만 Gitea에 반영합니다. 토큰은 코드에 접근하지 않는 `publish` 단계에만 전달합니다. 두 단계를 한 컨테이너에서 실행하는 `run --repo owner/name`도 있습니다.

> **보안 주의:** `run`은 신뢰할 수 없는 저장소의 코드와 설정을 토큰이 있고 네트워크에 연결된 같은 프로세스에서 처리합니다. qualgate는 설정 파일 심볼릭 링크를 거부하고, 설정 오류에 원문을 출력하지 않고, Opengrep과 git 자식 프로세스에서 `GITEA_TOKEN`을 지우지만, 이것으로 격리가 되지는 않습니다. 외부 기여가 들어오는 저장소에는 `run` 대신 `scan`(`--network none`, 토큰 없음)과 `publish`를 나눠 실행하세요.

`/out`은 컨테이너의 비root 사용자(uid 10001)가 쓸 수 있어야 하므로 `chmod 777 .qualgate-out`을 하거나 같은 uid로 만드세요. 이슈 동기화는 스캔한 브랜치가 저장소의 기본 브랜치일 때만 일어납니다. 브랜치는 git에서 읽으며, CI에서 detached HEAD라면 `-e QUALGATE_BRANCH=<브랜치>`를 넘기세요. Gitea Actions 예시는 [`docs/gitea-actions-example.yaml`](docs/gitea-actions-example.yaml)에 있습니다.

## 설정

저장소 루트의 `.qualgate.toml`이 없으면 아래 기본값으로 동작합니다.

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

설정 파일에 알 수 없는 키가 있거나 형식이 틀리면 스캔 전에 종료 코드 2로 끝납니다. 오류 메시지에는 줄과 열만 나오고 설정 내용은 나오지 않습니다. 다음 값도 종료 코드 2입니다: `.qualgate.toml`이 심볼릭 링크, `blocking_severities`에 `info`·`low`·`medium`·`high`·`critical` 외의 값, `block_lines`나 `min_tokens`가 0, `labels`에 `qualgate:ignored`, `rules.extra_paths`가 저장소 밖(심볼릭 링크 포함)을 가리킴. `max_new_issues`는 1~100으로 조정하고 경고합니다.

`exclude`가 분석 대상을 정하는 유일한 필터입니다. `.gitignore`, `.semgrepignore`, Opengrep의 파일 크기 제한과 기본 제외 폴더(`tests/` 등)는 적용하지 않습니다. 대상 파일 중 Opengrep이 스캔하지 않은 파일은 분석 실패로 기록하고 그 파일의 이슈는 닫지 않습니다. 단, 기본 규칙이 대상으로 하지 않는 `.js`, `.ts`, `.mjs`, `.cjs`, `.mts`, `.cts`(React 규칙은 `.jsx`, `.tsx`만 검사)는 스캔되지 않아도 분석 실패로 보지 않으며, 중복도와 커버리지만 측정합니다. Opengrep 1.33.1은 `.mts`, `.cts`를 아예 스캔하지 않습니다. 이 예외는 확장자 단위입니다. Opengrep이 그 확장자의 파일을 하나라도 스캔했다면(예: 외부 규칙이 `.js`를 대상으로 하는 경우) 같은 확장자의 스캔되지 않은 파일은 분석 실패입니다. 따라서 외부 규칙을 `paths.include`로 일부 `.js` 파일에만 한정하면 나머지 `.js` 파일은 분석 실패로 기록됩니다.

## 지원 언어와 커버리지 형식

테스트는 실행하지 않고 리포트만 읽습니다. 프로젝트의 CI에서 먼저 테스트를 실행해 리포트를 만들어 두세요.

| 언어 | 커버리지 형식 | 기본 경로 |
|---|---|---|
| Java, Kotlin | JaCoCo XML | `**/build/reports/jacoco/**/*.xml`, `**/target/site/jacoco/jacoco.xml` |
| Python | Cobertura XML | `coverage.xml` |
| React (JSX/TSX), Vue | LCOV | `coverage/lcov.info` |
| Go | coverprofile | `coverage.out` |

리포트가 없으면 0%가 아니라 "측정 안 됨"으로 표시하며, `required = true`일 때만 게이트 실패로 처리합니다. 리포트가 있어도 저장소 파일과 하나도 매칭되지 않으면 `required = true`에서는 실패입니다. 리포트의 파일 경로는 저장소 경로와 뒤에서부터 구성 요소 단위로 비교하며, 한쪽 경로 전체가 다른 쪽의 끝부분과 같아야 매칭됩니다. 후보가 여럿이거나 없으면 버리고 요약에 표시합니다. `test_paths`에 해당하는 파일과 `exclude`된 파일의 기록은 지표에 넣지 않습니다.

## 종료 코드

| 코드 | 의미 |
|---|---|
| 0 | 게이트 통과 |
| 1 | 게이트 실패 (결과는 정상적으로 기록됨) |
| 2 | 실행 오류 (설정 오류, Opengrep 실패, Gitea 인증·권한 오류, 재시도 후에도 실패한 HTTP 오류 등) |

## 필요한 토큰 범위

`GITEA_TOKEN`에는 다음 범위가 필요하고, 이것이면 충분합니다 (Gitea 1.27에서 실측, [`docs/verification.md`](docs/verification.md) 참고).

- `write:issue`
- `read:repository`

`publish`가 기본 브랜치를 알기 위해 `GET /repos/{owner}/{repo}`를 호출하므로 `read:repository`가 필요합니다. `write:issue`만 있으면 403으로 종료 코드 2가 됩니다. 토큰은 환경 변수로만 받고 로그에 출력하지 않습니다.

## 이슈 규칙

| 유형 | 단위 | 제목 예시 |
|---|---|---|
| 규칙 위반 | 규칙 + 파일 | `[qualgate] java.sql-injection: src/Foo.java (3건)` |
| 중복 | 복제 그룹 | `[qualgate] 중복 코드: src/A.kt 외 2개 파일 (42줄)` |
| 요약 | 저장소당 1개, 고정 | `[qualgate] 품질 요약` |

- 커버리지는 파일별 이슈 없이 요약 이슈에 넣습니다. 요약에는 게이트 결과, 지표, 측정 안 된 항목, 분석에 실패한 파일, 생성 제한으로 등록하지 못한 건수가 들어갑니다.
- 새 그룹은 심각도 순으로 실행당 `max_new_issues`개까지 만듭니다.
- 모든 이슈에 `qualgate` 라벨을 붙이며, 이슈를 찾을 때는 이 라벨만 씁니다. `publish.labels`는 여기에 더해 붙이는 라벨이므로 바꿔도 기존 이슈를 잃지 않습니다.
- 항목과 제목이 같으면 API를 호출하지 않으므로 새 커밋으로 재실행해도 알림이 늘지 않습니다. 이슈 본문의 링크는 이슈를 마지막으로 갱신한 커밋을 가리킵니다. 요약 이슈는 지표나 게이트가 바뀔 때만 갱신합니다.
- 고쳐서 항목이 모두 사라지면 코멘트를 달고 닫습니다. 닫힌 이슈의 항목이 재발하면 다시 열고 코멘트합니다. 사람이 수동으로 닫은 이슈도 재발하면 다시 열립니다.
- `qualgate:ignored` 라벨이 있는 이슈는 다시 열지도, 수정하지도 않습니다. 더 이상 보고 싶지 않은 항목에 이 라벨을 다세요.
- 스캔이 실패했거나 해당 파일을 분석하지 못했다면 그 범위의 이슈는 닫지 않습니다.
- 이슈 본문 끝의 HTML 주석(`<!-- qualgate:v1 {...} -->`)에 상태를 저장하므로 지우지 마세요. 본문이 크면 `(2/n)`으로 나뉩니다.
- 제목이 `[qualgate]`로 시작하는 이슈는 상태 주석 아래에 사람이 글을 덧붙여도 계속 추적합니다. 하지만 다음 본문 갱신 때 그 글은 지워지므로 메모는 코멘트로 남기세요. 제목이 `[qualgate]`로 시작하지 않으면 상태 주석이 본문 맨 끝에 있을 때만 qualgate 이슈로 봅니다.
- 이슈 본문에 넣는 저장소의 글(메시지, 파일 이름)은 멘션, 링크, 이미지, HTML이 되지 않도록 이스케이프합니다.
- 재시도: GET과 PATCH는 전송 오류와 HTTP 5xx를, 모든 요청은 429를 최대 3회 재시도합니다. POST(이슈·코멘트·라벨 생성)는 중복 생성을 막기 위해 5xx와 전송 오류를 재시도하지 않고, 다음 실행이 상태를 맞춥니다.

## 사내 CA 사용법

Gitea가 사내 CA 인증서를 쓰면, 이미지의 `/etc/ssl/certs`에 인증서를 넣고 `update-ca-certificates`를 실행하는 파생 이미지를 만드세요.

```dockerfile
FROM registry.example.com/qualgate:latest
USER root
COPY internal-ca.crt /usr/local/share/ca-certificates/internal-ca.crt
RUN update-ca-certificates
USER qualgate
```

## 한계

- 품질 게이트는 보안 경계가 아닙니다. 저장소가 자기 `.qualgate.toml`로 기준값을 바꿀 수 있습니다. 기준을 강제하려면 설정 변경을 코드 리뷰로 막으세요.
- 테스트 파일도 규칙 검사 대상이며 이슈로 등록되고 `new_blocking`에 포함됩니다 (사용자 결정 사항). 중복도와 커버리지 지표에서만 `test_paths`를 제외합니다. 테스트 코드를 검사하지 않으려면 `exclude`에 넣으세요.
- 측정할 수 있는 게이트 조건이 하나도 없으면 게이트는 통과로 끝나지만 경고를 출력합니다.
- 기본 규칙은 언어별 20개씩입니다. 오탐률은 아직 측정하지 않았습니다 (사내 저장소 검증 대기 중, `docs/verification.md` 참고). `rules.extra_paths`로 직접 쓴 Opengrep 규칙 폴더를 추가할 수 있습니다.
- Vue는 `<script>`와 `<script setup>`만 분석하며 `<template>`은 분석하지 않습니다.
- 파일 간 데이터 흐름 분석이 없습니다. 파일 하나 안에서 보이는 패턴만 찾습니다.
- 중복도는 Sonar의 토큰 줄 방식을 따르지만 토크나이저가 달라 수치가 정확히 같지는 않습니다. 작은 샘플에서 Sonar Community와 약 7%포인트 차이를 확인했습니다 (`docs/verification.md`).
- 1차 범위에서 PR 코멘트, 담당자 지정, 의존성 취약점 검사, 저장소 간 중복 탐지는 제외했습니다.
- 이미지는 amd64만 지원합니다.

## 라이선스와 출처

프로젝트 라이선스는 LGPL-3.0입니다 (`LICENSE`).

- `SonarSource/sonarqube` 공개 소스(LGPL-3.0): 이슈 추적(`Tracker`)과 중복 블록 알고리즘(`PmdBlockChunker` 등)을 Rust로 포팅했습니다. 포팅한 파일에는 원본 저작권 고지를 유지했습니다.
- Opengrep(LGPL-2.1): 수정 없이 릴리스 바이너리를 외부 프로세스로 실행합니다. 이미지를 만들 때 릴리스 서명을 검증합니다.
- tree-sitter와 언어 문법 크레이트(MIT): 의존성으로 사용합니다.
- `semgrep-rules`, `opengrep-rules`는 저장소에 포함하지 않았습니다. 기본 규칙은 이 프로젝트에서 직접 작성했습니다.
