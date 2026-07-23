import assert from "node:assert/strict";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const SCRIPT_URL = new URL("./sipher-release-config.mjs", import.meta.url);

async function loadModule() {
  return import(SCRIPT_URL);
}

test("buildSipherReleaseConfig builds the required production updater config", async () => {
  const { buildSipherReleaseConfig } = await loadModule();

  const config = buildSipherReleaseConfig({
    BUZZ_UPDATER_PUBLIC_KEY: "public-key",
    BUZZ_UPDATER_ENDPOINT: "https://updates.sipher.gg/latest.json",
    BUZZ_RELAY_URL: "wss://relay.sipher.gg",
    BUZZ_RELAY_HTTP: "https://relay.sipher.gg",
  });

  assert.equal(config.productName, "Buzz");
  assert.equal(config.identifier, "gg.sipher.buzz");
  assert.deepEqual(config.plugins.updater.endpoints, [
    "https://updates.sipher.gg/latest.json",
  ]);
  assert.equal(config.plugins.updater.pubkey, "public-key");
  assert.equal(config.bundle.createUpdaterArtifacts, true);
  assert.equal(config.bundle.macOS.minimumSystemVersion, "10.15");
  assert.equal(config.bundle.windows, undefined);
});

test("buildSipherReleaseConfig rejects missing env vars and non-production urls", async () => {
  const { buildSipherReleaseConfig } = await loadModule();

  assert.throws(
    () =>
      buildSipherReleaseConfig({
        BUZZ_UPDATER_PUBLIC_KEY: "public-key",
        BUZZ_UPDATER_ENDPOINT: "http://updates.sipher.gg/latest.json",
        BUZZ_RELAY_URL: "wss://relay.sipher.gg",
        BUZZ_RELAY_HTTP: "https://relay.sipher.gg",
      }),
    /BUZZ_UPDATER_ENDPOINT must use https:\/\//,
  );

  assert.throws(
    () =>
      buildSipherReleaseConfig({
        BUZZ_UPDATER_PUBLIC_KEY: "public-key",
        BUZZ_UPDATER_ENDPOINT: "https://updates.sipher.gg/latest.json",
        BUZZ_RELAY_URL: "ws://localhost:3000",
        BUZZ_RELAY_HTTP: "http://localhost:3000",
      }),
    /BUZZ_RELAY_URL must use wss:\/\/ and BUZZ_RELAY_HTTP must use https:\/\//,
  );

  assert.throws(
    () =>
      buildSipherReleaseConfig({
        BUZZ_UPDATER_PUBLIC_KEY: "public-key",
        BUZZ_RELAY_URL: "wss://relay.sipher.gg",
        BUZZ_RELAY_HTTP: "https://relay.sipher.gg",
      }),
    /required environment variable\(s\) missing: BUZZ_UPDATER_ENDPOINT/,
  );
});

test("writeSipherReleaseConfig writes the tauri merge config and optional windows signing settings", async () => {
  const { buildSipherReleaseConfig, writeSipherReleaseConfig } =
    await loadModule();

  const cwd = mkdtempSync(join(tmpdir(), "sipher-release-config-"));
  const config = buildSipherReleaseConfig({
    BUZZ_UPDATER_PUBLIC_KEY: "public-key",
    BUZZ_UPDATER_ENDPOINT: "https://updates.sipher.gg/latest.json",
    BUZZ_RELAY_URL: "wss://relay.sipher.gg",
    BUZZ_RELAY_HTTP: "https://relay.sipher.gg",
    BUZZ_WINDOWS_CERTIFICATE_THUMBPRINT: "ABCDEF0123456789",
  });

  const outputPath = writeSipherReleaseConfig({ cwd, config });
  const written = JSON.parse(readFileSync(outputPath, "utf8"));

  assert.equal(
    outputPath,
    resolve(cwd, "src-tauri/tauri.sipher.release.conf.json"),
  );
  assert.deepEqual(written, config);
  assert.deepEqual(written.bundle.windows, {
    certificateThumbprint: "ABCDEF0123456789",
    digestAlgorithm: "sha256",
    timestampUrl: "http://timestamp.digicert.com",
  });
});
