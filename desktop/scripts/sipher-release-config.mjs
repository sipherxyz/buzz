import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const REQUIRED_ENV_KEYS = [
  "BUZZ_UPDATER_PUBLIC_KEY",
  "BUZZ_UPDATER_ENDPOINT",
  "BUZZ_RELAY_URL",
  "BUZZ_RELAY_HTTP",
];

const DEFAULT_WINDOWS_TIMESTAMP_URL = "http://timestamp.digicert.com";

function readNonEmpty(env, key) {
  const value = env[key];
  if (typeof value !== "string") return null;

  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function assertRequiredEnv(env) {
  const missing = REQUIRED_ENV_KEYS.filter(
    (key) => readNonEmpty(env, key) === null,
  );
  if (missing.length > 0) {
    throw new Error(
      `required environment variable(s) missing: ${missing.join(", ")}`,
    );
  }
}

function assertProductionRelayUrls(relayWsUrl, relayHttpUrl) {
  if (
    !relayWsUrl.startsWith("wss://") ||
    !relayHttpUrl.startsWith("https://")
  ) {
    throw new Error(
      "BUZZ_RELAY_URL must use wss:// and BUZZ_RELAY_HTTP must use https://",
    );
  }
}

export function buildSipherReleaseConfig(env) {
  assertRequiredEnv(env);

  const updaterPubkey = readNonEmpty(env, "BUZZ_UPDATER_PUBLIC_KEY");
  const updaterEndpoint = readNonEmpty(env, "BUZZ_UPDATER_ENDPOINT");
  const relayWsUrl = readNonEmpty(env, "BUZZ_RELAY_URL");
  const relayHttpUrl = readNonEmpty(env, "BUZZ_RELAY_HTTP");
  const certificateThumbprint = readNonEmpty(
    env,
    "BUZZ_WINDOWS_CERTIFICATE_THUMBPRINT",
  );

  assertProductionRelayUrls(relayWsUrl, relayHttpUrl);

  const releaseConfig = {
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
        pubkey: updaterPubkey,
        endpoints: [updaterEndpoint],
      },
    },
  };

  if (certificateThumbprint !== null) {
    releaseConfig.bundle.windows = {
      certificateThumbprint,
      digestAlgorithm: "sha256",
      timestampUrl: DEFAULT_WINDOWS_TIMESTAMP_URL,
    };
  }

  return releaseConfig;
}

export function writeSipherReleaseConfig({ cwd = process.cwd(), config }) {
  const outputPath = resolve(cwd, "src-tauri/tauri.sipher.release.conf.json");
  mkdirSync(resolve(cwd, "src-tauri"), { recursive: true });
  writeFileSync(outputPath, `${JSON.stringify(config, null, 2)}\n`);
  return outputPath;
}

function main() {
  const config = buildSipherReleaseConfig(process.env);
  const outputPath = writeSipherReleaseConfig({ config });
  console.log(`Updater enabled -> ${config.plugins.updater.endpoints[0]}`);
  console.log(`Wrote ${outputPath}`);
}

if (import.meta.url === new URL(process.argv[1], "file:").href) {
  try {
    main();
  } catch (error) {
    console.error(`Error: ${error.message}`);
    process.exit(1);
  }
}
