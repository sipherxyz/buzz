#!/usr/bin/env bash
set -euo pipefail

BUZZ_PREFLIGHT_TESTING=1
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/preflight.sh"

assert_success() {
  if ! "$@"; then
    echo "expected success: $*" >&2
    exit 1
  fi
}

assert_failure() {
  if "$@"; then
    echo "expected failure: $*" >&2
    exit 1
  fi
}

assert_output_contains() {
  local output="$1"
  local expected="$2"
  grep -Fq "$expected" <<<"$output" || {
    echo "expected output to contain: $expected" >&2
    exit 1
  }
}

test_dir="$(mktemp -d)"
trap 'rm -rf "$test_dir"' EXIT

write_env_fixture() {
  local auth_required="$1"
  local membership_required="$2"
  local relay_private_key="$3"
  local owner_pubkey="$4"
  local relay_url="$5"

  cat >"$test_dir/.env" <<EOF
POSTGRES_PASSWORD=postgres-secret
REDIS_PASSWORD=redis-secret
BUZZ_S3_ENDPOINT=https://s3.ap-southeast-1.amazonaws.com
BUZZ_S3_ACCESS_KEY=access-key
BUZZ_S3_SECRET_KEY=secret-key
BUZZ_S3_BUCKET=buzz-production
BUZZ_UPS_CONFIRMED=true
RELAY_URL=$relay_url
BUZZ_REQUIRE_AUTH_TOKEN=$auth_required
BUZZ_REQUIRE_RELAY_MEMBERSHIP=$membership_required
EOF
  if test -n "$relay_private_key"; then
    printf 'BUZZ_RELAY_PRIVATE_KEY=%s\n' "$relay_private_key" >>"$test_dir/.env"
  fi
  if test -n "$owner_pubkey"; then
    printf 'RELAY_OWNER_PUBKEY=%s\n' "$owner_pubkey" >>"$test_dir/.env"
  fi
  ENV_FILE="$test_dir/.env"
}

relay_private_key="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
owner_pubkey="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
production_relay_url="wss://buzz.sipher.gg:8443"

assert_success meets_minimum_bytes 500 500
assert_success meets_minimum_bytes 501 500
assert_failure meets_minimum_bytes 499 500
assert_success is_external_s3_endpoint "https://s3.ap-southeast-1.amazonaws.com"
assert_failure is_external_s3_endpoint "http://minio:9000"
assert_failure is_external_s3_endpoint "http://localhost:9000"
assert_failure is_external_s3_endpoint "http://127.0.0.1:9000"

security_output="$(
  report_host_security \
    "FileVault is Off." \
    "Firewall is disabled. (State = 0)" \
    2>&1
)"
grep -q '^WARN  FileVault is disabled; disk-at-rest protection is an accepted operational risk$' \
  <<<"$security_output"
grep -q '^WARN  macOS application firewall is disabled; host services must be protected by the network perimeter$' \
  <<<"$security_output"

write_env_fixture false false "" "" "$production_relay_url"
assert_success check_env

write_env_fixture false false "$relay_private_key" "" "$production_relay_url"
assert_success check_env

write_env_fixture false false "not-a-valid-key" "" "$production_relay_url"
assert_failure check_env

write_env_fixture false false "" "" "$production_relay_url"
printf 'BUZZ_RELAY_PRIVATE_KEY=\n' >>"$ENV_FILE"
assert_failure check_env

write_env_fixture true false "" "" "$production_relay_url"
assert_failure check_env

write_env_fixture 1 false "$relay_private_key" "" "$production_relay_url"
assert_failure check_env

write_env_fixture typo false "$relay_private_key" "" "$production_relay_url"
assert_failure check_env

write_env_fixture false 1 "$relay_private_key" "$owner_pubkey" "$production_relay_url"
assert_failure check_env

write_env_fixture false typo "$relay_private_key" "$owner_pubkey" "$production_relay_url"
assert_failure check_env

write_env_fixture false true "$relay_private_key" "$owner_pubkey" "$production_relay_url"
assert_failure check_env

write_env_fixture true true "$relay_private_key" "" "$production_relay_url"
assert_failure check_env

write_env_fixture false false "" "" "ws://localhost:3000"
assert_failure check_env

write_env_fixture true false "$relay_private_key" "" "wss://other.example.com"
assert_failure check_env

write_env_fixture true true "$relay_private_key" "$owner_pubkey" "ws://localhost:3000"
assert_failure check_env

write_env_fixture true true "$relay_private_key" "$owner_pubkey" "$production_relay_url"
assert_success check_env

write_env_fixture false false "" "" "$production_relay_url"
compose_output="$(
  BUZZ_IMAGE=sipher-buzz:test \
    BUZZ_ENV_FILE="$ENV_FILE" \
    RELAY_URL=wss://shell-override.example.com \
    BUZZ_REQUIRE_AUTH_TOKEN=true \
    BUZZ_REQUIRE_RELAY_MEMBERSHIP=true \
    docker compose --env-file "$ENV_FILE" -f "$SCRIPT_DIR/compose.yml" config
)"
assert_output_contains "$compose_output" "RELAY_URL: $production_relay_url"
assert_output_contains "$compose_output" 'BUZZ_REQUIRE_AUTH_TOKEN: "false"'
assert_output_contains "$compose_output" 'BUZZ_REQUIRE_RELAY_MEMBERSHIP: "false"'

echo "preflight tests passed"
