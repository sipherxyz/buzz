#!/usr/bin/env bash
set -euo pipefail

export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="${BUZZ_ENV_FILE:-$SCRIPT_DIR/.env}"
export BUZZ_ENV_FILE="$ENV_FILE"
COMPOSE_FILE="$SCRIPT_DIR/compose.yml"
STATE_DIR="$SCRIPT_DIR/state"

state_file="${1:-$STATE_DIR/rollback.env}"
test -f "$state_file" || {
  printf 'Rollback state not found: %s\n' "$state_file" >&2
  exit 1
}

image="$(sed -n 's/^BUZZ_IMAGE=//p' "$state_file" | head -n 1)"
sha="$(sed -n 's/^BUZZ_DEPLOYED_SHA=//p' "$state_file" | head -n 1)"
test -n "$image" || {
  printf 'Rollback state has no BUZZ_IMAGE: %s\n' "$state_file" >&2
  exit 1
}
docker image inspect "$image" >/dev/null

BUZZ_IMAGE="$image" docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up -d --remove-orphans
container="$(BUZZ_IMAGE="$image" docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" ps -q relay)"
deadline=$((SECONDS + ${BUZZ_HEALTH_TIMEOUT_SECONDS:-180}))
while test "$SECONDS" -lt "$deadline"; do
  status="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container")"
  test "$status" != "healthy" || {
    cp "$state_file" "$STATE_DIR/current.env"
    printf 'Rollback healthy: %s (%s)\n' "$image" "$sha"
    exit 0
  }
  test "$status" != "unhealthy" || break
  sleep 3
done

printf 'Rollback image did not become healthy: %s\n' "$image" >&2
exit 1
