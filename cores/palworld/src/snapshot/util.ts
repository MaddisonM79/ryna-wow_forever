/** Small helpers for reading unwrapped save data. Shared by snapshot.ts and world.ts. A port of snapshot_util.py. */

import type { Plain } from "../gvas/unwrap.js";

export type Dict = { [k: string]: Plain };

export const ZERO_GUID = "00000000-0000-0000-0000-000000000000";
// The game's placeholder name for a base nobody renamed (Japanese, "new base template name 1 (temp)").
export const DEFAULT_BASE_NAME_PREFIX = "新規生成拠点テンプレート名";

// GotStatusPointList names are Japanese in the save.
const STATUS_NAMES: Record<string, string> = {
  最大HP: "hp",
  最大SP: "stamina",
  攻撃力: "attack",
  所持重量: "weight",
  捕獲率: "captureRate",
  作業速度: "workSpeed",
  // Added in 1.0 (the save stores the Japanese labels, whatever the game's language).
  移動速度アップ: "moveSpeed",
  空腹率低減: "hungerReduction",
  経験値ボーナス: "expBonus",
  // The effigy upgrades, keyed as the relic types name them (seen on the 2026-10-24 save).
  泳ぎ速度: "swimSpeed",
  食料腐敗低減: "foodDecayReduction",
  滑空速度: "gliderSpeed",
  崖登り速度: "climbSpeed",
  状態異常耐性: "statusAilmentResist",
};

export const INVENTORY_CONTAINERS: Record<string, string> = {
  common: "CommonContainerId",
  essential: "EssentialContainerId",
  weapons: "WeaponLoadOutContainerId",
  armor: "PlayerEquipArmorContainerId",
  food: "FoodEquipContainerId",
  drop: "DropSlotContainerId",
};

export const isDict = (v: unknown): v is Dict => v !== null && typeof v === "object" && !Array.isArray(v);
const isList = (v: unknown): v is Plain[] => Array.isArray(v);
export const dict = (v: unknown): Dict => (isDict(v) ? v : {});
export const list = (v: unknown): Plain[] => (isList(v) ? v : []);
export const isNum = (v: unknown): v is number => typeof v === "number";
/** Python's `isinstance(v, int)`: an integer value, as a Number or a bigint. */
export const isInt = (v: unknown): v is number | bigint => typeof v === "bigint" || (typeof v === "number" && Number.isInteger(v));
export const str = (v: unknown): string | null => (typeof v === "string" ? v : null);

/**
 * Python's round(): half to even on the value's exact decimal expansion. A tie
 * only happens for a dyadic fraction (0.125 to 2 places), where x * 10^n is exact too.
 */
export function pyRound(x: number, digits = 0): number {
  const m = 10 ** digits;
  const y = x * m;
  const f = Math.floor(y);
  let r: number;
  if (y - f === 0.5) r = f % 2 === 0 ? f : f + 1;
  else r = Math.round(y);
  return digits === 0 ? r : r / m;
}

// 0001-01-01T00:00:00Z to 1970-01-01T00:00:00Z, in seconds.
const UE_EPOCH_OFFSET_S = 62135596800n;

const pad = (n: number, w: number) => String(n).padStart(w, "0");

/**
 * Unreal DateTime ticks (100 ns since 0001-01-01) as UTC ISO-8601, formatted as
 * Python's isoformat(): microseconds only when they aren't zero.
 *
 * The game is inconsistent: save `Timestamp`s and a pal's `OwnedTime` are this
 * Mac's wall-clock time (they match the backup folder names), while
 * `LastOnlineDateTime` and a camp's `ClearDate` are UTC. `local` reads the
 * ticks as wall-clock time in this machine's zone.
 */
export function ticksToIso(ticks: unknown, local = false): string | null {
  if (!isInt(ticks)) return null;
  const t = BigInt(ticks);
  if (t <= 0n) return null;
  const micros = t / 10n;
  const unixSec = Number(micros / 1_000_000n - UE_EPOCH_OFFSET_S);
  const frac = Number(micros % 1_000_000n);
  let d = new Date(unixSec * 1000);
  if (local) {
    // The same wall-clock fields, read in this machine's zone.
    d = new Date(d.getUTCFullYear(), d.getUTCMonth(), d.getUTCDate(), d.getUTCHours(), d.getUTCMinutes(), d.getUTCSeconds());
  }
  const base = `${pad(d.getUTCFullYear(), 4)}-${pad(d.getUTCMonth() + 1, 2)}-${pad(d.getUTCDate(), 2)}T${pad(d.getUTCHours(), 2)}:${pad(d.getUTCMinutes(), 2)}:${pad(d.getUTCSeconds(), 2)}`;
  return frac ? `${base}.${pad(frac, 6)}Z` : `${base}Z`;
}

/** `EPalGenderType::Female` -> `Female`. */
export function enumName<T>(v: T): T | string {
  return typeof v === "string" && v.includes("::") ? v.slice(v.indexOf("::") + 2) : v;
}

/**
 * A map object's decoded ConcreteModel fields, or {}: the parser leaves a model
 * type it can't decode as its raw base64 string (a new structure after a game
 * update), and an empty one as [].
 */
export function concreteRaw(concrete: Dict): Dict {
  return dict(concrete["RawData"]);
}

/** A MapProperty ([{"key", "value"}]) as a dict. */
export function mapd(entries: unknown): Dict {
  const out: Dict = {};
  if (!isList(entries)) return out;
  for (const e of entries) {
    if (isDict(e) && "key" in e) out[String(e["key"])] = e["value"] ?? null;
  }
  return out;
}

/** The keys of a name->bool map that are set, sorted. */
export function flags(entries: unknown): string[] {
  return Object.entries(mapd(entries))
    .filter(([, v]) => !!v)
    .map(([k]) => k)
    .sort(byCodePoint);
}

/** A PalContainerId ({"ID": guid}) or a bare guid, as a guid string. */
export function cid(v: unknown): string | null {
  const g = isDict(v) ? v["ID"] : v;
  return typeof g === "string" && g !== ZERO_GUID ? g : null;
}

/**
 * A guid in any spelling as 32 lower-case hex, the form the API keys spawners by
 * (gamedata.ts guidKey). The save writes "6a13305d-4923-3b02-…", the game's map
 * "6A13305D-49233B02-…". null for the zero guid.
 */
export function guidKey(v: unknown): string | null {
  const key = typeof v === "string" ? v.toLowerCase().replace(/[^0-9a-f]/g, "") : "";
  return key.replace(/0/g, "") ? key : null;
}

export function num(v: unknown, digits = 2): number | null {
  if (typeof v === "bigint") return pyRound(Number(v), digits);
  return typeof v === "number" ? pyRound(v, digits) : null;
}

/** FixedPoint64 HP ({"Value": 648000}) is thousandths. */
export function fixed(v: unknown): number | null {
  if (isDict(v) && isNum(v["Value"])) return pyRound(v["Value"] / 1000, 1);
  return null;
}

export type Position = { x: number; y: number; z: number; mapX: number; mapY: number };

/**
 * World coordinates, plus the in-game map's coordinates. The map transform is
 * the community-derived one for the main island (x' = (y - 158000) / 459,
 * y' = (x + 123888) / 459); treat it as approximate.
 */
export function position(v: unknown): Position | null {
  let p = v;
  if (isDict(p) && "translation" in p) p = p["translation"];
  if (isDict(p) && "Translation" in p) p = p["Translation"];
  if (!isDict(p) || !isNum(p["x"]) || !isNum(p["y"]) || !isNum(p["z"])) return null;
  return {
    x: pyRound(p["x"], 1),
    y: pyRound(p["y"], 1),
    z: pyRound(p["z"], 1),
    mapX: pyRound((p["y"] - 158000) / 459),
    mapY: pyRound((p["x"] + 123888) / 459),
  };
}

export function statusPoints(entries: unknown): Record<string, number> {
  const out: Record<string, number> = {};
  for (const e of list(entries)) {
    if (!isDict(e)) continue;
    const raw = e["StatusName"];
    const name = (typeof raw === "string" && STATUS_NAMES[raw]) || raw;
    if (name) out[String(name)] = Math.trunc(Number(e["StatusPoint"] ?? 0) || 0);
  }
  return out;
}

/** Python's default string order: by code point. */
export const byCodePoint = (a: string, b: string): number => (a < b ? -1 : a > b ? 1 : 0);

/** Sum counts per item, sorted by item: Python's `Counter` then `sorted(items())`. */
export function countItems(pairs: Iterable<[string, number]>): { item: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const [k, n] of pairs) counts.set(k, (counts.get(k) ?? 0) + n);
  return [...counts.entries()].sort(([a], [b]) => byCodePoint(a, b)).map(([item, count]) => ({ item, count }));
}
