#!/usr/bin/env bash
# 대상 버전(gitea/gitea:1.26.4)의 파일/커밋/diff API 응답을 실측해 출력한다. 로컬 일회용 컨테이너만 쓴다.
set -eu
. "$(dirname "$0")/lib/gitea_local.sh"
work=""; BODY="$(cygpath -m "$(mktemp)" 2>/dev/null || mktemp)"
cleanup() {
  gitea_local_cleanup
  [ -z "$work" ] || rm -rf "$work"
  rm -f "$BODY"
}
trap cleanup EXIT
gitea_local_start gitea/gitea:1.26.4 qg-probe
gitea_local_admin_user qg qgpass1234
ADMIN=$(gitea_local_token qg admin all)
# 읽기 전용 토큰: 운영 토큰과 같은 최소 범위로 diff 경로의 인증을 확인한다.
RO=$(gitea_local_token qg ro "read:repository,read:issue")
ROOT="http://127.0.0.1:$PORT"
V="$ROOT/api/v1"
GURL="http://qg:qgpass1234@127.0.0.1:$PORT/qg/app.git"
echo "== version: $(curl -s "$V/version")"

# api METHOD PATH [json]: 상태 코드와 본문 앞부분을 출력한다.
api() {
  local m="$1" p="$2" d="${3:-}" tok="${TOK:-$ADMIN}"
  echo "--- $m $p"
  if [ -n "$d" ]; then
    curl -s -o "$BODY" -w "status=%{http_code} type=%{content_type}\n" -X "$m" -H "Authorization: token $tok" -H 'Content-Type: application/json' -d "$d" "$V$p"
  else
    curl -s -o "$BODY" -w "status=%{http_code} type=%{content_type}\n" -X "$m" -H "Authorization: token $tok" "$V$p"
  fi
  head -c "${HEADN:-700}" "$BODY"; echo
}
# raw URL [token]: 인증 헤더 유무와 상태 코드, 본문 앞부분을 출력한다.
raw() {
  echo "--- GET $1 (auth: ${2:+token})"
  if [ -n "${2:-}" ]; then
    curl -s -o "$BODY" -w "status=%{http_code} type=%{content_type} size=%{size_download}\n" -H "Authorization: token $2" "$1"
  else
    curl -s -o "$BODY" -w "status=%{http_code} type=%{content_type} size=%{size_download}\n" "$1"
  fi
  head -c "${HEADN:-500}" "$BODY"; echo
}
jget() { python -I -c "import json,sys; d=json.load(sys.stdin); print($1)" < "$BODY"; }
b64() { printf '%s' "$1" | base64 | tr -d '\n'; }

api POST /user/repos '{"name":"cfg","default_branch":"main","auto_init":true}' >/dev/null
api POST /user/repos '{"name":"app","default_branch":"main"}' >/dev/null
echo "=== A. config repo file API"
api POST /repos/qg/cfg/contents/qualgate.toml "{\"content\":\"$(b64 'version = 1')\",\"message\":\"add qualgate.toml\",\"branch\":\"main\"}" | head -3
api POST /repos/qg/cfg/contents/rulesets/a.toml "{\"content\":\"$(b64 'name = "a"')\",\"message\":\"add a\",\"branch\":\"main\"}" | head -1
api POST /repos/qg/cfg/contents/projects/qg/app.toml "{\"content\":\"$(b64 'repo = "qg/app"')\",\"message\":\"add app\",\"branch\":\"main\"}" | head -1
HEADN=900 api GET "/repos/qg/cfg/contents/qualgate.toml?ref=main"
echo "content decodes to: $(jget "d['content']" | base64 -d) | encoding=$(jget "d['encoding']") type=$(jget "d['type']") sha=$(jget "d['sha']")"
api GET "/repos/qg/cfg/contents/projects/qg/app.toml?ref=main" | head -2
api GET "/repos/qg/cfg/contents/rulesets/none.toml?ref=main"
HEADN=400 api GET "/repos/qg/cfg/contents/rulesets?ref=main"
api GET "/repos/qg/cfg/contents/qualgate.toml?ref=nobranch" | head -3
TOK=$RO api GET "/repos/qg/cfg/contents/qualgate.toml?ref=main" | head -2
echo "-- raw endpoint"
raw "$V/repos/qg/cfg/raw/qualgate.toml?ref=main" "$RO"
HEADN=400 api GET "/repos/qg/cfg/commits?sha=main&limit=1"
echo "cfg head sha: $(jget "d[0]['sha']")"
curl -s -D - -o /dev/null -H "Authorization: token $ADMIN" "$V/repos/qg/cfg/commits?sha=main&limit=1" | grep -i -E 'x-total|link' || echo "(no x-total/link headers)"

echo "=== B. dated commits (author and committer dates differ on purpose)"
work="$(mktemp -d)"
w="$(cd "$work" && (pwd -W 2>/dev/null || pwd))"
git -C "$w" init -qb main
git -C "$w" config core.autocrlf false
mk() { # name author_date committer_date
  printf '%s\n' "$1" >> "$work/f.txt"; printf 'line %s\n' "$1" > "$work/$1.txt"
  git -C "$w" add -A
  GIT_AUTHOR_DATE="$2" GIT_COMMITTER_DATE="$3" git -C "$w" -c user.email=t@e -c user.name=t commit -qm "c-$1"
}
mk c1 "2020-01-10T12:00:00+00:00" "2020-01-10T12:00:00+00:00"
mk c2 "2020-01-05T12:00:00+00:00" "2020-03-10T12:00:00+00:00"
mk c3 "2020-05-10T12:00:00+00:00" "2020-05-10T12:00:00+00:00"
git -C "$w" push -q "$GURL" main 2>&1 | tail -2
# 푸시 직후 한동안 commits API가 409(Git Repository is empty)를 돌려주는지 측정한다.
n409=0; st=""
for _ in $(seq 40); do
  st=$(curl -s -o "$BODY" -w "%{http_code}" -H "Authorization: token $ADMIN" "$V/repos/qg/app/commits?sha=main&limit=1")
  [ "$st" = 200 ] && break; n409=$((n409+1)); sleep 0.5
done
[ "$st" = 200 ] || { echo "FAIL: qg/app never became ready after push (last status $st)"; exit 1; }
echo "after push: commits API non-200 polls (0.5s each) before 200: $n409 (last status $st)"
C1=$(git -C "$w" rev-parse HEAD~2); C2=$(git -C "$w" rev-parse HEAD~1); C3=$(git -C "$w" rev-parse HEAD)
echo "c1=$C1 (A=C=01-10) c2=$C2 (author 01-05, committer 03-10) c3=$C3 (05-10)"
until_list() {
  local st
  st=$(curl -s -o "$BODY" -w "%{http_code}" -H "Authorization: token $ADMIN" "$V/repos/qg/app/commits?sha=main&limit=${2:-1}&until=$1")
  echo "until=$1 limit=${2:-1} status=$st -> $(jget "[c['commit']['message'].strip() for c in d] if isinstance(d,list) else d")"
}
until_list "2020-02-01T00:00:00Z" 10
until_list "2020-02-01T00:00:00Z" 1
until_list "2020-04-01T00:00:00Z" 10
until_list "2020-04-01T00:00:00Z" 1
until_list "2020-01-07T00:00:00Z" 10
until_list "2019-01-01T00:00:00Z" 10
until_list "2020-04-01T00:00:00%2B09:00" 10
until_list "2020-04-01" 10
curl -s -H "Authorization: token $ADMIN" "$V/repos/qg/app/commits?sha=main&limit=10" > "$BODY"
echo "no until: $(jget "[(c['commit']['message'].strip(), c['commit']['author']['date'], c['commit']['committer']['date'], c['created']) for c in d]")"
curl -s -H "Authorization: token $ADMIN" "$V/repos/qg/app/commits?sha=main&limit=10&since=2020-02-01T00:00:00Z" > "$BODY"
echo "since=2020-02-01 -> $(jget "[c['commit']['message'].strip() for c in d]")"

echo "=== C. range diff"
HEADN=2500 api GET "/repos/qg/app/compare/$C1...$C3"
echo "compare keys: $(jget "sorted(d.keys())")"
echo "compare total_commits=$(jget "d.get('total_commits')") top-level files present: $(jget "'files' in d") per-commit files: $(jget "[c['files'] for c in d['commits']]")"
echo "-- two dots"
api GET "/repos/qg/app/compare/$C1..$C3" | head -2
echo "-- compare via RO token"
TOK=$RO api GET "/repos/qg/app/compare/$C1...$C3" | head -2
echo "-- compare via branch names"
git -C "$w" branch -q base "$C1"; git -C "$w" push -q "$GURL" base 2>&1 | tail -1
api GET "/repos/qg/app/compare/base...main" | head -2
echo "-- .diff/.patch variants under API"
raw "$V/repos/qg/app/compare/$C1...$C3.diff" "$RO"
raw "$V/repos/qg/app/compare/$C1...$C3.patch" "$RO"
echo "-- single commit diff (API)"
raw "$V/repos/qg/app/git/commits/$C3.diff" "$RO"
raw "$V/repos/qg/app/git/commits/$C3.patch" "$RO"
echo "-- web path diff"
raw "$ROOT/qg/app/compare/$C1...$C3.diff" "$RO"
raw "$ROOT/qg/app/compare/$C1...$C3.diff" "$ADMIN"
raw "$ROOT/qg/app/commit/$C3.diff" "$RO"
echo "-- web diff without auth"
raw "$ROOT/qg/app/compare/$C1...$C3.diff"
echo "-- web diff with basic auth"
curl -s -o "$BODY" -w "status=%{http_code} size=%{size_download}\n" -u qg:qgpass1234 "$ROOT/qg/app/compare/$C1...$C3.diff"; head -c 300 "$BODY"; echo
echo "-- web diff: Bearer / ?token="
curl -s -o "$BODY" -w "bearer status=%{http_code} size=%{size_download}\n" -H "Authorization: Bearer $RO" "$ROOT/qg/app/compare/$C1...$C3.diff"
curl -s -o "$BODY" -w "?token= status=%{http_code} size=%{size_download}\n" "$ROOT/qg/app/compare/$C1...$C3.diff?token=$RO"

echo "-- extra candidate range-diff paths"
for u in "$V/repos/qg/app/git/commits/$C1...$C3.diff" "$V/repos/qg/app/compare/$C1...$C3?files=true" "$V/repos/qg/app/commits/$C3/diff" "$V/repos/qg/app/compare/$C1...$C3/diff" "$ROOT/qg/app/compare/$C1...$C3" "$ROOT/qg/app/compare/$C1...$C3.patch" "$V/repos/qg/app/pulls/1.diff"; do
  raw "$u" "$RO" | head -2
done
echo "-- PR method: create PR base<-main, fetch pulls/N.diff"
HEADN=200 api POST /repos/qg/app/pulls '{"base":"base","head":"main","title":"probe range"}'
echo "pr number: $(jget "d.get('number')")"
raw "$V/repos/qg/app/pulls/1.diff" "$RO" | head -2
echo "pr diff files: $(grep -c '^diff --git' "$BODY"); lines=$(wc -l < "$BODY")"
raw "$V/repos/qg/app/pulls/1.patch" "$RO" | head -2
HEADN=600 api GET /repos/qg/app/pulls/1/files
echo "pulls/1/files: $(jget "[(f['filename'], f['status'], 'patch' in f) for f in d]")"
curl -s -D - -o /dev/null -H "Authorization: token $RO" "$V/repos/qg/app/pulls/1/files" | grep -i -E 'x-total|link|x-hasmore' || true
raw "$V/repos/qg/app/pulls/1/commits" "$RO" | head -2
echo "-- per-commit diff approach: compare commits then each .diff"
echo "=== D. large diff and limits"
python -I -c "print('\n'.join('line %d %s' % (i,'x'*60) for i in range(60000)))" > "$work/big.txt"
git -C "$w" add -A; git -C "$w" -c user.email=t@e -c user.name=t commit -qm big
git -C "$w" push -q "$GURL" main 2>&1 | tail -1
C4=$(git -C "$w" rev-parse HEAD)
echo "big file size: $(wc -c < "$work/big.txt") bytes, 60000 lines"
HEADN=300 api GET "/repos/qg/app/compare/$C3...$C4" >/dev/null
echo "compare keys: $(jget "sorted(d.keys())") per-commit files: $(jget "[c['files'] for c in d['commits']]")"
raw "$ROOT/qg/app/compare/$C3...$C4.diff" "$RO" | head -2
echo "web diff size: $(wc -c < "$BODY"); lines: $(wc -l < "$BODY"); head: $(head -c 200 "$BODY" | head -3 | tr '\n' '|')"
raw "$V/repos/qg/app/git/commits/$C4.diff" "$RO" | head -2
echo "commit diff size: $(wc -c < "$BODY"); lines: $(wc -l < "$BODY")"
raw "$ROOT/qg/app/compare/$C1...$C4.diff" "$RO" | head -1
echo "range C1...C4 web diff size: $(wc -c < "$BODY")"
echo "-- many files"
for i in $(seq 1 35); do printf 'f%s\n' "$i" > "$work/many_$i.txt"; done
git -C "$w" add -A; git -C "$w" -c user.email=t@e -c user.name=t commit -qm many
git -C "$w" push -q "$GURL" main 2>&1 | tail -1
C5=$(git -C "$w" rev-parse HEAD)
api GET "/repos/qg/app/compare/$C4...$C5" >/dev/null
echo "files in compare: keys=$(jget "sorted(d.keys())")"
echo "total_commits=$(jget "d['total_commits']") per-commit files=$(jget "[len(c['files']) for c in d['commits']]")"
echo "web diff files: $(curl -s -H "Authorization: token $RO" "$ROOT/qg/app/compare/$C4...$C5.diff" | grep -c '^diff --git')"

echo "=== E. issue labels"
api POST /repos/qg/app/labels '{"name":"qualgate:ignored","color":"#cccccc"}' | head -3
api POST /repos/qg/app/issues '{"title":"t","body":"b"}' | head -2
LID=$(curl -s -H "Authorization: token $ADMIN" "$V/repos/qg/app/labels" | python -I -c "import json,sys; print(json.load(sys.stdin)[0]['id'])")
api POST /repos/qg/app/issues/1/labels "{\"labels\":[$LID]}" | head -3
api POST /repos/qg/app/issues/1/labels '{"labels":["qualgate:ignored"]}' | head -3
HEADN=300 api GET "/repos/qg/app/issues?state=all&type=issues&labels=qualgate:ignored" | head -4
curl -s -H "Authorization: token $ADMIN" "$V/repos/qg/app/issues/1" > "$BODY"
echo "issue labels: $(jget "[(l['name'], l['id']) for l in d['labels']]")"
echo "probe done"
