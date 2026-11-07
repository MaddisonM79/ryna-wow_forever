import { describe, expect, it } from "vitest";
import { headerState, type HeaderStatus } from "./describe";

const NOW = 1_762_500_000;

function at(over: Partial<HeaderStatus>): HeaderStatus {
  return {
    signedIn: true,
    watching: true,
    lastRun: { at: NOW - 60, ok: true, error: null, fix: null, advice: null },
    lastError: null,
    accounts: [{ phase: "pushed" }, { phase: "waiting" }],
    ...over,
  };
}

describe("the header's chip", () => {
  it("is Connected (green) when signed in, watching, and the last run was fine", () => {
    expect(headerState(at({}))).toEqual({ state: "connected", text: "Connected", detail: null, chip: "pushed" });
    expect(headerState(at({ lastRun: null })).state).toBe("connected");
  });

  it("is Syncing… (blue) while an account is being pushed", () => {
    expect(headerState(at({ accounts: [{ phase: "pushing" }] }))).toEqual({
      state: "syncing",
      text: "Syncing…",
      detail: null,
      chip: "pushing",
    });
  });

  it("is Problem (red) with the fix words when the last run failed", () => {
    const run = { at: NOW - 60, ok: false, error: "sign-in provider unreachable: dns", fix: "network" as const, advice: "Couldn't reach the API" };
    expect(headerState(at({ lastRun: run }))).toEqual({
      state: "problem",
      text: "Problem",
      detail: "Couldn't reach the API",
      chip: "error",
    });
    expect(headerState(at({ lastRun: { ...run, advice: null } })).detail).toBe("sign-in provider unreachable: dns");
    expect(headerState(at({ lastRun: { ...run, advice: null, error: null } })).detail).toBe("the last sync didn't finish");
  });

  it("is Problem too when this session's loop reported a failure that hasn't cleared", () => {
    expect(headerState(at({ lastError: "Couldn't report in to the API" }))).toMatchObject({
      state: "problem",
      detail: "Couldn't report in to the API",
    });
  });

  it("is Paused (amber) when signed in but not watching", () => {
    expect(headerState(at({ watching: false }))).toEqual({ state: "paused", text: "Paused", detail: null, chip: "update" });
    expect(headerState(at({ watching: false, lastError: "x" })).state).toBe("paused");
  });

  it("is Signed out (grey) without a sign-in, whatever else", () => {
    expect(headerState(at({ signedIn: false, watching: false }))).toEqual({
      state: "signedOut",
      text: "Signed out",
      detail: null,
      chip: "waiting",
    });
    expect(headerState(at({ signedIn: false, lastError: "x" })).state).toBe("signedOut");
  });
});
