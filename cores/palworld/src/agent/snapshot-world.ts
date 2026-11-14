/** Parse a world's save files and build its snapshot: the step between reading the files and pushing them. */

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { readSave } from "../decode.js";
import { SaveFormatError } from "../sav.js";
import * as fog from "../snapshot/fog.js";
import { build } from "../snapshot/snapshot.js";
import type { Dict } from "../snapshot/util.js";
import { VERSION } from "../version.js";

// The installed game: its Info.plist says which build is installed (extract.sh reads the same keys).
const gameApp = (): string => process.env["PALWORLD_APP"] ?? "/Applications/Palworld.app";

/** The parser this agent runs, so a snapshot says which parser read it. */
const SAVE_TOOLS = "port of oMaN-Rod/palworld-save-tools@790e0bc";

/**
 * The installed game's {version, build}: Palworld.app's CFBundleShortVersionString
 * and CFBundleVersion, the pair scripts/palworld-gamedata/extract.sh records in
 * gamedata/meta.json. null if the app can't be read.
 */
function gameBuild(app = gameApp()): { version: string | null; build: string | null } | null {
  const plist = `${app}/Contents/Info.plist`;
  try {
    const json = execFileSync("plutil", ["-convert", "json", "-o", "-", plist], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
    const info = JSON.parse(json) as Record<string, unknown>;
    const version = info["CFBundleShortVersionString"];
    const buildNo = info["CFBundleVersion"];
    return { version: version == null ? null : String(version), build: buildNo == null ? null : String(buildNo) };
  } catch {
    // no plutil (Linux CI): an XML plist can still be read; a binary one can't
    try {
      const xml = readFileSync(plist, "utf8");
      if (!xml.startsWith("<?xml")) return null;
      const key = (k: string) => new RegExp(`<key>${k}</key>\\s*<string>([^<]*)</string>`).exec(xml)?.[1] ?? null;
      return { version: key("CFBundleShortVersionString"), build: key("CFBundleVersion") };
    } catch {
      // it only labels the snapshot; never let it stop a push
      return null;
    }
  }
}

export type RawFiles = Record<string, Uint8Array>;

/**
 * Parse one file. Whatever the parser throws (a game update it can't read yet, a damaged
 * save) becomes a SaveFormatError naming the file, so the run fails only this world. Only
 * the parse is covered: an error in building the snapshot from parsed values still ends the run.
 */
function parseFile(name: string, data: Uint8Array, parse: (data: Uint8Array) => Dict): Dict {
  try {
    return parse(data);
  } catch (e) {
    throw new SaveFormatError(`${name}: ${e instanceof Error ? e.message : String(e)}`, { cause: e });
  }
}

/** Parse the files read for a world and build the snapshot; returns it with the files to upload. */
export function snapshotWorld(worldId: string, raw: RawFiles, parse: (data: Uint8Array) => Dict = readSave): { snap: Dict; files: RawFiles } {
  const parsed = (name: string): Dict | null => (name in raw ? parseFile(name, raw[name]!, parse) : null);
  const level = parsed("Level.sav");
  // A SaveFormatError, so a world read without its Level.sav fails that world only (cli.ts isExpected).
  if (!level) throw new SaveFormatError("no Level.sav");
  const players: Record<string, Dict> = {};
  for (const [name, data] of Object.entries(raw)) {
    if (name.startsWith("Players/")) players[name.slice("Players/".length).replace(/\.sav$/, "")] = parseFile(name, data, parse);
  }
  const local = parsed("LocalData.sav");
  const mask = fog.maskBytes(local);
  const grid = mask ? fog.exploredGrid(mask) : null;
  const snap = build({
    worldId,
    level,
    meta: parsed("LevelMeta.sav"),
    options: parsed("WorldOption.sav"),
    players,
    agent: { version: VERSION, saveTools: SAVE_TOOLS },
    local,
    exploredPct: grid ? fog.exploredPct(grid) : null,
    gameBuild: gameBuild(),
  });
  // The fog of war rides along as a tiny PNG; the API keeps the latest for the map.
  // It goes into a copy: it's derived, so it stays out of the fingerprinted files.
  const files = grid ? { ...raw, "fog.png": fog.png(grid) } : raw;
  return { snap, files };
}
