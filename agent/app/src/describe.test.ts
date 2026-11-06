import { describe as group, expect, it } from "vitest";
import { ago, describe } from "./describe";

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

  it("says how long ago", () => {
    expect(ago(null)).toBe("never");
    expect(ago(1000, 1010)).toBe("just now");
    expect(ago(1000, 1000 + 180)).toBe("3 minutes ago");
    expect(ago(1000, 1000 + 7200)).toBe("2 hours ago");
  });
});
