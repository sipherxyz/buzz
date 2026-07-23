#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
workflow="$repo_root/.github/workflows/sipher-desktop-release.yml"
runbook="$repo_root/docs/deployment/sipher-desktop-release.md"

fail() {
  echo "sipher desktop release contract: $*" >&2
  exit 1
}

require_file() {
  [[ -f "$1" ]] || fail "missing $1"
}

require_fixed() {
  local file=$1
  local value=$2
  local description=$3
  grep -Fq -- "$value" "$file" || fail "missing $description"
}

forbid_fixed() {
  local file=$1
  local value=$2
  local description=$3
  if grep -Fiq -- "$value" "$file"; then
    fail "forbidden $description"
  fi
}

require_file "$workflow"
require_file "$runbook"
ruby -e 'require "yaml"; YAML.parse_file(ARGV.fetch(0))' "$workflow"
if grep -Eq 'uses: [^[:space:]]+@v[0-9]' "$workflow"; then
  fail "GitHub Actions dependencies must be pinned to immutable commits"
fi

require_fixed "$workflow" "github.repository == 'sipherxyz/buzz'" "Sipher repository gate"
require_fixed "$workflow" "sipher-v[0-9]*" "sipher-v tag trigger"
require_fixed "$workflow" "sipher-desktop-latest" "rolling updater release"
require_fixed "$workflow" "wss://buzz.sipher.gg:8443" "production relay WebSocket URL"
require_fixed "$workflow" "https://buzz.sipher.gg:8443" "production relay HTTP URL"
require_fixed "$workflow" "aarch64-apple-darwin" "Apple Silicon target"
require_fixed "$workflow" "x86_64-apple-darwin" "Intel macOS target"
require_fixed "$workflow" "x86_64-pc-windows-msvc" "Windows x64 target"

required_secrets=(
  SIPHER_APPLE_CERTIFICATE
  SIPHER_APPLE_CERTIFICATE_PASSWORD
  SIPHER_APPLE_SIGNING_IDENTITY
  SIPHER_APPLE_ID
  SIPHER_APPLE_PASSWORD
  SIPHER_APPLE_TEAM_ID
  SIPHER_KEYCHAIN_PASSWORD
  SIPHER_UPDATER_PUBLIC_KEY
  SIPHER_UPDATER_PRIVATE_KEY
  SIPHER_UPDATER_PRIVATE_KEY_PASSWORD
)
for secret in "${required_secrets[@]}"; do
  require_fixed "$workflow" "$secret" "$secret validation"
  require_fixed "$runbook" "$secret" "$secret documentation"
done

for optional_secret in \
  SIPHER_WINDOWS_CERTIFICATE \
  SIPHER_WINDOWS_CERTIFICATE_PASSWORD \
  SIPHER_WINDOWS_CERTIFICATE_THUMBPRINT; do
  require_fixed "$workflow" "$optional_secret" "$optional_secret handling"
  require_fixed "$runbook" "$optional_secret" "$optional_secret documentation"
done

verify_count=$(grep -Fc 'scripts/verify-release-ref.sh sipher-v "$VERSION"' "$workflow")
((verify_count >= 4)) ||
  fail "every setup/publisher job must verify the immutable sipher-v tag"

require_fixed "$workflow" "continue-on-error: true" "non-blocking Windows job"
require_fixed "$workflow" "always()" "manifest execution after optional Windows failure"
require_fixed "$workflow" "needs.macos.result == 'success'" "macOS success gate"

require_fixed "$workflow" "security create-keychain" "temporary macOS keychain creation"
require_fixed "$workflow" "security import" "Developer ID P12 import"
require_fixed "$workflow" "APPLE_SIGNING_IDENTITY" "Tauri signing identity"
require_fixed "$workflow" "APPLE_ID" "Apple notarization account"
require_fixed "$workflow" "APPLE_PASSWORD" "Apple notarization password"
require_fixed "$workflow" "APPLE_TEAM_ID" "Apple notarization team"
require_fixed "$workflow" "codesign --verify" "codesign verification"
require_fixed "$workflow" "spctl --assess" "Gatekeeper verification"
require_fixed "$workflow" "desktop/scripts/verify-macos-entitlements.sh" "entitlements verification"

require_fixed "$workflow" "-p buzz-acp" "buzz-acp sidecar build"
require_fixed "$workflow" "-p buzz-agent" "buzz-agent sidecar build"
require_fixed "$workflow" "-p buzz-dev-mcp" "buzz-dev-mcp sidecar build"
require_fixed "$workflow" "-p git-credential-nostr" "git-credential-nostr sidecar build"
require_fixed "$workflow" "-p buzz-cli" "buzz CLI sidecar build"
require_fixed "$workflow" "scripts/bundle-sidecars.sh" "sidecar bundling"
require_fixed "$workflow" "desktop/scripts/sipher-release-config.mjs" "Sipher Tauri config generation"

require_fixed "$workflow" "Import optional Windows signing certificate" "optional PFX import"
require_fixed "$workflow" "_unsigned.exe" "unsigned Windows preview marker"
require_fixed "$workflow" "Get-AuthenticodeSignature" "signed Windows installer verification"
require_fixed "$workflow" "steps.signing.outputs.signed == 'true'" "signed-only Windows updater publication"
require_fixed "$workflow" "desktop/scripts/generate-oss-latest-json.sh" "unified updater manifest generation"
require_fixed "$workflow" '[[ -n "$archive_name" && -s "$sig_file" ]]' "updater metadata completeness check"

forbid_fixed "$workflow" "block/apple-codesign-action" "Block signing action"
forbid_fixed "$workflow" "github.com/block/buzz/releases" "Block updater endpoint"
forbid_fixed "$workflow" "buzz-desktop-latest" "Block rolling release"
forbid_fixed "$workflow" "inputs.ref" "caller-selected source ref"

require_fixed "$runbook" "git tag sipher-v<VERSION>" "release tag command"
require_fixed "$runbook" "git push origin sipher-v<VERSION>" "release tag push command"
require_fixed "$runbook" "tauri signer generate" "updater keypair generation"
require_fixed "$runbook" "Developer ID Application" "Apple Developer ID export"
require_fixed "$runbook" "unsigned preview" "unsigned Windows behavior"
require_fixed "$runbook" "Do not create or push a release tag" "preflight tag prohibition"

echo "sipher desktop release contract passed"
