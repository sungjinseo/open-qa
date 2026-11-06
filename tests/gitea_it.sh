#!/usr/bin/env bash
# 실제 Gitea로 생성, 재실행 무변화, 종료, 재오픈, 무시 라벨을 검증한다.
set -euo pipefail
export MSYS_NO_PATHCONV=1
NET=qg-it
docker network create "$NET" >/dev/null
cleanup() { docker rm -f qg-gitea >/dev/null 2>&1 || true; docker network rm "$NET" >/dev/null 2>&1 || true; }
trap cleanup EXIT
docker run -d --name qg-gitea --network "$NET" -p 127.0.0.1:3999:3000 -e GITEA__security__INSTALL_LOCK=true -e GITEA__database__DB_TYPE=sqlite3 gitea/gitea:1 >/dev/null
until curl -sf localhost:3999/api/healthz >/dev/null; do sleep 2; done
docker exec -u git qg-gitea gitea admin user create --admin --username qg --password qgpass1234 --email qg@example.com >/dev/null
ADMIN=$(docker exec -u git qg-gitea gitea admin user generate-access-token --username qg --token-name admin --scopes all --raw)
TOKEN=$(docker exec -u git qg-gitea gitea admin user generate-access-token --username qg --token-name it --scopes "${QG_TOKEN_SCOPES:-write:issue,read:repository}" --raw)
curl -sf -H "Authorization: token $ADMIN" -H 'Content-Type: application/json' -d '{"name":"demo","default_branch":"main"}' localhost:3999/api/v1/user/repos >/dev/null

work="$(mktemp -d)"; out="$(mktemp -d)"; chmod 777 "$out"
w="$(cd "$work" && (pwd -W 2>/dev/null || pwd))"; o="$(cd "$out" && (pwd -W 2>/dev/null || pwd))"
git -C "$w" init -qb main
printf 'x = 1\neval(input())\n' > "$work/a.py"
git -C "$w" add -A; git -C "$w" -c user.email=t@e -c user.name=t commit -qm one
publish() {
  docker run --rm --network none -v "$w:/src:ro" -v "$o:/out" qualgate:dev scan || [ $? -eq 1 ]
  docker run --rm --network "$NET" -v "$o:/out:ro" -e GITEA_URL=http://qg-gitea:3000 -e GITEA_TOKEN="$TOKEN" qualgate:dev publish --repo qg/demo || [ $? -eq 1 ]
}
api() { curl -sf -H "Authorization: token $ADMIN" "localhost:3999/api/v1/repos/qg/demo/issues?state=$1&type=issues&limit=50"; }
count() { api "$1" | python -I -c "import json,sys; print(len(json.load(sys.stdin)))"; }
expect() { [ "$2" = "$3" ] && echo "ok: $1" || { echo "FAIL: $1 기대=$3 실제=$2"; exit 1; }; }

publish
expect "생성: 규칙 + 요약" "$(count open)" 2
before=$(api all | python -I -c "import json,sys; print(sorted(i['updated_at'] for i in json.load(sys.stdin)))")
publish
after=$(api all | python -I -c "import json,sys; print(sorted(i['updated_at'] for i in json.load(sys.stdin)))")
expect "재실행 무변화" "$after" "$before"

printf 'x = 1\n' > "$work/a.py"; git -C "$w" add -A; git -C "$w" -c user.email=t@e -c user.name=t commit -qm fix
publish
expect "수정 후 종료" "$(count closed)" 1

printf 'x = 1\neval(input())\n' > "$work/a.py"; git -C "$w" add -A; git -C "$w" -c user.email=t@e -c user.name=t commit -qm again
publish
expect "재발 시 재오픈" "$(count closed)" 0
# 무시 라벨: 닫힌 규칙 이슈에 qualgate:ignored를 달면 재발해도 다시 열거나 건드리지 않는다.
printf 'x = 1
' > "$work/a.py"; git -C "$w" add -A; git -C "$w" -c user.email=t@e -c user.name=t commit -qm fix2
publish
expect "무시 라벨 준비: 종료" "$(count closed)" 1
rule_no=$(api closed | python -I -c "import json,sys; print(next(i['number'] for i in json.load(sys.stdin) if '품질 요약' not in i['title']))")
admin_post() { curl -sf -H "Authorization: token $ADMIN" -H 'Content-Type: application/json' -d "$2" "localhost:3999/api/v1/repos/qg/demo/$1"; }
label_id=$(curl -sf -H "Authorization: token $ADMIN" "localhost:3999/api/v1/repos/qg/demo/labels?limit=50" | python -I -c "import json,sys; print(next((l['id'] for l in json.load(sys.stdin) if l['name']=='qualgate:ignored'), ''))")
[ -n "$label_id" ] || label_id=$(admin_post labels '{"name":"qualgate:ignored","color":"#cccccc"}' | python -I -c "import json,sys; print(json.load(sys.stdin)['id'])")
admin_post "issues/$rule_no/labels" "{\"labels\":[$label_id]}" >/dev/null
before=$(api closed | python -I -c "import json,sys; print(sorted((i['number'], i['updated_at']) for i in json.load(sys.stdin) if i['number']==$rule_no))")
printf 'x = 1
eval(input())
' > "$work/a.py"; git -C "$w" add -A; git -C "$w" -c user.email=t@e -c user.name=t commit -qm again2
publish
expect "무시 라벨: 다시 열지 않음" "$(count closed)" 1
after=$(api closed | python -I -c "import json,sys; print(sorted((i['number'], i['updated_at']) for i in json.load(sys.stdin) if i['number']==$rule_no))")
expect "무시 라벨: 이슈 무변화" "$after" "$before"
echo "통합 테스트 통과 (토큰 범위: ${QG_TOKEN_SCOPES:-write:issue,read:repository})"
