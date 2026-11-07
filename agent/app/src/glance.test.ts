import { describe, expect, it } from "vitest";
import type { AccountView, CombatStatus, InstallView, UploadedFight } from "./api";
import { accountChip, accountsLine, combatState, FIGHTS_LABEL, fightWords, gameLine, lastFightLine } from "./describe";
import { logText } from "./clipboard";
import { checkResult } from "./updateWords";

const NOW = 1_762_500_000;

const fight: UploadedFight = {
  at: NOW - 120,
  key: "k",
  kind: "world",
  name: null,
  zone: "Zephras Isle",
  durationMs: 240_000,
  players: 1,
};

describe("the last fight's line", () => {
  it("uses middots throughout", () => {
    expect(lastFightLine(fight, NOW)).toBe("Zephras Isle · 4 min · 1 player · 2 minutes ago");
  });

  it("prefers the boss's name, and short fights in seconds", () => {
    expect(lastFightLine({ ...fight, name: "Hogger", durationMs: 45_000, players: 5, at: NOW }, NOW)).toBe(
      "Hogger · 45 s · 5 players · just now",
    );
  });

  it("keeps the log's comma form, which matches the core's", () => {
    expect(fightWords("Zephras Isle", null, 180_000, 2)).toBe("Zephras Isle, 3 min, 2 players");
  });

  it("says what the count is, not when", () => {
    expect(FIGHTS_LABEL).toBe("Fights, last 12 h");
  });
});

function install(over: Partial<InstallView>): InstallView {
  return {
    path: "/wow/_forever_",
    flavor: "wowf",
    forever: true,
    hasAddon: true,
    addonVersion: "0.2.0",
    addon: "current",
    ...over,
  };
}

describe("the Status tab's lines", () => {
  it("says the Forever install and its addon", () => {
    expect(gameLine([install({})], "0.2.0")).toEqual({
      text: "World of Warcraft: Forever · Addon 0.2.0 installed",
      chip: "pushed",
    });
    expect(gameLine([install({ path: "/wow/_retail_", flavor: "wow", forever: false, hasAddon: false }), install({})], "0.2.0").text).toBe(
      "World of Warcraft: Forever · Addon 0.2.0 installed · 1 more",
    );
    expect(gameLine([], null).chip).toBe("waiting");
  });

  it("counts accounts and the newest push", () => {
    const a: AccountView = {
      path: "p",
      install: "i",
      key: "k",
      label: "MAIN",
      savedAt: NOW - 600,
      pushedAt: NOW - 300,
      characters: 3,
      lastError: null,
      phase: "pushed",
    };
    expect(accountsLine([a, { ...a, path: "q", pushedAt: NOW - 3600 }], NOW)).toEqual({
      text: "2 accounts · last pushed 5 minutes ago",
      chip: "pushed",
    });
    expect(accountsLine([{ ...a, phase: "error", lastError: "nope" }], NOW).chip).toBe("error");
    expect(accountsLine([], NOW).text).toBe("No saved data yet");
  });

  it("says where the combat log is", () => {
    const c: CombatStatus = {
      active: true,
      advanced: true,
      file: "WoWCombatLog-110725.txt",
      lastEventAt: NOW,
      pending: 2,
      fightsTonight: 17,
      lastFight: fight,
      lastError: null,
    };
    expect(combatState(c)).toEqual({ text: "Reading · WoWCombatLog-110725.txt · 2 waiting", chip: "pushed" });
    expect(combatState({ ...c, file: null }).chip).toBe("waiting");
    expect(combatState({ ...c, lastError: "Couldn't reach the API" })).toEqual({
      text: "Couldn't reach the API",
      chip: "error",
    });
  });
});

describe("the update check's result", () => {
  const check = { configured: true, current: "0.2.3", available: null };
  it("says nothing before the first check, then what it found", () => {
    expect(checkResult(null, "idle", null)).toBeNull();
    expect(checkResult(check, "idle", null)).toEqual({ text: "Up to date", chip: "pushed" });
    expect(checkResult({ ...check, available: { version: "0.2.4", notes: null, date: null } }, "idle", null)?.text).toBe(
      "Ryna 0.2.4 is available",
    );
    expect(checkResult(check, "checking", null)?.text).toBe("Checking…");
    expect(checkResult(check, "error", "offline")).toEqual({ text: "Couldn't check: offline", chip: "error" });
    expect(checkResult({ ...check, configured: false }, "idle", null)?.chip).toBe("waiting");
  });
});

describe("copying the activity log", () => {
  it("is oldest first, one line each", () => {
    const lines = [
      { at: 2, text: "Pushed MAIN: 3 characters" },
      { at: 1, text: "Pushing MAIN…" },
    ];
    expect(logText(lines, (at) => `t${at}`)).toBe("t1 Pushing MAIN…\nt2 Pushed MAIN: 3 characters");
  });
});

describe("an account row's chip", () => {
  const a: AccountView = {
    path: "p",
    install: "i",
    key: "k",
    label: "MAIN",
    savedAt: NOW - 3 * 3600,
    pushedAt: NOW - 3600,
    characters: 3,
    lastError: null,
    phase: "waiting",
  };

  it("says Pushed, not Waiting, for an account pushed after its last save", () => {
    expect(accountChip(a)).toEqual({ text: "Pushed", chip: "pushed" });
    expect(accountChip({ ...a, savedAt: a.pushedAt })).toEqual({ text: "Pushed", chip: "pushed" });
  });

  it("says Waiting only when the game saved after the last push, or nothing was pushed yet", () => {
    expect(accountChip({ ...a, savedAt: NOW - 600 })).toEqual({ text: "Waiting", chip: "waiting" });
    expect(accountChip({ ...a, pushedAt: null })).toEqual({ text: "Waiting", chip: "waiting" });
    expect(accountChip({ ...a, savedAt: null, pushedAt: null })).toEqual({ text: "Waiting", chip: "waiting" });
  });

  it("keeps the loop's own phase while it has one", () => {
    expect(accountChip({ ...a, phase: "pushing" })).toEqual({ text: "Pushing", chip: "pushing" });
    expect(accountChip({ ...a, phase: "pushed", savedAt: NOW })).toEqual({ text: "Pushed", chip: "pushed" });
    expect(accountChip({ ...a, phase: "error" })).toEqual({ text: "Error", chip: "error" });
  });
});
