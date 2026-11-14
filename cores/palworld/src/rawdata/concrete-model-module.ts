/** A map object's concrete-model modules (its chest, workee, switch, lock...). A port of map_concrete_model_module.py. */

import type { FArchiveReader, Node } from "../gvas/reader.js";

const moduleSlotIndexes = (r: FArchiveReader): Node => ({ attribute: r.byte(), indexes: r.tarray((x) => x.i32()) });
const playerLockInfo = (r: FArchiveReader): Node => ({ player_uid: r.guid(), try_failed_count: r.i32(), try_success_cache: r.u32() });
/** FPalColorSettingEntry: FName Key + FLinearColor Color. */
const colorSettingEntry = (r: FArchiveReader): Node => ({ key: r.fstring(), color: { r: r.float(), g: r.float(), b: r.float(), a: r.float() } });

export function concreteModelModuleBytes(parent: FArchiveReader, bytes: Uint8Array, moduleType: string): Node {
  if (bytes.byteLength === 0) return { values: [] };
  const r = parent.internalCopy(bytes);
  const data: Node = {};
  switch (moduleType) {
    case "EPalMapObjectConcreteModelModuleType::ItemContainer":
      data["target_container_id"] = r.guid();
      data["slot_attribute_indexes"] = r.tarray(moduleSlotIndexes);
      data["all_slot_attribute"] = r.tarray((x) => x.byte());
      data["drop_item_at_disposed"] = r.u32();
      data["usage_type"] = r.byte();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "EPalMapObjectConcreteModelModuleType::CharacterContainer":
      data["target_container_id"] = r.guid();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "EPalMapObjectConcreteModelModuleType::Workee":
      data["target_work_id"] = r.guid();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "EPalMapObjectConcreteModelModuleType::Switch":
      data["switch_state"] = r.byte();
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "EPalMapObjectConcreteModelModuleType::PasswordLock":
      data["lock_state"] = r.byte();
      data["password"] = r.fstring();
      data["player_infos"] = r.tarray(playerLockInfo);
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "EPalMapObjectConcreteModelModuleType::RequireElementalAction":
      data["unlock_item"] = r.fstring();
      data["trailing_bytes"] = r.byteList(12);
      break;
    case "EPalMapObjectConcreteModelModuleType::GuildSecurity":
      data["allowed_roles"] = r.tarray((x) => x.byte());
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "EPalMapObjectConcreteModelModuleType::ColorSetting":
      data["color_entries"] = r.tarray(colorSettingEntry);
      data["trailing_bytes"] = r.byteList(4);
      break;
    case "EPalMapObjectConcreteModelModuleType::OperationalLoad":
      data["current_load"] = r.float();
      data["trailing_bytes"] = r.byteList(8);
      break;
    default:
      // Energy, StatusObserver, ItemStack, PlayerRecord, BaseCampPassiveEffect: nothing serialised.
      break;
  }
  if (!r.eof()) throw new Error(`Warning: EOF not reached for module type ${moduleType}`);
  return data;
}
