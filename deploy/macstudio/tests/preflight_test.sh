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

echo "preflight tests passed"
