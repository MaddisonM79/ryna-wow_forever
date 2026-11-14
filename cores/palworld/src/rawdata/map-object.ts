/** MapObjectSaveData: every placed object, with its model, connector, build state and concrete model decoded. A port of map_object.py. */

import type { CustomReader } from "../gvas/reader.js";
import { node, nodes } from "./common.js";
import { concreteModelModuleBytes } from "./concrete-model-module.js";
import { concreteModelBytes } from "./concrete-model.js";
import { buildProcessBytes, connectorBytes, mapModelBytes } from "./simple.js";

export const mapObject: CustomReader = (reader, typeName, size, path) => {
  if (typeName !== "ArrayProperty") throw new Error(`Expected ArrayProperty, got ${typeName}`);
  const value = reader.property(typeName, size, path, path);
  for (const mo of nodes(node(value["value"])["values"])) {
    const model = node(node(mo["Model"])["value"]);
    const modelRaw = node(model["RawData"]);
    modelRaw["value"] = mapModelBytes(reader, node(modelRaw["value"])["values"] as Uint8Array);
    const connectorRaw = node(node(node(model["Connector"])["value"])["RawData"]);
    connectorRaw["value"] = connectorBytes(reader, node(connectorRaw["value"])["values"] as Uint8Array);
    const buildRaw = node(node(node(model["BuildProcess"])["value"])["RawData"]);
    buildRaw["value"] = buildProcessBytes(reader, node(buildRaw["value"])["values"] as Uint8Array);
    const objectId = node(mo["MapObjectId"])["value"] as string;
    const concrete = node(node(mo["ConcreteModel"])["value"]);
    const concreteRaw = node(concrete["RawData"]);
    concreteRaw["value"] = concreteModelBytes(reader, node(concreteRaw["value"])["values"] as Uint8Array, objectId);
    for (const module of nodes(node(concrete["ModuleMap"])["value"])) {
      const raw = node(node(module["value"])["RawData"]);
      raw["value"] = concreteModelModuleBytes(reader, node(raw["value"])["values"] as Uint8Array, module["key"] as string);
    }
  }
  return value;
};
