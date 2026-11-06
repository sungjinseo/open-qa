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
exit "$fail"
