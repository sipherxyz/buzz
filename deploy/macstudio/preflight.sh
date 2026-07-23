#!/usr/bin/env bash
set -euo pipefail

export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
ENV_FILE="${BUZZ_ENV_FILE:-$SCRIPT_DIR/.env}"
MIN_DISK_BYTES="${BUZZ_MIN_DISK_BYTES:-500000000000}"
MIN_MEMORY_BYTES="${BUZZ_MIN_MEMORY_BYTES:-8000000000}"

pass() {
  printf 'PASS  %s\n' "$1"
}

warn() {
  printf 'WARN  %s\n' "$1" >&2
}

fail() {
  printf 'FAIL  %s\n' "$1" >&2
  return 1
}

meets_minimum_bytes() {
  test "$1" -ge "$2"
}

is_external_s3_endpoint() {
  case "$1" in
    http://minio:*|https://minio:*|http://localhost:*|https://localhost:*|http://127.0.0.1:*|https://127.0.0.1:*)
      return 1
      ;;
    http://*|https://*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

is_hex_key() {
  test "${#1}" -eq 64 && ! grep -q '[^[:xdigit:]]' <<<"$1"
}

is_production_relay_url() {
  test "$1" = "wss://buzz.sipher.gg:8443"
}

env_value() {
  local key="$1"
  local file="$2"
  awk -v wanted="$key" '
    /^[[:space:]]*#/ { next }
    {
      split($0, parts, "=")
      name = parts[1]
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", name)
      if (name == wanted) {
        sub(/^[^=]*=/, "")
        gsub(/\r$/, "")
        gsub(/^[[:space:]]+|[[:space:]]+$/, "")
        gsub(/^["'\'']|["'\'']$/, "")
        print
        exit
      }
    }
  ' "$file"
}

available_memory_bytes() {
  if test -n "${BUZZ_PREFLIGHT_AVAILABLE_BYTES:-}"; then
    printf '%s\n' "$BUZZ_PREFLIGHT_AVAILABLE_BYTES"
    return
  fi
  if test "$(uname -s)" = "Darwin"; then
    vm_stat | awk '
      /page size of/ {
        for (i = 1; i <= NF; i++) {
          if ($i ~ /^[0-9]+$/) {
            page_size = $i
            break
          }
        }
      }
      /Pages free:|Pages inactive:|Pages speculative:|Pages purgeable:/ {
        gsub(/\./, "", $NF)
        pages += $NF
      }
      END { printf "%.0f\n", pages * page_size }
    '
    return
  fi
  awk '/MemAvailable:/ { print $2 * 1024 }' /proc/meminfo
}

disk_free_bytes() {
  if test -n "${BUZZ_PREFLIGHT_DISK_BYTES:-}"; then
    printf '%s\n' "$BUZZ_PREFLIGHT_DISK_BYTES"
    return
  fi
  df -Pk "$REPO_DIR" | awk 'NR == 2 { print $4 * 1024 }'
}

check_git() {
  local branch
  branch="$(git -C "$REPO_DIR" branch --show-current)"
  test "$branch" = "main" || {
    fail "checkout must be on main (currently $branch)"
    return 1
  }
  test -z "$(git -C "$REPO_DIR" status --porcelain)" || {
    fail "checkout must be clean before deployment"
    return 1
  }
  pass "clean main checkout"
}

check_resources() {
  local disk memory
  disk="$(disk_free_bytes)"
  memory="$(available_memory_bytes)"
  meets_minimum_bytes "$disk" "$MIN_DISK_BYTES" || {
    fail "at least $MIN_DISK_BYTES bytes of free disk are required (found $disk)"
    return 1
  }
  meets_minimum_bytes "$memory" "$MIN_MEMORY_BYTES" || {
    fail "at least $MIN_MEMORY_BYTES bytes of reclaimable memory are required (found $memory)"
    return 1
  }
  pass "disk and memory headroom"
}

check_env() {
  test -f "$ENV_FILE" || {
    fail "missing production env file: $ENV_FILE"
    return 1
  }
  local key value
  for key in POSTGRES_PASSWORD REDIS_PASSWORD BUZZ_S3_ENDPOINT BUZZ_S3_ACCESS_KEY BUZZ_S3_SECRET_KEY BUZZ_S3_BUCKET; do
    value="$(env_value "$key" "$ENV_FILE")"
    test -n "$value" || {
      fail "$key is missing from $ENV_FILE"
      return 1
    }
  done
  value="$(env_value BUZZ_S3_ENDPOINT "$ENV_FILE")"
  is_external_s3_endpoint "$value" || {
    fail "BUZZ_S3_ENDPOINT must point to external HTTP(S) object storage, not local MinIO"
    return 1
  }
  test "$(env_value BUZZ_UPS_CONFIRMED "$ENV_FILE")" = "true" || {
    fail "set BUZZ_UPS_CONFIRMED=true after verifying the Mac Studio and network equipment are UPS-backed"
    return 1
  }

  local require_auth require_membership relay_private_key owner_pubkey relay_url
  require_auth="$(env_value BUZZ_REQUIRE_AUTH_TOKEN "$ENV_FILE")"
  require_membership="$(env_value BUZZ_REQUIRE_RELAY_MEMBERSHIP "$ENV_FILE")"
  relay_private_key="$(env_value BUZZ_RELAY_PRIVATE_KEY "$ENV_FILE")"
  owner_pubkey="$(env_value RELAY_OWNER_PUBKEY "$ENV_FILE")"
  relay_url="$(env_value RELAY_URL "$ENV_FILE")"

  if test "$require_membership" = "true"; then
    test "$require_auth" = "true" || {
      fail "BUZZ_REQUIRE_AUTH_TOKEN=true is required before BUZZ_REQUIRE_RELAY_MEMBERSHIP=true"
      return 1
    }
  fi

  if test "$require_auth" = "true"; then
    is_hex_key "$relay_private_key" || {
      fail "BUZZ_RELAY_PRIVATE_KEY must be a stable 64-character hex key when BUZZ_REQUIRE_AUTH_TOKEN=true"
      return 1
    }
  fi

  if test "$require_membership" = "true"; then
    is_hex_key "$owner_pubkey" || {
      fail "RELAY_OWNER_PUBKEY must be a 64-character hex human owner pubkey when BUZZ_REQUIRE_RELAY_MEMBERSHIP=true"
      return 1
    }
    is_production_relay_url "$relay_url" || {
      fail "RELAY_URL must be wss://buzz.sipher.gg:8443 when BUZZ_REQUIRE_RELAY_MEMBERSHIP=true"
      return 1
    }
  fi

  pass "production configuration and external S3"
}

report_host_security() {
  local filevault_status="$1"
  local firewall_status="$2"
  local warned=0

  if ! grep -q "FileVault is On" <<<"$filevault_status"; then
    warn "FileVault is disabled; disk-at-rest protection is an accepted operational risk"
    warned=1
  fi
  if ! grep -qi "enabled" <<<"$firewall_status"; then
    warn "macOS application firewall is disabled; host services must be protected by the network perimeter"
    warned=1
  fi
  if test "$warned" -eq 0; then
    pass "FileVault and macOS firewall"
  fi
}

check_host_security() {
  if test "$(uname -s)" != "Darwin"; then
    pass "host security checks skipped on non-macOS host"
    return
  fi

  local filevault_status firewall_status
  filevault_status="$(fdesetup status 2>&1 || true)"
  firewall_status="$(
    /usr/libexec/ApplicationFirewall/socketfilterfw --getglobalstate 2>&1 || true
  )"
  report_host_security "$filevault_status" "$firewall_status"
}

check_docker() {
  command -v docker >/dev/null 2>&1 || {
    fail "docker is not installed"
    return 1
  }
  docker info >/dev/null 2>&1 || {
    fail "Docker/OrbStack engine is not running"
    return 1
  }
  docker compose version >/dev/null 2>&1 || {
    fail "Docker Compose v2 is unavailable"
    return 1
  }
  pass "Docker engine and Compose"
}

main() {
  local failed=0
  check_git || failed=1
  check_resources || failed=1
  check_env || failed=1
  check_host_security || failed=1
  check_docker || failed=1
  if test "$failed" -ne 0; then
    printf 'Preflight failed; no deployment changes were made.\n' >&2
    exit 1
  fi
  printf 'Mac Studio production preflight passed.\n'
}

if test "${BUZZ_PREFLIGHT_TESTING:-0}" != "1"; then
  main "$@"
fi
