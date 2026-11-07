// What `pnpm tauri build` adds to tauri.conf.json for this build, from what exists here and
// now: the updater's public key (agent/app/updater.pub), whether to make signed updater
// artifacts (only with TAURI_SIGNING_PRIVATE_KEY too), and Windows code signing (only with the
// Azure Artifact Signing names). With none of it the build is the plain unsigned one, which
// can't update itself and says so. scripts/tauri.mjs passes the result as `--config <json>`.

/** Azure Artifact Signing (formerly Trusted Signing): what names the certificate profile. */
export const AZURE_NAMES = [
  "AZURE_TRUSTED_SIGNING_ENDPOINT",
  "AZURE_TRUSTED_SIGNING_ACCOUNT",
  "AZURE_TRUSTED_SIGNING_PROFILE",
];
/** What artifact-signing-cli signs in as (read from the environment by the tool itself). */
export const AZURE_CREDENTIALS = ["AZURE_CLIENT_ID", "AZURE_TENANT_ID", "AZURE_CLIENT_SECRET"];

/** The tool Tauri's `bundle.windows.signCommand` runs on each binary and the installer. */
export const SIGN_TOOL = "artifact-signing-cli";

const set = (env, name) => typeof env[name] === "string" && env[name].trim() !== "";

/**
 * @param {{ command: string | undefined, pubkey: string | null, env: Record<string, string | undefined>, platform?: string }} input
 * @returns {{ config: Record<string, unknown> | null, notes: string[] }}
 */
export function releaseConfig({ command, pubkey, env, platform = process.platform }) {
  const notes = [];
  const building = command === "build" || command === "bundle";
  if (!building && command !== "dev") return { config: null, notes };

  const key = typeof pubkey === "string" && pubkey.trim() !== "" ? pubkey.trim() : null;
  const config = {};
  if (key) config.plugins = { updater: { pubkey: key } };
  if (!building) return { config: key ? config : null, notes };

  const bundle = {};
  const privateKey = set(env, "TAURI_SIGNING_PRIVATE_KEY");
  if (key && privateKey) {
    bundle.createUpdaterArtifacts = true;
    // macOS's updater artifact is the .app as a .tar.gz, made only when "app" is a target
    // (tauri.conf.json asks for the .dmg alone). Windows' is the NSIS installer itself.
    if (platform === "darwin") bundle.targets = ["app", "dmg"];
    notes.push("updater: key found, making signed updater artifacts");
  } else if (key) {
    notes.push("updater: key found but no TAURI_SIGNING_PRIVATE_KEY, so this build can update but makes no updater artifacts");
  } else if (privateKey) {
    notes.push("updater: TAURI_SIGNING_PRIVATE_KEY is set but agent/app/updater.pub is missing, so updates aren't set up in this build");
  } else {
    notes.push("updater: no agent/app/updater.pub, so updates aren't set up in this build");
  }

  const names = AZURE_NAMES.filter((n) => set(env, n));
  if (names.length === AZURE_NAMES.length) {
    const missing = AZURE_CREDENTIALS.filter((n) => !set(env, n));
    if (missing.length) {
      notes.push(`windows signing: off, the Artifact Signing names are set but not ${missing.join(", ")}`);
    } else {
      bundle.windows = {
        signCommand: {
          cmd: SIGN_TOOL,
          args: [
            "-e",
            env.AZURE_TRUSTED_SIGNING_ENDPOINT.trim(),
            "-a",
            env.AZURE_TRUSTED_SIGNING_ACCOUNT.trim(),
            "-c",
            env.AZURE_TRUSTED_SIGNING_PROFILE.trim(),
            "-d",
            "Ryna",
            "%1",
          ],
        },
      };
      notes.push(`windows signing: on, through ${SIGN_TOOL}`);
    }
  } else if (names.length) {
    const missing = AZURE_NAMES.filter((n) => !set(env, n));
    notes.push(`windows signing: off, missing ${missing.join(", ")}`);
  }

  if (Object.keys(bundle).length) config.bundle = bundle;
  return { config: Object.keys(config).length ? config : null, notes };
}
