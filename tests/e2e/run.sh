#!/usr/bin/env bash
# 언어별 샘플을 중앙 설정으로 네트워크 없는 컨테이너에서 스캔하고 결과를 검증한다.
set -euo pipefail
export MSYS_NO_PATHCONV=1
cd "$(dirname "$0")"
root="$(mktemp -d)"
root_docker="$(cd "$root" && (pwd -W 2>/dev/null || pwd))"
PY_IMAGE="${QUALGATE_TEST_PYTHON_IMAGE:-python:3-alpine}"
fail=0
py_check() { docker run --rm -i --network none -v "$root_docker:/data:ro" --entrypoint python3 "$PY_IMAGE" -I "$@"; }
cleanup() {
  docker run --rm --user 0 -v "$root_docker:/cleanup" --entrypoint sh qualgate:dev -c 'chmod -R 777 /cleanup' >/dev/null 2>&1 || true
  rm -rf "$root"
}
trap cleanup EXIT

# 언어:기대 규칙(쉼표):Bad 파일(리포트가 해석해야 하는 파일)
SAMPLES=(
  "java:java.system-out:src/main/java/a/Bad.java"
  "kotlin:kotlin.catch-throwable,kotlin.println:src/main/kotlin/a/Bad.kt"
  "python:python.eval:pkg/bad.py"
  "react:ts.code-eval:src/Bad.tsx"
  "vue:ts.code-eval:src/Bad.vue"
  "go:go.ignored-error:pkg/bad.go"
)
for sample in "${SAMPLES[@]}"; do
  IFS=: read -r lang rules badfile <<< "$sample"
  work="$root/src-$lang"; out="$root/out-$lang"; config="$root/config-$lang"
  mkdir -p "$work" "$out" "$config"
  chmod 777 "$config"
  cp -r "repos/$lang/." "$work"
  w="$(cd "$work" && (pwd -W 2>/dev/null || pwd))"
  o="$(cd "$out" && (pwd -W 2>/dev/null || pwd))"
  c="$(cd "$config" && (pwd -W 2>/dev/null || pwd))"
  git -C "$w" init -qb main
  git -C "$w" add -A
  git -C "$w" -c user.email=t@e -c user.name=t commit -qm init
  docker run --rm --network none -v "$c:/config" qualgate:dev init-config --out /config
  cat > "$config/source.json" <<JSON
{"config_repo":"e2e/config","ref":"main","config_commit":"local","repo":"e2e/$lang","branch":"main","registered":false}
JSON
  chmod 777 "$out"
  set +e
  docker run --rm --network none -v "$w:/src:ro" -v "$o:/out" -v "$c:/config:ro" \
    qualgate:dev scan --src /src --config /config --out /out
  code=$?
  set -e
  if [ "$code" -gt 1 ]; then echo "FAIL $lang exit=$code"; fail=1; continue; fi
  py_check - "/data/out-$lang/result.json" "$lang" "$rules" "$badfile" <<'PY' || fail=1
import json, sys
r = json.load(open(sys.argv[1], encoding="utf-8"))
lang, rules, badfile = sys.argv[2], sys.argv[3].split(","), sys.argv[4]
errs = []
if not r["findings"]: errs.append("위반 없음")
found = {(f["rule"], f["file"]) for f in r["findings"]}
for rule in rules:
    if (rule, badfile) not in found: errs.append(f"기대한 규칙 없음 {rule}")
if not r["clones"]: errs.append("중복 없음")
if r["metrics"]["coverage"] is None: errs.append("커버리지 없음")
cs = r["coverage_status"]
if cs["unresolved"]: errs.append(f"리포트 경로 매칭 실패 {cs['unresolved']}")
if badfile in cs["not_in_report"]: errs.append(f"리포트가 {badfile}을 해석하지 못함")
if r["unanalyzed"]: errs.append(f"분석 실패 {r['unanalyzed']}")
print(lang, "OK" if not errs else "FAIL " + "; ".join(errs), len(r["findings"]), len(r["clones"]), r["metrics"]["coverage"])
sys.exit(1 if errs else 0)
PY
done

# 읽기 권한 동작도 설정 디렉터리 기반 scan으로 검증한다. 대상 저장소의 .qualgate.toml은
# 설정을 바꾸지 않고 경고만 남기며, 제외 패턴은 중앙 프로젝트 설정에 둔다.
perm="$root/perm-config"; mkdir -p "$perm/default" "$perm/excluded"; chmod 777 "$perm" "$perm/default" "$perm/excluded"
p="$(cd "$perm" && (pwd -W 2>/dev/null || pwd))"
docker run --rm --network none -v "$p:/configs" qualgate:dev init-config --out /configs/default
cp -a "$perm/default/." "$perm/excluded/"
for config_dir in "$perm/default" "$perm/excluded"; do
  cat > "$config_dir/source.json" <<'JSON'
{"config_repo":"e2e/config","ref":"main","config_commit":"local","repo":"e2e/perm","branch":"main","registered":false}
JSON
done
mkdir -p "$perm/excluded/projects/e2e"
cat > "$perm/excluded/projects/e2e/perm.toml" <<'TOML'
schema = 1
repo = "e2e/perm"
ruleset = "standard"
languages = ["python"]

[scan]
exclude = ["**/priv/**"]
TOML
docker run --rm --network none --entrypoint sh -v "$p:/configs:ro" qualgate:dev -c '
set -u
mk() {
  chmod -R u+rwx /tmp/r 2>/dev/null || true; rm -rf /tmp/r /tmp/o1 /tmp/o2 /tmp/o3
  mkdir -p /tmp/r/priv && cd /tmp/r && git init -qb main
  printf "eval(input())\n" > a.py; printf "eval(input())\n" > priv/p.py
  git add -A && git -c user.email=t@e -c user.name=t commit -qm init
}
mk; chmod 700 priv; chmod 000 priv
qualgate scan --src /tmp/r --config /configs/default --out /tmp/o1 >/dev/null 2>/tmp/err; code=$?
[ "$code" -eq 2 ] && echo "unreadable-dir OK exit=2" || { echo "FAIL unreadable-dir exit=$code"; exit 1; }
mk; printf "[scan]\nexclude = [\"**/priv/**\"]\n" > .qualgate.toml; chmod 700 priv; chmod 000 priv
qualgate scan --src /tmp/r --config /configs/default --out /tmp/o2 >/dev/null 2>/tmp/err2; code=$?
[ "$code" -eq 2 ] && grep -qi "qualgate.toml" /tmp/err2 && echo "local-config-ignored OK" || { echo "FAIL .qualgate.toml was not ignored as expected (exit=$code)"; cat /tmp/err2; exit 1; }
mk; chmod 700 priv; chmod 000 priv
qualgate scan --src /tmp/r --config /configs/excluded --out /tmp/o3 >/dev/null 2>/tmp/err3; code=$?
[ "$code" -le 1 ] && echo "central-exclude OK exit=$code" || { echo "FAIL central-exclude exit=$code"; cat /tmp/err3; exit 1; }
mk; chmod 000 priv/p.py
qualgate scan --src /tmp/r --config /configs/default --out /tmp/o1 >/dev/null 2>&1; code=$?
grep -q "\"priv/p.py\"" /tmp/o1/result.json && [ "$code" -le 1 ] && echo "unreadable-file OK unanalyzed" || { echo "FAIL unreadable-file exit=$code"; cat /tmp/o1/result.json; exit 1; }
' || fail=1
exit "$fail"
