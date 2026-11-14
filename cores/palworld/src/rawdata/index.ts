/**
 * Palworld's own binary blobs inside the property tree, decoded by path: the
 * library's PALWORLD_CUSTOM_PROPERTIES table, read side only. The decode
 * replaces the blob's bytes in the tree, so unwrap sees plain values.
 */

import type { CustomReader } from "../gvas/reader.js";
import { baseCampModule } from "./base-camp-module.js";
import { dynamicItem } from "./dynamic-item.js";
import { characterContainer, foliageModel, foliageModelInstance } from "./foliage.js";
import { group } from "./group.js";
import { mapObject } from "./map-object.js";
import { baseCamp, character, guildItemStorage, guildLab, itemContainer, itemContainerSlots, workCollection, workerDirector } from "./simple.js";
import { work } from "./work.js";

export const CUSTOM_PROPERTIES: Record<string, CustomReader> = {
  ".worldSaveData.GroupSaveDataMap": group,
  ".worldSaveData.CharacterSaveParameterMap.Value.RawData": character,
  ".worldSaveData.ItemContainerSaveData.Value.RawData": itemContainer,
  ".worldSaveData.ItemContainerSaveData.Value.Slots.Slots.RawData": itemContainerSlots,
  ".worldSaveData.CharacterContainerSaveData.Value.Slots.Slots.RawData": characterContainer,
  ".worldSaveData.DynamicItemSaveData.DynamicItemSaveData.RawData": dynamicItem,
  ".worldSaveData.FoliageGridSaveDataMap.Value.ModelMap.Value.RawData": foliageModel,
  ".worldSaveData.FoliageGridSaveDataMap.Value.ModelMap.Value.InstanceDataMap.Value.RawData": foliageModelInstance,
  ".worldSaveData.BaseCampSaveData.Value.RawData": baseCamp,
  ".worldSaveData.BaseCampSaveData.Value.WorkerDirector.RawData": workerDirector,
  ".worldSaveData.BaseCampSaveData.Value.WorkCollection.RawData": workCollection,
  ".worldSaveData.BaseCampSaveData.Value.ModuleMap": baseCampModule,
  ".worldSaveData.WorkSaveData": work,
  ".worldSaveData.MapObjectSaveData": mapObject,
  ".worldSaveData.GuildExtraSaveDataMap.Value.GuildItemStorage.RawData": guildItemStorage,
  ".worldSaveData.GuildExtraSaveDataMap.Value.Lab.RawData": guildLab,
};
