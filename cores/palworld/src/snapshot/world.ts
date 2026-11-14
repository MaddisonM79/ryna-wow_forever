/**
 * Snapshot schema 2: everything in the save beyond players, pals and bases.
 *
 * Eggs and gear (DynamicItemSaveData), base work and incubators, and what
 * comes back in the world: raids, enemy camps, dungeons and their entrances,
 * oil rigs, supply drops, bosses down, map-object spawners and the nodes they
 * own. Also the in-game clock, and LocalData's encounter/summon/exploration
 * records. Pure functions, unwrapped dicts in. A port of world.py.
 */

import type { Plain } from "../gvas/unwrap.js";
import {
  ZERO_GUID,
  byCodePoint,
  cid,
  concreteRaw,
  dict,
  enumName,
  flags,
  guidKey,
  isDict,
  isInt,
  isNum,
  list,
  mapd,
  num,
  position,
  pyRound,
  str,
  ticksToIso,
  type Dict,
} from "./util.js";

// One in-game day in Unreal ticks.
export const DAY_TICKS = 24 * 60 * 60 * 10_000_000;

/** Python's `d.get(k)`: null when missing (undefined would vanish from JSON). */
const g = (d: Dict, k: string): Plain => d[k] ?? null;
/** Python's `d.get(k, default)`: the default only when the key is missing. */
const getOr = <T>(d: Dict, k: string, dflt: T): Plain | T => (k in d ? d[k]! : dflt);
const asInt = (v: unknown): number | null => (isInt(v) ? Number(v) : null);

export function clock(gameTicks: unknown): { day: number; time: string; hour: number } | null {
  const t = asInt(gameTicks);
  if (t === null || t <= 0) return null;
  const day = Math.floor(t / DAY_TICKS);
  const rest = t - day * DAY_TICKS;
  const minutes = Math.floor((rest * 24 * 60) / DAY_TICKS);
  const hour = Math.floor(minutes / 60);
  return { day: day + 1, time: `${String(hour).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`, hour };
}

/** A DateTime struct's ticks ({"Ticks": n}); null when unset (0). */
function instant(v: unknown): number | null {
  const t = isDict(v) ? asInt(v["Ticks"]) : null;
  return t !== null && t > 0 ? t : null;
}

/** Game-clock seconds from now until `at` (an `instant`; negative: already past). */
function gameSecondsUntil(at: number | null, nowTicks: unknown): number | null {
  const now = asInt(nowTicks);
  if (at === null || now === null) return null;
  return Math.floor((at - now) / 10_000_000);
}

// ---------------------------------------------------------------- dynamic items

export type Instance = { kind: Plain; item: Plain; durability?: number | null; ammo?: Plain; passives?: Plain[]; species?: Plain };

/** Per-instance item data (durability, ammo, egg contents), by local id. */
function dynamicItems(world: Dict): Record<string, Instance> {
  const out: Record<string, Instance> = {};
  for (const d of list(world["DynamicItemSaveData"])) {
    const raw = dict(dict(d)["RawData"]);
    const id = dict(raw["id"]);
    const local = str(id["local_id_in_created_world"]) ?? "";
    if (!local || local === ZERO_GUID) continue;
    const kind = g(raw, "type");
    const entry: Instance = { kind, item: g(id, "static_id") };
    if (kind === "weapon" || kind === "armor") entry.durability = num(raw["durability"], 1);
    if (kind === "weapon") {
      entry.ammo = g(raw, "remaining_bullets");
      entry.passives = [...list(raw["passive_skill_list"])];
    }
    if (kind === "egg") entry.species = g(raw, "character_id");
    out[local] = entry;
  }
  return out;
}

/** (static item id, dynamic local id) for each slot that has per-instance data. */
function slotDynamicIds(container: Dict): [Plain, string][] {
  const out: [Plain, string][] = [];
  for (const slot of list(dict(container["value"])["Slots"])) {
    const raw = dict(dict(slot)["RawData"]);
    const item = dict(raw["item"]);
    const local = str(dict(item["dynamic_id"])["local_id_in_created_world"]) ?? "";
    const count = raw["count"];
    if (local && local !== ZERO_GUID && isNum(count) && count > 0) out.push([g(item, "static_id"), local]);
  }
  return out;
}

/** Slots and used slots per item container: what a "storage full" check reads. */
export function containerCapacity(world: Dict): Record<string, { slots: Plain; used: number }> {
  const out: Record<string, { slots: Plain; used: number }> = {};
  for (const c of list(world["ItemContainerSaveData"])) {
    const cd = dict(c);
    const key = cid(cd["key"]);
    const v = dict(cd["value"]);
    if (!key) continue;
    let used = 0;
    for (const s of list(v["Slots"])) {
      const count = dict(dict(s)["RawData"])["count"];
      if (isNum(count) && count > 0) used++;
    }
    out[key] = { slots: v["SlotNum"] || 0, used };
  }
  return out;
}

/** Gear and eggs, per container id. */
export function instancesByContainer(world: Dict): Record<string, Instance[]> {
  const dyn = dynamicItems(world);
  const out: Record<string, Instance[]> = {};
  for (const c of list(world["ItemContainerSaveData"])) {
    const cd = dict(c);
    const key = cid(cd["key"]);
    if (!key) continue;
    for (const [stat, local] of slotDynamicIds(cd)) {
      const d = dyn[local];
      if (d) (out[key] ??= []).push({ ...d, item: stat ?? d.item });
    }
  }
  return out;
}

// ---------------------------------------------------------------- bases

const workerIds = (w: Dict): Plain[] =>
  list(w["WorkAssignMap"])
    .map((a) => g(dict(dict(dict(dict(a)["value"])["RawData"])["assigned_individual_id"]), "instance_id"))
    .filter((x) => !!x);

/** EPalWorkSuitability by value, as a work entry's bytes give it (the game's own order, which names.work keeps). */
const WORK_SUITABILITY = ["None", "EmitFlame", "Watering", "Seeding", "GenerateElectricity", "Handcraft", "Collection", "Deforest", "Mining", "OilExtraction", "ProductMedicine", "Cool", "Transport", "MonsterFarm", "Anyone"];

/**
 * Each base's work items: the task, its structure, progress and who's on it. A station that takes
 * more than one kind of work (the Ancient Furnace: Kindling and Cooling) lists them as `jobs`; the
 * game's assign table has no row for it (checked on the 2026-11-02 save).
 */
export function baseWork(world: Dict): Record<string, Dict[]> {
  const modelToObject: Record<string, Plain> = {};
  for (const mo of list(world["MapObjectSaveData"])) {
    const mod = dict(mo);
    const iid = str(dict(dict(mod["Model"])["RawData"])["instance_id"]);
    if (iid) modelToObject[iid] = g(mod, "MapObjectId");
  }
  const out: Record<string, Dict[]> = {};
  for (const w of list(world["WorkSaveData"])) {
    const wd = dict(w);
    const raw = dict(wd["RawData"]);
    const base = str(raw["base_camp_id_belong_to"]);
    if (!base || base === ZERO_GUID) continue;
    const required = (raw["required_work_amount"] as number | null) || 0;
    const current = (raw["current_work_amount"] as number | null) || 0;
    const owner = str(raw["owner_map_object_model_id"]);
    (out[base] ??= []).push({
      id: g(raw, "id"),
      type: enumName(g(wd, "WorkableType")),
      task: g(raw, "assign_define_data_id"),
      structure: (owner && modelToObject[owner]) ?? null,
      workers: workerIds(wd),
      progress: required ? pyRound(current / required, 3) : null,
      workAmount: num(current, 1),
      ...(Array.isArray(raw["progress_entries"]) ? { jobs: list(raw["progress_entries"]).map((e) => WORK_SUITABILITY[Number(dict(e)["work_suitability"])]).filter((j): j is string => !!j && j !== "None") } : {}),
    });
  }
  return out;
}

// Station model type -> production kind. Anything else isn't a producer.
const PRODUCER_KINDS: Record<string, string> = {
  PalMapObjectConvertItemModel: "craft",
  PalMapObjectProductItemModel: "produce",
  PalMapObjectBreedFarmModel: "breed",
  PalMapObjectMultiHatchingEggWithBreedModel: "breed",
  PalMapObjectMonsterFarmModel: "ranch",
};

export type ItemCount = { item: string; count: number };

/**
 * What each base's stations are making, per base.
 *
 * Crafting stations save their recipe and `requested_product_num`, which is
 * the number of crafts still queued (it matches the inputs sitting in the
 * station's box). `remain_product_num` isn't one consistent thing across
 * stations, so it's left out; finished output is counted from the box
 * instead. `workDone` is the work put into the current craft; the save has
 * no total, so the API divides it by the recipe's work amount.
 */
export function production(world: Dict, items: Record<string, ItemCount[]>): Record<string, Dict[]> {
  const workersByWork: Record<string, Plain[]> = {};
  const doneByWork: Record<string, number> = {};
  for (const w of list(world["WorkSaveData"])) {
    const wd = dict(w);
    const raw = dict(wd["RawData"]);
    const wid = str(raw["id"]);
    const done = raw["current_work_amount"];
    if (wid && isNum(done)) doneByWork[wid] = done;
    if (wid) workersByWork[wid] = workerIds(wd);
  }
  const out: Record<string, Dict[]> = {};
  for (const mo of list(world["MapObjectSaveData"])) {
    const mod = dict(mo);
    const concrete = dict(mod["ConcreteModel"]);
    const raw = concreteRaw(concrete);
    const kind = PRODUCER_KINDS[str(raw["concrete_model_type"]) ?? ""];
    if (!kind) continue;
    const model = dict(dict(mod["Model"])["RawData"]);
    const base = str(model["base_camp_id_belong_to"]);
    if (!base || base === ZERO_GUID) continue;
    let container: string | null = null;
    let workId: string | null = null;
    for (const module of list(concrete["ModuleMap"])) {
      const md = dict(module);
      const m = enumName(md["key"]);
      const mraw = dict(dict(md["value"])["RawData"]);
      if (m === "ItemContainer") container = cid(mraw["target_container_id"]);
      else if (m === "Workee") workId = str(mraw["target_work_id"]);
    }
    // The Ancient Hatchery breeds and hatches in one: its own box holds the eggs (`incubators` lists
    // them), so its station box is the cake box its bytes name, and nothing lies around to collect.
    const hatchery = raw["concrete_model_type"] === "PalMapObjectMultiHatchingEggWithBreedModel";
    const box = items[(hatchery ? cid(raw["cake_container_id"]) : container) ?? ""] ?? [];
    let recipe: Plain = kind === "craft" ? g(raw, "current_recipe_id") : kind === "produce" ? g(raw, "product_item_id") : null;
    if (kind === "breed") {
      const targets = list(raw["target_breed_item_ids"]);
      recipe = targets.length ? targets[0]! : null;
    }
    if (recipe === null || recipe === "None" || recipe === "") recipe = null;
    const queued = kind === "craft" ? raw["requested_product_num"] : null;
    const workers = workersByWork[workId ?? ""] ?? [];
    const recipeStr = typeof recipe === "string" ? recipe.toLowerCase() : null;
    (out[base] ??= []).push({
      // The station's own id: keeps one station's alerts apart from its twin's.
      id: g(model, "instance_id"),
      structure: g(mod, "MapObjectId"),
      kind,
      recipe,
      queued: isInt(queued) ? Number(queued) : null,
      // Finished product waiting in the station (the box also holds inputs).
      outputReady: recipeStr !== null ? box.filter((i) => i.item.toLowerCase() === recipeStr).reduce((n, i) => n + i.count, 0) : null,
      box,
      eggsLaid: kind === "breed" ? list(raw["spawned_egg_instance_ids"]).length : null,
      workers,
      // Work put into the current craft; the API divides by the recipe's work amount.
      workDone: num(doneByWork[workId ?? ""], 1),
      active: !!(recipe && (queued || workers.length)) || !!(kind === "ranch" && workers.length) || (hatchery && workers.length >= 2 && box.some((i) => i.count > 0)),
    });
  }
  for (const lst of Object.values(out)) {
    lst.sort((a, b) => Number(!a["active"]) - Number(!b["active"]) || byCodePoint(str(a["structure"]) ?? "", str(b["structure"]) ?? ""));
  }
  return out;
}

/** A map object's module targets: its item container and its work entry (the Workee module). */
function modules(concrete: Dict): { container: string | null; workId: string | null } {
  let container: string | null = null;
  let workId: string | null = null;
  for (const module of list(concrete["ModuleMap"])) {
    const md = dict(module);
    const raw = dict(dict(md["value"])["RawData"]);
    if (enumName(md["key"]) === "ItemContainer") container = cid(raw["target_container_id"]);
    if (enumName(md["key"]) === "Workee") workId = str(raw["target_work_id"]);
  }
  return { container, workId };
}

/** WorkSaveData by work id, for the stations that point at their work entry. */
function workById(world: Dict): Record<string, Dict> {
  const out: Record<string, Dict> = {};
  for (const w of list(world["WorkSaveData"])) {
    const wd = dict(w);
    const id = str(dict(wd["RawData"])["id"]);
    if (id) out[id] = wd;
  }
  return out;
}

/**
 * Eggs sitting in incubators, per base. The incubator's work entry is its
 * clock: `current_work_amount` is the egg's hatch time in seconds of play
 * (150, 900 or 1800 on this world) and `auto_work_self_amount_by_sec`, despite
 * the name, how far it has got; `current_state` 3 is a hatched egg waiting to
 * be taken (checked over 130 archived saves on 2026-10-13).
 *
 * A Large-Scale Electric Egg Incubator (`PalMapObjectMultiHatchingEggModel`, up to 10 eggs) has
 * no work entry and no clock in the save: its eggs are the egg items in its container, one entry
 * each (`multi: true`, the hatch fields null), and an empty one is one entry with no egg. Its own
 * bytes hold, per egg, the slot and the pal that will hatch, which the egg item already names
 * (checked on the 2026-10-28 save: slot 5, PalEgg_Normal_05, BOSS_Alpaca, both sides). The Ancient
 * Hatchery (`PalMapObjectMultiHatchingEggWithBreedModel`) holds its own laid eggs the same way.
 */
export function incubators(world: Dict, instances: Record<string, Instance[]>): Record<string, Dict[]> {
  const out: Record<string, Dict[]> = {};
  const works = workById(world);
  for (const mo of list(world["MapObjectSaveData"])) {
    const mod = dict(mo);
    const concrete = dict(mod["ConcreteModel"]);
    const craw = concreteRaw(concrete);
    const type = craw["concrete_model_type"];
    const multi = type === "PalMapObjectMultiHatchingEggModel" || type === "PalMapObjectMultiHatchingEggWithBreedModel";
    if (type !== "PalMapObjectHatchingEggModel" && !multi) continue;
    const base = str(dict(dict(mod["Model"])["RawData"])["base_camp_id_belong_to"]);
    const { container, workId } = modules(concrete);
    const eggs = (instances[container ?? ""] ?? []).filter((e) => e.kind === "egg");
    if (multi) {
      const each = eggs.length ? eggs : [null];
      for (const egg of each) {
        (out[base ?? ""] ??= []).push({
          structure: g(mod, "MapObjectId"),
          egg: egg as unknown as Dict | null,
          temperatureDiff: null,
          hatchSeconds: null,
          hatchProgress: null,
          hatchState: null,
          multi: true,
        });
      }
      continue;
    }
    const work = dict(works[workId ?? ""]?.["RawData"]);
    (out[base ?? ""] ??= []).push({
      structure: g(mod, "MapObjectId"),
      egg: eggs.length ? (eggs[0] as unknown as Dict) : null,
      temperatureDiff: g(craw, "current_pal_egg_temp_diff"),
      hatchSeconds: num(work["current_work_amount"], 1),
      hatchProgress: num(work["auto_work_self_amount_by_sec"], 1),
      hatchState: g(work, "current_state"),
    });
  }
  return out;
}

/**
 * Each base's generators: the charge they hold, the draw on them and what
 * their worker puts in, as of the save (`PalMapObjectGenerateEnergyModel`),
 * with the pal assigned to crank it.
 */
export function power(world: Dict): Record<string, Dict[]> {
  const out: Record<string, Dict[]> = {};
  const works = workById(world);
  for (const mo of list(world["MapObjectSaveData"])) {
    const mod = dict(mo);
    const concrete = dict(mod["ConcreteModel"]);
    const craw = concreteRaw(concrete);
    if (craw["concrete_model_type"] !== "PalMapObjectGenerateEnergyModel") continue;
    const base = str(dict(dict(mod["Model"])["RawData"])["base_camp_id_belong_to"]);
    const { workId } = modules(concrete);
    const work = works[workId ?? ""];
    (out[base ?? ""] ??= []).push({
      structure: g(mod, "MapObjectId"),
      stored: num(craw["stored_energy_amount"], 1),
      consumePerSecond: num(craw["consume_energy_speed"], 3),
      generatePerSecond: num(craw["generate_energy_rate_by_worker"], 3),
      workers: work ? workerIds(work) : [],
    });
  }
  return out;
}

/**
 * Each base's Expedition station (`PalMapObjectCharacterTeamMissionModel`):
 * the mission chosen, who went and when. `state` 2 is a team out on it (its
 * `assigned_individuals` are the pals, up to 100); 1 is a mission picked with
 * nobody sent, or the station between trips; 3 a trip that has come home.
 * `startedAtPlaySeconds` is on the play clock (RealDateTimeTicks), so the
 * mission's required seconds count only while the game runs.
 */
export function expeditions(world: Dict): Record<string, Dict[]> {
  const out: Record<string, Dict[]> = {};
  for (const mo of list(world["MapObjectSaveData"])) {
    const mod = dict(mo);
    const craw = concreteRaw(dict(mod["ConcreteModel"]));
    if (craw["concrete_model_type"] !== "PalMapObjectCharacterTeamMissionModel") continue;
    const base = str(dict(dict(mod["Model"])["RawData"])["base_camp_id_belong_to"]);
    const mission = str(craw["mission_id"]);
    const start = asInt(craw["start_time"]);
    (out[base ?? ""] ??= []).push({
      structure: g(mod, "MapObjectId"),
      mission: mission && mission !== "None" ? mission : null,
      state: g(craw, "state"),
      startedAtPlaySeconds: start === null ? null : Math.floor(start / 10_000_000),
      pals: list(craw["assigned_individuals"])
        .map((a) => str(dict(a)["instance_id"]))
        .filter((id): id is string => !!id && id !== ZERO_GUID),
    });
  }
  return out;
}

/**
 * Each base's raid cooldown. CoolTimeElapsed and CoolTimeFinish are seconds
 * of play (they run 1:1 with RealDateTimeTicks), not game-world time. A
 * finish of 0 is a spent cooldown that hasn't rolled a raid or restarted yet.
 */
export function raids(world: Dict): Record<string, Dict> {
  const out: Record<string, Dict> = {};
  for (const e of list(world["InvaderSaveData"])) {
    const ed = dict(e);
    const v = dict(ed["value"]);
    const elapsed = v["CoolTimeElapsed"];
    const finish = v["CoolTimeFinish"];
    out[String(ed["key"])] = {
      underAttack: !!v["bIsInvading"],
      // Seconds of play left before a raid can roll again.
      cooldownLeft: isNum(finish) && isNum(elapsed) ? pyRound(finish - elapsed) : null,
      elapsedPlaySeconds: num(elapsed),
      finishPlaySeconds: num(finish),
      pending: finish === 0,
    };
  }
  return out;
}

// ---------------------------------------------------------------- the world

/**
 * Lock gimmicks cleared (a barrier's hacking minigame, its targets, or its pal
 * fight), by id. The save keeps only the cleared ones; which barrier each
 * opens, and the barriers' positions, come from the game's map.
 */
export function locksCleared(world: Dict): string[] {
  const out: string[] = [];
  for (const e of list(world["LockGimmickSaveData"])) {
    if (isDict(e) && dict(e["value"])["bCleared"] && e["key"]) out.push(String(e["key"]));
  }
  return out.sort(byCodePoint);
}

/**
 * Enemy camps by spawner name. ClearDate is UTC ticks, unlike the save's
 * Timestamp (local), so it's read without `local`. ElapsedTime is seconds of
 * play since that clear (-1 while uncleared).
 */
export function enemyCamps(world: Dict): Dict[] {
  const out: Dict[] = [];
  for (const e of list(dict(world["EnemyCampSaveData"])["EnemyCampStatusMap"])) {
    const ed = dict(e);
    // The map holds an entry keyed "None", which is no camp.
    const id = noneIsNull(g(ed, "key"));
    if (!id) continue;
    const v = dict(ed["value"]);
    const elapsed = v["ElapsedTime"];
    out.push({
      id,
      spawned: !!v["bIsSpawned"],
      cleared: !!v["bIsClear"],
      allDead: !!v["bIsEnemyAllDead"],
      rewardReceived: !!v["bRewardReceived"],
      rewardPal: g(v, "RewardPalId"),
      rewardLevel: g(v, "RewardPalLevel"),
      clearedAt: ticksToIso(v["ClearDate"]),
      elapsedPlaySeconds: isNum(elapsed) && elapsed >= 0 ? num(elapsed) : null,
      chests: list(v["TreasureBoxInfoMapBySpawnerName"]).length,
    });
  }
  return out;
}

/** Boss spawners whose boss is down right now (RespawnDisableFlag). Often for under five minutes. */
export function bossesDown(world: Dict): string[] {
  return flags(dict(world["BossSpawnerSaveData"])["RespawnDisableFlag"]);
}

/**
 * One spawner slot, from its NextLotteryGameTime: -1 its object stands, 0
 * nothing spawned, > 0 taken and due to re-roll at that game-clock tick. null
 * for a re-roll time with an object id still set.
 */
function spawnerSlot(v: Dict): number | null {
  const t = v["NextLotteryGameTime"];
  if (t === -1) return -1;
  return isInt(t) && Number(t) >= 0 && !cid(v["MapObjectInstanceId"]) ? Number(t) : null;
}

/**
 * Every map-object spawner the save holds, keyed by guidKey of its
 * LevelObjectInstanceId: a spawnerSlot, or a list of them for a multi-slot
 * spawner (a skill fruit tree). Zero slots stay in.
 */
export function spawners(world: Dict): Record<string, number | null | (number | null)[]> {
  const stages = list(world["MapObjectSpawnerInStageSaveData"]);
  // The main world's stage, so far the only one.
  const entries = stages.length ? list(dict(dict(stages[0])["value"])["SpawnerDataMapByLevelObjectInstanceId"]) : [];
  const out: Record<string, number | null | (number | null)[]> = {};
  for (const e of entries) {
    const ed = dict(e);
    const items = [...list(dict(ed["value"])["ItemMap"])].sort((a, b) => {
      const ka = dict(a)["key"];
      const kb = dict(b)["key"];
      return (isInt(ka) ? Number(ka) : 0) - (isInt(kb) ? Number(kb) : 0);
    });
    const slots = items.map((s) => spawnerSlot(dict(dict(s)["value"])));
    const key = guidKey(ed["key"]);
    if (key && slots.length) out[key] = slots.length === 1 ? slots[0]! : slots;
  }
  return out;
}

/** The map objects a spawner owns that the save writes down: rocks once they're hit, and chests. */
export function nodes(world: Dict): Dict[] {
  const out: Dict[] = [];
  for (const mo of list(world["MapObjectSaveData"])) {
    const mod = dict(mo);
    const raw = dict(dict(mod["Model"])["RawData"]);
    const spawner = guidKey(raw["owner_spawner_level_object_instance_id"]);
    if (!spawner) continue;
    const hp = dict(raw["hp"]);
    const current = hp["current"];
    const most = hp["max"];
    const at = position(raw["initital_transform_cache"]); // the game's spelling
    out.push({
      spawner,
      mapObjectId: g(mod, "MapObjectId"),
      hp: isNum(current) && isNum(most) && most > 0 ? { current, max: most } : null,
      ...(at ? { x: at.x, y: at.y, z: at.z } : {}),
    });
  }
  return out;
}

const noneIsNull = (v: Plain): Plain => (v === null || v === "None" ? null : v);

export function dungeons(world: Dict, nowTicks: unknown): Dict[] {
  const out: Dict[] = [];
  for (const d of list(world["DungeonSaveData"])) {
    const dd = dict(d);
    const closes = instant(dd["DisappearTimeAt"]);
    const respawn = instant(dd["RespawnBossTimeAt"]);
    const marker = g(dd, "MarkerPointId");
    out.push({
      id: g(dd, "InstanceId"),
      type: enumName(g(dd, "DungeonType")),
      area: noneIsNull(g(dd, "DungeonSpawnAreaId")),
      level: noneIsNull(g(dd, "DungeonLevelName")),
      bossState: enumName(g(dd, "BossState")),
      // Joins to the game's tables: the entrance marker's position, and the boss sheet.
      marker: marker === ZERO_GUID ? null : marker,
      bossRow: noneIsNull(g(dd, "EnemySpawnerDataBossRowName")),
      closesInGameSeconds: gameSecondsUntil(closes, nowTicks),
      bossRespawnInGameSeconds: gameSecondsUntil(respawn, nowTicks),
      // The same two instants on the game clock, for the API to time against later saves.
      disappearAtGameTicks: closes,
      respawnBossAtGameTicks: respawn,
      rewards: list(dd["RewardSaveDataMap"]).length,
      bonusExpPlayers: list(dd["ReceivedBonusExpPlayerIds"]).length,
    });
  }
  return out;
}

/** Dungeon entrances: the dungeon open at each (null while closed), and when a closed one rolls again (game clock). */
export function dungeonMarkers(world: Dict): Dict[] {
  return list(world["DungeonPointMarkerSaveData"]).map((m) => {
    const md = dict(m);
    return { id: g(md, "MarkerPointId"), instanceId: cid(md["ConnectedDungeonInstanceId"]), nextRespawnGameTicks: instant(md["NextRespawnGameTime"]) };
  });
}

export function oilRigs(world: Dict): Dict[] {
  return list(dict(world["OilrigSaveData"])["OilrigMap"]).map((e) => {
    const ed = dict(e);
    const v = dict(ed["value"]);
    return {
      type: enumName(g(ed, "key")),
      cleared: !!v["Clear"],
      alarm: !!v["Alarm"],
      cannonsDestroyed: list(v["DestroyedCannon"]).length,
      gasTanksDestroyed: list(v["DestroyedGasTank"]).length,
      treasureOpened: list(v["OpenedTreasureBoxSpawner"]).length,
      machineSpawned: !!v["IsMachineSpawned"],
      machineDestroyed: !!v["IsMachineDestroyed"],
      spawnersWiped: list(v["WipedOutEnemySpawner"]).length,
      goalBox: g(v, "GoalTreasureBoxSpawnerIndex"),
      // Seconds of play on the cleared rig's reset count. No save has shown it other than 0 yet.
      resetTimerSeconds: num(v["ResetTimer"]),
    };
  });
}

/**
 * Meteors and supply drops. Their times are on the real-time clock
 * (RealDateTimeTicks): seconds of play. `lastGuid` is the newest drop's id.
 */
export function supplyDrops(world: Dict, realTicks: unknown): Dict {
  const s = dict(world["SupplySaveData"]);
  const real = asInt(realTicks);
  const playSeconds = (ticks: unknown): number | null => (isInt(ticks) && Number(ticks) > 0 ? Math.floor(Number(ticks) / 10_000_000) : null);
  const ago = (ticks: unknown): number | null => (real !== null && isInt(ticks) && Number(ticks) > 0 ? Math.floor((real - Number(ticks)) / 10_000_000) : null);
  const drops = list(s["SupplyInfos"]).map((e) => {
    const ed = dict(e);
    const v = dict(ed["value"]);
    const landed = v["SupplyLandedTime"];
    return {
      guid: g(ed, "key"),
      type: enumName(g(v, "SupplyType")),
      atPlaySeconds: playSeconds(v["SupplyTime"]),
      landed: isInt(landed) && Number(landed) > 0,
      secondsAgo: ago(landed),
      guardsDefeated: !!v["bWipedOut_Pal"] || !!v["bWipedOut_NPC"],
      wipedPal: !!v["bWipedOut_Pal"],
      wipedNpc: !!v["bWipedOut_NPC"],
      items: list(v["DropItemGuids"]).length,
      mapLocationId: cid(v["MapLocationId"]),
    };
  });
  const last = s["LastSupplyTime"];
  return { lastDropSecondsAgo: ago(last), lastGuid: cid(s["LastSupplyGuid"]), lastAtPlaySeconds: playSeconds(last), drops };
}

// ---------------------------------------------------------------- player extras

/**
 * The character maker's choices. The colour fields aren't RGB: each is the
 * adjustment the game applies to that part's own base texture (r = hue shift,
 * g = saturation multiplier, b = brightness multiplier), passed through as is.
 */
export function appearance(make: unknown): Dict | null {
  if (!make || !isDict(make)) return null;
  const adjust = (c: unknown): Dict | null => (isDict(c) ? { hue: num(c["r"], 3), saturation: num(c["g"], 3), brightness: num(c["b"], 3) } : null);
  const colorAdjust: Dict = {};
  for (const k of ["hair", "brow", "body", "eye"]) colorAdjust[k] = adjust(make[`${k[0]!.toUpperCase()}${k.slice(1)}Color`]);
  return {
    body: g(make, "BodyMeshName"),
    head: g(make, "HeadMeshName"),
    hair: g(make, "HairMeshName"),
    eyes: g(make, "EyeMaterialName"),
    voice: g(make, "VoiceID"),
    colorAdjust,
  };
}

const enumKeys = (m: Dict): Dict => Object.fromEntries(Object.entries(m).map(([k, v]) => [String(enumName(k)), v]));

export function recordExtras(rec: Dict): Dict {
  return {
    relicsByType: enumKeys(mapd(rec["RelicPossessNumMap"])),
    relicsPickedUp: flags(rec["RelicObtainForInstanceFlag"]).length,
    // The ids themselves, so the API can place them (and the ones still out there) on the map.
    relicIds: flags(rec["RelicObtainForInstanceFlag"]),
    itemPickupIds: flags(rec["ItemPickupObtainForInstanceFlag"]),
    notes: flags(rec["NoteObtainForInstanceFlag"]),
    itemPickups: flags(rec["ItemPickupObtainForInstanceFlag"]).length,
    npcsTalkedTo: Object.keys(mapd(rec["NPCTalkCountMap"])).length,
    emotesLearned: list(rec["CompletedEmoteNPCIDArray"]).length,
    worldMapsUnlocked: flags(rec["UnlockedWorldMapFlags"]),
    // The game's own lifetime counters: they predate the API's event log.
    fixedDungeonsCleared: getOr(rec, "FixedDungeonClearCount", 0),
    rigsCleared: getOr(rec, "OilrigClearCount", 0),
    predatorsDefeated: getOr(rec, "PredatorDefeatCount", 0),
    // Camps where every enemy was killed; freeing the pal by stealth doesn't count, so it's below the clears.
    campsConquered: getOr(rec, "CampConqueredCount", 0),
    // Condensations by the rank reached ("2": 16 pals taken to rank 2), butchers by species, fish by id.
    condensations: mapd(rec["PalRankupCount"]),
    butchered: mapd(rec["PalButcherCount"]),
    fished: mapd(rec["FishingCountMap"]),
  };
}

/** LocalData.sav: this machine's view of the player. null when the file wasn't read. */
export function localExtras(local: Dict | null, explored: number | null): Dict | null {
  if (!local) return null;
  const sd = dict(local["SaveData"]);
  const summons = Object.entries(enumKeys(mapd(sd["Local_ActivateOtomoCount"]))).sort(([, a], [, b]) => Number(b) - Number(a));
  // One key per boss spawner, true while its location is still hidden on her map. hiddenLocationsFound
  // counts the true ones, as it always has whatever its name says; the revealed ones are listed by id.
  const hidden = sd["Local_HiddenLocationFlagMap"];
  return {
    speciesEncountered: flags(sd["Local_PalEncountFlag"]).map((k) => String(enumName(k))).sort(byCodePoint),
    summons: Object.fromEntries(summons),
    trackingQuest: g(sd, "TrackingQuestId"),
    hiddenLocationsFound: flags(hidden).length,
    mapExploredPct: explored,
    bossLocationsRevealed: Object.entries(mapd(hidden))
      .filter(([, v]) => !v)
      .map(([k]) => k)
      .sort(byCodePoint),
  };
}

/**
 * Every egg in the world with the species inside and where it is. An egg in a
 * container nobody owns is a wild egg lying somewhere in the world.
 */
export function eggsEverywhere(instances: Record<string, Instance[]>, places: Record<string, string>): Dict[] {
  const out: { item: Plain; species: string | null; alpha: boolean; where: string }[] = [];
  for (const [container, items] of Object.entries(instances)) {
    for (const e of items) {
      if (e.kind !== "egg") continue;
      const raw = String(e.species ?? "");
      const alpha = raw.toUpperCase().startsWith("BOSS_");
      out.push({ item: e.item, species: alpha ? raw.slice(5) : raw || null, alpha, where: places[container] ?? "wild" });
    }
  }
  return out.sort(
    (a, b) => Number(a.where === "wild") - Number(b.where === "wild") || byCodePoint(a.species ?? "", b.species ?? "") || byCodePoint(a.where, b.where),
  );
}
