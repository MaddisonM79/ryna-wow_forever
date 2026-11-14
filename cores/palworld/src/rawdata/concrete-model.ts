/** A map object's concrete model: the per-class state (a furnace's recipe, a chest's lock, a crop's growth). A port of map_concrete_model.py. */

import type { FArchiveReader, Node } from "../gvas/reader.js";
import { guid, palItemAndNum, palItemBoothTradeInfo } from "./common.js";
import { MAP_OBJECT_NAME_TO_CONCRETE_MODEL_CLASS } from "./concrete-model-names.js";

const palInstanceId = (r: FArchiveReader): Node => ({ player_uid: r.guid(), instance_id: r.guid() });

/** Classes whose state is only the 4 trailing bytes. */
const TRAILER_ONLY = new Set([
  "PalMapObjectPlayerBedModel",
  "PalBuildObject",
  "PalMapObjectCharacterStatusOperatorModel",
  "PalMapObjectRankUpCharacterModel",
  "BlueprintGeneratedClass",
  "PalMapObjectMedicalPalBedModel",
  "PalMapObjectDoorModel",
  "PalMapObjectMonsterFarmModel",
  "PalMapObjectAmusementModel",
  "PalMapObjectLabModel",
  "PalMapObjectRepairItemModel",
  "PalMapObjectBaseCampPassiveWorkHardModel",
  "PalMapObjectBaseCampPassiveEffectModel",
  "PalMapObjectBaseCampItemDispenserModel",
  "PalMapObjectGuildChestModel",
  "PalMapObjectCharacterMakeModel",
  "PalMapObjectPalFoodBoxModel",
  "PalMapObjectPlayerSitModel",
  "PalMapObjectBaseCampWorkerDirectorModel",
  "PalMapObjectPalMedicineBoxModel",
  "PalMapObjectDefenseWaitModel",
  "PalMapObjectHeatSourceModel",
  "PalMapObjectDisplayCharacterModel",
  "Default_PalMapObjectConcreteModelBase",
  "PalMapObjectDamagedScarecrowModel",
  "PalMapObjectGlobalPalStorageModel",
]);

export function concreteModelBytes(parent: FArchiveReader, bytes: Uint8Array, objectId: string): Node {
  if (bytes.byteLength === 0) return { values: [] };
  const cls = MAP_OBJECT_NAME_TO_CONCRETE_MODEL_CLASS[objectId.toLowerCase()];
  if (!cls) return { values: bytes };
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  data["instance_id"] = r.guid();
  data["model_instance_id"] = r.guid();
  data["concrete_model_type"] = cls;
  switch (cls) {
    case "PalMapObjectCharacterTeamMissionModel":
      data["leading_bytes"] = r.byteList(4);
      data["mission_id"] = r.fstring();
      data["assigned_individuals"] = r.tarray(palInstanceId);
      data["state"] = r.byte();
      data["start_time"] = r.i64();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "PalMapObjectFarmSkillFruitsModel":
      data["leading_bytes"] = r.byteList(4);
      data["skill_fruits_id"] = r.fstring();
      data["current_state"] = r.byte();
      data["progress_rate"] = r.float();
      data["trailing_bytes"] = r.byteList(20);
      break;
    case "PalMapObjectSupplyStorageModel":
      data["created_at_real_time"] = r.i64();
      data["trailing_bytes"] = r.byteList(8);
      break;
    case "PalMapObjectItemBoothModel":
      data["leading_bytes"] = r.byteList(4);
      data["private_lock_player_uid"] = r.guid();
      data["trade_infos"] = r.tarray(palItemBoothTradeInfo);
      data["trailing_bytes"] = r.byteList(20);
      break;
    case "PalMapObjectPalBoothModel":
    case "PalMapObjectMultiHatchingEggModel":
      data["unknown_bytes"] = r.readToEnd();
      break;
    case "PalMapObjectMultiHatchingEggWithBreedModel": {
      // The Ancient Hatchery: an entry per egg it holds (its size grows with them), then its breeding
      // state, then the id of its cake box and 4 bytes. Only that tail is read; the parents are its
      // work entry's and the eggs its egg box's (checked on six saves of 2026-11-02, 0 to 10 eggs).
      const rest = r.readToEnd();
      data["unknown_bytes"] = rest;
      if (rest.byteLength >= 20) data["cake_container_id"] = parent.internalCopy(rest.subarray(rest.byteLength - 20)).guid();
      break;
    }
    case "PalMapObjectEnergyStorageModel":
      data["stored_energy_amount"] = r.float();
      data["trailing_bytes"] = r.byteList(8);
      break;
    case "PalMapObjectDeathDroppedCharacterModel":
      data["stored_parameter_id"] = r.guid();
      data["owner_player_uid"] = r.guid();
      if (!r.eof()) data["unknown_bytes"] = r.readToEnd();
      break;
    case "PalMapObjectConvertItemModel":
      data["leading_bytes"] = r.byteList(4);
      data["current_recipe_id"] = r.fstring();
      data["requested_product_num"] = r.i32();
      data["remain_product_num"] = r.i32();
      data["work_speed_additional_rate"] = r.float();
      data["trailing_bytes"] = r.byteList(8);
      // The Ancient Furnace and Ancient Workbench add one byte (1 on the furnace, 0 on the bench; 2026-11-02 save).
      if (!r.eof()) data["unknown_bytes"] = r.readToEnd();
      break;
    case "PalMapObjectPickupItemOnLevelModel":
      data["auto_picked_up"] = r.u32();
      break;
    case "PalMapObjectDropItemModel":
      data["auto_picked_up"] = r.u32();
      data["pickupdable_player_uid"] = r.guid();
      data["remove_pickup_guard_timer_handle"] = r.i64();
      data["item_id"] = { static_id: r.fstring(), dynamic_id: { created_world_id: r.guid(), local_id_in_created_world: r.guid() } };
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "PalMapObjectItemDropOnDamagModel":
      data["drop_item_infos"] = r.tarray(palItemAndNum);
      if (!r.eof()) data["unknown_bytes"] = r.readToEnd();
      break;
    case "PalMapObjectDeathPenaltyStorageModel":
      data["auto_destroy_if_empty"] = r.u32();
      data["owner_player_uid"] = r.guid();
      data["created_at"] = r.u64();
      if (!r.eof()) data["trailing_bytes"] = r.byteList(4);
      break;
    case "PalMapObjectDefenseBulletLauncherModel":
      data["leading_bytes"] = r.byteList(4);
      data["remaining_bullets"] = r.i32();
      data["magazine_size"] = r.i32();
      data["bullet_item_name"] = r.fstring();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "PalMapObjectGenerateEnergyModel":
      data["generate_energy_rate_by_worker"] = r.float();
      data["stored_energy_amount"] = r.float();
      data["consume_energy_speed"] = r.float();
      break;
    case "PalMapObjectFarmBlockV2Model":
      data["crop_progress_rate"] = r.float();
      data["crop_data_id"] = r.fstring();
      data["current_state"] = r.byte();
      data["crop_progress_rate_value"] = r.float();
      data["water_stack_rate_value"] = r.float();
      data["state_machine"] = { growup_required_time: r.float(), growup_progress_time: r.float() };
      data["trailing_bytes"] = r.byteList(8);
      break;
    case "PalMapObjectFastTravelPointModel":
      data["location_instance_id"] = r.guid();
      if (!r.eof()) data["unknown_bytes"] = r.readToEnd();
      break;
    case "PalMapObjectShippingItemModel":
      data["shipping_hours"] = r.tarray((x) => x.i32());
      break;
    case "PalMapObjectProductItemModel":
      data["leading_bytes"] = r.byteList(4);
      data["work_speed_additional_rate"] = r.float();
      data["product_item_id"] = r.fstring();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "PalMapObjectRecoverOtomoModel":
      data["recover_amount_by_sec"] = r.float();
      break;
    case "PalMapObjectHatchingEggModel":
      data["leading_bytes"] = r.byteList(4);
      data["hatched_character_save_parameter"] = r.propertiesUntilEnd();
      data["current_pal_egg_temp_diff"] = r.i32();
      data["hatched_character_guid"] = r.guid();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "PalMapObjectTreasureBoxModel":
      data["treasure_grade_type"] = r.byte();
      data["treasure_special_type"] = r.byte();
      data["opened"] = r.byte();
      data["long_hold_interaction_duration"] = r.float();
      data["interact_player_action_type"] = r.byte();
      data["is_lock_riding"] = r.byte();
      break;
    case "PalMapObjectBreedFarmModel":
      data["leading_bytes"] = r.byteList(4);
      data["spawned_egg_instance_ids"] = r.tarray(guid);
      data["trailing_bytes"] = r.byteList(4);
      if (!r.eof()) {
        // Appended by the 2026-07 update; absent in older saves.
        data["last_proceed_worker_individual_ids"] = r.tarray(palInstanceId);
        data["target_breed_item_ids"] = r.tarray((x) => x.fstring());
      }
      break;
    case "PalMapObjectSignboardModel":
      data["leading_bytes"] = r.byteList(4);
      data["signboard_text"] = r.fstring();
      data["last_modified_player_uid"] = r.guid();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "PalMapObjectTorchModel":
      data["ignition_minutes"] = r.i32();
      data["extinction_date_time"] = r.i64();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "PalMapObjectPalEggModel":
      data["auto_picked_up"] = r.u32();
      data["pickupdable_player_uid"] = r.guid();
      data["remove_pickup_guard_timer_handle"] = r.i64();
      break;
    case "PalMapObjectBaseCampPoint":
      data["leading_bytes"] = r.byteList(4);
      data["base_camp_id"] = r.guid();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "PalMapObjectItemChestModel":
    case "PalMapObjectItemChest_AffectCorruption":
      data["leading_bytes"] = r.byteList(4);
      data["private_lock_player_uid"] = r.guid();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "PalMapObjectDimensionPalStorageModel":
      data["trailing_bytes"] = r.byteList(12);
      break;
    case "PalMapObjectLampModel":
      data["trailing_bytes"] = r.byteList(4);
      if (!r.eof()) {
        // Appended by the 2026-07 lamp light-colour update; absent in older saves.
        data["is_manually_turned_off"] = r.u32();
        data["unknown_bytes"] = r.byteList(4);
      }
      break;
    default:
      if (TRAILER_ONLY.has(cls)) {
        data["trailing_bytes"] = r.byteList(4);
        break;
      }
      return { values: bytes };
  }
  if (!r.eof()) throw new Error(`Warning: EOF not reached for ${objectId} ${cls}: remaining ${r.size - r.tell()}`);
  return data;
}
