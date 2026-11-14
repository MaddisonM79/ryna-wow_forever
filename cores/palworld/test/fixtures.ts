/** A small hand-made world (one player, three pals, one base, one guild), and trimmed real sections. Ported from the Python tests. */

import { readFileSync } from "node:fs";
import type { Dict } from "../src/snapshot/util.js";

export const UID = "00000000-0000-0000-0000-000000000001";
const PARTY = "c-party";
const BOX = "c-box";
const WORKERS = "c-workers";
const CHEST = "c-chest";
const BAG = "c-bag";
const HATCHER = "c-hatcher";
const MULTI_HATCHER = "c-multi-hatcher";
export const GUILD_CHEST = "c-guild-chest";
export const BASE = "base-1";
const GUILD = "guild-1";
export const ZERO = "00000000-0000-0000-0000-000000000000";

/** An item slot; `dyn` is the per-instance id gear and eggs carry (DynamicItemSaveData). */
const slot = (item: string, count: number, dyn?: string): Dict => ({
  RawData: { count, item: { static_id: item, ...(dyn ? { dynamic_id: { local_id_in_created_world: dyn } } : {}) } },
});

const egg = (dyn: string, item: string, species: string): Dict => ({
  RawData: { type: "egg", id: { local_id_in_created_world: dyn, static_id: item }, character_id: species },
});

const character = (key: Dict, sp: Dict): Dict => ({ key, value: { RawData: { object: { SaveParameter: sp } } } });

export const WORLD: Dict = {
  Timestamp: 639269610204320000n,
  worldSaveData: {
    CharacterSaveParameterMap: [
      character({ PlayerUId: UID, InstanceId: "p" }, { IsPlayer: true, NickName: "Tamer", Level: 25, GotStatusPointList: [{ StatusName: "攻撃力", StatusPoint: 11 }, { StatusName: "移動速度アップ", StatusPoint: 2 }, { StatusName: "滑空速度", StatusPoint: 2 }] }),
      character(
        { PlayerUId: UID, InstanceId: "a" },
        {
          CharacterID: "PinkCat", Level: 16, Talent_Shot: 88, PassiveSkillList: ["Swift"], EquipWaza: ["EPalWazaID::AirCanon"], Gender: "EPalGenderType::Male",
          SlotId: { ContainerId: { ID: WORKERS }, SlotIndex: 4 }, WorkerSick: "EPalBaseCampWorkerSickType::Cold",
        },
      ),
      character({ PlayerUId: UID, InstanceId: "b" }, { CharacterID: "BOSS_SheepBall", IsRarePal: true, FavoriteIndex: 3, SlotId: { ContainerId: { ID: PARTY }, SlotIndex: 0 }, Rank: 3, GotWorkSuitabilityAddRankList: [{ WorkSuitability: "EPalWorkSuitability::Seeding", Rank: 1 }] }),
      character({ PlayerUId: UID, InstanceId: "c" }, { CharacterID: "Deer", Level: 2, SlotId: { ContainerId: { ID: "c-cage" }, SlotIndex: 0 } }),
    ],
    ItemContainerSaveData: [
      { key: { ID: CHEST }, value: { Slots: [slot("Wood", 10), slot("Wood", 5), slot("Stone", 0), slot("None", 3), slot("PalEgg_Dark_01", 1, "d-egg-chest")] } },
      { key: { ID: BAG }, value: { Slots: [slot("Money", 783), slot("Bow", 1, "d-bow")] } },
      { key: { ID: HATCHER }, value: { Slots: [slot("PalEgg_Normal_01", 1, "d-egg-hatcher")] } },
      { key: { ID: MULTI_HATCHER }, value: { Slots: [slot("PalEgg_Normal_05", 1, "d-egg-multi-1"), slot("PalEgg_Ice_01", 1, "d-egg-multi-2")] } },
      { key: { ID: GUILD_CHEST }, value: { Slots: [slot("PalEgg_Fire_01", 1, "d-egg-guild")] } },
    ],
    DynamicItemSaveData: [
      { RawData: { type: "weapon", id: { local_id_in_created_world: "d-bow", static_id: "Bow" }, durability: 87.25, remaining_bullets: 3, passive_skill_list: ["Fire"] } },
      egg("d-egg-chest", "PalEgg_Dark_01", "Anubis"),
      egg("d-egg-hatcher", "PalEgg_Normal_01", "PinkCat"),
      egg("d-egg-multi-1", "PalEgg_Normal_05", "BOSS_Alpaca"),
      egg("d-egg-multi-2", "PalEgg_Ice_01", "Penguin"),
      egg("d-egg-guild", "PalEgg_Fire_01", "BOSS_Kitsunebi"),
    ],
    BaseCampSaveData: [
      {
        key: BASE,
        value: {
          RawData: { id: BASE, name: "新規生成拠点テンプレート名1(仮)", group_id_belong_to: GUILD, area_range: 3500.0, transform: { translation: { x: -123888.0, y: 158000.0, z: 0.0 } } },
          WorkerDirector: { RawData: { container_id: WORKERS } },
        },
      },
    ],
    MapObjectSaveData: [
      {
        MapObjectId: "ItemChest", Model: { RawData: { base_camp_id_belong_to: BASE, instance_id: "m-chest" } },
        ConcreteModel: { RawData: {}, ModuleMap: [{ key: "EPalMapObjectConcreteModelModuleType::ItemContainer", value: { RawData: { target_container_id: CHEST } } }] },
      },
      {
        MapObjectId: "HatchingPalEgg", Model: { RawData: { base_camp_id_belong_to: BASE } },
        ConcreteModel: {
          RawData: { concrete_model_type: "PalMapObjectHatchingEggModel", current_pal_egg_temp_diff: -1 },
          ModuleMap: [
            { key: "EPalMapObjectConcreteModelModuleType::ItemContainer", value: { RawData: { target_container_id: HATCHER } } },
            { key: "EPalMapObjectConcreteModelModuleType::Workee", value: { RawData: { target_work_id: "w-hatch" } } },
          ],
        },
      },
      {
        // A Large-Scale Electric Egg Incubator: no work entry, its eggs are what its container holds.
        MapObjectId: "MultiElectricHatchingPalEgg", Model: { RawData: { base_camp_id_belong_to: BASE } },
        ConcreteModel: {
          RawData: { concrete_model_type: "PalMapObjectMultiHatchingEggModel", unknown_bytes: "AAAAAA==" },
          ModuleMap: [{ key: "EPalMapObjectConcreteModelModuleType::ItemContainer", value: { RawData: { target_container_id: MULTI_HATCHER } } }],
        },
      },
      {
        MapObjectId: "MultiElectricHatchingPalEgg", Model: { RawData: { base_camp_id_belong_to: BASE } },
        ConcreteModel: { RawData: { concrete_model_type: "PalMapObjectMultiHatchingEggModel", unknown_bytes: "AAAAAA==" }, ModuleMap: [] },
      },
      {
        MapObjectId: "ElectricGenerator", Model: { RawData: { base_camp_id_belong_to: BASE, instance_id: "m-gen" } },
        ConcreteModel: {
          RawData: { concrete_model_type: "PalMapObjectGenerateEnergyModel", stored_energy_amount: 247997.203125, consume_energy_speed: 12.5, generate_energy_rate_by_worker: 0 },
          ModuleMap: [{ key: "EPalMapObjectConcreteModelModuleType::Workee", value: { RawData: { target_work_id: "w-gen" } } }],
        },
      },
      {
        MapObjectId: "Expedition", Model: { RawData: { base_camp_id_belong_to: BASE, instance_id: "m-exp" } },
        ConcreteModel: {
          RawData: {
            concrete_model_type: "PalMapObjectCharacterTeamMissionModel", mission_id: "DUNGEON_GRASS", state: 2, start_time: 777502340000,
            assigned_individuals: [{ player_uid: UID, instance_id: "c" }, { player_uid: ZERO, instance_id: ZERO }],
          },
          ModuleMap: [],
        },
      },
      { MapObjectId: "FarmBlockV2_wheet", Model: { RawData: { base_camp_id_belong_to: BASE } }, ConcreteModel: { RawData: { crop_data_id: "Wheat", current_state: 3, crop_progress_rate_value: 0.5 } } },
      { MapObjectId: "TreasureBox", Model: { RawData: { base_camp_id_belong_to: ZERO } } },
    ],
    GroupSaveDataMap: [
      { key: "org", value: { GroupType: "EPalGroupType::Organization", RawData: {} } },
      {
        key: GUILD,
        value: {
          GroupType: "EPalGroupType::Guild",
          RawData: { group_id: GUILD, guild_name: "Unnamed Guild", base_camp_level: 8, base_ids: [BASE], players: [{ player_uid: UID, player_info: { player_name: "Tamer" }, role: 1 }] },
        },
      },
    ],
    GuildExtraSaveDataMap: [{ key: GUILD, value: { Lab: { RawData: { research_info: [{ research_id: "Handcraft1", work_amount: 0.0 }], current_research_id: "Handcraft1" } }, GuildItemStorage: { RawData: { container_id: { ID: GUILD_CHEST } } } } }],
    WorkSaveData: [
      {
        WorkableType: "EPalWorkableType::Transport",
        WorkAssignMap: [{ key: 0, value: { RawData: { assigned_individual_id: { instance_id: "a" } } } }],
        RawData: { id: "w-chest", base_camp_id_belong_to: BASE, owner_map_object_model_id: "m-chest", assign_define_data_id: "Transport_ItemChest", required_work_amount: 0.0, current_work_amount: 0.0 },
      },
      // The incubator's clock: 150 s to hatch, 90 s in. The generator's crank, with pal "a" on it.
      { WorkableType: "EPalWorkableType::Progress", WorkAssignMap: [], RawData: { id: "w-hatch", base_camp_id_belong_to: BASE, assign_define_data_id: "HatchingPalEgg_0", required_work_amount: 0.0, current_work_amount: 150.0, auto_work_self_amount_by_sec: 90.25, current_state: 1 } },
      {
        WorkableType: "EPalWorkableType::Assign",
        WorkAssignMap: [{ key: 0, value: { RawData: { assigned_individual_id: { instance_id: "a" } } } }],
        RawData: { id: "w-gen", base_camp_id_belong_to: BASE, owner_map_object_model_id: "m-gen", assign_define_data_id: "ElectricGenerator_0", required_work_amount: 0.0, current_work_amount: 0.0 },
      },
    ],
  },
};

export const PLAYER_SAVE: Dict = {
  SaveData: {
    PlayerUId: UID,
    OtomoCharacterContainerId: { ID: PARTY },
    PalStorageContainerId: { ID: BOX },
    TechnologyPoint: 20,
    InventoryInfo: { CommonContainerId: { ID: BAG } },
    RecordData: {
      PaldeckUnlockFlag: [{ key: "PinkCat", value: true }, { key: "Deer", value: false }],
      PalCaptureCount: [{ key: "PinkCat", value: 9 }],
      TribeCaptureCount: 1,
      FixedDungeonClearCount: 7, OilrigClearCount: 1, PredatorDefeatCount: 2, CampConqueredCount: 1,
      PalRankupCount: [{ key: "2", value: 16 }, { key: "5", value: 1 }],
      PalButcherCount: [{ key: "PinkCat", value: 7 }],
      FishingCountMap: [{ key: "FishShadow_LazyCatfish_Nushi", value: 1 }],
    },
    OrderedQuestArray_FullRelease: [
      {
        QuestName: "Main_DefeatForestBoss",
        BlockIndex: 2,
        IntegerMap: [{ key: "QuestBlock_DeliveryItem_TotalDeliveredCount", value: 3 }, { key: "CanCompleteFlag_0", value: 0 }],
        StringMap: [],
      },
    ],
  },
};

/** Real saves' repeatables sections: a camp cleared ten minutes before the save, a dungeon with rewards, the newest save, and LocalData. */
// The real sections carry 18-digit tick integers, which JSON.parse would round to doubles; the reader keeps those exact as bigints, so the fixtures do too.
const exact = (_k: string, v: unknown, ctx: { source?: string }): unknown =>
  typeof v === "number" && Number.isInteger(v) && !Number.isSafeInteger(v) && ctx.source ? BigInt(ctx.source) : v;
export const SAVES = JSON.parse(readFileSync(new URL("./fixtures/repeatables.json", import.meta.url), "utf8"), exact as never) as Record<string, Dict>;
export const CLEARED = SAVES["cleared"]!;
export const META: Dict = { Timestamp: 639269610204320000n, SaveData: { WorldName: "Test Islands", InGameDay: 20 } };
export const OPTIONS: Dict = { OptionWorldData: { Settings: { Difficulty: "EPalOptionWorldDifficulty::Easy", AdminPassword: "hunter2", ServerPassword: "x" } } };
