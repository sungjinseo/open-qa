# qualgate

`qualgate`는 저장소를 컨테이너에서 스캔해 규칙 위반, 중복도, 커버리지를 판정하고, 결과를 Gitea 이슈로 등록하는 도구입니다. 서버, DB, 웹 UI가 없습니다. 상태는 Gitea 이슈 본문에 저장하며, 코드를 고치면 다음 스캔에서 해당 이슈가 자동으로 닫힙니다.

(`qualgate`는 작업용 이름이며 공개 전에 바뀔 수 있습니다.)

## 빠른 시작

```bash
docker run --rm --network none -v "$PWD:/src:ro" -v "$PWD/.qualgate-out:/out" qualgate scan
docker run --rm -v "$PWD/.qualgate-out:/out:ro" -e GITEA_URL -e GITEA_TOKEN qualgate publish --repo owner/name
```

`scan`은 네트워크가 필요 없고 `/out/result.json`을 씁니다. `publish`는 코드를 읽지 않고 결과 파일만 Gitea에 반영합니다. 토큰은 코드에 접근하지 않는 `publish` 단계에만 전달합니다. 두 단계를 한 컨테이너에서 실행하는 `run --repo owner/name`도 있습니다.

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
labels = ["qualgate"]
```

설정 파일에 알 수 없는 키가 있거나 형식이 틀리면 스캔 전에 종료 코드 2로 끝납니다.

## 지원 언어와 커버리지 형식

테스트는 실행하지 않고 리포트만 읽습니다. 프로젝트의 CI에서 먼저 테스트를 실행해 리포트를 만들어 두세요.

| 언어 | 커버리지 형식 | 기본 경로 |
|---|---|---|
| Java, Kotlin | JaCoCo XML | `**/build/reports/jacoco/**/*.xml`, `**/target/site/jacoco/jacoco.xml` |
| Python | Cobertura XML | `coverage.xml` |
| React (JSX/TSX), Vue | LCOV | `coverage/lcov.info` |
| Go | coverprofile | `coverage.out` |

리포트가 없으면 0%가 아니라 "측정 안 됨"으로 표시하며, `required = true`일 때만 게이트 실패로 처리합니다. 리포트의 파일 경로는 저장소 경로와 뒤에서부터 비교하며, 후보가 여럿이거나 없으면 버리고 요약에 표시합니다.

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
- 내용이 같으면 API를 호출하지 않으므로 재실행해도 알림이 늘지 않습니다.
- 고쳐서 항목이 모두 사라지면 코멘트를 달고 닫습니다. 닫힌 이슈의 항목이 재발하면 다시 열고 코멘트합니다. 사람이 수동으로 닫은 이슈도 재발하면 다시 열립니다.
- `qualgate:ignored` 라벨이 있는 이슈는 다시 열지도, 수정하지도 않습니다. 더 이상 보고 싶지 않은 항목에 이 라벨을 다세요.
- 스캔이 실패했거나 해당 파일을 분석하지 못했다면 그 범위의 이슈는 닫지 않습니다.
- 이슈 본문 끝의 HTML 주석(`<!-- qualgate:v1 {...} -->`)에 상태를 저장하므로 지우지 마세요. 본문이 크면 `(2/n)`으로 나뉩니다.
- HTTP 5xx와 429는 최대 3회 재시도합니다.

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

- 기본 규칙은 언어별 20개씩이며, 오탐을 줄이기 위해 보수적으로 골랐습니다. `rules.extra_paths`로 직접 쓴 Opengrep 규칙 폴더를 추가할 수 있습니다.
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
