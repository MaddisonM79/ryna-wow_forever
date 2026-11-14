/**
 * The shape of test/fixtures/rawdata.json, and the one JSON view of a decoded node that the
 * capture script (scripts/capture-rawdata.ts) and the fixture test share, so a BigInt or a byte
 * string compares the same way on both sides.
 */

/** One custom property as the save presented it: the bytes the reader consumed for it, and what it decoded them to. */
export type RawFixture = {
  /** The save it came from (its Level.sav's mtime), for the next capture. */
  savedAt: string;
  typeName: string;
  /** The `size` the GVAS header gave for the property: what the reader is told. */
  size: number;
  /** Every byte the decoder consumed, base64. */
  bytes: string;
  decoded: unknown;
};

export const b64 = (s: string): Uint8Array => new Uint8Array(Buffer.from(s, "base64"));

/** A decoded node as plain JSON: BigInts as `{ $bigint }`, byte strings as `{ $bytes }` (base64), and -0 as 0 (JSON has no -0, and toEqual tells them apart). */
export function plain(v: unknown): unknown {
  if (typeof v === "bigint") return { $bigint: v.toString() };
  if (typeof v === "number") return Object.is(v, -0) ? 0 : v;
  if (v instanceof Uint8Array) return { $bytes: Buffer.from(v).toString("base64") };
  if (Array.isArray(v)) return v.map(plain);
  if (v && typeof v === "object") return Object.fromEntries(Object.entries(v as Record<string, unknown>).map(([k, x]) => [k, plain(x)]));
  return v;
}
