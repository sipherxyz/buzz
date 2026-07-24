import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import test from "node:test";

const SCRIPT_URL = new URL("./sipher-release-config.mjs", import.meta.url);

async function loadModule() {
  return import(SCRIPT_URL);
}

function validEnv(overrides = {}) {
  return {
    BUZZ_UPDATER_PUBLIC_KEY: "public-key",
    BUZZ_UPDATER_ENDPOINT: "https://updates.sipher.gg/latest.json",
    BUZZ_RELAY_URL: "wss://relay.sipher.gg",
    BUZZ_RELAY_HTTP: "https://relay.sipher.gg",
    BUZZ_WINDOWS_CERTIFICATE_THUMBPRINT: "",
    ...overrides,
  };
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

test("buildSipherReleaseConfig disables updater artifacts for unsigned test builds", async () => {
  const { buildSipherReleaseConfig } = await loadModule();

  const config = buildSipherReleaseConfig(
    validEnv({ BUZZ_UNSIGNED_TEST_BUILD: "1" }),
  );

  assert.equal(config.bundle.createUpdaterArtifacts, false);
  assert.equal(config.bundle.macOS.signingIdentity, "-");
  assert.deepEqual(config.plugins.updater.endpoints, []);
  assert.equal(config.plugins.updater.pubkey, undefined);
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

test("buildSipherReleaseConfig rejects a scheme-only updater endpoint", async () => {
  const { buildSipherReleaseConfig } = await loadModule();

  assert.throws(
    () =>
      buildSipherReleaseConfig(validEnv({ BUZZ_UPDATER_ENDPOINT: "https://" })),
    /BUZZ_UPDATER_ENDPOINT must be a valid https:\/\/ URL with a hostname/,
  );
});

test("buildSipherReleaseConfig rejects a malformed relay WebSocket URL", async () => {
  const { buildSipherReleaseConfig } = await loadModule();

  assert.throws(
    () =>
      buildSipherReleaseConfig(
        validEnv({ BUZZ_RELAY_URL: "wss://relay sipher.gg" }),
      ),
    /BUZZ_RELAY_URL must be a valid wss:\/\/ URL with a hostname/,
  );
});

test("buildSipherReleaseConfig rejects a scheme-only relay HTTP URL", async () => {
  const { buildSipherReleaseConfig } = await loadModule();

  assert.throws(
    () => buildSipherReleaseConfig(validEnv({ BUZZ_RELAY_HTTP: "https://" })),
    /BUZZ_RELAY_HTTP must be a valid https:\/\/ URL with a hostname/,
  );
});

test("writeSipherReleaseConfig writes the tauri merge config and optional windows signing settings", async () => {
  const { buildSipherReleaseConfig, writeSipherReleaseConfig } =
    await loadModule();

  const cwd = mkdtempSync(join(tmpdir(), "sipher-release-config-"));
  const expectedOutputPath = resolve(cwd, "injected/release-config.json");
  const config = buildSipherReleaseConfig({
    BUZZ_UPDATER_PUBLIC_KEY: "public-key",
    BUZZ_UPDATER_ENDPOINT: "https://updates.sipher.gg/latest.json",
    BUZZ_RELAY_URL: "wss://relay.sipher.gg",
    BUZZ_RELAY_HTTP: "https://relay.sipher.gg",
    BUZZ_WINDOWS_CERTIFICATE_THUMBPRINT: "ABCDEF0123456789",
  });

  const outputPath = writeSipherReleaseConfig({
    outputPath: expectedOutputPath,
    config,
  });
  const written = JSON.parse(readFileSync(outputPath, "utf8"));

  assert.equal(outputPath, expectedOutputPath);
  assert.deepEqual(written, config);
  assert.deepEqual(written.bundle.windows, {
    certificateThumbprint: "ABCDEF0123456789",
    digestAlgorithm: "sha256",
    timestampUrl: "http://timestamp.digicert.com",
  });
});

test("CLI resolves its entrypoint safely and writes relative to the desktop directory", () => {
  const root = realpathSync(
    mkdtempSync(join(tmpdir(), "sipher-release-config-#-")),
  );
  const scriptPath = resolve(root, "desktop/scripts/sipher-release-config.mjs");
  const expectedOutputPath = resolve(
    root,
    "desktop/src-tauri/tauri.sipher.release.conf.json",
  );
  mkdirSync(dirname(scriptPath), { recursive: true });
  copyFileSync(SCRIPT_URL, scriptPath);

  const result = spawnSync(process.execPath, [scriptPath], {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, ...validEnv() },
  });

  assert.equal(result.status, 0, result.stderr);
  assert.equal(existsSync(expectedOutputPath), true);
  assert.deepEqual(
    JSON.parse(readFileSync(expectedOutputPath, "utf8")),
    buildExpectedConfig(),
  );
});

function buildExpectedConfig() {
  return {
    productName: "Buzz",
    identifier: "gg.sipher.buzz",
    bundle: {
      macOS: {
        minimumSystemVersion: "10.15",
      },
      createUpdaterArtifacts: true,
    },
    plugins: {
      updater: {
        pubkey: "public-key",
        endpoints: ["https://updates.sipher.gg/latest.json"],
      },
    },
  };
}
