#!/usr/bin/env bash
# 언어별 샘플을 네트워크 없는 이미지에서 스캔하고 결과를 검증한다.
set -euo pipefail
export MSYS_NO_PATHCONV=1
cd "$(dirname "$0")"
fail=0
for lang in java kotlin python react vue go; do
  work="$(mktemp -d)"; out="$(mktemp -d)"
  cp -r "repos/$lang/." "$work"
  w="$(cd "$work" && (pwd -W 2>/dev/null || pwd))"; o="$(cd "$out" && (pwd -W 2>/dev/null || pwd))"
  git -C "$w" init -qb main
  git -C "$w" add -A
  git -C "$w" -c user.email=t@e -c user.name=t commit -qm init
  chmod 777 "$out"
  set +e
  docker run --rm --network none -v "$w:/src:ro" -v "$o:/out" qualgate:dev scan
  code=$?
  set -e
  if [ "$code" -gt 1 ]; then echo "FAIL $lang exit=$code"; fail=1; continue; fi
  python -I - "$o/result.json" "$lang" <<'PY' || fail=1
import json, sys
r = json.load(open(sys.argv[1], encoding="utf-8"))
lang = sys.argv[2]
errs = []
if not r["findings"]: errs.append("위반 없음")
if not r["clones"]: errs.append("중복 없음")
if r["metrics"]["coverage"] is None: errs.append("커버리지 없음")
if r["unanalyzed"]: errs.append(f"분석 실패 {r['unanalyzed']}")
print(lang, "OK" if not errs else "FAIL " + "; ".join(errs),
      len(r["findings"]), len(r["clones"]), r["metrics"]["coverage"])
sys.exit(1 if errs else 0)
PY
done

# 읽을 수 없는 디렉터리는 조용히 건너뛰지 않고 실행 오류(2)여야 하고, 읽을 수 없는 파일은 분석 실패로
# 남아야 한다. 컨테이너 안에서 비root 사용자로 만들어야 권한이 실제로 적용된다.
docker run --rm --network none --entrypoint sh qualgate:dev -c '
set -u
mk() { chmod -R u+rwx /tmp/r 2>/dev/null; rm -rf /tmp/r; mkdir -p /tmp/r/priv && cd /tmp/r && git init -qb main
  printf "eval(input())
" > a.py; printf "eval(input())
" > priv/p.py
  git add -A && git -c user.email=t@e -c user.name=t commit -qm init; }
mk; chmod 700 priv; chmod 000 priv
qualgate scan --src /tmp/r --out /tmp/o1 >/dev/null 2>/tmp/err; code=$?
[ "$code" -eq 2 ] && echo "unreadable-dir OK exit=2 ($(head -c 120 /tmp/err))" || { echo "FAIL unreadable-dir exit=$code"; exit 1; }
# 제외한 디렉터리는 읽을 수 없어도 탐색에서 건너뛴다(A1).
mk; printf "[scan]
exclude = [\"**/priv/**\"]
" > .qualgate.toml; chmod 700 priv; chmod 000 priv
qualgate scan --src /tmp/r --out /tmp/o3 >/dev/null 2>/tmp/err3; code=$?
[ "$code" -le 1 ] && echo "excluded-unreadable-dir OK exit=$code" || { echo "FAIL excluded-unreadable-dir exit=$code"; head -c 300 /tmp/err3; exit 1; }
mk; chmod 000 priv/p.py
qualgate scan --src /tmp/r --out /tmp/o2 >/dev/null 2>&1; code=$?
grep -q "\"priv/p.py\"" /tmp/o2/result.json && [ "$code" -le 1 ] && echo "unreadable-file OK unanalyzed" || { echo "FAIL unreadable-file exit=$code"; cat /tmp/o2/result.json; exit 1; }
' || fail=1
exit "$fail"
