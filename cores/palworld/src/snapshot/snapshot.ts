/**
 * Build the snapshot the API stores from a world's unwrapped save files.
 *
 * The snapshot is the contract with the API's ingest (its reader mirrors this
 * file and refuses a SCHEMA it doesn't know; bump SCHEMA when a field changes
 * meaning, and test/snapshot.test.ts pins it and a fixture snapshot). It keeps
 * the game's own ids (`PinkCat`, `PAL_conceited`, `Wood`) rather than display
 * names: names aren't in the save, and an id is what a later name table keys
 * on. Everything here is pure: dicts in, dict out. A port of snapshot.py.
 */

import type { Plain } from "../gvas/unwrap.js";
import {
  DEFAULT_BASE_NAME_PREFIX,
  INVENTORY_CONTAINERS,
  ZERO_GUID,
  byCodePoint,
  cid,
  concreteRaw,
  countItems,
  dict,
  enumName,
  fixed,
  flags,
  isDict,
  isNum,
  list,
  mapd,
  num,
  position,
  statusPoints,
  str,
  ticksToIso,
  type Dict,
} from "./util.js";
import * as w2 from "./world.js";

export const SCHEMA = 2;

const g = (d: Dict, k: string): Plain => d[k] ?? null;
const getOr = <T>(d: Dict, k: string, dflt: T): Plain | T => (k in d ? d[k]! : dflt);

// ---------------------------------------------------------------- containers

/** Every item container, by id: [{item, count}] summed per item, sorted. */
export function itemContainers(world: Dict): Record<string, w2.ItemCount[]> {
  const out: Record<string, w2.ItemCount[]> = {};
  for (const c of list(world["ItemContainerSaveData"])) {
    const cd = dict(c);
    const key = cid(cd["key"]);
    if (!key) continue;
    const pairs: [string, number][] = [];
    for (const slot of list(dict(cd["value"])["Slots"])) {
      const raw = dict(dict(slot)["RawData"]);
      const item = dict(raw["item"])["static_id"];
      const count = raw["count"] || 0;
      if (typeof item === "string" && item && item !== "None" && isNum(count) && count > 0) pairs.push([item, count]);
    }
    out[key] = countItems(pairs);
  }
  return out;
}

function mergeItems(...lists: w2.ItemCount[][]): w2.ItemCount[] {
  return countItems(lists.flat().map((e): [string, number] => [e.item, e.count]));
}

// ---------------------------------------------------------------- players

function player(save: Dict, character: Dict | null, items: Record<string, w2.ItemCount[]>, instances: Record<string, w2.Instance[]> = {}, local: Dict | null = null): Dict {
  const sd = dict(save["SaveData"]);
  const ch = character ?? {};
  const rec = dict(sd["RecordData"]);
  const inv = dict(sd["InventoryInfo"]);
  const ordered = list(sd["OrderedQuestArray_FullRelease"]).filter(isDict);
  const questsActive = ordered.map((q) => q["QuestName"]);
  // Where each active quest is: the step it's on (BlockIndex, 0-based) and the step's own counters
  // (items handed in so far, CanCompleteFlag_<n>), as the game keeps them. Since agent 0.5.9.
  const questProgress = ordered
    .filter((q) => typeof q["QuestName"] === "string" && q["QuestName"])
    .map((q) => ({
      id: q["QuestName"] as string,
      step: num(q["BlockIndex"]) ?? 0,
      counts: Object.fromEntries(list(q["IntegerMap"]).filter(isDict).filter((e) => typeof e["key"] === "string" && isNum(e["value"])).map((e) => [e["key"] as string, e["value"] as number])),
    }));
  const gear: Dict[] = [];
  for (const [name, field] of Object.entries(INVENTORY_CONTAINERS)) {
    for (const x of instances[cid(inv[field]) ?? ""] ?? []) {
      if (x.kind === "weapon" || x.kind === "armor") gear.push({ ...(x as unknown as Dict), section: name });
    }
  }
  return {
    uid: g(sd, "PlayerUId"),
    name: g(ch, "NickName"),
    level: getOr(ch, "Level", 1),
    exp: getOr(ch, "Exp", 0),
    hp: fixed(ch["Hp"]),
    shield: fixed(ch["ShieldHP"]),
    stomach: num(ch["FullStomach"]),
    statusPoints: statusPoints(ch["GotStatusPointList"]),
    exStatusPoints: statusPoints(ch["GotExStatusPointList"]),
    techPoints: getOr(sd, "TechnologyPoint", 0),
    ancientTechPoints: getOr(sd, "bossTechnologyPoint", 0),
    unlockedRecipes: [...list(sd["UnlockedRecipeTechnologyNames"])].map(String).sort(byCodePoint),
    position: position(sd["LastTransform"]),
    lastOnline: ticksToIso(sd["LastOnlineDateTime"]),
    platform: enumName(g(sd, "PlayerPlatform")),
    partyContainer: cid(sd["OtomoCharacterContainerId"]),
    palboxContainer: cid(sd["PalStorageContainerId"]),
    quests: { completed: [...list(sd["CompletedQuestArray_FullRelease"])], active: questsActive.filter((q) => !!q) as Plain[], progress: questProgress },
    records: {
      bossesDefeated: flags(rec["NormalBossDefeatFlag"]),
      towerBossesDefeated: flags(rec["TowerBossDefeatFlag"]),
      towerClears: mapd(rec["TowerBossDefeatCount"]),
      speciesCaptured: getOr(rec, "TribeCaptureCount", 0),
      captures: mapd(rec["PalCaptureCount"]),
      paldeckUnlocked: flags(rec["PaldeckUnlockFlag"]),
      relics: getOr(rec, "RelicPossessNum", 0),
      notesFound: flags(rec["NoteObtainForInstanceFlag"]).length,
      fastTravelUnlocked: flags(rec["FastTravelPointUnlockFlag"]),
      areasFound: flags(rec["FindAreaFlagMap"]),
      dungeonsCleared: getOr(rec, "NormalDungeonClearCount", 0),
      crafted: mapd(rec["CraftItemCount"]),
      ...w2.recordExtras(rec),
    },
    inventory: Object.fromEntries(Object.entries(INVENTORY_CONTAINERS).map(([name, field]) => [name, items[cid(inv[field]) ?? ""] ?? []])),
    // Weapons and armour carried or worn, with durability, ammo and weapon passives.
    gear,
    appearance: w2.appearance(sd["PlayerCharacterMakeData"]),
    skins: list(dict(sd["SkinInventoryInfo"])["InGameData"]).map((s) => ({ skin: g(dict(s), "SkinName"), count: g(dict(s), "Num") })),
    local,
  };
}

// ---------------------------------------------------------------- pals

function pal(key: Dict, sp: Dict): Dict {
  const rawId = String(sp["CharacterID"] ?? "");
  const isBoss = rawId.toUpperCase().startsWith("BOSS_");
  // Lucky pals carry the BOSS_ prefix too; IsRarePal is what tells them from alphas.
  const lucky = !!sp["IsRarePal"];
  const alpha = isBoss && !lucky;
  const slot = dict(sp["SlotId"]);
  const off = list(dict(sp["WorkSuitabilityOptionInfo"])["OffWorkSuitabilityList"]);
  return {
    instanceId: g(key, "InstanceId"),
    ownerUid: g(sp, "OwnerPlayerUId"),
    species: isBoss ? rawId.slice(5) : rawId,
    nickname: sp["NickName"] || null,
    alpha,
    lucky,
    // Marked as a favourite in the Palbox (FavoriteIndex is set), and which favourite number she gave it.
    favourite: sp["FavoriteIndex"] !== undefined && sp["FavoriteIndex"] !== null,
    favouriteIndex: typeof sp["FavoriteIndex"] === "number" ? sp["FavoriteIndex"] : null,
    gender: enumName(g(sp, "Gender")),
    level: getOr(sp, "Level", 1),
    exp: getOr(sp, "Exp", 0),
    // Rank 1 is unconsolidated; each condensation adds one (max 5).
    rank: getOr(sp, "Rank", 1),
    // Work suitability tickets used on the pal: work id → ranks gained (0.5.2+).
    workAddRanks: Object.fromEntries(list(sp["GotWorkSuitabilityAddRankList"]).map((e) => [enumName(dict(e)["WorkSuitability"]), getOr(dict(e), "Rank", 0)])),
    talents: { hp: getOr(sp, "Talent_HP", 0), attack: getOr(sp, "Talent_Shot", 0), defense: getOr(sp, "Talent_Defense", 0) },
    souls: { hp: getOr(sp, "Rank_HP", 0), attack: getOr(sp, "Rank_Attack", 0), defense: getOr(sp, "Rank_Defence", 0), workSpeed: getOr(sp, "Rank_CraftSpeed", 0) },
    passives: [...list(sp["PassiveSkillList"])],
    activeSkills: list(sp["EquipWaza"]).map((w) => enumName(w)),
    learnedSkills: list(sp["MasteredWaza"]).map((w) => enumName(w)),
    hp: fixed(sp["Hp"]),
    stomach: num(sp["FullStomach"]),
    sanity: num(sp["SanityValue"]),
    friendship: getOr(sp, "FriendshipPoint", 0),
    sick: enumName(g(sp, "WorkerSick")) || null,
    workOff: off.map((w) => enumName(w)),
    ownedAt: ticksToIso(sp["OwnedTime"], true),
    container: cid(slot["ContainerId"]),
    slot: g(slot, "SlotIndex"),
    lastPosition: position(sp["LastJumpedLocation"]),
  };
}

// ---------------------------------------------------------------- bases & guilds

function bases(world: Dict, items: Record<string, w2.ItemCount[]>, capacity: Record<string, { slots: Plain; used: number }> = {}, instances: Record<string, w2.Instance[]> = {}): Dict[] {
  const work = w2.baseWork(world);
  const producing = w2.production(world, items);
  const incubators = w2.incubators(world, instances);
  const raids = w2.raids(world);
  const power = w2.power(world);
  const expeditions = w2.expeditions(world);
  const byBase: Record<string, Dict[]> = {};
  for (const mo of list(world["MapObjectSaveData"])) {
    const mod = dict(mo);
    const baseId = str(dict(dict(mod["Model"])["RawData"])["base_camp_id_belong_to"]);
    if (baseId && baseId !== ZERO_GUID) (byBase[baseId] ??= []).push(mod);
  }
  const out: Dict[] = [];
  for (const b of list(world["BaseCampSaveData"])) {
    const bd = dict(b);
    const raw = dict(dict(bd["value"])["RawData"]);
    const baseId = (str(raw["id"]) || str(bd["key"])) ?? "";
    const name = str(raw["name"]) || "";
    const objects = byBase[baseId] ?? [];
    const storage: Dict[] = [];
    const crops: Dict[] = [];
    const structures: [string, number][] = [];
    for (const mo of objects) {
      const concrete = dict(mo["ConcreteModel"]);
      const craw = concreteRaw(concrete);
      structures.push([String(mo["MapObjectId"]), 1]);
      for (const module of list(concrete["ModuleMap"])) {
        const md = dict(module);
        if (enumName(md["key"]) === "ItemContainer") {
          const container = cid(dict(dict(md["value"])["RawData"])["target_container_id"]);
          if (container && container in items) {
            const cap = capacity[container];
            storage.push({ structure: g(mo, "MapObjectId"), container, items: items[container]!, slots: cap?.slots ?? null, used: cap?.used ?? null });
          }
        }
      }
      if (craw["crop_data_id"]) crops.push({ crop: g(craw, "crop_data_id"), state: g(craw, "current_state"), progress: num(craw["crop_progress_rate_value"], 3) });
    }
    out.push({
      id: baseId,
      name: name.startsWith(DEFAULT_BASE_NAME_PREFIX) ? null : name,
      guildId: g(raw, "group_id_belong_to"),
      position: position(raw["transform"]),
      areaRange: g(raw, "area_range"),
      workerContainer: cid(dict(dict(dict(bd["value"])["WorkerDirector"])["RawData"])["container_id"]),
      structures: Object.fromEntries(countItems(structures).map((e) => [e.item, e.count])),
      storage,
      storageTotals: mergeItems(...storage.map((s) => s["items"] as w2.ItemCount[])),
      crops,
      work: work[baseId] ?? [],
      production: producing[baseId] ?? [],
      incubators: incubators[baseId] ?? [],
      raid: raids[baseId] ?? null,
      power: power[baseId] ?? [],
      expeditions: expeditions[baseId] ?? [],
    });
  }
  return out;
}

function guilds(world: Dict, items: Record<string, w2.ItemCount[]>): Dict[] {
  const extra: Record<string, Dict> = {};
  for (const e of list(world["GuildExtraSaveDataMap"])) if (isDict(e)) extra[String(e["key"])] = dict(e["value"]);
  const out: Dict[] = [];
  for (const gr of list(world["GroupSaveDataMap"])) {
    const gd = dict(gr);
    const v = dict(gd["value"]);
    if (enumName(v["GroupType"]) !== "Guild") continue;
    const raw = dict(v["RawData"]);
    const ex = extra[String(gd["key"])] ?? {};
    const chest = cid(dict(dict(ex["GuildItemStorage"])["RawData"])["container_id"]);
    const lab = dict(dict(ex["Lab"])["RawData"]);
    const research = list(lab["research_info"]);
    const current = g(lab, "current_research_id");
    out.push({
      id: raw["group_id"] || g(gd, "key"),
      name: g(raw, "guild_name"),
      baseCampLevel: g(raw, "base_camp_level"),
      adminUid: g(raw, "admin_player_uid"),
      members: list(raw["players"]).map((p) => ({ uid: g(dict(p), "player_uid"), name: g(dict(dict(p)["player_info"]), "player_name"), role: g(dict(p), "role") })),
      baseIds: [...list(raw["base_ids"])],
      chest: items[chest ?? ""] ?? [],
      chestContainer: chest,
      research: research.map((r) => ({ id: g(dict(r), "research_id"), workAmount: num(dict(r)["work_amount"]) })),
      // What the lab is researching now (0.5.6+): "None" or empty when nothing is chosen.
      currentResearchId: typeof current === "string" && current !== "None" && current !== "" ? current : null,
    });
  }
  return out;
}

// ---------------------------------------------------------------- the snapshot

export type BuildInput = {
  worldId: string;
  level: Dict;
  meta: Dict | null;
  options: Dict | null;
  /** By the player file's stem (32 hex). */
  players: Record<string, Dict>;
  agent: Dict;
  local?: Dict | null;
  exploredPct?: number | null;
  gameBuild?: Dict | null;
};

export function build({ worldId, level, meta, options, players, agent, local = null, exploredPct = null, gameBuild = null }: BuildInput): Dict {
  const world = dict(level["worldSaveData"]);
  const items = itemContainers(world);
  const capacity = w2.containerCapacity(world);
  const instances = w2.instancesByContainer(world);
  const localView = w2.localExtras(local, exploredPct);

  const characters: Record<string, Dict> = {};
  const pals: Dict[] = [];
  for (const entry of list(world["CharacterSaveParameterMap"])) {
    const ed = dict(entry);
    const key = dict(ed["key"]);
    const sp = dict(dict(dict(dict(ed["value"])["RawData"])["object"])["SaveParameter"]);
    if (sp["IsPlayer"]) characters[String(key["PlayerUId"])] = sp;
    else if (sp["CharacterID"]) pals.push(pal(key, sp));
  }

  const playerList = Object.entries(players)
    .sort(([a], [b]) => byCodePoint(a, b))
    .map(([, save], i) =>
      // LocalData is this Mac's, so it describes the host (the first player).
      player(save, characters[String(dict(save["SaveData"])["PlayerUId"])] ?? null, items, instances, i === 0 ? localView : null),
    );
  const baseList = bases(world, items, capacity, instances);
  const guildList = guilds(world, items);

  // Where each pal is, in words. A container nobody claims (a viewing cage,
  // an incubator, a dimensional box) stays "other" with its id.
  const where: Record<string, Dict> = {};
  for (const p of playerList) {
    if (p["partyContainer"]) where[String(p["partyContainer"])] = { kind: "party", playerUid: p["uid"] ?? null };
    if (p["palboxContainer"]) where[String(p["palboxContainer"])] = { kind: "palbox", playerUid: p["uid"] ?? null };
  }
  for (const b of baseList) if (b["workerContainer"]) where[String(b["workerContainer"])] = { kind: "base", baseId: b["id"] ?? null };
  // A pal out on an expedition keeps its container (the party or a base), so the station's list wins.
  const away: Record<string, Dict> = {};
  for (const b of baseList) {
    for (const e of b["expeditions"] as Dict[]) if (e["state"] === 2) for (const id of e["pals"] as string[]) away[id] = { kind: "expedition", baseId: b["id"] ?? null };
  }
  for (const p of pals) p["location"] = away[String(p["instanceId"])] ?? where[String(p["container"] ?? "")] ?? { kind: "other" };

  // Where each container is, in words: for eggs.
  const places: Record<string, string> = {};
  for (const p of playerList) {
    const inv = dict(dict(players[String(p["uid"]).replace(/-/g, "").toUpperCase()]?.["SaveData"])["InventoryInfo"]);
    for (const [name, field] of Object.entries(INVENTORY_CONTAINERS)) {
      const c = cid(inv[field]);
      if (c) places[c] = `inventory: ${name}`;
    }
  }
  baseList.forEach((b, i) => {
    for (const s of b["storage"] as Dict[]) places[String(s["container"])] = `${b["name"] || `Base ${i + 1}`}: ${s["structure"]}`;
  });
  for (const gl of guildList) if (gl["chestContainer"]) places[String(gl["chestContainer"])] = "guild chest";

  const metaSd = dict(meta?.["SaveData"]);
  const settings = dict(dict(options?.["OptionWorldData"])["Settings"]);
  const gameTime = dict(world["GameTimeSaveData"]);

  return {
    schema: SCHEMA,
    agent,
    world: {
      id: worldId,
      name: g(metaSd, "WorldName"),
      hostName: g(metaSd, "HostPlayerName"),
      day: g(metaSd, "InGameDay"),
      savedAt: ticksToIso(meta?.["Timestamp"], true) ?? ticksToIso(level["Timestamp"], true),
      // The two play clocks: the game's (runs faster, jumps when she sleeps) and seconds of play.
      gameTimeTicks: g(gameTime, "GameDateTimeTicks"),
      realTimeTicks: g(gameTime, "RealDateTimeTicks"),
      // The installed game (Palworld.app's version and build), so the API can flag tables older than it.
      gameBuild,
      clock: w2.clock(gameTime["GameDateTimeTicks"]),
      bossesDown: w2.bossesDown(world),
      spawners: w2.spawners(world),
      nodes: w2.nodes(world),
      settings: Object.fromEntries(
        Object.keys(settings)
          .sort(byCodePoint)
          .filter((k) => !k.includes("Password"))
          .map((k) => [k, enumName(settings[k]!)]),
      ),
    },
    players: playerList,
    guilds: guildList,
    bases: baseList,
    pals: pals.sort(
      (a, b) =>
        byCodePoint(String(a["species"]), String(b["species"])) ||
        Number(b["level"] || 0) - Number(a["level"] || 0) ||
        byCodePoint(String(a["instanceId"] ?? ""), String(b["instanceId"] ?? "")),
    ),
    eggs: w2.eggsEverywhere(instances, places),
    locksCleared: w2.locksCleared(world),
    enemyCamps: w2.enemyCamps(world),
    dungeons: w2.dungeons(world, gameTime["GameDateTimeTicks"]),
    dungeonMarkers: w2.dungeonMarkers(world),
    oilRigs: w2.oilRigs(world),
    supply: w2.supplyDrops(world, gameTime["RealDateTimeTicks"]),
  };
}
