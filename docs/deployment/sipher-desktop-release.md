# Sipher Desktop Release

This runbook publishes Sipher-managed Buzz Desktop builds from the
`sipherxyz/buzz` repository. The workflow accepts only an immutable
`sipher-v<VERSION>` tag whose commit matches the checked-out `HEAD`. It creates
the matching versioned release and updates the `sipher-desktop-latest` rolling
updater release.

The macOS artifacts are the release priority. Both Apple Silicon and Intel
builds must sign, notarize, and pass verification before `latest.json` is
published. The Windows job is optional and cannot block the macOS release.
All Sipher release runs share one repository-wide concurrency group, so tag
builds and manual retries execute serially. The workflow uses `queue: max` so
up to 100 pending releases wait instead of replacing an already pending run.

## Required repository secrets

Configure every required secret before attempting a release.

| Secret | Value |
| --- | --- |
| `SIPHER_APPLE_CERTIFICATE` | Base64-encoded Developer ID Application `.p12` |
| `SIPHER_APPLE_CERTIFICATE_PASSWORD` | Password used when exporting the `.p12` |
| `SIPHER_APPLE_SIGNING_IDENTITY` | Exact identity, for example `Developer ID Application: Sipher Inc. (TEAMID)` |
| `SIPHER_APPLE_ID` | Apple account used for notarization |
| `SIPHER_APPLE_PASSWORD` | App-specific password for the Apple account |
| `SIPHER_APPLE_TEAM_ID` | Apple Developer team identifier |
| `SIPHER_KEYCHAIN_PASSWORD` | Random password for the temporary CI keychain |
| `SIPHER_UPDATER_PUBLIC_KEY` | Public half of the Sipher Tauri updater key |
| `SIPHER_UPDATER_PRIVATE_KEY` | Private half of the Sipher Tauri updater key |
| `SIPHER_UPDATER_PRIVATE_KEY_PASSWORD` | Password protecting the updater private key |

The updater public key and endpoint are embedded in the app. The private key
and password are step-scoped only to the updater key smoke and Tauri build
steps; checkout, dependency installation, configuration, and sidecar builds do
not receive them. Never commit the private key, `.p12`, passwords, or generated
release config.

## Create the updater keypair

Generate a Sipher-owned keypair on a trusted operator machine:

```sh
cd desktop
pnpm tauri signer generate -w ~/.config/sipher/buzz-updater.key
```

Store the displayed public key as `SIPHER_UPDATER_PUBLIC_KEY`. Store the full
private key file as `SIPHER_UPDATER_PRIVATE_KEY`, preserving its newlines, and
store the chosen password as `SIPHER_UPDATER_PRIVATE_KEY_PASSWORD`. Keep an
offline encrypted backup. Do not reuse the Block updater keypair.

Before it creates a draft release, the workflow uses the Tauri signer to sign a
disposable payload, decodes the configured public key and signature, and runs
`minisign` verification. A mismatched private key, public key, or password
therefore fails without creating or publicizing a release. The `minisign`
package is installed in a separate step before any signing secret is exposed.

The production updater endpoint is:

```text
https://github.com/sipherxyz/buzz/releases/download/sipher-desktop-latest/latest.json
```

## Export the Apple Developer ID

1. In Keychain Access, locate the Sipher `Developer ID Application`
   certificate and its private key.
2. Export both together as a password-protected `.p12`.
3. Base64-encode the file without line wrapping:

   ```sh
   base64 -i Sipher-Developer-ID.p12 | tr -d '\n'
   ```

4. Store the result in `SIPHER_APPLE_CERTIFICATE` and the export password in
   `SIPHER_APPLE_CERTIFICATE_PASSWORD`.
5. Copy the exact codesigning identity into `SIPHER_APPLE_SIGNING_IDENTITY`.
6. Configure `SIPHER_APPLE_ID`, its app-specific `SIPHER_APPLE_PASSWORD`, and
   `SIPHER_APPLE_TEAM_ID` for notarization.
7. Generate a separate random `SIPHER_KEYCHAIN_PASSWORD`; it protects only the
   temporary keychain created during the workflow.

The workflow decodes the P12 into the runner's temporary directory, imports it
into a temporary keychain, passes `APPLE_SIGNING_IDENTITY` and Apple
notarization credentials to Tauri, then deletes the keychain and P12. Each app
must pass `codesign`, Gatekeeper `spctl`, entitlement verification, and stapler
validation.

## Optional Windows signing

Windows publishing supports these optional secrets:

| Secret | Value |
| --- | --- |
| `SIPHER_WINDOWS_CERTIFICATE` | Base64-encoded code-signing PFX/P12 |
| `SIPHER_WINDOWS_CERTIFICATE_PASSWORD` | PFX/P12 password |
| `SIPHER_WINDOWS_CERTIFICATE_THUMBPRINT` | Certificate SHA-1 thumbprint, with or without spaces |

If `SIPHER_WINDOWS_CERTIFICATE` is set, the password and thumbprint are
mandatory. The workflow imports the certificate into the current user's
certificate store, verifies its thumbprint, and passes the thumbprint to the
Tauri Windows signing configuration. The Authenticode-signed installer and its
Tauri updater signature are then eligible for `sipher-desktop-latest` and
`latest.json`.

If the certificate is absent, the workflow uploads an explicitly
`_unsigned.exe` unsigned preview only to the versioned release. The unsigned
preview and its updater signature are never uploaded to the rolling release and
are excluded from `latest.json`.

For a signed build, `Get-AuthenticodeSignature` must report `Valid` before the
installer is uploaded even as a temporary workflow artifact. The Windows job
never uploads directly to a GitHub Release.

## Pinned release runners

Release jobs use explicit stable GitHub-hosted images and assert the actual
host and Rust architectures before building:

| Build | Runner | Required host/target |
| --- | --- | --- |
| macOS Apple Silicon | `macos-15` | `arm64` / `aarch64-apple-darwin` |
| macOS Intel | `macos-15-intel` | `x86_64` / `x86_64-apple-darwin` |
| Windows | `windows-2025` | x64 / `x86_64-pc-windows-msvc` |
| Setup and promotion | `ubuntu-24.04` | control jobs only |

Do not replace these with floating `*-latest` labels. Update the pinned labels
deliberately when GitHub deprecates an image, and update the release contract in
the same change.

## Preflight

Before tagging:

1. Confirm all required Apple and updater secrets are present in
   `sipherxyz/buzz`.
2. Confirm the public key belongs to the private updater key configured in CI.
3. Confirm the Apple certificate is current and its identity text matches
   `SIPHER_APPLE_SIGNING_IDENTITY`.
4. Decide whether Windows is signed. If it is, configure all three optional
   Windows secrets.
5. Run the release contracts from a clean checkout:

   ```sh
   bash scripts/test-sipher-desktop-release-contract.sh
   bash scripts/test-release-ref-contract.sh
   ```

Do not create or push a release tag until the required repository secrets are
configured and the preflight checks pass.

## Release

Choose a semver that matches the desktop manifests. Create and push the
immutable tag:

```sh
git tag sipher-v<VERSION>
git push origin sipher-v<VERSION>
```

For example:

```sh
git tag sipher-v0.5.0
git push origin sipher-v0.5.0
```

The tag triggers `.github/workflows/sipher-desktop-release.yml`. A manual
dispatch is only a retry mechanism: select the existing `sipher-v<VERSION>` tag
in the GitHub ref picker and enter the same bare version. The workflow rejects
branches, mismatched versions, moved tags, and caller-selected source refs.

The setup job verifies the updater keypair and creates the versioned release as
a draft. Platform jobs upload only short-lived workflow artifacts. Once both
macOS matrix legs have passed signing, notarization, Gatekeeper, entitlement,
and stapler checks, the promotion job waits for the optional Windows job,
uploads both DMGs plus any verified Windows installer to the draft, and then
publishes it. Windows failure does not block promotion because only macOS
success is required. This prevents an empty, single-architecture, or
partially-attached public release. Unsigned previews remain
versioned-release-only.

The rolling updater is also protected by a monotonic SemVer guard. Before
touching `sipher-desktop-latest`, the final job reads the currently published
`latest.json`. An older queued tag or manual rerun may rebuild its immutable
versioned release, but it skips all rolling assets and manifest updates. An
equal version may retry an existing versioned draft, but it also skips every
rolling asset and manifest mutation. Only a newer version uploads payloads
before `latest.json`, so clients never observe a manifest pointing to missing
assets.

## Verify and cut over

After the workflow completes:

1. Confirm the `sipher-v<VERSION>` release contains both signed DMGs.
   It must no longer be a draft.
2. Confirm both `.app` bundles passed `codesign`, `spctl`, entitlements, and
   notarization checks in the workflow logs.
3. Confirm `sipher-desktop-latest` contains both macOS updater archives,
   signatures, and `latest.json`.
4. Inspect `latest.json`; it must contain `darwin-aarch64` and
   `darwin-x86_64`. It contains `windows-x86_64` only when Windows signing
   succeeded.
5. If Windows was unsigned, confirm the versioned release contains the
   `_unsigned.exe` preview and the rolling release does not.
6. Install each DMG on a clean machine, launch it through Gatekeeper, and
   confirm its default relay is `wss://buzz.sipher.gg:8443`.
7. Test an update from the previous Sipher build before directing users to the
   new versioned release.

Do not delete or move the immutable `sipher-v<VERSION>` tag. A retry must run
from that same tag and version.
