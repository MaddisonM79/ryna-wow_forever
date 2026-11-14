/** Shared pieces of the RawData decoders: the property node shapes and the item/pal id readers. */

import type { FArchiveReader, Node } from "../gvas/reader.js";

export const EOF_NOT_REACHED = "Warning: EOF not reached";

/** The bytes of an ArrayProperty node the generic reader produced (a RawData blob). */
const bytesOf = (node: Node): Uint8Array => (node["value"] as Node)["values"] as Uint8Array;

/** The generic read of a custom property's own bytes, then its decode in place of them. */
export function decodeArray(reader: FArchiveReader, typeName: string, size: number, path: string, decode: (bytes: Uint8Array) => unknown): Node {
  if (typeName !== "ArrayProperty") throw new Error(`Expected ArrayProperty, got ${typeName}`);
  const value = reader.property(typeName, size, path, path);
  value["value"] = decode(bytesOf(value));
  return value;
}

export const node = (v: unknown): Node => v as Node;
export const nodes = (v: unknown): Node[] => v as Node[];

export function palItemAndNum(r: FArchiveReader): Node {
  return {
    item_id: { static_id: r.fstring(), dynamic_id: { created_world_id: r.guid(), local_id_in_created_world: r.guid() } },
    num: r.u32(),
  };
}

export function palItemBoothTradeInfo(r: FArchiveReader): Node {
  return {
    product: { static_id: r.fstring(), dynamic_id: { created_world_id: r.guid(), local_id_in_created_world: r.guid() }, num: r.u32() },
    cost: { static_id: r.fstring(), dynamic_id: { created_world_id: r.guid(), local_id_in_created_world: r.guid() }, num: r.u32() },
    seller_player_uid: r.guid(),
  };
}

export function labResearchRepInfo(r: FArchiveReader): Node {
  return { research_id: r.fstring(), work_amount: r.float() };
}

export function instanceId(r: FArchiveReader): Node {
  return { guid: r.guid(), instance_id: r.guid() };
}

export const byte = (r: FArchiveReader): number => r.byte();
export const guid = (r: FArchiveReader): string => r.guid();
