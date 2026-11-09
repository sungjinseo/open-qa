#!/usr/bin/env bash
set -euo pipefail
export MSYS_NO_PATHCONV=1
cd "$(dirname "$0")/.."
docker build -q -t qualgate-dev -f docker/dev.Dockerfile docker >/dev/null
root="$(pwd -W 2>/dev/null || pwd)"
exec docker run --rm \
  -v "$root:/w" \
  -v qualgate-cargo-registry:/usr/local/cargo/registry \
  -v qualgate-target:/w/target \
  -w /w qualgate-dev cargo "$@"
