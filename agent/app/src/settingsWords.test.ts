import { describe as group, expect, it } from "vitest";
import type { CopyPlan, InventoryView, ModView, RestoreReport, Status } from "./api";
import { modAction, modRow } from "./describe";
import {
  backupDoneLine,
  backupWhen,
  charName,
  copyableAddons,
  copyableGameFiles,
  copyDoneLine,
  copyPreviewLines,
  diffText,
  gameBanner,
  GAME_RUNNING,
  progressText,
  restoreDoneLine,
  restorePreviewLines,
  scopeText,
  settingsModOn,
  sizeText,
  STORED_ENCRYPTED,
} from "./settingsWords";

const aerin = { name: "Aerin", realm: "Stormcall" };
const bryn = { name: "Bryn", realm: "Doomhowl" };

function settingsMod(on: boolean): ModView {
  return {
    id: "settings",
    name: "Settings",
    summary: "Backs up your addons' settings.",
    kind: "agent",
    actsInGame: false,
    defaultOn: false,
    on,
    bundled: "0.3.7",
    addons: [],
    installs: [],
  };
}

const inv: InventoryView = {
  install: "/wow/_forever_",
  flavor: "wowf",
  account: "ACCOUNT1",
  accountKey: "0123456789abcdef",
  characters: [aerin, bryn],
  characterFolders: [aerin, bryn],
  addons: [
    {
      addon: "ElvUI",
      title: "ElvUI",
      installed: true,
      accountVariables: ["ElvDB"],
      characterVariables: [],
      scope: "both",
      files: [],
      characters: [aerin, bryn],
      acedb: true,
      bytes: 2048,
      modified: 1,
    },
    {
      addon: "Details",
      title: null,
      installed: true,
      accountVariables: [],
      characterVariables: ["DetailsCharDB"],
      scope: "character",
      files: [],
      characters: [aerin],
      acedb: false,
      bytes: 10,
      modified: 1,
    },
  ],
  gameFiles: [
    { path: "Stormcall/Aerin/config-cache.wtf", character: aerin, bytes: 5, modified: 1 },
    { path: "Stormcall/Aerin/AddOns.txt", character: aerin, bytes: 5, modified: 1 },
    { path: "Doomhowl/Bryn/config-cache.wtf", character: bryn, bytes: 5, modified: 1 },
  ],
};

group("the Settings mod", () => {
  it("is an agent mod: on or off, nothing to install", () => {
    expect(modRow(settingsMod(true), "/wow/_forever_")).toEqual({ text: "On · runs in the app", chip: "pushed" });
    expect(modRow(settingsMod(false), "/wow/_forever_")).toEqual({ text: "Off", chip: "waiting" });
    expect(modAction(settingsMod(true), "/wow/_forever_")).toBeNull();
    expect(settingsModOn({ mods: [settingsMod(true)] } as unknown as Status)).toBe(true);
    expect(settingsModOn({ mods: [settingsMod(false)] } as unknown as Status)).toBe(false);
    expect(settingsModOn(null)).toBe(false);
  });

  it("greys writes out while the game runs, or while it can't be told", () => {
    expect(gameBanner({ running: false, unknown: null })).toBeNull();
    expect(gameBanner(null)).toBeNull();
    expect(gameBanner({ running: true, unknown: null })).toBe(GAME_RUNNING);
    expect(gameBanner({ running: true, unknown: "pgrep failed" })).toBe(
      "Ryna can't tell whether the game is running (pgrep failed), so copies and restores are off until it can.",
    );
  });

  it("words sizes, scopes and characters", () => {
    expect(sizeText(512)).toBe("512 B");
    expect(sizeText(2048)).toBe("2.0 KB");
    expect(sizeText(3 * 1024 * 1024)).toBe("3.0 MB");
    expect(scopeText("account")).toBe("Account");
    expect(scopeText("character")).toBe("Per character");
    expect(scopeText("both")).toBe("Account and per character");
    expect(charName(aerin)).toBe("Aerin-Stormcall");
  });

  it("ticks the addons and game files the source has", () => {
    expect(copyableAddons(inv, aerin).map((a) => a.addon)).toEqual(["ElvUI", "Details"]);
    expect(copyableAddons(inv, { name: "bryn", realm: "doomhowl" }).map((a) => a.addon)).toEqual(["ElvUI"]);
    expect(copyableGameFiles(inv, aerin)).toEqual(["config-cache.wtf", "AddOns.txt"]);
    expect(copyableGameFiles(inv, bryn)).toEqual(["config-cache.wtf"]);
  });

  it("previews every file a copy writes, the .ryna.bak, and what it leaves", () => {
    const plan: CopyPlan = {
      from: aerin,
      to: bryn,
      newFolder: false,
      writes: [
        { item: "Details", path: "Doomhowl/Bryn/SavedVariables/Details.lua", replaces: false, bytes: 10, how: "file", from: "Stormcall/Aerin/SavedVariables/Details.lua" },
        { item: "ElvUI", path: "SavedVariables/ElvUI.lua", replaces: true, bytes: 99, how: "aceDb", variables: ["ElvDB"] },
      ],
      skipped: [{ item: "Plain", reason: "its settings are account-wide: every character on this account already shares them" }],
    };
    expect(copyPreviewLines(plan)).toEqual([
      "Doomhowl/Bryn/SavedVariables/Details.lua (Details: copied from Aerin-Stormcall)",
      "SavedVariables/ElvUI.lua (ElvUI: Bryn-Doomhowl's entries made from Aerin-Stormcall's; the file there is kept as ElvUI.lua.ryna.bak)",
      "Left Plain: its settings are account-wide: every character on this account already shares them.",
    ]);
    expect(copyPreviewLines({ ...plan, newFolder: true, writes: [], skipped: [] })[0]).toContain("has no folder yet");
    expect(copyDoneLine(plan, 2, 1)).toBe(
      "Copied Aerin-Stormcall's settings to Bryn-Doomhowl: 2 files written, 1 replaced file kept as .ryna.bak.",
    );
    expect(copyDoneLine(plan, 1, 0)).toBe("Copied Aerin-Stormcall's settings to Bryn-Doomhowl: 1 file written.");
  });

  it("says how two characters differ", () => {
    expect(diffText({ item: "ElvUI", gameFile: false, difference: "same" }, aerin, bryn)).toBe("Same");
    expect(diffText({ item: "ElvUI", gameFile: false, difference: "differ" }, aerin, bryn)).toBe("Different");
    expect(diffText({ item: "ElvUI", gameFile: false, difference: "onlyA" }, aerin, bryn)).toBe("Only Aerin-Stormcall");
    expect(diffText({ item: "ElvUI", gameFile: false, difference: "onlyB" }, aerin, bryn)).toBe("Only Bryn-Doomhowl");
  });

  it("says where a backup goes, in one sentence, and how it's getting on", () => {
    expect(STORED_ENCRYPTED).toBe("Stored encrypted for you on ryna.io.");
    expect(backupDoneLine({ id: "x", createdAt: 1, bytes: 2048, files: 3, parts: 1 })).toBe(
      "Backed up 3 files (2.0 KB). Stored encrypted for you on ryna.io.",
    );
    expect(progressText({ stage: "uploading", done: 1, total: 3 })).toBe("Uploading part 2 of 3…");
    expect(progressText({ stage: "uploading", done: 0, total: 1 })).toBe("Uploading…");
    expect(progressText({ stage: "writing", done: 0, total: 2 })).toBe("Writing file 1 of 2…");
    expect(backupWhen(null)).toBe("");
    expect(backupWhen(1_800_000_000)).toBe(new Date(1_800_000_000_000).toLocaleString());
    expect(backupWhen(1_800_000_000_000)).toBe(new Date(1_800_000_000_000).toLocaleString());
    expect(backupWhen("2026-11-08T10:00:00Z")).toBe(new Date("2026-11-08T10:00:00Z").toLocaleString());
  });

  it("previews and reports a restore", () => {
    const r: RestoreReport = {
      id: "b1",
      account: "ACCOUNT1",
      files: [
        { path: "SavedVariables/ElvUI.lua", bytes: 2048, replaces: true },
        { path: "Stormcall/Aerin/macros-cache.txt", bytes: 10, replaces: false },
      ],
      written: true,
      backups: ["SavedVariables/ElvUI.lua.ryna.bak"],
    };
    expect(restorePreviewLines(r)).toEqual([
      "SavedVariables/ElvUI.lua (2.0 KB); the file there is kept as ElvUI.lua.ryna.bak",
      "Stormcall/Aerin/macros-cache.txt (10 B)",
    ]);
    expect(restoreDoneLine(r)).toBe("Restored 2 files into ACCOUNT1, 1 kept as .ryna.bak.");
  });
});
