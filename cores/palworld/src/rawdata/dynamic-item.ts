/** DynamicItemSaveData: eggs, weapons and armour with their per-instance state. A port of dynamic_item.py. */

import type { CustomReader, FArchiveReader, Node } from "../gvas/reader.js";
import { EOF_NOT_REACHED, decodeArray } from "./common.js";

function tryReadEgg(r: FArchiveReader): Node | null {
  const start = r.tell();
  try {
    const data: Node = { type: "egg" };
    data["leading_bytes"] = r.byteList(4);
    data["character_id"] = r.fstring();
    data["object"] = r.propertiesUntilEnd();
    data["trailing_bytes"] = r.byteList(28);
    if (!r.eof()) throw new Error(EOF_NOT_REACHED);
    return data;
  } catch (e) {
    if (e instanceof Error && e.message === EOF_NOT_REACHED) throw e;
    r.seek(start);
    return null;
  }
}

function dynamicItemBytes(parent: FArchiveReader, bytes: Uint8Array): Node | null {
  if (bytes.byteLength === 0) return null;
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  data["type"] = "unknown";
  data["id"] = { created_world_id: r.guid(), local_id_in_created_world: r.guid(), static_id: r.fstring() };
  const egg = tryReadEgg(r);
  if (egg) {
    Object.assign(data, egg);
  } else if (r.size - r.tell() === 12) {
    data["type"] = "armor";
    data["leading_bytes"] = r.byteList(4);
    data["durability"] = r.float();
    data["trailing_bytes"] = r.byteList(4);
    if (!r.eof()) throw new Error(EOF_NOT_REACHED);
  } else {
    const start = r.tell();
    const temp: Node = { type: "weapon" };
    try {
      temp["leading_bytes"] = r.byteList(4);
      temp["durability"] = r.float();
      temp["remaining_bullets"] = r.i32();
      temp["passive_skill_list"] = r.tarray((x) => x.fstring());
      if (r.size - r.tell() > 4) temp["unknown_str"] = r.fstring();
      temp["trailing_bytes"] = r.byteList(4);
      if (!r.eof()) throw new Error(EOF_NOT_REACHED);
      Object.assign(data, temp);
    } catch {
      // Not a weapon either: keep the bytes, as the library does.
      r.seek(start);
      data["trailer"] = r.readToEnd();
    }
  }
  return data;
}

export const dynamicItem: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => dynamicItemBytes(reader, b));
