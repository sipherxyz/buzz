#!/usr/bin/env bash
set -euo pipefail

BUZZ_DEPLOY_TESTING=1
source "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/deploy.sh"

actual="$(image_tag_for_sha 0123456789abcdef)"
test "$actual" = "sipher-buzz:0123456789ab"

actual="$(backup_filename 0123456789abcdef "20260723T120000Z")"
test "$actual" = "buzz-20260723T120000Z-0123456789ab.dump"

temp_dir="$(mktemp -d)"
trap 'rm -rf "$temp_dir"' EXIT
printf 'BUZZ_IMAGE=sipher-buzz:last-known-good\n' >"$temp_dir/current.env"
prepare_rollback_state "$temp_dir"
grep -q '^BUZZ_IMAGE=sipher-buzz:last-known-good$' "$temp_dir/rollback.env"

mkdir -p "$temp_dir/repo/migrations"
git -C "$temp_dir/repo" init -q
git -C "$temp_dir/repo" config user.name test
git -C "$temp_dir/repo" config user.email test@example.com
touch "$temp_dir/repo/migrations/.keep"
git -C "$temp_dir/repo" add .
git -C "$temp_dir/repo" commit -qm base
base_sha="$(git -C "$temp_dir/repo" rev-parse HEAD)"
printf 'create table example();\n' >"$temp_dir/repo/migrations/0001.sql"
git -C "$temp_dir/repo" add .
git -C "$temp_dir/repo" commit -qm migration
head_sha="$(git -C "$temp_dir/repo" rev-parse HEAD)"
has_migration_changes "$temp_dir/repo" "$base_sha" "$head_sha"

echo "deploy helper tests passed"
