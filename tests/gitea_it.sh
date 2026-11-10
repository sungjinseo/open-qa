#!/usr/bin/env bash
# Gitea 1.26.4: centralized config, Basic auth, gate/hotspot lifecycle and shallow history.
set -euo pipefail
export MSYS_NO_PATHCONV=1
. "$(dirname "$0")/lib/gitea_local.sh"
tmp=""
PY_IMAGE="${QUALGATE_TEST_PYTHON_IMAGE:-python:3-alpine}"
py() { docker run --rm -i --network none --entrypoint python3 "$PY_IMAGE" -I "$@"; }
cleanup() {
  gitea_local_cleanup
  [ -z "$tmp" ] || {
    local tmp_docker; tmp_docker="$(cd "$tmp" && (pwd -W 2>/dev/null || pwd))"
    docker run --rm --user 0 -v "$tmp_docker:/cleanup" --entrypoint sh qualgate:dev -c 'chmod -R 777 /cleanup' >/dev/null 2>&1 || true
    rm -rf "$tmp"
  }
}
trap cleanup EXIT

gitea_local_start gitea/gitea:1.26.4 qg-it
gitea_local_admin_user qg qgpass1234
ADMIN="$(gitea_local_token qg admin all)"
create_repo() {
  curl -sf -H "Authorization: token $ADMIN" -H 'Content-Type: application/json' \
    -d "{\"name\":\"$1\",\"default_branch\":\"main\"}" "localhost:$PORT/api/v1/user/repos" >/dev/null
}
create_repo cfg
create_repo demo

tmp="$(mktemp -d)"; chmod 777 "$tmp"
cfg="$tmp/cfg"; demo="$tmp/demo"; out="$tmp/out"; shallow="$tmp/shallow"
mkdir -p "$cfg" "$demo" "$out"
chmod 777 "$cfg"; chmod 755 "$demo"; chmod 777 "$out"
creds="$tmp/.git-credentials"
# The localhost entry is for host-side test pushes; the container hostname entry is for qualgate.
printf 'http://qg:qgpass1234@localhost:%s\nhttp://qg:qgpass1234@%s:3000\n' "$PORT" "$GITEA" > "$creds"
chmod 666 "$creds"
cred_path="$(cd "$tmp" && (pwd -W 2>/dev/null || pwd))/.git-credentials"
cfg_path="$(cd "$cfg" && (pwd -W 2>/dev/null || pwd))"
demo_path="$(cd "$demo" && (pwd -W 2>/dev/null || pwd))"

docker run --rm --network none -v "$cfg_path:/config" qualgate:dev init-config --out /config
docker run --rm --user 0 -v "$cfg_path:/config" --entrypoint sh qualgate:dev -c 'chmod -R 777 /config'
mkdir -p "$cfg/projects/qg"
cat > "$cfg/projects/qg/demo.toml" <<'TOML'
schema = 1
repo = "qg/demo"
ruleset = "standard"
languages = ["python"]
new_code_days = 30

[[gate]]
metric = "new_hotspots_reviewed"
op = "lt"
value = 100.0
TOML
git -C "$cfg" init -qb main
git -C "$cfg" add -A
git -C "$cfg" -c user.email=qg@example.test -c user.name=qg commit -qm 'initialize central config'
git -C "$cfg" remote add origin "http://localhost:$PORT/qg/cfg.git"
git -c credential.helper="store --file=$cred_path" -C "$cfg" push -u origin main >/dev/null

git -C "$demo" init -qb main
cat > "$demo/app.py" <<'PY'
eval(input())
PY
git -C "$demo" add -A
GIT_AUTHOR_DATE='2020-01-01T00:00:00Z' GIT_COMMITTER_DATE='2020-01-01T00:00:00Z' \
  git -C "$demo" -c user.email=qg@example.test -c user.name=qg commit -qm 'initial sample'
git -C "$demo" remote add origin "http://localhost:$PORT/qg/demo.git"
git -c credential.helper="store --file=$cred_path" -C "$demo" push -u origin main >/dev/null

api="localhost:$PORT/api/v1/repos/qg/demo"
admin_get() { curl -sf -H "Authorization: token $ADMIN" "$api/$1"; }
issue_list() { curl -sf -H "Authorization: token $ADMIN" "localhost:$PORT/api/v1/repos/qg/demo/issues?state=$1&type=issues&limit=50"; }
issue_count() { issue_list "$1" | py -c 'import json,sys; print(len(json.load(sys.stdin)))'; }
expect() { [ "$2" = "$3" ] && echo "ok: $1" || { echo "FAIL: $1 expected=$3 actual=$2"; exit 1; }; }
issue_number() {
  py -c 'import json,sys; needle=sys.argv[1]; print(next(str(i["number"]) for i in json.load(sys.stdin) if needle in i["title"]))' "$1"
}
commit_demo() {
  git -C "$demo" add -A
  git -C "$demo" -c user.email=qg@example.test -c user.name=qg commit -qm "$1"
  local branch; branch="$(git -C "$demo" branch --show-current)"
  git -c credential.helper="store --file=$cred_path" -C "$demo" push -u origin "$branch" >/dev/null
}
run_flow() {
  local src="$1" branch="$2" head="$3"
  if [ -d "$out" ]; then
    local old_out_path; old_out_path="$(cd "$out" && (pwd -W 2>/dev/null || pwd))"
    docker run --rm --user 0 -v "$old_out_path:/cleanup" --entrypoint sh qualgate:dev -c 'chmod -R 777 /cleanup'
  fi
  rm -rf "$out"; mkdir -p "$out"; chmod 777 "$out"
  cp "$creds" "$out/.git-credentials"; chmod 644 "$out/.git-credentials"
  local out_path; out_path="$(cd "$out" && (pwd -W 2>/dev/null || pwd))"
  docker run --rm --network "$NET" -v "$src:/src:ro" -v "$out_path:/out" \
    -e GITEA_URL="http://$GITEA:3000" -e QUALGATE_GIT_CREDENTIALS=/out/.git-credentials \
    qualgate:dev fetch-config --repo qg/demo --branch "$branch" --commit "$head" \
      --config-repo qg/cfg --ref main --out /out/config
  docker run --rm --network none -v "$src:/src:ro" -v "$out_path:/out" \
    -e QUALGATE_BRANCH="$branch" qualgate:dev scan --src /src --config /out/config --out /out
  docker run --rm --network "$NET" -v "$out_path:/out" \
    -e GITEA_URL="http://$GITEA:3000" -e QUALGATE_GIT_CREDENTIALS=/out/.git-credentials \
    qualgate:dev publish --repo qg/demo --out /out --gate-result /out/gate-result --fail-on-gate false
}

run_flow "$demo_path" main "$(git -C "$demo" rev-parse HEAD)"
expect "initial gate result" "$(cat "$out/gate-result")" passed
expect "initial issue count" "$(issue_list open | py -c 'import json,sys; print(len(json.load(sys.stdin)))')" 2
eval_issue="$(issue_list open | issue_number 'python.eval')"
issue_list open | issue_number '품질 요약' >/dev/null
before="$(issue_list all | py -c 'import json,sys; print(sorted((i["number"],i["updated_at"]) for i in json.load(sys.stdin)))')"
run_flow "$demo_path" main "$(git -C "$demo" rev-parse HEAD)"
after="$(issue_list all | py -c 'import json,sys; print(sorted((i["number"],i["updated_at"]) for i in json.load(sys.stdin)))')"
expect "rerun leaves issue timestamps unchanged" "$after" "$before"

cat >> "$demo/app.py" <<'PY'
token = random.random()
PY
commit_demo 'add a new hotspot'
run_flow "$demo_path" main "$(git -C "$demo" rev-parse HEAD)"
expect "new hotspot fails the gate" "$(cat "$out/gate-result")" failed
hotspot_issue="$(issue_list open | issue_number '핫스팟 python.insecure-random')"
safe_id="$(admin_get 'labels?limit=50' | py -c 'import json,sys; print(next((str(x["id"]) for x in json.load(sys.stdin) if x["name"]=="qualgate:safe"), ""))')"
if [ -z "$safe_id" ]; then
  safe_id="$(curl -sf -H "Authorization: token $ADMIN" -H 'Content-Type: application/json' \
    -d '{"name":"qualgate:safe","color":"#00aa00"}' "localhost:$PORT/api/v1/repos/qg/demo/labels" | py -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
fi
curl -sf -H "Authorization: token $ADMIN" -H 'Content-Type: application/json' \
  -d "{\"labels\":[$safe_id]}" "localhost:$PORT/api/v1/repos/qg/demo/issues/$hotspot_issue/labels" >/dev/null
safe_before="$(admin_get "issues/$hotspot_issue" | py -c 'import json,sys; x=json.load(sys.stdin); print((x["title"],x["body"]))')"
run_flow "$demo_path" main "$(git -C "$demo" rev-parse HEAD)"
expect "safe hotspot passes the gate" "$(cat "$out/gate-result")" passed
safe_after="$(admin_get "issues/$hotspot_issue" | py -c 'import json,sys; x=json.load(sys.stdin); print((x["title"],x["body"]))')"
expect "safe hotspot issue body and title unchanged" "$safe_after" "$safe_before"

# Preserve the original lifecycle coverage: close, reopen with a comment, then ignore a recurrence.
cat > "$demo/app.py" <<'PY'
value = 1
PY
commit_demo 'remove current findings'
run_flow "$demo_path" main "$(git -C "$demo" rev-parse HEAD)"
expect "fixed findings close" "$(issue_count closed)" 2
cat > "$demo/app.py" <<'PY'
eval(input())
PY
commit_demo 'reintroduce python.eval'
run_flow "$demo_path" main "$(git -C "$demo" rev-parse HEAD)"
expect "recurrence reopens issue" "$(issue_count open)" 2
reopen_comments="$(admin_get "issues/$eval_issue/comments" | py -c 'import json,sys; print(sum("다시 열었습니다" in c["body"] for c in json.load(sys.stdin)))')"
expect "reopen comment recorded" "$reopen_comments" 1
cat > "$demo/app.py" <<'PY'
value = 1
PY
commit_demo 'fix python.eval again'
run_flow "$demo_path" main "$(git -C "$demo" rev-parse HEAD)"
rule_no="$eval_issue"
ignored_id="$(admin_get 'labels?limit=50' | py -c 'import json,sys; print(next((str(x["id"]) for x in json.load(sys.stdin) if x["name"]=="qualgate:ignored"), ""))')"
if [ -z "$ignored_id" ]; then
  ignored_id="$(curl -sf -H "Authorization: token $ADMIN" -H 'Content-Type: application/json' \
    -d '{"name":"qualgate:ignored","color":"#cccccc"}' "localhost:$PORT/api/v1/repos/qg/demo/labels" | py -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
fi
curl -sf -H "Authorization: token $ADMIN" -H 'Content-Type: application/json' \
  -d "{\"labels\":[$ignored_id]}" "localhost:$PORT/api/v1/repos/qg/demo/issues/$rule_no/labels" >/dev/null
ignored_before="$(admin_get "issues/$rule_no" | py -c 'import json,sys; x=json.load(sys.stdin); print((x["updated_at"],x["state"]))')"
cat > "$demo/app.py" <<'PY'
eval(input())
PY
commit_demo 'reintroduce ignored issue'
run_flow "$demo_path" main "$(git -C "$demo" rev-parse HEAD)"
expect "ignored issue remains closed" "$(admin_get "issues/$rule_no" | py -c 'import json,sys; print(json.load(sys.stdin)["state"])')" closed
ignored_after="$(admin_get "issues/$rule_no" | py -c 'import json,sys; x=json.load(sys.stdin); print((x["updated_at"],x["state"]))')"
expect "ignored issue unchanged" "$ignored_after" "$ignored_before"
curl -sf -X DELETE -H "Authorization: token $ADMIN" \
  "localhost:$PORT/api/v1/repos/qg/demo/issues/$rule_no/labels/$ignored_id" >/dev/null
run_flow "$demo_path" main "$(git -C "$demo" rev-parse HEAD)"
expect "issue reopens after ignored label removal" "$(admin_get "issues/$rule_no" | py -c 'import json,sys; print(json.load(sys.stdin)["state"])')" open

docker run --rm -i --network none -v "$(cd "$tmp" && (pwd -W 2>/dev/null || pwd)):/data" --entrypoint python3 "$PY_IMAGE" -I - /data/cfg/rulesets/standard.toml <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
s = p.read_text(encoding="utf-8")
needle = "[languages.python]\nenabled = true"
assert needle in s
p.write_text(s.replace(needle, needle + '\ndisabled_rules = ["python.eval"]', 1), encoding="utf-8")
PY
git -C "$cfg" add -A
git -C "$cfg" -c user.email=qg@example.test -c user.name=qg commit -qm 'disable python.eval'
git -c credential.helper="store --file=$cred_path" -C "$cfg" push >/dev/null
run_flow "$demo_path" main "$(git -C "$demo" rev-parse HEAD)"
eval_state="$(admin_get "issues/$eval_issue" | py -c 'import json,sys; print(json.load(sys.stdin)["state"])')"
expect "disabled rule issue closes" "$eval_state" closed
eval_comments="$(admin_get "issues/$eval_issue/comments" | py -c 'import json,sys; print(" ".join(x["body"] for x in json.load(sys.stdin)))')"
[[ "$eval_comments" == *"룰셋에서 제외"* ]] && echo "ok: disabled rule close reason" || { echo "FAIL: missing ruleset exclusion comment"; exit 1; }

# A depth-1 source clone must use fetch-config's matching newcode.json as a fallback.
git -c credential.helper="store --file=$cred_path" clone --quiet --depth 1 --branch main \
  "http://localhost:$PORT/qg/demo.git" "$shallow"
shallow_path="$(cd "$shallow" && (pwd -W 2>/dev/null || pwd))"
run_flow "$shallow_path" main "$(git -C "$shallow" rev-parse HEAD)"
docker run --rm -i --network none -v "$(cd "$tmp" && (pwd -W 2>/dev/null || pwd)):/data:ro" --entrypoint python3 "$PY_IMAGE" -I - /data/out/result.json <<'PY'
import json,sys
r=json.load(open(sys.argv[1],encoding="utf-8"))
assert r["new_code_measured"] is True, r["new_code_measured"]
print("ok: shallow source uses fetch-config newcode.json")
PY

# A non-default branch gets a gate result without changing issues.
git -C "$demo" checkout -qb feature
printf 'value = 1\n' > "$demo/feature.py"
commit_demo 'feature branch change'
before="$(issue_list all | py -c 'import json,sys; print(sorted((i["number"],i["updated_at"],i["state"]) for i in json.load(sys.stdin)))')"
run_flow "$demo_path" feature "$(git -C "$demo" rev-parse HEAD)"
[ -f "$out/gate-result" ] || { echo 'FAIL: non-default branch did not write gate result'; exit 1; }
after="$(issue_list all | py -c 'import json,sys; print(sorted((i["number"],i["updated_at"],i["state"]) for i in json.load(sys.stdin)))')"
expect "non-default branch leaves issues unchanged" "$after" "$before"
echo "Gitea 1.26.4 integration passed (Basic auth through .git-credentials)."
