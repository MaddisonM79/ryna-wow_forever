/** FoliageGridSaveDataMap (trees and bushes per grid cell) and CharacterContainerSaveData slots. Ports of foliage_model.py, foliage_model_instance.py and character_container.py. */

import type { CustomReader, FArchiveReader, Node } from "../gvas/reader.js";
import { EOF_NOT_REACHED, decodeArray } from "./common.js";

function foliageModelBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  data["model_id"] = r.fstring();
  data["foliage_preset_type"] = r.byte();
  data["cell_coord"] = { x: r.i64(), y: r.i64(), z: r.i64() };
  data["trailing_bytes"] = r.byteList(4);
  if (!r.eof()) throw new Error(EOF_NOT_REACHED);
  return data;
}

export const foliageModel: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => foliageModelBytes(reader, b));

function foliageModelInstanceBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  data["model_instance_id"] = r.guid();
  const [pitch, yaw, roll] = r.compressedShortRotator();
  const [x, y, z] = r.packedVector(1);
  data["world_transform"] = { rotator: { pitch, yaw, roll }, location: { x, y, z }, scale_x: r.float() };
  data["hp"] = r.i32();
  if (!r.eof()) data["unknown_bytes"] = r.readToEnd();
  return data;
}

export const foliageModelInstance: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => foliageModelInstanceBytes(reader, b));

function characterContainerBytes(parent: FArchiveReader, bytes: Uint8Array): Node | null {
  if (bytes.byteLength === 0) return null;
  const r = parent.internalCopy(bytes);
  const data: Node = { player_uid: r.guid(), instance_id: r.guid(), permission_tribe_id: r.byte() };
  if (!r.eof()) data["unknown_bytes"] = r.readToEnd();
  return data;
}

export const characterContainer: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => characterContainerBytes(reader, b));
