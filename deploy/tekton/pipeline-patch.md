# 공통 빌드 파이프라인에 qualgate 연결

`build-common-image-gitops` 파이프라인의 기존 `code-analysis` 태스크를 아래 블록으로 바꿉니다. 테스트 태스크가 만든 커버리지 리포트를 읽도록 `runAfter: [code-test]`를 유지합니다.

```yaml
  - displayName: Code Analysis
    name: code-analysis
    params:
      - name: IMAGE
        value: '{{image_registry}}/nnd/tekton-cicd/qualgate:0.1.0'
      - name: CONTEXT_DIR
        value: $(params.working-pr-dir)/$(params.code-repo-name)
      - name: GIT_HOST_URL
        value: $(params.git-host-url)
      - name: REPO_ORG
        value: $(params.code-repo-org)
      - name: REPO_NAME
        value: $(params.code-repo-name)
      - name: REVISION
        value: $(params.code-revision)
      - name: COMMIT
        value: $(tasks.code-fetch.results.commit)
      - name: CONFIG_REPO
        value: $(params.qualgate-config-repo)
      - name: CONFIG_REF
        value: main
      - name: CRT_FILENAME
        value: ca-certificates.crt
      - name: SSL_VERIFY
        value: 'false'
      - name: FAIL_ON_GATE
        value: $(params.qualgate-fail-on-gate)
    runAfter:
      - code-test
    taskRef:
      resolver: cluster
      params:
        - name: kind
          value: task
        - name: name
          value: qualgate
        - name: namespace
          value: tekton-catalog
    when:
      - input: $(params.skip-code-analysis)
        operator: in
        values:
          - 'false'
    workspaces:
      - name: source
        workspace: shared-data
      - name: basic-auth
        workspace: gitconfig
      - name: ssl-ca-directory
        workspace: ssl-ca-directory
```

`SSL_VERIFY`는 `code-fetch`의 `sslVerify`와 같은 값이어야 합니다. 예시는 기존 `'false'` 값에 맞췄습니다. 인증서 검증을 켜면 `'true'`로 바꾸고 `ssl-ca-directory` workspace에 CA 인증서를 연결합니다.

파이프라인 `spec.params`에 다음 파라미터를 추가합니다.

```yaml
  - name: qualgate-config-repo
    type: string
    default: jenny/qualgate-config
    description: Central qualgate configuration repository (owner/repo)
  - name: qualgate-fail-on-gate
    type: string
    default: 'true'
    description: Fail the pipeline when the quality gate fails
```

`failure-issue-skip-tasks` 기본값에 `code-analysis`를 포함합니다. qualgate가 상세 이슈를 등록하므로 마지막 Failure Issue 태스크가 같은 실패를 중복 등록하지 않게 합니다.

```yaml
  - name: failure-issue-skip-tasks
    type: string
    default: code-analysis
```

필요하면 다른 태스크 이름도 공백으로 이어 붙입니다(예: `code-test code-analysis image-scan`). `code-fetch`는 변경하지 않습니다. qualgate는 커밋 결과와 Gitea 이력으로 얕은 클론의 신규 코드 범위를 계산합니다.

Task를 카탈로그 네임스페이스에 등록합니다.

```bash
kubectl apply -n tekton-catalog -f deploy/tekton/task-qualgate.yaml
```

`qualgate-fail-on-gate`가 `'false'`이면 게이트 실패에도 `code-analysis` 태스크가 성공하고, `gate` 결과에 `failed`를 씁니다.
