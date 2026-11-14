/** Schema-2 sections over small hand-made save fragments, and over trimmed real ones (fixtures/repeatables.json). Ported from test_world.py. */

import { inflateSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import * as fog from "../src/snapshot/fog.js";
import { itemContainers } from "../src/snapshot/snapshot.js";
import { guidKey, pyRound, type Dict } from "../src/snapshot/util.js";
import * as w2 from "../src/snapshot/world.js";
import { SAVES, ZERO } from "./fixtures.js";

const d = (v: unknown) => v as Dict;
const [CLEARED, REWARDED, NEWEST] = ["cleared", "rewarded", "newest"].map((k) => d(SAVES[k]!["worldSaveData"]));

const container = (cid: string, slots: [string, number, string][], slotNum = 4): Dict => ({
  key: { ID: cid },
  value: { SlotNum: slotNum, Slots: slots.map(([item, n, dyn]) => ({ RawData: { count: n, item: { static_id: item, dynamic_id: { local_id_in_created_world: dyn } } } })) },
});

const WORLD: Dict = {
  ItemContainerSaveData: [container("bag", [["Bow", 1, "d-bow"], ["Wood", 5, ZERO]], 10), container("nest", [["PalEgg_Dark_03", 1, "d-egg"]], 1)],
  DynamicItemSaveData: [
    { RawData: { type: "weapon", id: { local_id_in_created_world: "d-bow", static_id: "Bow" }, durability: 87.25, remaining_bullets: 3, passive_skill_list: ["Fire"] } },
    { RawData: { type: "egg", id: { local_id_in_created_world: "d-egg", static_id: "PalEgg_Dark_03" }, character_id: "BOSS_NegativeKoala" } },
    { RawData: { type: "armor", id: { local_id_in_created_world: "d-orphan", static_id: "ClothArmor" }, durability: 1.0 } },
  ],
  MapObjectSaveData: [{ MapObjectId: "CampFire", Model: { RawData: { instance_id: "m-fire", base_camp_id_belong_to: "base" } } }],
  WorkSaveData: [
    {
      WorkableType: "EPalWorkableType::Progress",
      WorkAssignMap: [{ key: 0, value: { RawData: { assigned_individual_id: { instance_id: "pal-1" } } } }],
      RawData: { id: "w1", base_camp_id_belong_to: "base", owner_map_object_model_id: "m-fire", assign_define_data_id: "CampFire_0", required_work_amount: 1000.0, current_work_amount: 250.0 },
    },
    { WorkableType: "EPalWorkableType::Progress", RawData: { id: "w2", base_camp_id_belong_to: ZERO } },
  ],
  InvaderSaveData: [{ key: "base", value: { bIsInvading: true, CoolTimeElapsed: 100.4, CoolTimeFinish: 400.0 } }],
  EnemyCampSaveData: { EnemyCampStatusMap: [{ key: "None", value: {} }, { key: "Grass", value: { bIsSpawned: true, bIsClear: true, RewardPalId: "Penguin", RewardPalLevel: 8 } }] },
  DungeonSaveData: [
    {
      InstanceId: "d1", DungeonType: "EPalDungeonType::Normal", DungeonSpawnAreaId: "Viking001", MarkerPointId: "m-1", EnemySpawnerDataBossRowName: "27",
      DungeonLevelName: "Viking001_01", BossState: "EPalDungeonInstanceBossState::Spawned", DisappearTimeAt: { Ticks: 50 * 10_000_000 }, RespawnBossTimeAt: { Ticks: 0 },
    },
  ],
  OilrigSaveData: { OilrigMap: [{ key: "EPalOilrigType::TypeA", value: { Clear: true, DestroyedCannon: ["a", "b"] } }] },
  SupplySaveData: { LastSupplyTime: 10 * 10_000_000, SupplyInfos: [{ key: "drop-1", value: { SupplyType: "EPalSupplyType::Meteor", SupplyLandedTime: 20 * 10_000_000, bWipedOut_NPC: true, DropItemGuids: ["x", "y"] } }] },
};

describe("world", () => {
  it("rounds like Python: half to even", () => {
    expect([pyRound(0.125, 2), pyRound(0.375, 2), pyRound(2.5), pyRound(3.5), pyRound(87.25, 1), pyRound(0.6666665, 3)]).toEqual([0.12, 0.38, 2, 4, 87.2, 0.667]);
  });

  it("reads the game day and time from the clock", () => {
    expect(w2.clock(w2.DAY_TICKS * 24 + w2.DAY_TICKS / 2)).toEqual({ day: 25, time: "12:00", hour: 12 });
    expect(w2.clock(null)).toBeNull();
  });

  it("takes gear and eggs from the slots that hold them", () => {
    const inst = w2.instancesByContainer(WORLD);
    expect(inst["bag"]).toEqual([{ kind: "weapon", item: "Bow", durability: 87.2, ammo: 3, passives: ["Fire"] }]);
    expect(w2.eggsEverywhere(inst, { bag: "inventory: common" })).toEqual([{ item: "PalEgg_Dark_03", species: "NegativeKoala", alpha: true, where: "wild" }]);
  });

  it("counts used slots", () => {
    expect(w2.containerCapacity(WORLD)["bag"]).toEqual({ slots: 10, used: 2 });
  });

  it("names a base's work, its structure, progress and workers", () => {
    expect(w2.baseWork(WORLD)).toEqual({ base: [{ id: "w1", type: "Progress", task: "CampFire_0", structure: "CampFire", workers: ["pal-1"], progress: 0.25, workAmount: 250 }] });
  });

  it("reads raids, camps (skipping the map's \"None\" entry), dungeons, rigs and supply", () => {
    expect(w2.enemyCamps(WORLD).map((c) => c["id"])).toEqual(["Grass"]);
    expect(w2.raids(WORLD)).toEqual({ base: { underAttack: true, cooldownLeft: 300, elapsedPlaySeconds: 100.4, finishPlaySeconds: 400, pending: false } });
    expect(w2.enemyCamps(WORLD)[0]).toEqual({
      id: "Grass", spawned: true, cleared: true, allDead: false, rewardReceived: false, rewardPal: "Penguin", rewardLevel: 8, clearedAt: null, elapsedPlaySeconds: null, chests: 0,
    });
    const [dg] = w2.dungeons(WORLD, 20 * 10_000_000);
    expect([dg!["area"], dg!["level"], dg!["bossState"], dg!["closesInGameSeconds"], dg!["bossRespawnInGameSeconds"]]).toEqual(["Viking001", "Viking001_01", "Spawned", 30, null]);
    expect([dg!["marker"], dg!["bossRow"]]).toEqual(["m-1", "27"]);
    expect([w2.oilRigs(WORLD)[0]!["cannonsDestroyed"], w2.oilRigs(WORLD)[0]!["cleared"]]).toEqual([2, true]);
    expect(w2.supplyDrops(WORLD, 30 * 10_000_000)).toEqual({
      lastDropSecondsAgo: 20, lastGuid: null, lastAtPlaySeconds: 10,
      drops: [{ guid: "drop-1", type: "Meteor", atPlaySeconds: null, landed: true, secondsAgo: 10, guardsDefeated: true, wipedPal: false, wipedNpc: true, items: 2, mapLocationId: null }],
    });
  });

  it("gives a cleared camp its clear time and the play since", () => {
    const [forest, seabase] = w2.enemyCamps(CLEARED!);
    expect(forest).toEqual({
      id: "Forest1", spawned: true, cleared: true, allDead: false, rewardReceived: true, rewardPal: "GrassPanda_Electric", rewardLevel: 23,
      clearedAt: "2026-10-07T13:49:37.273000Z", elapsedPlaySeconds: 613.16, chests: 0,
    });
    expect([seabase!["cleared"], seabase!["clearedAt"], seabase!["elapsedPlaySeconds"], seabase!["chests"]]).toEqual([false, null, null, 4]);
  });

  it("keeps dungeons' game-clock instants and rewards", () => {
    const now = d(NEWEST!["GameTimeSaveData"])["GameDateTimeTicks"];
    const [dead, standing, random] = w2.dungeons(NEWEST!, now);
    expect([dead!["type"], dead!["bossState"], dead!["respawnBossAtGameTicks"], dead!["bossRespawnInGameSeconds"], dead!["disappearAtGameTicks"]]).toEqual(["Fixed", "Dead", 40873990000000, 103672, null]);
    expect([standing!["bossState"], standing!["respawnBossAtGameTicks"], standing!["disappearAtGameTicks"]]).toEqual(["Spawned", null, null]);
    expect([random!["type"], random!["disappearAtGameTicks"], random!["closesInGameSeconds"], random!["respawnBossAtGameTicks"]]).toEqual(["Normal", 47160950000000, 732368, null]);
    expect([random!["rewards"], random!["bonusExpPlayers"]]).toEqual([0, 0]);
    const [cleared] = w2.dungeons(REWARDED!, d(REWARDED!["GameTimeSaveData"])["GameDateTimeTicks"]);
    expect([cleared!["bossState"], cleared!["rewards"], cleared!["bonusExpPlayers"]]).toEqual(["Dead", 2, 1]);
  });

  it("names the open dungeon or the next roll at each marker", () => {
    const [random] = w2.dungeons(NEWEST!, null).filter((x) => x["type"] === "Normal");
    const [open, closed] = w2.dungeonMarkers(NEWEST!);
    expect(open).toEqual({ id: random!["marker"], instanceId: random!["id"], nextRespawnGameTicks: null });
    expect(closed).toEqual({ id: "bccd52b9-3e57-7faa-8909-4f5e4bd22983", instanceId: null, nextRespawnGameTicks: 40680950000000 });
    expect(w2.dungeonMarkers({})).toEqual([]);
  });

  it("keeps a raid's cooldown in play seconds and says when it is spent", () => {
    expect(w2.raids(NEWEST!)).toEqual({
      "7cdba55f-216a-a79d-bba8-d0f565e3b5a4": { underAttack: false, cooldownLeft: 5557, elapsedPlaySeconds: 19.37, finishPlaySeconds: 5576.74, pending: false },
      "d6ff5c48-385c-4e54-4d42-1338fbd1ae86": { underAttack: false, cooldownLeft: 0, elapsedPlaySeconds: 0, finishPlaySeconds: 0, pending: true },
    });
  });

  it("names the newest drop and when each fell in play", () => {
    const real = d(NEWEST!["GameTimeSaveData"])["RealDateTimeTicks"];
    const capsule = "8f1cb1b5-e478-2a40-d7b1-11530db5e79d";
    expect(w2.supplyDrops(NEWEST!, real)).toEqual({
      lastDropSecondsAgo: 8292, lastGuid: capsule, lastAtPlaySeconds: 78551,
      drops: [{ guid: capsule, type: "Capsule", atPlaySeconds: 78551, landed: true, secondsAgo: 8277, guardsDefeated: false, wipedPal: false, wipedNpc: false, items: 0, mapLocationId: "a66a21d8-a48e-b4fd-cab3-d7eb505df36c" }],
    });
    expect(w2.supplyDrops({}, real)).toEqual({ lastDropSecondsAgo: null, lastGuid: null, lastAtPlaySeconds: null, drops: [] });
  });

  it("counts a rig's wiped spawners and keeps the reset timer", () => {
    const [a, b, c] = w2.oilRigs(NEWEST!);
    expect([a!["type"], a!["cleared"], a!["spawnersWiped"], a!["goalBox"], a!["resetTimerSeconds"]]).toEqual(["TypeA", false, 0, 2, 0]);
    expect([a, b, c].map((r) => r!["goalBox"])).toEqual([2, 1, 4]);
    const rig = structuredClone(d(d(NEWEST!["OilrigSaveData"])["OilrigMap"] as unknown as Dict[])[0]!) as Dict;
    Object.assign(d(rig["value"]), { Clear: true, WipedOutEnemySpawner: ["s-1", "s-2"], ResetTimer: 12.5 });
    const [r] = w2.oilRigs({ OilrigSaveData: { OilrigMap: [rig] } });
    expect([r!["cleared"], r!["spawnersWiped"], r!["resetTimerSeconds"]]).toEqual([true, 2, 12.5]);
  });

  it("reads appearance and local extras", () => {
    const a = w2.appearance({ BodyMeshName: "TypeA", VoiceID: 2, HairColor: { r: -0.6806, g: 1.1206, b: 1.5296 } })!;
    expect([a["body"], a["voice"]]).toEqual(["TypeA", 2]);
    expect(d(a["colorAdjust"])["hair"]).toEqual({ hue: -0.681, saturation: 1.121, brightness: 1.53 });
    expect(d(a["colorAdjust"])["eye"]).toBeNull();
    const local: Dict = {
      SaveData: { Local_PalEncountFlag: [{ key: "EPalTribeID::PinkCat", value: true }], Local_ActivateOtomoCount: [{ key: "EPalTribeID::SheepBall", value: 19 }], TrackingQuestId: "Main_CatchPal" },
    };
    expect(w2.localExtras(local, 1.5)).toEqual({ speciesEncountered: ["PinkCat"], summons: { SheepBall: 19 }, trackingQuest: "Main_CatchPal", hiddenLocationsFound: 0, mapExploredPct: 1.5, bossLocationsRevealed: [] });
    expect(w2.localExtras(null, null)).toBeNull();
  });

  it("lists the revealed boss locations as the hidden flags set false", () => {
    const local = w2.localExtras(SAVES["local"]!, null)!;
    const flags = Object.fromEntries((d(SAVES["local"]!["SaveData"])["Local_HiddenLocationFlagMap"] as Dict[]).map((e) => [String(e["key"]), e["value"]]));
    expect([Object.keys(flags).length, Object.values(flags).filter(Boolean).length]).toEqual([125, 108]);
    expect([local["hiddenLocationsFound"], (local["bossLocationsRevealed"] as string[]).length]).toEqual([108, 17]);
    expect(local["bossLocationsRevealed"]).toEqual(Object.keys(flags).filter((k) => !flags[k]).sort());
    expect(local["bossLocationsRevealed"]).toEqual(expect.arrayContaining(["81_1_grass_FBOSS_2", "BOSS_Male_Soldier04"]));
  });

  it("lists bosses down as the spawners with respawn disabled", () => {
    expect(w2.bossesDown(NEWEST!)).toEqual(["BOSS_Believer_CrossBow"]);
    expect(w2.bossesDown({})).toEqual([]);
  });

  it("reads every guid spelling as 32 hex", () => {
    expect(guidKey("7aec754e-e957-f58a-3911-61e97cbe7fd8")).toBe("7aec754ee957f58a391161e97cbe7fd8");
    expect(guidKey("7AEC754E-E957F58A-391161E9-7CBE7FD8")).toBe("7aec754ee957f58a391161e97cbe7fd8");
    expect([guidKey(ZERO), guidKey("00000000-00000000-00000000-00000000"), guidKey(null)]).toEqual([null, null, null]);
  });

  it("keeps each spawner's slot by spawner id", () => {
    expect(w2.spawners(NEWEST!)).toEqual({
      "bc9cf5e32e2720d4af2088f4a87acdbb": 0,
      "794090d6a3c7f1f00dc7fcc7ba1d2cd8": 0,
      "ef648a4a7c86d47b2951b07f264dca5b": -1,
      be3a598a2a074ec5e75a10c0eeb86368: -1,
      "f9b18cb65bb837e4d38ef8f3d3c5a413": -1,
      c741a417b13aea49b91345f40b20e091: -1,
      d9bc451be36fe625500e9eb7c9933f30: -1,
      "a54eb26b016089f5ee852b9a40ea92f4": -1,
      e2e2d3940446b37d65079b283c113b3f: -1,
      "658965b20378bdde4828074c04037e2f": 31359870000000,
      f8052cc68d44c45b021256a7aeb9031b: 28119840000000,
      "1a4c5d6e46f4100b03c12686c909cf59": null,
      a6c9d1c18ff7df2c6ece43a7d9b6f602: null,
      "9af36d5fcd37394b0968c43883bd5235": [-1, -1, -1],
    });
  });

  it("lists a multi-slot spawner's slots in slot order", () => {
    const [stage] = structuredClone(NEWEST!["MapObjectSpawnerInStageSaveData"]) as Dict[];
    const [tree] = (d(stage!["value"])["SpawnerDataMapByLevelObjectInstanceId"] as Dict[]).filter((e) => (d(e["value"])["ItemMap"] as unknown[]).length === 3);
    const slots = d(tree!["value"])["ItemMap"] as Dict[];
    slots[1]!["value"] = { NextLotteryGameTime: 40000000000000, MapObjectInstanceId: ZERO }; // slot 1 picked
    d(tree!["value"])["ItemMap"] = [slots[2]!, slots[0]!, slots[1]!];
    expect(w2.spawners({ MapObjectSpawnerInStageSaveData: [{ value: { SpawnerDataMapByLevelObjectInstanceId: [tree!] } }] })).toEqual({ [guidKey(tree!["key"])!]: [-1, 40000000000000, -1] });
    expect(w2.spawners({})).toEqual({});
  });

  it("lists the objects spawners own with hp and position", () => {
    const [rock, chest, junk] = w2.nodes(NEWEST!); // the base's own structure has no spawner
    expect(rock).toEqual({ spawner: "f9b18cb65bb837e4d38ef8f3d3c5a413", mapObjectId: "DamagableRock0002", hp: { current: 4496, max: 4500 }, x: -279969.1, y: 152230.8, z: 1952.9 });
    expect([chest!["mapObjectId"], chest!["hp"], junk!["mapObjectId"], junk!["hp"]]).toEqual(["TreasureBox_RequiredLongHold", null, "TreasureBox_FishingJunk_RequiredLongHold", null]);
    const spawners = new Set(Object.keys(w2.spawners(NEWEST!)));
    expect([rock!["spawner"], chest!["spawner"], junk!["spawner"]].every((s) => spawners.has(String(s)))).toBe(true);
  });

  it("reads a station's recipe, queue, output and workers", () => {
    const world: Dict = {
      ItemContainerSaveData: [container("wb", [["Arrow", 20, ZERO], ["Stone", 344, ZERO]]), container("pit", [["Stone", 9, ZERO]])],
      WorkSaveData: [{ RawData: { id: "w-wb", current_work_amount: 400.0 }, WorkAssignMap: [{ value: { RawData: { assigned_individual_id: { instance_id: "pal-1" } } } }] }],
      MapObjectSaveData: [
        {
          MapObjectId: "Workbench", Model: { RawData: { base_camp_id_belong_to: "base", instance_id: "m-wb" } },
          ConcreteModel: {
            RawData: { concrete_model_type: "PalMapObjectConvertItemModel", current_recipe_id: "Arrow", requested_product_num: 172, remain_product_num: 200 },
            ModuleMap: [
              { key: "EPalMapObjectConcreteModelModuleType::ItemContainer", value: { RawData: { target_container_id: "wb" } } },
              { key: "EPalMapObjectConcreteModelModuleType::Workee", value: { RawData: { target_work_id: "w-wb" } } },
            ],
          },
        },
        { MapObjectId: "MedicineFacility_01", Model: { RawData: { base_camp_id_belong_to: "base" } }, ConcreteModel: { RawData: { concrete_model_type: "PalMapObjectConvertItemModel", current_recipe_id: "None", requested_product_num: 0 } } },
        {
          MapObjectId: "StonePit", Model: { RawData: { base_camp_id_belong_to: "base" } },
          ConcreteModel: { RawData: { concrete_model_type: "PalMapObjectProductItemModel", product_item_id: "Stone" }, ModuleMap: [{ key: "EPalMapObjectConcreteModelModuleType::ItemContainer", value: { RawData: { target_container_id: "pit" } } }] },
        },
        { MapObjectId: "ItemChest", Model: { RawData: { base_camp_id_belong_to: "base" } }, ConcreteModel: { RawData: { concrete_model_type: "PalMapObjectItemChestModel" } } },
      ],
    };
    const [wb, med, pit] = w2.production(world, itemContainers(world))["base"]!;
    expect([wb!["structure"], wb!["kind"], wb!["recipe"], wb!["queued"], wb!["outputReady"], wb!["workers"], wb!["active"]]).toEqual(["Workbench", "craft", "Arrow", 172, 20, ["pal-1"], true]);
    expect([wb!["workDone"], pit!["workDone"], wb!["id"]]).toEqual([400, null, "m-wb"]);
    expect([pit!["kind"], pit!["recipe"], pit!["queued"], pit!["outputReady"], pit!["active"]]).toEqual(["produce", "Stone", null, 9, false]);
    expect([med!["recipe"], med!["queued"], med!["active"]]).toEqual([null, 0, false]);
  });

  it("reads the Ancient Hatchery as a breeding farm fed from its cake box, and its eggs as an incubator's", () => {
    const world: Dict = {
      ItemContainerSaveData: [container("eggs", [["PalEgg_Leaf_02", 1, ZERO]], 10), container("cake", [["Cake", 118, ZERO]], 5)],
      WorkSaveData: [{ RawData: { id: "w-breed" }, WorkAssignMap: [0, 1].map((i) => ({ value: { RawData: { assigned_individual_id: { instance_id: `bee-${i}` } } } })) }],
      MapObjectSaveData: [
        {
          MapObjectId: "MultiElectricHatchingPalEggWithBreed", Model: { RawData: { base_camp_id_belong_to: "base", instance_id: "m-hatchery" } },
          ConcreteModel: {
            RawData: { concrete_model_type: "PalMapObjectMultiHatchingEggWithBreedModel", cake_container_id: "cake" },
            ModuleMap: [
              { key: "EPalMapObjectConcreteModelModuleType::ItemContainer", value: { RawData: { target_container_id: "eggs" } } },
              { key: "EPalMapObjectConcreteModelModuleType::Workee", value: { RawData: { target_work_id: "w-breed" } } },
            ],
          },
        },
      ],
    };
    const [farm] = w2.production(world, itemContainers(world))["base"]!;
    expect(farm).toMatchObject({ structure: "MultiElectricHatchingPalEggWithBreed", kind: "breed", recipe: null, box: [{ item: "Cake", count: 118 }], eggsLaid: 0, workers: ["bee-0", "bee-1"], active: true });
    const [inc] = w2.incubators(world, w2.instancesByContainer(world))["base"]!;
    expect([inc!["structure"], inc!["multi"]]).toEqual(["MultiElectricHatchingPalEggWithBreed", true]);
  });

  it("skips a model the parser left undecoded", () => {
    // A structure the parser can't decode keeps its RawData as a base64 string; an empty one is [].
    const world: Dict = {
      MapObjectSaveData: [
        { MapObjectId: "BaseCampWorkHard02", Model: { RawData: { base_camp_id_belong_to: "base" } }, ConcreteModel: { RawData: "mOj+e6kSRW6OleZ9NIgCowa4b1QwN0nbn2fpLYBhCKAAAAAA" } },
        { MapObjectId: "Wooden_wall", Model: { RawData: { base_camp_id_belong_to: "base" } }, ConcreteModel: { RawData: [] } },
      ],
    };
    expect(w2.production(world, {})).toEqual({});
    expect(w2.incubators(world, {})).toEqual({});
  });

  it("lists only cleared lock gimmicks", () => {
    const world: Dict = {
      LockGimmickSaveData: [
        { key: "effb3b06-58e5-4679-24e6-834185e19b11", value: { bCleared: true } },
        { key: "7aec754e-e957-f58a-3911-61e97cbe7fd8", value: { bCleared: true } },
        { key: "00000000-0000-0000-0000-000000000009", value: { bCleared: false } },
      ],
    };
    expect(w2.locksCleared(world)).toEqual(["7aec754e-e957-f58a-3911-61e97cbe7fd8", "effb3b06-58e5-4679-24e6-834185e19b11"]);
    expect(w2.locksCleared({})).toEqual([]);
  });
});

/** An RGBA mask fogged (alpha 255) everywhere but the given (x, y) pixels, which get alphas below 255. */
function fogMask(side: number, ...explored: [number, number][]): Uint8Array {
  const mask = new Uint8Array(side * side * 4);
  for (let i = 3; i < mask.length; i += 4) mask[i] = 255;
  explored.forEach(([x, y], n) => {
    mask[(y * side + x) * 4 + 3] = [0, 128, 254][n % 3]!;
  });
  return mask;
}

/** A fog PNG's grey levels, row by row (one IDAT, filter byte 0 on every row). */
function pngRows(png: Buffer): number[][] {
  const size = png.readUInt32BE(16);
  const at = png.indexOf("IDAT");
  const n = png.readUInt32BE(at - 4);
  const data = inflateSync(png.subarray(at + 4, at + 4 + n));
  const rows: number[][] = [];
  for (let r = 0; r < size; r++) {
    expect(data[r * (size + 1)]).toBe(0);
    rows.push([...data.subarray(r * (size + 1) + 1, (r + 1) * (size + 1))]);
  }
  return rows;
}

describe("fog", () => {
  it("is white where explored", () => {
    const grid = fog.exploredGrid(fogMask(4, [0, 0], [1, 0], [0, 1], [1, 1])); // the top-left 2 x 2 block
    expect(fog.exploredPct(grid)).toBe(25);
    const out = fog.png(grid, 2);
    expect(out.subarray(0, 4)).toEqual(Buffer.from([0x89, 0x50, 0x4e, 0x47]));
    expect(out.includes("IHDR") && out.includes("IEND")).toBe(true);
    expect(pngRows(out)).toEqual([[255, 0], [0, 0]]);
  });

  it("keeps x across and y down", () => {
    const grid = fog.exploredGrid(fogMask(4, [3, 0])); // top-right pixel only
    expect(fog.exploredPct(grid)).toBe(6.25);
    expect(pngRows(fog.png(grid, 2))).toEqual([[0, 64], [0, 0]]);
    expect(pngRows(fog.png(grid, 4))[0]).toEqual([0, 0, 0, 255]);
  });

  it("averages uneven blocks pixel by pixel", () => {
    const side = 12;
    const mask = fogMask(side);
    for (let i = 0; i < side * side; i++) if ((i * 7) % 5 < 2 || i % side === 3) mask[i * 4 + 3] = i % 250;
    const grid = fog.exploredGrid(mask);
    let explored = 0;
    for (let i = 0; i < side * side; i++) if (mask[i * 4 + 3]! < 255) explored++;
    expect(grid.cells.reduce((n, c) => n + c, 0)).toBe(explored);
    expect(pngRows(fog.png(grid, 5))).toHaveLength(5); // 12 % 5 != 0
  });

  it("reads alpha, not colour", () => {
    // A real mask carries colour where it's fogged and none where it's explored.
    const mask = Uint8Array.from([22, 22, 22, 255, 0, 0, 0, 0, 0, 0, 0, 255, 2, 2, 2, 40]);
    const grid = fog.exploredGrid(mask);
    expect([grid.side, [...grid.cells]]).toEqual([2, [0, 1, 0, 1]]);
  });

  it("reads a fully explored mask as all white", () => {
    const grid = fog.exploredGrid(new Uint8Array(32 * 32 * 4));
    expect(pngRows(fog.png(grid, 2))).toEqual([[255, 255], [255, 255]]);
  });
});
