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

echo "preflight tests passed"
