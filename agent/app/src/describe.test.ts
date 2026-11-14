import { describe as group, expect, it } from "vitest";
import type { AddonState, InstallView } from "./api";
import { aboutAddon, addonRow, ago, describe, installedLines, installedMessage, signInNote } from "./describe";

function install(addon: AddonState, addonVersion: string | null): InstallView {
  return { path: "/wow/_forever_", flavor: "wowf", forever: true, hasAddon: addon !== "notInstalled", addonVersion, addon };
}

group("the activity log's words", () => {
  it("counts a push's characters and journal", () => {
    expect(
      describe({
        event: "pushed",
        label: "WoW1",
        path: "/x/Ryna.lua",
        characters: 2,
        result: { snapshotId: 7, journal: { stored: 131, repeated: 12 } },
      }),
    ).toBe("Pushed WoW1: 2 characters, 143 journal events, 12 repeated");
    expect(
      describe({ event: "pushed", label: "WoW1", path: "/x", characters: 1, result: {} }),
    ).toBe("Pushed WoW1: 1 character");
  });

  it("shows the core's words for an error", () => {
    expect(
      describe({
        event: "error",
        label: null,
        path: null,
        message: "sign-in provider unreachable: dns",
        fix: "network",
        advice: "Couldn't reach the API",
      }),
    ).toBe("Couldn't reach the API");
    expect(
      describe({ event: "error", label: "WoW1", path: "/x", message: "bad file", fix: "other", advice: "bad file" }),
    ).toBe("WoW1: bad file");
  });

  it("skips the routine events", () => {
    expect(describe({ event: "signedIn", signedIn: true })).toBeNull();
    expect(describe({ event: "passFinished", ok: true, nextInSecs: 60 })).toBeNull();
    expect(describe({ event: "heartbeat", ok: true, error: null })).toBeNull();
  });

  it("names fights", () => {
    expect(
      describe({
        event: "fightUploaded",
        key: "k",
        kind: "world",
        zone: "Zephras Isle",
        name: null,
        durationMs: 186_674,
        players: 2,
        id: 7,
        merged: false,
      }),
    ).toBe("Fight: Zephras Isle, 3 min, 2 players: uploaded");
    expect(
      describe({
        event: "fightFailed",
        key: "k",
        fight: "Onyxia, 45 s, 1 player",
        message: "the API is unreachable",
        fix: "network",
        advice: "Couldn't reach the API",
      }),
    ).toBe("Fight: Onyxia, 45 s, 1 player: Couldn't reach the API");
    expect(
      describe({ event: "fightFailed", key: "", fight: "Fights", message: "x", fix: "signIn", advice: "Sign in again" }),
    ).toBe("Fights: Sign in again");
    expect(describe({ event: "combatLogFound", file: "WoWCombatLog-1.txt", path: "/x" })).toBe(
      "Reading the combat log WoWCombatLog-1.txt",
    );
    expect(
      describe({ event: "fightQueued", key: "k", kind: "trash", zone: "Z", name: null, durationMs: 1, players: 1 }),
    ).toBeNull();
  });

  it("says how long ago", () => {
    expect(ago(null)).toBe("never");
    expect(ago(1000, 1010)).toBe("just now");
    expect(ago(1000, 1000 + 180)).toBe("3 minutes ago");
    expect(ago(1000, 1000 + 7200)).toBe("2 hours ago");
  });
});

group("the addon's row", () => {
  it("offers an install when it isn't there", () => {
    expect(addonRow(install("notInstalled", null), "0.1.0")).toEqual({
      text: "Addon not installed",
      chip: "waiting",
      action: "Install addon 0.1.0",
      primary: true,
    });
  });

  it("offers a reinstall when it's current", () => {
    expect(addonRow(install("current", "0.1.0"), "0.1.0")).toEqual({
      text: "Addon 0.1.0 installed",
      chip: "pushed",
      action: "Reinstall",
      primary: false,
    });
  });

  it("offers an update when it's older", () => {
    expect(addonRow(install("older", "0.1.0"), "0.2.0")).toEqual({
      text: "Addon 0.1.0 installed, 0.2.0 available",
      chip: "update",
      action: "Update to 0.2.0",
      primary: true,
    });
  });

  it("says so when it's newer than the bundled one", () => {
    expect(addonRow(install("newer", "0.3.0"), "0.2.0")).toEqual({
      text: "Addon 0.3.0 installed (this app bundles 0.2.0)",
      chip: "pushed",
      action: "Reinstall 0.2.0",
      primary: false,
    });
  });

  it("treats an unversioned copy as needing the update, and offers nothing without a bundle", () => {
    expect(addonRow(install("unknown", null), "0.2.0")).toMatchObject({
      text: "Addon installed (version unknown), 0.2.0 available",
      action: "Update to 0.2.0",
    });
    expect(addonRow(install("notInstalled", null), null).action).toBeNull();
    expect(addonRow(install("unknown", "0.1.0"), null)).toMatchObject({ text: "Addon 0.1.0 installed", action: null });
  });

  it("says what went in and how the game loads it", () => {
    const done = {
      version: "0.2.0",
      installed: [{ install: "/wow/_forever_", path: "/wow/_forever_/Interface/AddOns/Ryna" }],
      mods: [],
      gameRunning: false,
    };
    expect(installedLines(done)).toEqual(["Installed addon 0.2.0 into /wow/_forever_/Interface/AddOns/Ryna"]);
    expect(installedMessage(done)).toEqual(["Installed 0.2.0. The game loads it at the next login or /reload."]);
    expect(installedMessage({ ...done, gameRunning: true })[1]).toBe(
      "The game is running: /reload or log out and in to load it.",
    );
  });
});

group("the sign-in note", () => {
  it("warns only when a kept sign-in won't outlive a restart", () => {
    expect(signInNote({ signedIn: true, signInUntilReboot: true })).toMatch(/until the computer restarts/);
    expect(signInNote({ signedIn: true, signInUntilReboot: true })).toMatch(/gnome-keyring or KeePassXC/);
    expect(signInNote({ signedIn: true, signInUntilReboot: false })).toBeNull();
    expect(signInNote({ signedIn: false, signInUntilReboot: true })).toBeNull();
  });
});

group("About's addon line", () => {
  const install = (addonVersion: string | null, hasAddon = true) => ({ hasAddon, addonVersion });

  it("is this app's version when the game folder holds the same, or no addon", () => {
    expect(aboutAddon([install("0.4.0")], "0.4.0")).toEqual({ text: "0.4.0", old: false });
    expect(aboutAddon([install(null, false)], "0.4.0")).toEqual({ text: "0.4.0", old: false });
    expect(aboutAddon([], null)).toEqual({ text: "None in this build", old: false });
  });

  it("shows the game folder's copy beside the app's when they differ, the oldest of several", () => {
    expect(aboutAddon([install("0.4.0"), install("0.3.6")], "0.4.0")).toEqual({
      text: "0.3.6 in the game folder · 0.4.0 with this app",
      old: true,
    });
    expect(aboutAddon([install("0.10.0")], "0.9.1").old).toBe(false);
    expect(aboutAddon([install("0.3.6")], null).text).toBe("0.3.6 in the game folder · none in this build");
  });
});
