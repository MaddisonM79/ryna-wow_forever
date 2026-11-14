/** WorkSaveData: every job in the world and who is assigned to it. A port of work.py. */

import type { CustomReader, FArchiveReader, Node } from "../gvas/reader.js";
import { node, nodes } from "./common.js";

const WORK_BASE_TYPES = new Set([
  "EPalWorkableType::Progress",
  "EPalWorkableType::Progress_MultiType",
  "EPalWorkableType::TransportItemInBaseCamp",
  "EPalWorkableType::ReviveCharacter",
  "EPalWorkableType::Booth",
  "EPalWorkableType::LevelObject",
  "EPalWorkableType::Repair",
  "EPalWorkableType::Defense",
  "EPalWorkableType::BootUp",
  "EPalWorkableType::OnlyJoin",
  "EPalWorkableType::OnlyJoinAndWalkAround",
  // Not in work.py: the Ancient Hatchery's breeding pair (checked on the 2026-11-02 saves).
  "EPalWorkableType::OnlyJoinInvisible",
  "EPalWorkableType::RemoveMapObjectEffect",
  "EPalWorkableType::MonsterFarm",
]);

const assignLocation = (r: FArchiveReader): Node => ({ location: r.vectorDict(), facing_direction: r.vectorDict() });
const progressEntry = (r: FArchiveReader): Node => ({ work_suitability: r.byte(), current_progress: r.float(), max_progress: r.float(), max_storable_progress: r.float() });
const suitabilityInfoEntry = (r: FArchiveReader): Node => ({ work_suitability: r.byte(), required_rank: r.i32() });

export function workBytes(parent: FArchiveReader, bytes: Uint8Array, workType: string): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  if (WORK_BASE_TYPES.has(workType)) {
    data["id"] = r.guid();
    data["workable_bounds"] = {
      location: r.vectorDict(),
      rotation: r.quatDict(),
      box_sphere_bounds: { origin: r.vectorDict(), box_extent: r.vectorDict(), sphere_radius: r.double() },
    };
    data["base_camp_id_belong_to"] = r.guid();
    data["owner_map_object_model_id"] = r.guid();
    data["owner_map_object_concrete_model_id"] = r.guid();
    data["current_state"] = r.byte();
    data["assign_locations"] = r.tarray(assignLocation);
    data["behaviour_type"] = r.byte();
    data["assign_define_data_id"] = r.fstring();
    data["override_work_type"] = r.byte();
    data["assignable_fixed_type"] = r.byte();
    data["assignable_otomo"] = r.u32();
    data["can_trigger_worker_event"] = r.u32();
    data["can_steal_assign"] = r.u32();
    switch (workType) {
      case "EPalWorkableType::Defense":
        data["leading_bytes"] = r.byteList(4);
        data["defense_combat_type"] = r.byte();
        data["trailing_bytes"] = r.byteList(4);
        break;
      case "EPalWorkableType::Progress":
      case "EPalWorkableType::Progress_MultiType":
        data["required_work_amount"] = r.float();
        data["current_work_amount"] = r.float();
        data["work_exp"] = r.i32();
        data["work_exp_calc_type"] = r.byte();
        data["auto_work_self_amount_by_sec"] = r.float();
        data["progress_time_since_last_tick"] = r.float();
        data["tick_process_min_interval"] = r.float();
        if (workType === "EPalWorkableType::Progress_MultiType") {
          data["progress_entries"] = r.tarray(progressEntry);
          data["suitability_info_entries"] = r.tarray(suitabilityInfoEntry);
        }
        break;
      case "EPalWorkableType::ReviveCharacter":
        data["target_individual_id"] = { player_uid: r.guid(), instance_id: r.guid() };
        break;
      case "EPalWorkableType::Repair":
      case "EPalWorkableType::MonsterFarm":
      case "EPalWorkableType::OnlyJoinAndWalkAround":
      case "EPalWorkableType::OnlyJoin":
      case "EPalWorkableType::OnlyJoinInvisible":
      case "EPalWorkableType::Booth":
        data["required_work_amount"] = r.float();
        break;
      default:
        break;
    }
  } else if (workType === "EPalWorkableType::Assign" || workType === "EPalWorkableType::LevelObject") {
    data["handle_id"] = r.guid();
    data["location_index"] = r.i32();
    data["assign_type"] = r.byte();
    data["assigned_individual_id"] = { player_uid: r.guid(), instance_id: r.guid() };
    data["state"] = r.byte();
    data["fixed"] = r.u32();
    if (workType === "EPalWorkableType::LevelObject") data["target_map_object_model_id"] = r.guid();
  }
  if (Object.keys(data).length === 0) return { values: bytes };
  // UPalWorkProgressTransformBase->SerializeProperties
  const transformType = r.byte();
  const transform: Node = { type: transformType };
  data["transform"] = transform;
  if (transformType === 2) {
    transform["map_object_instance_id"] = r.guid();
    transform["trailing_bytes"] = r.byteList(8);
  }
  if (!r.eof()) throw new Error(`Warning: EOF not reached for ${workType}`);
  return data;
}

function workAssignBytes(parent: FArchiveReader, bytes: Uint8Array, workType: string): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  data["id"] = r.guid();
  data["location_index"] = r.i32();
  data["assign_type"] = r.byte();
  data["assigned_individual_id"] = { player_uid: r.guid(), instance_id: r.guid() };
  data["state"] = r.byte();
  data["fixed"] = r.u32();
  data["trailing_bytes"] = r.byteList(4);
  if (workType === "EPalWorkableType::Progress_MultiType") {
    data["assigned_work_suitability"] = r.byte();
    data["assigned_work_type"] = r.byte();
    data["assigned_work_action_type"] = r.byte();
    data["multi_type_trailing_bytes"] = r.byteList(4);
  }
  if (!r.eof()) throw new Error(`Warning: EOF not reached for ${workType} work assign`);
  return data;
}

export const work: CustomReader = (reader, typeName, size, path) => {
  if (typeName !== "ArrayProperty") throw new Error(`Expected ArrayProperty, got ${typeName}`);
  const value = reader.property(typeName, size, path, path);
  for (const element of nodes(node(value["value"])["values"])) {
    const workType = node(node(element["WorkableType"])["value"])["value"] as string;
    const raw = node(element["RawData"]);
    raw["value"] = workBytes(reader, node(raw["value"])["values"] as Uint8Array, workType);
    for (const assign of nodes(node(element["WorkAssignMap"])["value"])) {
      const araw = node(node(assign["value"])["RawData"]);
      araw["value"] = workAssignBytes(reader, node(araw["value"])["values"] as Uint8Array, workType);
    }
  }
  return value;
};
