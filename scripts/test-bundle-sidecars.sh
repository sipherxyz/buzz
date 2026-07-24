#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bundle_script="$repo_root/scripts/bundle-sidecars.sh"
target="aarch64-apple-darwin"
sidecars=(buzz-acp buzz-agent buzz-dev-mcp git-credential-nostr buzz)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mkdir -p \
  "$tmp/target/$target/release" \
  "$tmp/desktop/src-tauri/binaries"

for sidecar in "${sidecars[@]}"; do
  source_path="$tmp/target/$target/release/$sidecar"
  destination_path="$tmp/desktop/src-tauri/binaries/$sidecar-$target"
  printf '#!/usr/bin/env sh\nexit 0\n' >"$source_path"
  chmod 0755 "$source_path"

  # Reproduce a local or runner retry over an existing non-executable file.
  : >"$destination_path"
  chmod 0644 "$destination_path"
done

(
  cd "$tmp"
  "$bundle_script" "$target"
)

for sidecar in "${sidecars[@]}"; do
  destination_path="$tmp/desktop/src-tauri/binaries/$sidecar-$target"
  [[ -x "$destination_path" ]] || {
    echo "bundled sidecar is not executable: $destination_path" >&2
    exit 1
  }
done

echo "bundle sidecars executable-bit contract passed"
