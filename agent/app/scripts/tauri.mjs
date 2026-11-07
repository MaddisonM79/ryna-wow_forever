// `pnpm tauri …`: the Tauri CLI, with this build's additions (release-config.mjs) merged in as
// `--config <json>` right after the subcommand. Anything else passes through untouched.
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { releaseConfig } from "./release-config.mjs";

const require = createRequire(import.meta.url);
const cli = require("@tauri-apps/cli");

const pubFile = join(dirname(fileURLToPath(import.meta.url)), "..", "updater.pub");
const pubkey = existsSync(pubFile) ? readFileSync(pubFile, "utf8") : null;

const args = process.argv.slice(2);
const { config, notes } = releaseConfig({ command: args[0], pubkey, env: process.env });
for (const note of notes) console.error(`[ryna] ${note}`);
const finalArgs = config ? [args[0], "--config", JSON.stringify(config), ...args.slice(1)] : args;

cli.run(finalArgs, "pnpm tauri").catch((err) => {
  cli.logError(err.message);
  process.exit(1);
});
