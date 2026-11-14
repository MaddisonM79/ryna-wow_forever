/** build() over a small hand-made world: one player, three pals, one base, one guild. Ported from test_snapshot.py. */

import { readFileSync, writeFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { build, SCHEMA } from "../src/snapshot/snapshot.js";
import { ticksToIso } from "../src/snapshot/util.js";
import type { Dict } from "../src/snapshot/util.js";
import { BASE, CLEARED, GUILD_CHEST, META, OPTIONS, PLAYER_SAVE, SAVES, UID, WORLD } from "./fixtures.js";

const snap = () => build({ worldId: "W1", level: WORLD, meta: META, options: OPTIONS, players: { "00000000000000000000000000000001": PLAYER_SAVE }, agent: { version: "test" } });
const d = (v: unknown) => v as Dict;
const l = (v: unknown) => v as Dict[];

describe("build", () => {
  /**
   * The wire contract. The API's ingest holds its own copy of SCHEMA and refuses one it doesn't know,
   * so SCHEMA moves only on purpose, with the API's reader. fixtures/snapshot.json is the snapshot
   * of the hand-made world, checked against the API's snapshot schema when this core moved here
   * (2026-11-13); a change to it is a change to what ingest receives. Rewrite it on purpose with
   * UPDATE_SNAPSHOT=1 pnpm test, and say in the commit what the API has to read.
   */
  it("holds SCHEMA and the snapshot it builds to what ingest was checked against", () => {
    expect(SCHEMA).toBe(2);
    // The hand-made world's "W1" stands in for a world id; ingest wants the save folder's 32 hex digits.
    const built = JSON.parse(JSON.stringify(build({ worldId: "0123456789ABCDEF0123456789ABCDEF", level: WORLD, meta: META, options: OPTIONS, players: { "00000000000000000000000000000001": PLAYER_SAVE }, agent: { version: "test" } }), (_k, v: unknown) => (typeof v === "bigint" ? Number(v) : v))) as Dict;
    const path = new URL("./fixtures/snapshot.json", import.meta.url);
    if (process.env["UPDATE_SNAPSHOT"]) writeFileSync(path, `${JSON.stringify(built, null, 1)}\n`);
    expect(built).toEqual(JSON.parse(readFileSync(path, "utf8")));
    expect(d(built["world"])["id"]).toBe("0123456789ABCDEF0123456789ABCDEF");
  });

  it("never carries passwords in the world's settings", () => {
    const w = d(snap()["world"]);
    expect([w["name"], w["day"], w["id"]]).toEqual(["Test Islands", 20, "W1"]);
    expect(w["settings"]).toEqual({ Difficulty: "Easy" });
    expect(w["savedAt"]).toBe(ticksToIso(639269610204320000n, true));
  });

  it("reads local ticks in this machine's zone (the tests run in America/Chicago)", () => {
    // 2026-10-07 09:42:50 wall clock in Chicago (CDT, UTC-5)
    const ticks = 639269629704670000n;
    expect(ticksToIso(ticks)).toBe("2026-10-07T09:42:50.467000Z");
    expect(ticksToIso(ticks, true)).toBe("2026-10-07T14:42:50.467000Z");
    // Python's isoformat leaves the microseconds out when they are zero.
    expect(ticksToIso(639269629700000000n)).toBe("2026-10-07T09:42:50Z");
    expect(ticksToIso(0)).toBeNull();
  });

  const chicago = (level: Dict) => build({ worldId: "W1", level, meta: null, options: null, players: {}, agent: { version: "test" } });

  it("carries both play clocks", () => {
    const w = d(chicago(CLEARED)["world"]);
    expect([w["gameTimeTicks"], w["realTimeTicks"]]).toEqual([17534270000000, 372765390000]);
  });

  it("agrees a camp's UTC clear time with the local save time", () => {
    // ClearDate is UTC ticks and the save's Timestamp local ones (CDT here): read each as what it is,
    // and the gap between them is the play since the clear, which the camp counts itself.
    const s = chicago(CLEARED);
    const [forest] = l(s["enemyCamps"]);
    const gap = (Date.parse(String(d(s["world"])["savedAt"])) - Date.parse(String(forest!["clearedAt"]))) / 1000;
    expect(d(s["world"])["savedAt"]).toBe("2026-10-07T13:59:50.434000Z");
    expect(Math.abs(gap - Number(forest!["elapsedPlaySeconds"]))).toBeLessThan(2);
  });

  it("lands the repeatables where the API reads them", () => {
    const s = chicago(SAVES["newest"]!);
    const w = d(s["world"]);
    expect([w["savedAt"], w["bossesDown"], Object.keys(d(w["spawners"])).length, l(w["nodes"]).length]).toEqual(["2026-10-08T21:11:13.368000Z", ["BOSS_Believer_CrossBow"], 14, 3]);
    expect(["dungeons", "dungeonMarkers", "oilRigs"].map((k) => l(s[k]).length)).toEqual([3, 2, 3]);
    expect(d(s["supply"])["lastGuid"]).toBe("8f1cb1b5-e478-2a40-d7b1-11530db5e79d");
  });

  it("joins a player to its character by uid, not by file name", () => {
    const p = l(snap()["players"])[0]!;
    expect([p["name"], p["level"], p["statusPoints"]]).toEqual(["Tamer", 25, { attack: 11, moveSpeed: 2, gliderSpeed: 2 }]); // the 1.0 stats are named too, the effigy upgrades included
    expect(d(p["inventory"])["common"]).toEqual([{ item: "Bow", count: 1 }, { item: "Money", count: 783 }]);
    const r = d(p["records"]);
    expect(r["paldeckUnlocked"]).toEqual(["PinkCat"]);
    // The game's own lifetime counters, which predate the API's event log.
    expect([r["fixedDungeonsCleared"], r["rigsCleared"], r["predatorsDefeated"], r["campsConquered"]]).toEqual([7, 1, 2, 1]);
    expect([r["condensations"], r["butchered"], r["fished"]]).toEqual([{ "2": 16, "5": 1 }, { PinkCat: 7 }, { FishShadow_LazyCatfish_Nushi: 1 }]);
    expect(d(p["quests"])["active"]).toEqual(["Main_DefeatForestBoss"]);
    // The step it's on and that step's counters (0.5.9).
    expect(d(p["quests"])["progress"]).toEqual([
      { id: "Main_DefeatForestBoss", step: 2, counts: { QuestBlock_DeliveryItem_TotalDeliveredCount: 3, CanCompleteFlag_0: 0 } },
    ]);
  });

  it("gives pals species flags and a location", () => {
    const pals = Object.fromEntries(l(snap()["pals"]).map((p) => [String(p["instanceId"]), p]));
    expect(Object.keys(pals).sort()).toEqual(["a", "b", "c"]); // the player character is not a pal
    const [a, b, c] = [pals["a"]!, pals["b"]!, pals["c"]!];
    expect([a["species"], d(a["talents"])["attack"], a["activeSkills"], a["gender"], a["sick"]]).toEqual(["PinkCat", 88, ["AirCanon"], "Male", "Cold"]);
    expect(a["location"]).toEqual({ kind: "base", baseId: BASE });
    // A lucky pal has the BOSS_ id too, so it is lucky, not an alpha.
    expect([b["species"], b["alpha"], b["lucky"], b["rank"], b["level"]]).toEqual(["SheepBall", false, true, 3, 1]);
    expect([b["favourite"], a["favourite"]]).toEqual([true, false]); // FavoriteIndex set, or not
    expect([b["favouriteIndex"], a["favouriteIndex"]]).toEqual([3, null]); // and which favourite number
    expect([b["workAddRanks"], a["workAddRanks"]]).toEqual([{ Seeding: 1 }, {}]); // work suitability tickets used
    expect(b["location"]).toEqual({ kind: "party", playerUid: UID });
    // "c" sits in a cage container nobody claims, but the Expedition station lists it as out.
    expect(c["location"]).toEqual({ kind: "expedition", baseId: BASE });
  });

  it("sums a base's storage and keeps the default name out", () => {
    const [base] = l(snap()["bases"]);
    expect(base!["name"]).toBeNull();
    expect([d(base!["position"])["mapX"], d(base!["position"])["mapY"]]).toEqual([0, 0]);
    expect(base!["structures"]).toEqual({ ElectricGenerator: 1, Expedition: 1, FarmBlockV2_wheet: 1, HatchingPalEgg: 1, ItemChest: 1, MultiElectricHatchingPalEgg: 2 });
    expect(base!["storageTotals"]).toEqual([{ item: "PalEgg_Dark_01", count: 1 }, { item: "PalEgg_Ice_01", count: 1 }, { item: "PalEgg_Normal_01", count: 1 }, { item: "PalEgg_Normal_05", count: 1 }, { item: "Wood", count: 15 }]);
    expect(base!["crops"]).toEqual([{ crop: "Wheat", state: 3, progress: 0.5 }]);
  });

  it("counts only guild groups as guilds", () => {
    const [gl] = l(snap()["guilds"]);
    expect([gl!["name"], gl!["baseCampLevel"], l(gl!["members"])[0]!["name"]]).toEqual(["Unnamed Guild", 8, "Tamer"]);
    expect(gl!["research"]).toEqual([{ id: "Handcraft1", workAmount: 0 }]);
    expect(gl!["currentResearchId"]).toBe("Handcraft1");
  });

  it("knows where gear, work, incubators and eggs are", () => {
    const s = snap();
    expect(l(s["players"])[0]!["gear"]).toEqual([{ kind: "weapon", item: "Bow", durability: 87.2, ammo: 3, passives: ["Fire"], section: "common" }]);
    const [base] = l(s["bases"]);
    expect(base!["work"]).toEqual([
      { id: "w-chest", type: "Transport", task: "Transport_ItemChest", structure: "ItemChest", workers: ["a"], progress: null, workAmount: 0 },
      { id: "w-hatch", type: "Progress", task: "HatchingPalEgg_0", structure: null, workers: [], progress: null, workAmount: 150 },
      { id: "w-gen", type: "Assign", task: "ElectricGenerator_0", structure: "ElectricGenerator", workers: ["a"], progress: null, workAmount: 0 },
    ]);
    expect(base!["incubators"]).toEqual([
      { structure: "HatchingPalEgg", temperatureDiff: -1, egg: { kind: "egg", item: "PalEgg_Normal_01", species: "PinkCat" }, hatchSeconds: 150, hatchProgress: 90.2, hatchState: 1 },
      // The large electric one: an entry per egg in its container, no clock (the save keeps none for it); empty, one entry without an egg.
      { structure: "MultiElectricHatchingPalEgg", temperatureDiff: null, egg: { kind: "egg", item: "PalEgg_Normal_05", species: "BOSS_Alpaca" }, hatchSeconds: null, hatchProgress: null, hatchState: null, multi: true },
      { structure: "MultiElectricHatchingPalEgg", temperatureDiff: null, egg: { kind: "egg", item: "PalEgg_Ice_01", species: "Penguin" }, hatchSeconds: null, hatchProgress: null, hatchState: null, multi: true },
      { structure: "MultiElectricHatchingPalEgg", temperatureDiff: null, egg: null, hatchSeconds: null, hatchProgress: null, hatchState: null, multi: true },
    ]);
    expect(base!["power"]).toEqual([{ structure: "ElectricGenerator", stored: 247997.2, consumePerSecond: 12.5, generatePerSecond: 0, workers: ["a"] }]);
    expect(base!["expeditions"]).toEqual([{ structure: "Expedition", mission: "DUNGEON_GRASS", state: 2, startedAtPlaySeconds: 77750, pals: ["c"] }]);
    const [guild] = l(s["guilds"]);
    expect([guild!["chestContainer"], guild!["chest"]]).toEqual([GUILD_CHEST, [{ item: "PalEgg_Fire_01", count: 1 }]]);
    expect(s["eggs"]).toEqual([
      { item: "PalEgg_Normal_05", species: "Alpaca", alpha: true, where: "Base 1: MultiElectricHatchingPalEgg" },
      { item: "PalEgg_Dark_01", species: "Anubis", alpha: false, where: "Base 1: ItemChest" },
      { item: "PalEgg_Fire_01", species: "Kitsunebi", alpha: true, where: "guild chest" },
      { item: "PalEgg_Ice_01", species: "Penguin", alpha: false, where: "Base 1: MultiElectricHatchingPalEgg" },
      { item: "PalEgg_Normal_01", species: "PinkCat", alpha: false, where: "Base 1: HatchingPalEgg" },
    ]);
  });
});
