/** This computer's Palworld worlds on disk: finding them, waiting for a save to settle, reading and fingerprinting it. */

import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join } from "node:path";
import type { RawFiles } from "./agent/snapshot-world.js";
import { writeAtomic } from "./agent/status.js";
import type { Dict } from "./snapshot/util.js";
import { VERSION } from "./version.js";

/** The Mac App Store build's saves (core.json `detect.folders`), one folder per Epic user, then one per world. */
export const SAVE_ROOT = join(homedir(), "Library/Containers/com.pocketpair.palworld.mac/Data/Library/Application Support/Epic/Pal/Saved/SaveGames");
// The game writes Level.sav in place; a file touched this recently may be half-written.
export const SETTLE_SECONDS = 15;
export const RAW_FILES = ["Level.sav", "LevelMeta.sav", "WorldOption.sav", "LocalData.sav"] as const;

const entries = (dir: string): string[] => {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
};

/** Every world folder (SaveGames/<user>/<world>/Level.sav). */
export function worlds(root = SAVE_ROOT): string[] {
  const out: string[] = [];
  for (const user of entries(root)) {
    for (const name of entries(join(root, user))) if (existsSync(join(root, user, name, "Level.sav"))) out.push(join(root, user, name));
  }
  return out.sort();
}

export function saveFiles(world: string): string[] {
  const files = RAW_FILES.map((n) => join(world, n)).filter((p) => existsSync(p));
  const players = existsSync(join(world, "Players")) ? readdirSync(join(world, "Players")).filter((f) => f.endsWith(".sav")).sort() : [];
  return [...files, ...players.map((f) => join(world, "Players", f))];
}

export type Clock = { now: () => number; sleep: (s: number, signal?: AbortSignal) => Promise<void> };

/** A sleep that `stop` cuts short (it rejects with the signal's reason). */
export const realClock: Clock = {
  now: () => Date.now() / 1000,
  sleep: (s, signal) =>
    new Promise((resolve, reject) => {
      if (signal?.aborted) return reject(signal.reason as Error);
      const t = setTimeout(() => {
        signal?.removeEventListener("abort", onAbort);
        resolve();
      }, s * 1000);
      const onAbort = () => {
        clearTimeout(t);
        reject(signal!.reason as Error);
      };
      signal?.addEventListener("abort", onAbort, { once: true });
    }),
};

/**
 * The world's files once none has been written for SETTLE_SECONDS. The game
 * autosaves every 30 s while playing, so a pass usually lands inside a settle
 * window; waiting it out keeps pushes on the five-minute cadence. null if it
 * never settles in `tries`.
 */
export async function settled(world: string, tries = 4, clock: Clock = realClock, signal?: AbortSignal): Promise<string[] | null> {
  for (let i = 0; i < tries; i++) {
    const files = saveFiles(world);
    // Level.sav went away after worlds() listed the world (deleted in-game): fail this world only,
    // as any file that goes away does, rather than settle on the rest (or on nothing).
    const level = join(world, "Level.sav");
    if (!files.includes(level)) throw Object.assign(new Error(`ENOENT: no such file or directory, ${level}`), { code: "ENOENT" });
    const age = clock.now() - Math.max(...files.map((f) => statSync(f).mtimeMs / 1000));
    if (age >= SETTLE_SECONDS) return files;
    await clock.sleep(SETTLE_SECONDS - age + 1, signal);
  }
  return null;
}

/** The world's files by upload name, read once: the fingerprint and the push share these bytes. */
export function readFiles(files: string[]): RawFiles {
  const out: RawFiles = {};
  for (const f of files) out[basename(dirname(f)) === "Players" ? `Players/${basename(f)}` : basename(f)] = readFileSync(f);
  return out;
}

/**
 * The files' bytes and this core's version: a new release pushes each world's
 * newest save once, which ingest stores over the old snapshot (`upgraded`), or
 * skips if it already holds this version.
 */
export function fingerprint(raw: RawFiles, version = VERSION): string {
  const h = createHash("sha256");
  h.update(`palworld-agent ${version}\0`);
  for (const name of Object.keys(raw).sort()) {
    const data = raw[name]!;
    h.update(`${name}\0${data.byteLength}\0`);
    h.update(data);
  }
  return h.digest("hex");
}

/** state.json: each world's last accepted push (fingerprint, pushedAt, savedAt). Never a credential. */
export function loadState(path: string): Dict {
  try {
    return JSON.parse(readFileSync(path, "utf8")) as Dict;
  } catch {
    return {};
  }
}

/** Through a rename: a crash mid-write must not lose every fingerprint and re-upload every world. */
export function saveState(state: Dict, path: string): void {
  writeAtomic(path, state);
}

/** The newest world (by Level.sav), or the one named. */
export function pickWorld(found: readonly string[], id?: string): string | undefined {
  if (id) return found.find((w) => basename(w) === id);
  return found.length ? found.reduce((a, b) => (statSync(join(b, "Level.sav")).mtimeMs > statSync(join(a, "Level.sav")).mtimeMs ? b : a)) : undefined;
}
