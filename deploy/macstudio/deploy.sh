#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
ENV_FILE="${BUZZ_ENV_FILE:-$SCRIPT_DIR/.env}"
export BUZZ_ENV_FILE="$ENV_FILE"
COMPOSE_FILE="$SCRIPT_DIR/compose.yml"
STATE_DIR="$SCRIPT_DIR/state"
BACKUP_DIR="${BUZZ_BACKUP_DIR:-$SCRIPT_DIR/backups}"

image_tag_for_sha() {
  printf 'sipher-buzz:%s\n' "$(printf '%s' "$1" | cut -c1-12)"
}

backup_filename() {
  printf 'buzz-%s-%s.dump\n' "$2" "$(printf '%s' "$1" | cut -c1-12)"
}

prepare_rollback_state() {
  local state_dir="${1:-$STATE_DIR}"
  mkdir -p "$state_dir"
  if test -f "$state_dir/current.env"; then
    cp "$state_dir/current.env" "$state_dir/rollback.env"
  elif test -f "$state_dir/rollback.env"; then
    mv "$state_dir/rollback.env" \
      "$state_dir/rollback.orphaned.$(date -u +%Y%m%dT%H%M%SZ).env"
  fi
}

has_migration_changes() {
  local repo_dir="$1"
  local base_sha="$2"
  local head_sha="$3"
  ! git -C "$repo_dir" diff --quiet "$base_sha..$head_sha" -- migrations
}

compose() {
  docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

wait_for_relay_health() {
  local deadline container status
  deadline=$((SECONDS + ${BUZZ_HEALTH_TIMEOUT_SECONDS:-180}))
  container="$(compose ps -q relay)"
  test -n "$container" || return 1
  while test "$SECONDS" -lt "$deadline"; do
    status="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container")"
    case "$status" in
      healthy)
        return 0
        ;;
      unhealthy|exited|dead)
        return 1
        ;;
    esac
    sleep 3
  done
  return 1
}

backup_database() {
  local sha="$1"
  local timestamp filename
  if test -z "$(compose ps -q postgres 2>/dev/null)"; then
    printf 'No existing Postgres container; skipping pre-deploy backup.\n'
    return
  fi
  mkdir -p "$BACKUP_DIR"
  timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
  filename="$(backup_filename "$sha" "$timestamp")"
  compose exec -T postgres sh -ec \
    'exec pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Fc' \
    >"$BACKUP_DIR/$filename"
  printf 'Database backup: %s\n' "$BACKUP_DIR/$filename"
}

record_state() {
  local sha="$1"
  local image="$2"
  mkdir -p "$STATE_DIR"
  if test -f "$STATE_DIR/current.env"; then
    cp "$STATE_DIR/current.env" "$STATE_DIR/previous.env"
  fi
  {
    printf 'BUZZ_DEPLOYED_SHA=%s\n' "$sha"
    printf 'BUZZ_IMAGE=%s\n' "$image"
    printf 'BUZZ_DEPLOYED_AT=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  } >"$STATE_DIR/current.env"
}

main() {
  test "${1:-}" != "--dry-run" || {
    "$SCRIPT_DIR/preflight.sh"
    git -C "$REPO_DIR" fetch origin main
    printf 'Dry run passed; origin/main was fetched and no deployment state changed.\n'
    return
  }

  "$SCRIPT_DIR/preflight.sh"
  git -C "$REPO_DIR" fetch origin main
  git -C "$REPO_DIR" merge --ff-only origin/main
  "$SCRIPT_DIR/preflight.sh"

  local sha image previous_sha
  sha="$(git -C "$REPO_DIR" rev-parse HEAD)"
  image="$(image_tag_for_sha "$sha")"
  export BUZZ_IMAGE="$image"
  previous_sha="$sha"
  if test -f "$STATE_DIR/current.env"; then
    previous_sha="$(sed -n 's/^BUZZ_DEPLOYED_SHA=//p' "$STATE_DIR/current.env" | head -n 1)"
  fi
  if test "$previous_sha" != "$sha" &&
    has_migration_changes "$REPO_DIR" "$previous_sha" "$sha"; then
    printf '%s\n' \
      "This release changes migrations/. Automated deployment is blocked because image-only rollback would be unsafe." \
      "Prepare and approve a database migration/rollback runbook, then deploy it manually." >&2
    exit 1
  fi

  printf 'Building %s from %s\n' "$image" "$sha"
  docker build --pull --tag "$image" "$REPO_DIR"

  BUZZ_IMAGE="$image" compose config --quiet
  backup_database "$previous_sha"
  prepare_rollback_state "$STATE_DIR"

  printf 'Starting Buzz services with %s\n' "$image"
  BUZZ_IMAGE="$image" compose up -d --remove-orphans
  if ! BUZZ_IMAGE="$image" wait_for_relay_health; then
    BUZZ_IMAGE="$image" compose logs --tail 200 relay >&2 || true
    printf 'Relay failed its health check. Run rollback.sh to restore the previous image.\n' >&2
    exit 1
  fi

  record_state "$sha" "$image"
  printf 'Deployment healthy: %s (%s)\n' "$image" "$sha"
}

if test "${BUZZ_DEPLOY_TESTING:-0}" != "1"; then
  main "$@"
fi
