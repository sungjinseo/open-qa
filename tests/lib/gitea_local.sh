#!/usr/bin/env bash
# 로컬 일회용 Gitea 컨테이너 공용 함수. source해서 쓴다. 127.0.0.1에만 바인딩하고 이름과 포트는 무작위다.
# 호출 스크립트는 `set -e`와 `trap gitea_local_cleanup EXIT`를 직접 설정한다.
export MSYS_NO_PATHCONV=1

# gitea_local_start IMAGE PREFIX: NET, GITEA, PORT를 정하고 컨테이너를 띄워 준비될 때까지 기다린다.
gitea_local_start() {
  local image="$1" prefix="$2" sfx ready=0
  sfx="$(od -An -N4 -tx1 /dev/urandom | tr -d ' \n')"
  NET="$prefix-net-$sfx"; GITEA="$prefix-gitea-$sfx"
  PORT=$((20000 + RANDOM % 20000))
  docker network create "$NET" >/dev/null
  docker run -d --name "$GITEA" --network "$NET" -p "127.0.0.1:$PORT:3000" -e GITEA__security__INSTALL_LOCK=true -e GITEA__database__DB_TYPE=sqlite3 "$image" >/dev/null
  for _ in $(seq 60); do
    if curl -sf "127.0.0.1:$PORT/api/healthz" >/dev/null; then ready=1; break; fi
    sleep 2
  done
  [ "$ready" = 1 ] || { echo "FAIL: Gitea가 120초 안에 준비되지 않음"; docker logs "$GITEA" 2>&1 | tail -20; return 1; }
}

# gitea_local_admin_user USER PASSWORD
gitea_local_admin_user() {
  docker exec -u git "$GITEA" gitea admin user create --admin --username "$1" --password "$2" --email "$1@example.com" >/dev/null
}

# gitea_local_token USER NAME SCOPES: 토큰 문자열을 출력한다.
gitea_local_token() {
  docker exec -u git "$GITEA" gitea admin user generate-access-token --username "$1" --token-name "$2" --scopes "$3" --raw
}

gitea_local_cleanup() {
  docker rm -f "${GITEA:-}" >/dev/null 2>&1 || true
  docker network rm "${NET:-}" >/dev/null 2>&1 || true
}
