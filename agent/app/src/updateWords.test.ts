import { describe, expect, it } from "vitest";
import type { UpdateCheck } from "./api";
import { bannerText, checkButton, checkLog, progressText, UPDATE_INTERVAL_MS } from "./updateWords";

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
