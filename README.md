# qualgate

`qualgate`는 Rust CLI와 컨테이너로 동작하는 코드 품질 게이트입니다. 중앙 설정 저장소에서 대상 저장소의 룰셋을 받아 코드를 분석하고, 결과와 게이트 판정을 Gitea 이슈에 반영합니다. 실행 위치는 Tekton이며, 별도 서버·DB·웹 UI는 없습니다.

## 빠른 시작: Tekton

1. Gitea에 전역 설정, 룰셋, 프로젝트별 설정을 둘 `qualgate-config` 저장소를 만듭니다.
2. 초기 파일을 만들고 기본 룰셋을 필요에 맞게 수정합니다.

   ```bash
   mkdir qualgate-config
   docker run --rm -v "$PWD/qualgate-config:/config" \
     {{image_registry}}/nnd/tekton-cicd/qualgate:0.1.0 init-config --out /config
   docker run --rm -v "$PWD/qualgate-config:/config:ro" \
     {{image_registry}}/nnd/tekton-cicd/qualgate:0.1.0 validate-config --config-dir /config
   ```

3. `qualgate-config`를 Gitea에 push하고 공통 파이프라인에 [pipeline-patch.md](deploy/tekton/pipeline-patch.md)의 `code-analysis` 교체와 파라미터 변경을 적용합니다.
4. Task를 클러스터 카탈로그에 등록합니다.

   ```bash
   kubectl apply -n tekton-catalog -f deploy/tekton/task-qualgate.yaml
   ```

5. PipelineRun에서 `qualgate-config-repo`를 `owner/qualgate-config` 형식으로 지정합니다. 기존 `gitconfig` workspace의 `.git-credentials`를 통해 설정 저장소 읽기와 대상 저장소 이슈·라벨 쓰기 권한을 제공합니다.

Task는 `fetch-config → scan → publish` 순서로 실행합니다. 프로젝트 테스트는 이 Task 전에 실행해 커버리지 리포트를 만들어야 합니다. `fetch-config`는 설정과 신규 코드 범위를 받고, `scan`은 `/work/out/result.json`에 분석 결과를 씁니다. `publish`는 게이트 결과(`passed` 또는 `failed`)를 Tekton result `gate`에 씁니다.

## 설정 저장소

설정 저장소는 아래 구조를 사용합니다. 초기 예시와 전체 형식은 `init-config`와 [설정 형식 문서](docs/config-format.md)에 있습니다.

```text
qualgate.toml                 # 전역 기본 룰셋과 신규 코드 기간
rulesets/<name>.toml          # 언어별 실행 규칙과 끌 규칙
projects/<owner>/<repo>.toml  # 프로젝트 룰셋, 게이트, 커버리지 설정
```

`validate-config`는 저장소의 설정을 전체 검사하고, `catalog --json`은 사용 가능한 규칙 카탈로그를 출력합니다. 대상 코드 저장소의 `.qualgate.toml`은 설정으로 사용하지 않고 발견 시 경고합니다. 설정 기준은 중앙 설정 저장소입니다.

## CLI 명령

| 명령 | 용도 |
|---|---|
| `init-config --out <dir>` | 설정 저장소의 시작 파일 생성 |
| `validate-config --config-dir <dir>` | 전역·룰셋·프로젝트 설정 검증 |
| `catalog --json` | 규칙 카탈로그 JSON 출력 |
| `fetch-config --repo <owner/repo> --branch <branch> --commit <sha> --config-repo <owner/repo> --ref <ref> --out <dir>` | Gitea에서 설정과 신규 코드 정보를 받음 |
| `scan --src <dir> --config <dir> --out <dir>` | 네트워크 없이 코드를 분석하고 `result.json` 생성 |
| `publish --repo <owner/repo> --out <dir> [--gate-result <file>] [--fail-on-gate true|false]` | 게이트 판정 및 기본 브랜치의 Gitea 이슈 동기화 |

`publish`의 `--gate-result`는 줄바꿈 없는 `passed` 또는 `failed`를 씁니다. 게이트 실패 종료 코드는 기본적으로 1이며, `--fail-on-gate false`면 결과는 `failed`로 남기고 종료 코드 0을 반환합니다.

## 환경 변수와 인증

- `GITEA_URL`: Gitea 서버 주소.
- `GITEA_TOKEN`: 선택적 토큰 인증. 설정하면 `.git-credentials`보다 우선합니다.
- `QUALGATE_GIT_CREDENTIALS`: `GITEA_URL`과 호스트가 일치하는 `.git-credentials` 파일 경로. Tekton에서는 `gitconfig` workspace 파일을 사용합니다.
- `QUALGATE_CA_FILE`: Gitea 연결에 추가로 신뢰할 PEM CA 파일.
- `QUALGATE_SSL_VERIFY=false`: TLS 인증서 검증을 끕니다. 기본은 검증 켬이며, 사내 CA 파일 사용을 권장합니다.
- `QUALGATE_BRANCH`: CI가 브랜치를 git에서 확인할 수 없을 때 `scan`에 전달할 브랜치.
- `QUALGATE_RULES`, `QUALGATE_OPENGREP`: 규칙 디렉터리와 Opengrep 실행 파일을 바꿀 때 사용합니다. 기본값은 `/rules`, `opengrep`입니다.

필요한 Gitea 권한은 설정 저장소 읽기, 대상 저장소 읽기와 이슈·라벨 쓰기입니다. 자격 증명을 저장소 파일, 명령줄 인자 또는 로그에 넣지 마세요.

## 종료 코드

| 명령 | `0` | `1` | `2` |
|---|---|---|---|
| `fetch-config`, `validate-config`, `init-config`, `catalog` | 성공 | — | 설정·실행 오류 |
| `scan` | 분석 성공 | — | 설정·Opengrep 등 실행 오류 |
| `publish` | 게이트 통과 또는 `--fail-on-gate false` | 게이트 실패 | Gitea·결과 형식 등 실행 오류 |

측정할 수 있는 게이트 조건이 하나도 없으면 경고를 출력하고 통과합니다. 측정되지 않은 지표는 0으로 취급하지 않습니다. 다만 `coverage.required = true`인데 커버리지를 얻지 못하면 커버리지 게이트 조건은 실패합니다.

## 이슈와 핫스팟

규칙 위반은 규칙과 파일별로, 중복은 복제 그룹별로 이슈를 만듭니다. 저장소당 요약 이슈에는 게이트 조건별 실제값·기준·결과, 지표, 설정 출처와 사용한 룰셋이 표시됩니다.

- 핫스팟 이슈 본문에 검토 방법이 있습니다. 안전하다고 판단한 이슈에 `qualgate:safe` 라벨을 붙이면 신규 핫스팟 검토율에 반영됩니다. 라벨이 붙은 이슈는 다시 열거나 본문을 갱신하지 않으며, 코드에서 위반이 사라지면 닫습니다.
- 룰셋에서 비활성화한 규칙의 기존 이슈는 수정 완료로 표시하지 않고, 룰셋에서 제외됐다는 코멘트와 함께 닫습니다.
- `qualgate:ignored` 라벨은 해당 이슈의 자동 변경을 막습니다.
- 기본 브랜치가 아니면 이슈는 읽기만 하고 동기화하지 않습니다. 게이트 결과는 계속 계산합니다.
- 분석하지 못한 파일과 기존 위반이 겹치면 해당 이슈를 닫지 않습니다.

## 언어와 커버리지

Java·Kotlin은 JaCoCo XML, Python은 Cobertura XML, React·Vue는 LCOV, Go는 coverprofile을 읽습니다. 기본 검색 경로와 분석 범위는 설정 저장소의 프로젝트 설정 및 [설정 형식 문서](docs/config-format.md)를 참고하세요. 테스트는 qualgate가 실행하지 않으므로 파이프라인의 테스트 단계에서 리포트를 먼저 만들어야 합니다.

## Tekton 동작 범위

Task는 source, basic-auth, 선택 CA workspace를 사용하고 결과 파일은 `/work` `emptyDir`에 둡니다. `scan` 컨테이너에는 Gitea 인증·CA 환경 변수를 전달하지 않지만, Tekton은 Task에 바인딩한 workspace를 단계 사이에서 완전히 격리하지 않습니다. 따라서 `scan`도 workspace의 자격 증명 파일에 접근할 수 있습니다. 세 단계는 같은 Pod의 네트워크도 공유합니다. 이 구성은 환경 변수 전달 범위만 줄이며 강한 보안 경계를 만들지는 않습니다.

클러스터의 Task 등록과 PipelineRun은 해당 Kubernetes/Tekton 환경에서 확인해야 합니다. 로컬 구조 테스트는 `bash scripts/cargo.sh test --test tekton_yaml`로 실행합니다.

## 제한과 출처

- 이미지는 amd64를 지원합니다.
- 기본 규칙은 행정안전부 「소프트웨어 보안약점 진단가이드」를 기준으로 작성했습니다. 대응 범위는 [docs/mois49.md](docs/mois49.md)에 있습니다. 오탐률은 사내 저장소 검증이 더 필요합니다.
- SonarSource/sonarqube의 이슈 추적과 중복 블록 알고리즘을 LGPL-3.0 조건에 따라 포팅했습니다. Opengrep은 수정 없이 외부 프로세스로 실행합니다. 추가 출처와 라이선스는 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)에 있습니다.
- 프로젝트 라이선스는 LGPL-3.0입니다([LICENSE](LICENSE)).
