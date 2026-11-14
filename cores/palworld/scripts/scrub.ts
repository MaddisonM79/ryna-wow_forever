/**
 * Scrubbing for test fixtures cut from a real save, so nothing of one person's world is published:
 * names (player, guild, world, nicknames) become same-length placeholders, and every GUID the
 * world generated becomes a stand-in derived from it (the same stand-in in every fixture, so ids
 * that join still join). Lengths never change, so a scrubbed byte blob still decodes to the
 * scrubbed value, consuming the same bytes. The all-zero GUID and the host's
 * 00000000-0000-0000-0000-00000000000X ids are every world's and stay.
 *
 * capture-rawdata.ts scrubs through this before it writes; `wrapStored` rebuilds a `.sav` around
 * scrubbed GVAS bytes as uncompressed Oodle blocks, which ooz decodes like any other.
 */

import { createHash } from "node:crypto";

/** Keys whose string values are a person's words. */
export const NAME_KEYS = new Set(["guild_name", "player_name", "NickName", "FilteredNickName", "WorldName", "name", "WorldNameText"]);
/** The game's own defaults under those keys: not anyone's words. */
const DEFAULT_NAME = /^(?:新規生成拠点テンプレート名\d+\(仮\)|Unnamed Guild|)$/;

const HEX = "[0-9A-Fa-f]";
/** 8-4-4-4-12 (palworld-save-tools' spelling) and 8-8-8-8 (Unreal's). */
const DASHED = new RegExp(`\\b${HEX}{8}-${HEX}{4}-?${HEX}{4}-${HEX}{4}-?${HEX}{4}-?${HEX}{8}\\b`, "g");
const PLAIN = new RegExp(`\\b${HEX}{32}\\b`, "g");

/** A GUID's 32 hex digits, lower case, whatever the spelling. */
export const key32 = (guid: string): string => guid.replace(/-/g, "").toLowerCase();

/** Every world shares these: all zeros but the last few digits. */
export const isShared = (k: string): boolean => /^0{28}/.test(k);

/** The stand-in for a GUID: derived, so the same id maps the same way in every fixture. */
export function standIn(k: string): string {
  return isShared(k) ? k : createHash("sha256").update(`ryna-palworld-fixture\0${k}`).digest("hex").slice(0, 32);
}

/** The 16 bytes an FGuid string (src/gvas/uuid.ts guidAt's spelling) was read from. */
export function guidBytes(k: string): Uint8Array {
  const h = (i: number) => parseInt(k.slice(i * 2, i * 2 + 2), 16);
  // guidAt prints bytes 3 2 1 0 - 7 6 - 5 4 - 11 10 - 9 8 15 14 13 12.
  const order = [3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12];
  const out = new Uint8Array(16);
  order.forEach((byte, i) => (out[byte] = h(i)));
  return out;
}

/** Every GUID spelled in `text`, as 32 hex digits. */
export function guidsIn(text: string): Set<string> {
  const out = new Set<string>();
  for (const m of text.matchAll(DASHED)) out.add(key32(m[0]));
  return out;
}

/** Same case as the original. */
const recase = (orig: string, k: string): string => (orig === orig.toUpperCase() && /[A-F]/.test(orig) ? k.toUpperCase() : k);

/** `text` with each known GUID, in any spelling, replaced by its stand-in. */
export function scrubText(text: string, known: ReadonlySet<string>): string {
  const dashed = text.replace(DASHED, (m: string) => {
    const k = key32(m);
    if (isShared(k)) return m;
    const s = recase(m, standIn(k));
    // Keep the spelling: put the dashes back where they were.
    let i = 0;
    return m.replace(/[0-9A-Fa-f]/g, () => s[i++]!);
  });
  return dashed.replace(PLAIN, (m: string) => (known.has(key32(m)) && !isShared(key32(m)) ? recase(m, standIn(key32(m))) : m));
}

function replaceAll(data: Uint8Array, from: Uint8Array, to: Uint8Array): number {
  if (from.length !== to.length || !from.length) throw new Error("a scrub must keep the length");
  let n = 0;
  outer: for (let i = 0; i + from.length <= data.length; i++) {
    for (let j = 0; j < from.length; j++) if (data[i + j] !== from[j]) continue outer;
    data.set(to, i);
    n++;
    i += from.length - 1;
  }
  return n;
}

/** A same-length placeholder for a name: ASCII letters, as many UTF-16 units as the original. */
export function placeholder(name: string): string {
  return "Scrubbed name ".repeat(Math.ceil(name.length / 14) + 1).slice(0, name.length);
}

/** `bytes` (a copy) with each known GUID's 16 bytes and each name, in either FString encoding, replaced. */
export function scrubBytes(bytes: Uint8Array, known: ReadonlySet<string>, names: ReadonlyMap<string, string>): Uint8Array {
  const out = Uint8Array.from(bytes);
  for (const k of known) if (!isShared(k)) replaceAll(out, guidBytes(k), guidBytes(standIn(k)));
  for (const [from, to] of names) {
    if (/^[\x20-\x7e]*$/.test(from)) replaceAll(out, Buffer.from(from, "latin1"), Buffer.from(to, "latin1"));
    replaceAll(out, Buffer.from(from, "utf16le"), Buffer.from(to, "utf16le"));
  }
  return out;
}

/** The names under NAME_KEYS in a decoded tree, each with its placeholder. */
export function namesIn(tree: unknown, into = new Map<string, string>()): Map<string, string> {
  const walk = (v: unknown, k: string): void => {
    if (Array.isArray(v)) v.forEach((x) => walk(x, k));
    else if (v && typeof v === "object" && !(v instanceof Uint8Array)) for (const [a, b] of Object.entries(v)) walk(b, a);
    else if (typeof v === "string" && NAME_KEYS.has(k) && !DEFAULT_NAME.test(v)) into.set(v, placeholder(v));
  };
  walk(tree, "");
  return into;
}

/** A decoded tree with its names replaced (as plain JSON text, GUIDs are scrubText's). */
export function scrubNames<T>(tree: T, names: ReadonlyMap<string, string>): T {
  const walk = (v: unknown, k: string): unknown => {
    if (Array.isArray(v)) return v.map((x) => walk(x, k));
    if (v && typeof v === "object" && !(v instanceof Uint8Array)) return Object.fromEntries(Object.entries(v).map(([a, b]) => [a, walk(b, a)]));
    if (typeof v === "string" && NAME_KEYS.has(k)) return names.get(v) ?? v;
    return v;
  };
  return walk(tree, "") as T;
}

/** A `.sav` (PlM) around GVAS bytes, as uncompressed Oodle (Kraken) blocks: what ooz reads like any other. */
export function wrapStored(gvas: Uint8Array): Uint8Array {
  const BLOCK = 0x40000;
  const blocks = Math.ceil(gvas.length / BLOCK);
  const body = new Uint8Array(gvas.length + blocks * 2);
  let o = 0;
  for (let at = 0; at < gvas.length; at += BLOCK) {
    // 0xCC: a block header (low nibble C) that restarts the decoder and is stored uncompressed; 0x06: Kraken.
    body.set([0xcc, 0x06], o);
    body.set(gvas.subarray(at, at + BLOCK), o + 2);
    o += 2 + Math.min(BLOCK, gvas.length - at);
  }
  const out = new Uint8Array(12 + body.length);
  const dv = new DataView(out.buffer);
  dv.setUint32(0, gvas.length, true);
  dv.setUint32(4, body.length, true);
  out.set([0x50, 0x6c, 0x4d, 0x31], 8); // "PlM", type 0x31
  out.set(body, 12);
  return out;
}
