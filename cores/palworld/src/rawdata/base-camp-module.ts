/** BaseCampSaveData.Value.ModuleMap: a base's modules by type. A port of base_camp_module.py. */

import type { CustomReader, FArchiveReader, Node } from "../gvas/reader.js";
import { node, nodes, palItemAndNum } from "./common.js";

const NO_OP_TYPES = new Set([
  "EPalBaseCampModuleType::Energy",
  "EPalBaseCampModuleType::Medical",
  "EPalBaseCampModuleType::ResourceCollector",
  "EPalBaseCampModuleType::ItemStorages",
  "EPalBaseCampModuleType::FacilityReservation",
  "EPalBaseCampModuleType::ObjectMaintenance",
  "EPalBaseCampModuleType::ItemStackInfo",
]);

const PASSIVE_EFFECT_TYPES = new Set([0, 1, 2, 3, 4, 5]);

function transportItemCharacterInfo(r: FArchiveReader): Node {
  return { item_infos: r.tarray(palItemAndNum), character_location: r.vectorDict() };
}

function modulePassiveEffect(r: FArchiveReader): Node {
  const data: Node = {};
  const type = r.byte();
  data["type"] = type;
  if (!PASSIVE_EFFECT_TYPES.has(type)) throw new Error(`Unknown passive effect type ${type}`);
  if (type === 2) {
    data["work_hard_type"] = r.byte();
    data["unknown_trailer"] = r.read(12);
  }
  return data;
}

function baseCampModuleBytes(parent: FArchiveReader, bytes: Uint8Array, moduleType: string): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  if (NO_OP_TYPES.has(moduleType)) {
    // nothing serialised
  } else if (moduleType === "EPalBaseCampModuleType::TransportItemDirector") {
    try {
      data["transport_item_character_infos"] = r.tarray(transportItemCharacterInfo);
      data["trailing_bytes"] = r.byteList(4);
    } catch {
      return { values: bytes };
    }
  } else if (moduleType === "EPalBaseCampModuleType::PassiveEffect") {
    try {
      data["passive_effects"] = r.tarray(modulePassiveEffect);
    } catch {
      return { values: bytes };
    }
  } else {
    return { values: bytes };
  }
  if (!r.eof()) return { values: bytes };
  return data;
}

export const baseCampModule: CustomReader = (reader, typeName, size, path) => {
  if (typeName !== "MapProperty") throw new Error(`Expected MapProperty, got ${typeName}`);
  const value = reader.property(typeName, size, path, path);
  for (const module of nodes(value["value"])) {
    const raw = node(node(module["value"])["RawData"]);
    raw["value"] = baseCampModuleBytes(reader, node(raw["value"])["values"] as Uint8Array, module["key"] as string);
  }
  return value;
};
