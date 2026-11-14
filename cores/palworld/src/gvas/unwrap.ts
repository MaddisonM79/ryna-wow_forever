/**
 * Collapse the typed property tree into plain values, as the Python agent's
 * `unwrap` does: every property arrives as {type, value, id, ...} (plus
 * struct/array/map metadata); arrays become lists, maps become [{key, value}]
 * lists, byte and enum properties become their value. Byte blobs become
 * base64 strings, as the library's JSON encoder writes them.
 */

/** A bigint is a 64-bit value past 2^53 (a DateTime's ticks); everything that reaches JSON converts it first. */
export type Plain = null | boolean | number | bigint | string | Plain[] | { [k: string]: Plain };

const isNode = (o: Record<string, unknown>): boolean =>
  "type" in o && "value" in o && ("id" in o || "struct_type" in o || "array_type" in o || "key_type" in o);

export function unwrap(o: unknown): Plain {
  if (Array.isArray(o)) return o.map(unwrap);
  if (o instanceof Uint8Array) return Buffer.from(o.buffer, o.byteOffset, o.byteLength).toString("base64");
  if (o === null || o === undefined) return null;
  if (typeof o !== "object") return o as Plain;
  const d = o as Record<string, unknown>;
  if (isNode(d)) {
    const v = d["value"];
    const t = d["type"];
    if (t === "ArrayProperty") {
      if (v !== null && typeof v === "object" && !Array.isArray(v) && !(v instanceof Uint8Array)) {
        const vv = v as Record<string, unknown>;
        return unwrap("values" in vv ? vv["values"] : vv);
      }
      return unwrap(v);
    }
    if (t === "MapProperty") {
      return (v as Record<string, unknown>[]).map((e) => ({ key: unwrap(e["key"]), value: unwrap(e["value"]) }));
    }
    if ((t === "ByteProperty" || t === "EnumProperty") && v !== null && typeof v === "object" && !Array.isArray(v) && "value" in (v as object)) {
      return unwrap((v as Record<string, unknown>)["value"]);
    }
    return unwrap(v);
  }
  const keys = Object.keys(d);
  if (keys.length === 2 && "type" in d && "value" in d) return unwrap(d["value"]);
  const out: { [k: string]: Plain } = {};
  for (const k of keys) out[k] = unwrap(d[k]);
  return out;
}
