import { describe as group, expect, it } from "vitest";
import type { ModView } from "./api";
import { ACTS_IN_GAME, compareVersions, installedLines, modChangeLines, modRow, modToggledLines } from "./describe";

const AT = "/wow/_forever_";

function mod(on: boolean, installed: boolean, version: string | null, offInGame = 0, actsInGame = false): ModView {
  return {
    id: "chronicle",
    name: "Chronicle",
    summary: "Records your characters.",
    actsInGame,
    defaultOn: true,
    on,
    bundled: "0.3.0",
    addons: ["Ryna_Chronicle"],
    installs: [{ install: AT, installed, version, offInGame }],
  };
}

group("a mod's row in the Game tab", () => {
  it("says it's on and what's installed", () => {
    expect(modRow(mod(true, true, "0.3.0"), AT)).toEqual({ text: "On · 0.3.0 installed", chip: "pushed" });
    expect(modRow(mod(true, true, "0.2.6"), AT)).toEqual({
      text: "On · 0.2.6 installed, 0.3.0 available",
      chip: "update",
    });
    expect(modRow(mod(true, false, null), AT)).toEqual({
      text: "On · not installed yet: install the addon",
      chip: "waiting",
    });
  });

  it("says it's off, and when the folder is still there", () => {
    expect(modRow(mod(false, false, null), AT)).toEqual({ text: "Off", chip: "waiting" });
    expect(modRow(mod(false, true, "0.3.0"), AT).text).toBe("Off · still installed");
  });

  it("says when the game has it off, as the in-game toggle left it", () => {
    expect(modRow(mod(true, true, "0.3.0", 1), AT)).toEqual({
      text: "On · 0.3.0 installed · off in the game for 1 character",
      chip: "waiting",
    });
    expect(modRow(mod(true, true, "0.3.0", 3), AT).text).toMatch(/for 3 characters$/);
    expect(modRow(mod(true, true, "0.3.0"), "/elsewhere").text).toBe("On · not installed yet: install the addon");
  });

  it("has the sentence for a mod that acts in the game", () => {
    expect(ACTS_IN_GAME).toBe("This mod acts in the game for you.");
  });

  it("compares versions as the core does", () => {
    expect(compareVersions("0.10.0", "0.9.1")).toBe(1);
    expect(compareVersions("0.3", "0.3.0")).toBe(0);
    expect(compareVersions("0.2.6", "0.3.0")).toBe(-1);
  });
});

group("what a toggle and an install say", () => {
  it("names each folder that went in or came out", () => {
    expect(
      modChangeLines([
        { change: "installed", id: "chronicle", name: "Chronicle", folder: "Ryna_Chronicle", path: `${AT}/Interface/AddOns/Ryna_Chronicle`, version: "0.3.0" },
        { change: "removed", id: "chronicle", name: "Chronicle", folder: "Ryna_Chronicle", path: `${AT}/Interface/AddOns/Ryna_Chronicle` },
      ]),
    ).toEqual([
      `Installed the Chronicle mod 0.3.0 into ${AT}/Interface/AddOns/Ryna_Chronicle`,
      `Removed the Chronicle mod from ${AT}/Interface/AddOns/Ryna_Chronicle`,
    ]);
  });

  it("says on or off, what changed, what waits for the addon and how the game loads it", () => {
    const off = modToggledLines({
      id: "chronicle",
      name: "Chronicle",
      on: false,
      changes: [{ change: "removed", id: "chronicle", name: "Chronicle", folder: "Ryna_Chronicle", path: "/p" }],
      waitingForAddon: [],
      gameRunning: true,
    });
    expect(off).toEqual([
      "Chronicle: off.",
      "Removed the Chronicle mod from /p",
      "The game is running: /reload or log out and in to load the change.",
    ]);
    const on = modToggledLines({ id: "chronicle", name: "Chronicle", on: true, changes: [], waitingForAddon: [AT], gameRunning: false });
    expect(on).toEqual(["Chronicle: on.", `Chronicle goes into ${AT} with the addon: install the addon there.`]);
  });

  it("lists the mods an addon install brought with it", () => {
    expect(
      installedLines({
        version: "0.3.0",
        installed: [{ install: AT, path: `${AT}/Interface/AddOns/Ryna` }],
        mods: [{ change: "installed", id: "chronicle", name: "Chronicle", folder: "Ryna_Chronicle", path: `${AT}/Interface/AddOns/Ryna_Chronicle`, version: "0.3.0" }],
        gameRunning: false,
      }),
    ).toEqual([
      `Installed addon 0.3.0 into ${AT}/Interface/AddOns/Ryna`,
      `Installed the Chronicle mod 0.3.0 into ${AT}/Interface/AddOns/Ryna_Chronicle`,
    ]);
  });
});
