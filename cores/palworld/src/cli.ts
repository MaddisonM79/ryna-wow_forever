/**
 * The Palworld core: Ryna starts it as a sidecar and drives it over stdin and stdout.
 *
 *     node agent.mjs serve                                  the sidecar (serve.ts, docs/ryna-agent-cores-mods.md 13.4)
 *     node agent.mjs snapshot OUT.json [--world ID] [--root DIR]
 *                                                           build one world's snapshot locally and push nothing
 *
 * `snapshot` is for checking a parser fix by hand: it reads the newest world (or the one named)
 * under the save folder (or DIR, a folder of <user>/<world>/ folders) and writes the JSON.
 */

import { writeFileSync } from "node:fs";
import { basename } from "node:path";
import { fileURLToPath } from "node:url";
import { snapshotWorld } from "./agent/snapshot-world.js";
import { log } from "./agent/status.js";
import { serve } from "./serve.js";
import { SAVE_ROOT, pickWorld, readFiles, saveFiles, worlds } from "./worlds.js";

/** `snapshot`'s arguments, in any order: the output path is the first one that isn't a flag or a flag's value. */
export function snapshotArgs(rest: readonly string[]): { out?: string; world?: string; root?: string } {
  const valueOf = (flag: string): [string | undefined, number] => {
    const at = rest.indexOf(flag);
    return [at >= 0 ? rest[at + 1] : undefined, at];
  };
  const [world, wAt] = valueOf("--world");
  const [root, rAt] = valueOf("--root");
  const out = rest.find((a, i) => !a.startsWith("--") && !(wAt >= 0 && i === wAt + 1) && !(rAt >= 0 && i === rAt + 1));
  return { ...(out ? { out } : {}), ...(world ? { world } : {}), ...(root ? { root } : {}) };
}

export async function main(argv: string[]): Promise<number> {
  const [cmd, ...rest] = argv;
  if (cmd === "serve") {
    return serve({ input: process.stdin, out: (line) => void process.stdout.write(line), err: (line) => void process.stderr.write(line) });
  }
  if (cmd === "snapshot") {
    const { out, world: worldArg, root } = snapshotArgs(rest);
    if (!out) throw new Error("usage: snapshot OUT.json [--world ID] [--root DIR]");
    const world = pickWorld(worlds(root ?? SAVE_ROOT), worldArg);
    if (!world) throw new Error("no such world");
    const { snap } = snapshotWorld(basename(world), readFiles(saveFiles(world)));
    writeFileSync(out, JSON.stringify(snap, (_k, v: unknown) => (typeof v === "bigint" ? Number(v) : v), 1));
    log(`wrote ${out}: ${(snap["pals"] as unknown[]).length} pals, ${(snap["bases"] as unknown[]).length} bases`);
    return 0;
  }
  process.stderr.write("usage: agent.mjs serve | snapshot OUT.json [--world ID] [--root DIR]\n");
  return 2;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main(process.argv.slice(2)).then(
    (code) => {
      // Let stdout drain before exiting: the last protocol line must reach the agent.
      process.stdout.write("", () => process.exit(code));
    },
    (e: unknown) => {
      process.stderr.write(`${e instanceof Error ? (e.stack ?? e.message) : String(e)}\n`);
      process.exit(1);
    },
  );
}
