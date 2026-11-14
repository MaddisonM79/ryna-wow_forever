/**
 * Struct types for map keys and values, which the stream doesn't name: an
 * unhinted key is read as a 16-byte Guid, which misaligns everything after it
 * (today's symptom is an "Error decoding ascii string"). PALWORLD_TYPE_HINTS is
 * the library's table; EXTRA_TYPE_HINTS is what the agent learnt on top, and
 * wins where they overlap.
 */

const PALWORLD_TYPE_HINTS: Record<string, string> = {
  ".worldSaveData.CharacterContainerSaveData.Key": "StructProperty",
  ".worldSaveData.CharacterSaveParameterMap.Key": "StructProperty",
  ".worldSaveData.CharacterSaveParameterMap.Value": "StructProperty",
  ".worldSaveData.FoliageGridSaveDataMap.Key": "StructProperty",
  ".worldSaveData.FoliageGridSaveDataMap.Value.ModelMap.Value": "StructProperty",
  ".worldSaveData.FoliageGridSaveDataMap.Value.ModelMap.Value.InstanceDataMap.Key": "StructProperty",
  ".worldSaveData.FoliageGridSaveDataMap.Value.ModelMap.Value.InstanceDataMap.Value": "StructProperty",
  ".worldSaveData.FoliageGridSaveDataMap.Value": "StructProperty",
  ".worldSaveData.ItemContainerSaveData.Key": "StructProperty",
  ".worldSaveData.MapObjectSaveData.MapObjectSaveData.ConcreteModel.ModuleMap.Value": "StructProperty",
  ".worldSaveData.MapObjectSaveData.MapObjectSaveData.Model.EffectMap.Value": "StructProperty",
  ".worldSaveData.MapObjectSpawnerInStageSaveData.Key": "StructProperty",
  ".worldSaveData.MapObjectSpawnerInStageSaveData.Value": "StructProperty",
  ".worldSaveData.MapObjectSpawnerInStageSaveData.Value.SpawnerDataMapByLevelObjectInstanceId.Key": "Guid",
  ".worldSaveData.MapObjectSpawnerInStageSaveData.Value.SpawnerDataMapByLevelObjectInstanceId.Value": "StructProperty",
  ".worldSaveData.MapObjectSpawnerInStageSaveData.Value.SpawnerDataMapByLevelObjectInstanceId.Value.ItemMap.Value": "StructProperty",
  ".worldSaveData.WorkSaveData.WorkSaveData.WorkAssignMap.Value": "StructProperty",
  ".worldSaveData.BaseCampSaveData.Key": "Guid",
  ".worldSaveData.BaseCampSaveData.Value": "StructProperty",
  ".worldSaveData.BaseCampSaveData.Value.ModuleMap.Value": "StructProperty",
  ".worldSaveData.ItemContainerSaveData.Value": "StructProperty",
  ".worldSaveData.CharacterContainerSaveData.Value": "StructProperty",
  ".worldSaveData.GroupSaveDataMap.Key": "Guid",
  ".worldSaveData.GroupSaveDataMap.Value": "StructProperty",
  ".worldSaveData.EnemyCampSaveData.EnemyCampStatusMap.Value": "StructProperty",
  ".worldSaveData.DungeonSaveData.DungeonSaveData.MapObjectSaveData.MapObjectSaveData.Model.EffectMap.Value": "StructProperty",
  ".worldSaveData.DungeonSaveData.DungeonSaveData.MapObjectSaveData.MapObjectSaveData.ConcreteModel.ModuleMap.Value": "StructProperty",
  ".worldSaveData.InvaderSaveData.Key": "Guid",
  ".worldSaveData.InvaderSaveData.Value": "StructProperty",
  ".worldSaveData.OilrigSaveData.OilrigMap.Value": "StructProperty",
  ".worldSaveData.InvaderDeclarationSaveData.ValidatedStartPointIds.StructProperty": "Guid",
  ".worldSaveData.SupplySaveData.SupplyInfos.Key": "Guid",
  ".worldSaveData.SupplySaveData.SupplyInfos.Value": "StructProperty",
  ".worldSaveData.GuildExtraSaveDataMap.Key": "Guid",
  ".worldSaveData.GuildExtraSaveDataMap.Value": "StructProperty",
  ".worldSaveData.EnemyCampSaveData.EnemyCampStatusMap.Value.TreasureBoxInfoMapBySpawnerName.Value": "StructProperty",
  ".worldSaveData.DungeonSaveData.DungeonSaveData.RewardSaveDataMap.Key": "Guid",
  ".worldSaveData.DungeonSaveData.DungeonSaveData.RewardSaveDataMap.Value": "StructProperty",
  ".SaveData.Local_MaxFriendshipPalIds.Key": "Guid",
  ".SaveData.Local_MaxFriendshipPalIds.Value": "StructProperty",
};

const EXTRA_TYPE_HINTS: Record<string, string> = {
  // LocalData.sav: PalInstanceID keys (PlayerUId, InstanceId, DebugName). Appears once a pal hits max friendship.
  ".SaveData.Local_MaxFriendshipPalIds.Key": "StructProperty",
};

export const TYPE_HINTS: Record<string, string> = { ...PALWORLD_TYPE_HINTS, ...EXTRA_TYPE_HINTS };
