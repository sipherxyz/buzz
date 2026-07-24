import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const REQUIRED_ENV_KEYS = [
  "BUZZ_UPDATER_PUBLIC_KEY",
  "BUZZ_UPDATER_ENDPOINT",
  "BUZZ_RELAY_URL",
  "BUZZ_RELAY_HTTP",
];

const DEFAULT_WINDOWS_TIMESTAMP_URL = "http://timestamp.digicert.com";
const DEFAULT_OUTPUT_PATH = fileURLToPath(
  new URL("../src-tauri/tauri.sipher.release.conf.json", import.meta.url),
);

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

function assertProductionUrl(value, key, protocol, protocolError) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error(`${key} must be a valid ${protocol}// URL with a hostname`);
  }

  if (parsed.hostname.length === 0) {
    throw new Error(`${key} must be a valid ${protocol}// URL with a hostname`);
  }
  if (parsed.protocol !== protocol) {
    throw new Error(protocolError);
  }
}

function assertProductionRelayUrls(relayWsUrl, relayHttpUrl) {
  const protocolError =
    "BUZZ_RELAY_URL must use wss:// and BUZZ_RELAY_HTTP must use https://";
  assertProductionUrl(relayWsUrl, "BUZZ_RELAY_URL", "wss:", protocolError);
  assertProductionUrl(relayHttpUrl, "BUZZ_RELAY_HTTP", "https:", protocolError);
}

function assertProductionUpdaterEndpoint(updaterEndpoint) {
  assertProductionUrl(
    updaterEndpoint,
    "BUZZ_UPDATER_ENDPOINT",
    "https:",
    "BUZZ_UPDATER_ENDPOINT must use https://",
  );
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
  const unsignedTestBuild =
    readNonEmpty(env, "BUZZ_UNSIGNED_TEST_BUILD") === "1";

  assertProductionUpdaterEndpoint(updaterEndpoint);
  assertProductionRelayUrls(relayWsUrl, relayHttpUrl);

  const releaseConfig = {
    productName: "Buzz",
    identifier: "gg.sipher.buzz",
    bundle: {
      macOS: {
        minimumSystemVersion: "10.15",
        ...(unsignedTestBuild ? { signingIdentity: "-" } : {}),
      },
      createUpdaterArtifacts: !unsignedTestBuild,
    },
    plugins: {
      updater: unsignedTestBuild
        ? { endpoints: [] }
        : {
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

export function writeSipherReleaseConfig({
  outputPath = DEFAULT_OUTPUT_PATH,
  config,
}) {
  mkdirSync(dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, `${JSON.stringify(config, null, 2)}\n`);
  return outputPath;
}

function main() {
  const config = buildSipherReleaseConfig(process.env);
  const outputPath = writeSipherReleaseConfig({ config });
  console.log(`Updater enabled -> ${config.plugins.updater.endpoints[0]}`);
  console.log(`Wrote ${outputPath}`);
}

if (
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  try {
    main();
  } catch (error) {
    console.error(`Error: ${error.message}`);
    process.exit(1);
  }
}
