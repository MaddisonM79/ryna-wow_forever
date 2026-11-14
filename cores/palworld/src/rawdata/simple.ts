/**
 * The small RawData blobs: one record each, read straight through. Ports of
 * character.py, item_container.py, item_container_slots.py, base_camp.py,
 * worker_director.py, work_collection.py, guild_lab.py, guild_item_storage.py,
 * map_model.py, connector.py and build_process.py.
 */

import type { CustomReader, FArchiveReader, Node } from "../gvas/reader.js";
import { EOF_NOT_REACHED, decodeArray, guid, labResearchRepInfo } from "./common.js";

function characterBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = { object: r.propertiesUntilEnd(), unknown_bytes: r.byteList(4), group_id: r.guid() };
  data["trailing_bytes"] = r.byteList(4);
  if (!r.eof()) throw new Error(EOF_NOT_REACHED);
  return data;
}

export const character: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => characterBytes(reader, b));

function itemContainerBytes(parent: FArchiveReader, bytes: Uint8Array): Node | null {
  if (bytes.byteLength === 0) return null;
  const r = parent.internalCopy(bytes);
  const data: Node = {
    permission: { type_a: r.tarray((x) => x.byte()), type_b: r.tarray((x) => x.byte()), item_static_ids: r.tarray((x) => x.fstring()) },
  };
  if (!r.eof()) data["trailing_unparsed_data"] = r.readToEnd();
  return data;
}

export const itemContainer: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => itemContainerBytes(reader, b));

function itemContainerSlotsBytes(parent: FArchiveReader, bytes: Uint8Array): Node | null {
  if (bytes.byteLength === 0) return null;
  const r = parent.internalCopy(bytes);
  return {
    slot_index: r.i32(),
    count: r.i32(),
    item: { static_id: r.fstring(), dynamic_id: { created_world_id: r.guid(), local_id_in_created_world: r.guid() } },
    trailing_bytes: r.readToEnd(),
  };
}

export const itemContainerSlots: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => itemContainerSlotsBytes(reader, b));

function baseCampBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {
    id: r.guid(),
    name: r.fstring(),
    state: r.byte(),
    transform: r.ftransform(),
    area_range: r.float(),
    group_id_belong_to: r.guid(),
    fast_travel_local_transform: r.ftransform(),
    owner_map_object_instance_id: r.guid(),
  };
  data["trailing_bytes"] = r.byteList(4);
  if (!r.eof()) throw new Error(EOF_NOT_REACHED);
  return data;
}

export const baseCamp: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => baseCampBytes(reader, b));

function workerDirectorBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  data["id"] = r.guid();
  data["spawn_transform"] = r.ftransform();
  data["current_order_type"] = r.byte();
  data["current_battle_type"] = r.byte();
  data["container_id"] = r.guid();
  data["trailing_bytes"] = r.byteList(4);
  if (!r.eof()) throw new Error(EOF_NOT_REACHED);
  return data;
}

export const workerDirector: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => workerDirectorBytes(reader, b));

function workCollectionBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  data["id"] = r.guid();
  data["work_ids"] = r.tarray(guid);
  data["trailing_bytes"] = r.byteList(4);
  if (!r.eof()) throw new Error(EOF_NOT_REACHED);
  return data;
}

export const workCollection: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => workCollectionBytes(reader, b));

function guildLabBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  data["research_info"] = r.tarray(labResearchRepInfo);
  data["current_research_id"] = r.fstring();
  if (!r.eof()) data["trailing_bytes"] = r.readToEnd();
  return data;
}

export const guildLab: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => guildLabBytes(reader, b));

function guildItemStorageBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = { container_id: r.guid() };
  if (!r.eof()) data["trailing_bytes"] = r.readToEnd();
  return data;
}

export const guildItemStorage: CustomReader = (reader, typeName, size, path) => decodeArray(reader, typeName, size, path, (b) => guildItemStorageBytes(reader, b));

export function mapModelBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  data["instance_id"] = r.guid();
  data["concrete_model_instance_id"] = r.guid();
  data["base_camp_id_belong_to"] = r.guid();
  data["group_id_belong_to"] = r.guid();
  data["hp"] = { current: r.i32(), max: r.i32() };
  data["initital_transform_cache"] = r.ftransform();
  data["repair_work_id"] = r.guid();
  data["owner_spawner_level_object_instance_id"] = r.guid();
  data["owner_instance_id"] = r.guid();
  data["build_player_uid"] = r.guid();
  data["interact_restrict_type"] = r.byte();
  data["deterioration_damage"] = r.float();
  data["stage_instance_id_belong_to"] = { id: r.guid(), valid: r.u32() };
  if (!r.eof()) data["unknown_bytes"] = r.readToEnd();
  return data;
}

export function connectorBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  if (bytes.byteLength === 0) return { values: [] };
  const r = parent.internalCopy(bytes);
  const data: Node = {
    supported_level: r.i32(),
    connect: { index: r.byte(), any_place: r.tarray((x) => ({ connect_to_model_instance_id: x.guid(), index: x.byte() })) },
  };
  if (!r.eof()) data["unknown_bytes"] = r.readToEnd();
  return data;
}

export function buildProcessBytes(parent: FArchiveReader, bytes: Uint8Array): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = { state: r.byte(), id: r.guid() };
  data["trailing_bytes"] = r.byteList(4);
  if (!r.eof()) throw new Error(EOF_NOT_REACHED);
  return data;
}
