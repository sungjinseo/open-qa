# Gitea 1.26.4 API 실측 결과

`bash tests/gitea_probe.sh`로 로컬 일회용 `gitea/gitea:1.26.4` 컨테이너(127.0.0.1)에서 측정했다. 토큰은 관리자(all)와 읽기 전용(`read:repository,read:issue`) 두 가지를 썼다. `ro`로 표시한 것은 읽기 전용 토큰으로 성공한 호출이다. `/api/v1`은 아래에서 `V`로 줄여 쓴다.

## 1. 설정 저장소 파일 API

| 호출 | 상태 | 핵심 |
|---|---|---|
| `GET V/repos/qg/cfg/contents/qualgate.toml?ref=main` (ro) | 200 | JSON 객체: `type:"file"`, `encoding:"base64"`, `content`(base64), `sha`(blob), `size`, `path`, `last_commit_sha` |
| `GET V/repos/qg/cfg/contents/projects/qg/app.toml?ref=main` | 200 | 위와 같음(깊은 경로 가능) |
| `GET V/repos/qg/cfg/contents/rulesets/none.toml?ref=main` | 404 | JSON `{"message":"GetContentsOrList","errors":["object does not exist [...]"]}` |
| `GET V/repos/qg/cfg/contents/rulesets?ref=main` | 200 | 디렉터리는 JSON 배열. 항목은 `type:"file"`, `name`, `path`, `sha`, `content:null`, `encoding:null` |
| `GET V/repos/qg/cfg/contents/qualgate.toml?ref=nobranch` | 404 | `{"errors":["object does not exist [id: nobranch...]"],"message":"not found"}` |
| `GET V/repos/qg/cfg/raw/qualgate.toml?ref=main` (ro) | 200 | `text/plain`, 본문이 파일 원문(base64 아님) |
| `GET V/repos/qg/cfg/commits?sha=main&limit=1` | 200 | 배열, 첫 원소 `sha`가 설정 커밋. 헤더 `X-Total`, `X-Total-Count`, `Link` 있음 |

- 없는 파일과 없는 ref 모두 404지만 `message`가 다르다(`GetContentsOrList` / `not found`). 파일 없음 판별은 상태 코드 404로 한다.
- 설정 커밋 sha는 `commits?sha=main&limit=1`의 `[0].sha`. 같은 응답에서 `contents`의 `last_commit_sha`는 그 파일을 마지막으로 바꾼 커밋이라 저장소 HEAD와 다르다. 설정 전체의 버전 표시는 `commits`의 sha를 쓴다.
- 쓰기(`POST contents/...`)는 201.

## 2. `commits?until=`은 커밋터 날짜 기준

커밋 3개를 만들었다: c1(author=committer=2020-01-10), c2(author 2020-01-05, committer 2020-03-10), c3(2020-05-10). author와 committer 날짜를 다르게 한 이유가 이 구분이다.

| 호출 `commits?sha=main&until=...` | 결과 |
|---|---|
| `until=2020-02-01T00:00:00Z&limit=10` | `[c1]` |
| `until=2020-04-01T00:00:00Z&limit=10` | `[c2, c1]` (최신순) |
| `until=2020-04-01T00:00:00Z&limit=1` | `[c2]` (기준 커밋 1개를 얻는 방법) |
| `until=2020-01-07T00:00:00Z` | `[]` (c2의 author 날짜 01-05는 무시됨) |
| `until=2019-01-01T00:00:00Z` | `[]`, 200 |
| `until=2020-04-01T00:00:00%2B09:00` | `[c2, c1]` (오프셋 지원, `+`는 `%2B`로 인코딩) |
| `until=2020-04-01` | 422 `invalid 'until' format, expected ISO 8601 (RFC3339)` |
| `since=2020-02-01T00:00:00Z` | `[c3, c2]` |

결론: `until`은 committer 날짜(`commit.committer.date`, 최상위 `created`와 같은 값) 기준이다. RFC3339만 허용한다. 응답은 커밋터 날짜 내림차순이 아니라 git log 순서(위상 순서)이며 이 테스트에서는 둘이 같았다.

주의: 푸시 직후 약 1.5초(0.5초 간격 폴링 3회) 동안 `commits`가 `409 {"message":"Git Repository is empty."}`를 돌려줬다. 저장소를 막 푸시하는 테스트와 자동화는 200이 될 때까지 재시도해야 한다. `until` 결과가 아닌 저장소 상태 때문이다.

## 3. 범위 diff(base...head)

| 호출 | 상태 | 결과 |
|---|---|---|
| `GET V/repos/qg/app/compare/<base>...<head>` (ro) | 200 | `{"total_commits":N,"commits":[...]}`. 최상위 `files`와 `patch` 없음. 커밋마다 `files:[{filename,status}]`, `stats`만 있고 patch 없음. `..`(점 2개)과 브랜치명(`base...main`)도 200 |
| `GET V/repos/qg/app/compare/<base>...<head>.diff` / `.patch` (ro) | 404 | `{"message":"not found"}` |
| `GET V/repos/qg/app/compare/<base>...<head>?files=true` | 200 | 위와 같음, 달라지지 않음 |
| `GET V/repos/qg/app/compare/<base>...<head>/diff` | 404 | |
| `GET V/repos/qg/app/commits/<sha>/diff` | 404 | |
| `GET V/repos/qg/app/git/commits/<base>...<head>.diff` | 422 | 범위 불가 |
| `GET <root>/qg/app/compare/<base>...<head>.diff` (토큰 ro, 관리자, 없음, Basic, Bearer, `?token=`) | 404 | 웹 경로 범위 diff는 인증과 무관하게 `Not found.` |
| `GET <root>/qg/app/compare/<base>...<head>.patch` | 404 | |
| `GET <root>/qg/app/compare/<base>...<head>` | 200 | HTML(71KB), diff 아님 |
| `GET V/repos/qg/app/git/commits/<sha>.diff` (ro) | 200 | `text/plain`, git diff 원문(`diff --git ...`). 단일 커밋 |
| `GET V/repos/qg/app/git/commits/<sha>.patch` (ro) | 200 | `text/plain`, mbox 형식 patch |
| `GET <root>/qg/app/commit/<sha>.diff` (ro) | 200 | 단일 커밋 diff, 위와 같은 본문. 무토큰은 공개 저장소에서만 의미 있음 |

결론: 1.26.4에는 토큰으로 한 번에 받는 base...head 통합 diff 경로가 없다(compare API는 patch를 주지 않고 `.diff`는 404). 대안은 다음 둘이다.

### 대안 A(권장): 커밋별 diff 이어 붙이기, 호출 1 + N회

1. `GET V/repos/o/r/compare/<base>...<head>`로 `commits[]`를 얻는다(`total_commits`로 개수, 최신이 앞). 각 항목의 `sha`, `parents`, `files`를 쓴다.
2. 각 커밋마다 `GET V/repos/o/r/git/commits/<sha>.diff`(ro, 200, `text/plain`).
3. 오래된 순으로 이어 붙인다. 새 코드 판정용이면 커밋별 hunk를 그대로 합쳐 파일별로 추가 행(`+`)을 모으면 충분하다. 같은 줄을 여러 커밋이 고쳐도 중간 상태 줄이 나타날 수 있어, 순 효과(base→head) diff와는 다르다.
4. 병합 커밋은 `.diff`가 첫 부모 기준인지 이 프로브에서 측정하지 않았다(선형 이력만 측정).

### 대안 B: 임시 PR로 순 효과 diff 한 번에

1. `POST V/repos/o/r/pulls` `{"base":"<base 브랜치>","head":"<head 브랜치>","title":"..."}` → 201, `number`. 쓰기 권한(`write:repository`)과 브랜치 이름이 필요하다(커밋 sha 불가). 저장소에 PR이 남으므로 작업 후 닫아야 한다.
2. `GET V/repos/o/r/pulls/<n>.diff`(ro) → 200 `text/plain`, base와 head의 순 효과 통합 diff(3 파일, 22줄 확인). `.patch`도 200.
3. `GET V/repos/o/r/pulls/<n>/files` → 200 배열 `{filename,status,additions,deletions,changes}`이며 `patch`는 없다. 페이지 헤더 `X-Total-Count`, `X-HasMore`.

운영 도구가 읽기 전용 토큰으로만 동작해야 하면 B는 쓸 수 없다. A는 읽기 토큰으로 충분하다.

## 4. 대용량과 한도

- 4.37MB, 60000줄 한 파일을 추가한 커밋의 `git/commits/<sha>.diff`(ro)는 200, 4,429,013바이트, 60,006줄로 잘리지 않고 전체가 왔다(기본 설정). 별도 한도 헤더는 없었다.
- compare API는 커밋 35개 파일 추가 커밋에서 per-commit `files`가 35개 전부 나왔다(커밋당 파일 수 제한은 이 규모에서 없음). 파일 수가 매우 큰 경우는 측정하지 않았다.
- `pulls/<n>/files`는 `X-Total-Count`와 `X-HasMore` 페이지네이션 헤더가 있다(기본 한도는 이 규모에서 영향 없음).
- compare 응답의 `commits`가 페이지네이션되는지(기본 페이지 크기 초과)는 측정하지 않았다. 커밋 수가 많을 때는 `total_commits`와 받은 개수를 비교해야 한다.

## 5. 이슈 라벨 응답

| 호출 | 상태 | 응답 |
|---|---|---|
| `POST V/repos/qg/app/labels` `{"name":"qualgate:ignored","color":"#cccccc"}` | 201 | `{"id":1,"name":"qualgate:ignored","color":"cccccc","exclusive":false,...}` (`#` 제거됨) |
| `POST V/repos/qg/app/issues/1/labels` `{"labels":[1]}` | 200 | 이슈의 전체 라벨 배열 `[{id,name,color,...}]` |
| `POST V/repos/qg/app/issues/1/labels` `{"labels":["qualgate:ignored"]}` | 200 | 이름 문자열도 허용, 같은 응답(이미 있어도 중복 없음) |
| `GET V/repos/qg/app/issues?state=all&type=issues&labels=qualgate:ignored` | 200 | 이슈 배열 |
| `GET V/repos/qg/app/issues/1` | 200 | `labels:[{id,name,...}]` |

## 6. Task 8 지침 요약

- 설정 파일: `contents?ref=` JSON의 `content`를 base64 디코드. 404는 파일 없음.
- 기준 커밋: `commits?sha=<branch>&until=<RFC3339>&limit=1`의 `[0].sha`. committer 날짜 기준.
- 범위 diff: 단일 호출 경로 없음. 대안 A(compare 커밋 목록 + 커밋별 `git/commits/<sha>.diff`)를 기본으로 한다.
- 푸시 직후 409(빈 저장소)는 재시도로 처리.
