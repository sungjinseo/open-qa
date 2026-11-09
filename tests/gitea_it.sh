#!/usr/bin/env bash
# 실제 Gitea로 생성, 재실행 무변화, 종료, 재오픈(코멘트 포함), 무시 라벨을 검증한다.
set -euo pipefail
. "$(dirname "$0")/lib/gitea_local.sh"
work=""; out=""
cleanup() {
  gitea_local_cleanup
  [ -z "$work" ] || rm -rf "$work"
  [ -z "$out" ] || rm -rf "$out"
}
trap cleanup EXIT
gitea_local_start gitea/gitea:1 qg-it
gitea_local_admin_user qg qgpass1234
ADMIN=$(gitea_local_token qg admin all)
TOKEN=$(gitea_local_token qg it "${QG_TOKEN_SCOPES:-write:issue,read:repository}")
curl -sf -H "Authorization: token $ADMIN" -H 'Content-Type: application/json' -d '{"name":"demo","default_branch":"main"}' "localhost:$PORT/api/v1/user/repos" >/dev/null

work="$(mktemp -d)"; out="$(mktemp -d)"; chmod 777 "$out"
w="$(cd "$work" && (pwd -W 2>/dev/null || pwd))"; o="$(cd "$out" && (pwd -W 2>/dev/null || pwd))"
git -C "$w" init -qb main
commit() { git -C "$w" add -A; git -C "$w" -c user.email=t@e -c user.name=t commit -qm "$1"; }
bad() { printf 'x = 1\neval(input())\n' > "$work/a.py"; }
good() { printf 'x = 1\n' > "$work/a.py"; }
bad; commit one
publish() {
  docker run --rm --network none -v "$w:/src:ro" -v "$o:/out" qualgate:dev scan || [ $? -eq 1 ]
  docker run --rm --network "$NET" -v "$o:/out:ro" -e GITEA_URL="http://$GITEA:3000" -e GITEA_TOKEN="$TOKEN" qualgate:dev publish --repo qg/demo || [ $? -eq 1 ]
}
base="localhost:$PORT/api/v1/repos/qg/demo"
api() { curl -sf -H "Authorization: token $ADMIN" "$base/issues?state=$1&type=issues&limit=50"; }
count() { api "$1" | python -I -c "import json,sys; print(len(json.load(sys.stdin)))"; }
expect() { [ "$2" = "$3" ] && echo "ok: $1" || { echo "FAIL: $1 기대=$3 실제=$2"; exit 1; }; }
rule_issue() { python -I -c "import json,sys; print(next(i['number'] for i in json.load(sys.stdin) if '품질 요약' not in i['title']))"; }

publish
expect "생성: 규칙 + 요약" "$(count open)" 2
before=$(api all | python -I -c "import json,sys; print(sorted(i['updated_at'] for i in json.load(sys.stdin)))")
publish
after=$(api all | python -I -c "import json,sys; print(sorted(i['updated_at'] for i in json.load(sys.stdin)))")
expect "재실행 무변화" "$after" "$before"

good; commit fix
publish
expect "수정 후 종료" "$(count closed)" 1

bad; commit again
publish
expect "재발 시 재오픈" "$(count closed)" 0
reopen_no=$(api open | rule_issue)
reopen_comments=$(curl -sf -H "Authorization: token $ADMIN" "$base/issues/$reopen_no/comments" | python -I -c "import json,sys; print(sum('다시 열었습니다' in c['body'] for c in json.load(sys.stdin)))")
expect "재오픈 코멘트" "$reopen_comments" 1

# 무시 라벨: 닫힌 규칙 이슈에 qualgate:ignored를 달면 재발해도 다시 열거나 건드리지 않는다.
good; commit fix2
publish
expect "무시 라벨 준비: 종료" "$(count closed)" 1
rule_no=$(api closed | rule_issue)
admin_post() { curl -sf -H "Authorization: token $ADMIN" -H 'Content-Type: application/json' -d "$2" "$base/$1"; }
label_id=$(curl -sf -H "Authorization: token $ADMIN" "$base/labels?limit=50" | python -I -c "import json,sys; print(next((l['id'] for l in json.load(sys.stdin) if l['name']=='qualgate:ignored'), ''))")
[ -n "$label_id" ] || label_id=$(admin_post labels '{"name":"qualgate:ignored","color":"#cccccc"}' | python -I -c "import json,sys; print(json.load(sys.stdin)['id'])")
admin_post "issues/$rule_no/labels" "{\"labels\":[$label_id]}" >/dev/null
before=$(api closed | python -I -c "import json,sys; print(sorted((i['number'], i['updated_at']) for i in json.load(sys.stdin) if i['number']==$rule_no))")
bad; commit again2
publish
expect "무시 라벨: 다시 열지 않음" "$(count closed)" 1
after=$(api closed | python -I -c "import json,sys; print(sorted((i['number'], i['updated_at']) for i in json.load(sys.stdin) if i['number']==$rule_no))")
expect "무시 라벨: 이슈 무변화" "$after" "$before"
echo "통합 테스트 통과 (토큰 범위: ${QG_TOKEN_SCOPES:-write:issue,read:repository})"
