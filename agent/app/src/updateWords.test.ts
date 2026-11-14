import { describe, expect, it } from "vitest";
import type { Support, UpdateCheck } from "./api";
import { describe as describeEvent } from "./describe";
import {
  addonSentence,
  banner,
  bannerText,
  checkButton,
  checkLog,
  progressText,
  REDUCED_SENTENCE,
  supportLog,
  UNSUPPORTED_SENTENCE,
  UPDATE_INTERVAL_MS,
} from "./updateWords";

const unset: UpdateCheck = { configured: false, current: "0.2.0", available: null };
const current: UpdateCheck = { configured: true, current: "0.2.0", available: null };
const newer: UpdateCheck = {
  configured: true,
  current: "0.2.0",
  available: { version: "0.3.0", notes: null, date: "2026-12-01T00:00:00Z" },
};

describe("updates", () => {
  it("says a build without a key can't update", () => {
    expect(checkButton(unset, "idle")).toEqual({ label: "Updates aren't set up in this build", disabled: true });
    expect(checkLog(unset, true)).toBe("Updates aren't set up in this build.");
    expect(checkLog(unset, false)).toBeNull();
    expect(bannerText(unset, null)).toBeNull();
  });

  it("offers a newer release until Later puts that version off", () => {
    expect(bannerText(newer, null)).toBe("Ryna 0.3.0 is available");
    expect(bannerText(newer, "0.3.0")).toBeNull();
    expect(bannerText({ ...newer, available: { ...newer.available!, version: "0.3.1" } }, "0.3.0")).toBe(
      "Ryna 0.3.1 is available",
    );
    expect(checkLog(newer, false)).toBe("Ryna 0.3.0 is available (this is 0.2.0).");
  });

  it("logs 'up to date' only when asked", () => {
    expect(checkLog(current, true)).toBe("Ryna 0.2.0 is up to date.");
    expect(checkLog(current, false)).toBeNull();
    expect(bannerText(current, null)).toBeNull();
  });

  it("names the button for what's happening", () => {
    expect(checkButton(null, "idle")).toEqual({ label: "Check for updates", disabled: false });
    expect(checkButton(current, "checking")).toEqual({ label: "Checking…", disabled: true });
    expect(checkButton(newer, "downloading")).toEqual({ label: "Updating…", disabled: true });
    expect(checkButton(current, "error")).toEqual({ label: "Check for updates", disabled: false });
  });

  it("shows download progress", () => {
    expect(progressText(null)).toBe("Downloading…");
    expect(progressText({ downloaded: 3_200_000, total: 12_000_000 })).toBe("Downloading 3.2 of 12.0 MB (26%)");
    expect(progressText({ downloaded: 1_500_000, total: null })).toBe("Downloading 1.5 MB");
  });

  it("checks every 6 hours", () => {
    expect(UPDATE_INTERVAL_MS).toBe(21_600_000);
  });
});

function judged(band: Support["band"], extra: Partial<Support> = {}): Support {
  return {
    band,
    version: "0.2.0",
    addon: "0.2.0",
    channel: "stable",
    minimum: "0.1.5",
    preferred: "0.2.5",
    perfect: "0.3.0",
    note: null,
    update: "https://ryna.io/wow/guide#upkeep",
    ...extra,
  };
}

describe("the banner, by the API's judgment", () => {
  it("is the updater's alone with no judgment, or current, or behind", () => {
    expect(banner(newer, null, null)).toEqual({
      tone: "update",
      text: "Ryna 0.3.0 is available",
      note: null,
      canInstall: true,
      dismissable: true,
      key: "0.3.0",
      guide: null,
      addon: null,
    });
    expect(banner(newer, null, judged("current"))?.tone).toBe("update");
    expect(banner(newer, "0.3.0", judged("behind"))).toBeNull();
    expect(banner(current, null, judged("behind"))).toBeNull();
    expect(banner(null, null, null)).toBeNull();
  });

  it("puts the note under the behind line", () => {
    expect(banner(newer, null, judged("behind", { note: "0.2.0 mis-reads auction prices" }))?.note).toBe(
      "0.2.0 mis-reads auction prices",
    );
    // A note means nothing to someone who is current.
    expect(banner(newer, null, judged("current", { note: "x" }))?.note).toBeNull();
  });

  it("says reduced in amber, and Later puts it off until something changes", () => {
    const b = banner(newer, null, judged("reduced", { note: "0.2.0 mis-reads auction prices" }))!;
    expect(b).toEqual({
      tone: "reduced",
      text: REDUCED_SENTENCE,
      note: "0.2.0 mis-reads auction prices",
      canInstall: true,
      dismissable: true,
      key: "reduced:0.3.0",
      guide: null,
      addon: null,
    });
    expect(banner(newer, b.key, judged("reduced"))).toBeNull();
    // A newer release brings it back.
    expect(
      banner({ ...newer, available: { ...newer.available!, version: "0.3.1" } }, b.key, judged("reduced"))?.tone,
    ).toBe("reduced");
  });

  it("says unsupported in red, and Later can't put it off", () => {
    const b = banner(newer, "unsupported:0.3.0", judged("unsupported"))!;
    expect(b.tone).toBe("unsupported");
    expect(b.text).toBe(UNSUPPORTED_SENTENCE);
    expect(b.dismissable).toBe(false);
    expect(b.canInstall).toBe(true);
  });

  it("points at the update steps when the updater has nothing to install", () => {
    const b = banner(unset, null, judged("unsupported"))!;
    expect(b.canInstall).toBe(false);
    expect(b.guide).toBe("https://ryna.io/wow/guide#upkeep");
    // Only a link the app may open.
    expect(banner(current, null, judged("reduced", { update: "https://example.test/x" }))?.guide).toBeNull();
    expect(banner(current, null, judged("reduced", { update: null }))?.guide).toBeNull();
  });

  // mcarnes, 2026-11-13: the app at 0.4.0 said "this version is no longer supported" and "Up to
  // date" at once, because the addon in the game folder was still 0.3.6.
  const oldInGame = { version: "0.4.0", addon: "0.3.6", minimum: "0.3.9", preferred: null, perfect: "0.4.0" };
  const older = { installs: [{ addon: "older" as const }], bundled: "0.4.0" };

  it("names the addon in the game folder when that alone is below the mark, and offers to install this app's", () => {
    const b = banner(current, null, judged("unsupported", oldInGame), older)!;
    expect(b.tone).toBe("unsupported");
    expect(b.text).toBe(
      "The Ryna addon in your game folder is 0.3.6, which is no longer supported, so nothing syncs to ryna.io. Update the addon to carry on.",
    );
    expect(b.addon).toEqual({ installed: "0.3.6", to: "0.4.0", action: "install" });
    expect(b.canInstall).toBe(false);
    expect(b.dismissable).toBe(false);
    // Even with a newer app on offer, the addon is what stops the sync.
    expect(banner(newer, null, judged("unsupported", oldInGame), older)?.addon?.action).toBe("install");
    expect(supportLog(judged("unsupported", oldInGame), true)).toBe(addonSentence("unsupported", "0.3.6"));
  });

  it("says the same in amber for reduced, and Later holds until the addon changes", () => {
    const reduced = { ...oldInGame, minimum: null, preferred: "0.3.9" };
    const b = banner(current, null, judged("reduced", reduced), older)!;
    expect(b.tone).toBe("reduced");
    expect(b.text).toBe(addonSentence("reduced", "0.3.6"));
    expect(b.key).toBe("reduced:addon:0.3.6");
    expect(banner(current, b.key, judged("reduced", reduced), older)).toBeNull();
    expect(banner(current, b.key, judged("reduced", { ...reduced, addon: "0.3.7" }), older)?.tone).toBe("reduced");
  });

  it("asks the API again when the game folder already holds this app's addon", () => {
    const b = banner(current, null, judged("unsupported", oldInGame), { installs: [{ addon: "current" }], bundled: "0.4.0" })!;
    expect(b.addon).toEqual({ installed: "0.3.6", to: "0.4.0", action: "sync" });
  });

  it("stays about the app when the app is below the mark too, or its addon wouldn't clear it", () => {
    const both = judged("unsupported", { ...oldInGame, version: "0.3.4" });
    expect(banner(newer, null, both, older)).toMatchObject({ text: UNSUPPORTED_SENTENCE, addon: null, canInstall: true });
    const carriesOld = banner(current, null, judged("unsupported", oldInGame), { ...older, bundled: "0.3.8" })!;
    expect(carriesOld).toMatchObject({ text: UNSUPPORTED_SENTENCE, addon: null, guide: "https://ryna.io/wow/guide#upkeep" });
    expect(banner(current, null, judged("unsupported", oldInGame), { ...older, bundled: null })?.addon).toBeNull();
    // No addon judged, or nothing known about the game folders: as before.
    expect(banner(current, null, judged("unsupported", { ...oldInGame, addon: null }), older)?.addon).toBeNull();
    expect(banner(current, null, judged("unsupported", oldInGame))?.addon).toBeNull();
  });

  it("logs a band only when it moves to reduced or unsupported", () => {
    expect(supportLog(judged("reduced"), true)).toBe(REDUCED_SENTENCE);
    expect(supportLog(judged("unsupported", { note: "n" }), true)).toBe(`${UNSUPPORTED_SENTENCE} (n)`);
    expect(supportLog(judged("unsupported"), false)).toBeNull();
    expect(supportLog(judged("behind"), true)).toBeNull();
    expect(describeEvent({ event: "support", support: judged("reduced"), receivedAt: 1, changed: true })).toBe(
      REDUCED_SENTENCE,
    );
    expect(describeEvent({ event: "contributionsPaused", label: "UNT1" })).toBe(
      "UNT1: synced. What you add to everyone's census and prices is paused until you update.",
    );
  });

  it("uses the API's exact words", () => {
    expect(REDUCED_SENTENCE).toBe(
      "This version of Ryna is getting old. Your characters still sync, but what you add to everyone's census and prices is paused until you update.",
    );
    expect(UNSUPPORTED_SENTENCE).toBe(
      "This version of Ryna is no longer supported, so nothing syncs to ryna.io. Update to carry on.",
    );
  });
});
