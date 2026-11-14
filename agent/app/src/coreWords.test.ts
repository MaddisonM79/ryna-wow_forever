import { describe, expect, it } from "vitest";
import type { CoreState, CoreView } from "./api";
import { coreLines, corePanel, describeCoreEvent, megabytes, platformWords, runningCores, sidecarProblem } from "./coreWords";
import { headerState, type HeaderStatus } from "./describe";
import { pages } from "./tabs";

function core(state: CoreState, over: Partial<CoreView> = {}): CoreView {
  return {
    id: "palworld",
    name: "Palworld",
    version: "0.5.0",
    kind: "sidecar",
    rows: [],
    state,
    on: true,
    platforms: ["macos"],
    lastError: null,
    ...over,
  };
}

const wow: CoreView = {
  ...core({ is: "running" }),
  id: "wow",
  name: "World of Warcraft: Forever",
  kind: "builtin",
  platforms: [],
};

describe("a fetched core's page", () => {
  it("has no panel while it runs", () => {
    expect(corePanel(core({ is: "running" }))).toBeNull();
  });

  it("offers Allow for a core the sign-in doesn't cover yet", () => {
    expect(corePanel(core({ is: "allow" }))).toEqual({
      state: "allow",
      tone: "update",
      title: "Allow Palworld on this computer",
      text: "Your ryna.io account has Palworld. Allowing it is one approval in your browser, for every game your account has. Nothing of Palworld is downloaded or runs until then.",
      progress: null,
      action: { label: "Allow Palworld on this computer", does: "allow" },
    });
  });

  it("says Getting… with the download's progress", () => {
    const mb = 1024 * 1024;
    expect(corePanel(core({ is: "fetching", done: 12 * mb, total: 48 * mb }))).toMatchObject({
      title: "Getting Palworld…",
      text: "12.0 MB of 48.0 MB",
      progress: 0.25,
      action: null,
    });
    expect(corePanel(core({ is: "fetching", done: 3 * mb, total: null }))).toMatchObject({ text: "3.0 MB so far", progress: null });
    expect(corePanel(core({ is: "fetching", done: 0, total: null }))?.text).toBe("Starting the download");
  });

  it("offers Retry when the fetch failed", () => {
    expect(corePanel(core({ is: "fetchFailed", message: "couldn't download x: HTTP 404" }))).toMatchObject({
      tone: "error",
      title: "Couldn't get Palworld",
      text: "Couldn't download x: HTTP 404. Ryna tries again every hour.",
      action: { label: "Retry", does: "retry" },
    });
  });

  it("says where a core runs when this computer can't run it", () => {
    expect(corePanel(core({ is: "notHere", platforms: ["macos"] }))).toMatchObject({
      title: "Palworld isn't on this computer yet",
      text: "Your account has Palworld, but for now it runs on macOS only.",
      action: null,
    });
    expect(platformWords(["macos", "windows", "linux"])).toBe("macOS, Windows and Linux");
  });

  it("names a stop's last words and offers to start again", () => {
    expect(corePanel(core({ is: "stopped", message: "this core needs a newer Ryna", fix: null }))).toMatchObject({
      tone: "error",
      title: "Palworld stopped",
      text: "This core needs a newer Ryna.",
      action: { label: "Start again", does: "start" },
    });
    expect(corePanel(core({ is: "stopped", message: "no saves", fix: "open the game once" }))?.text).toBe(
      "No saves. Open the game once.",
    );
    expect(corePanel(core({ is: "off" }))?.title).toBe("Palworld is off on this computer");
  });

  it("shows only its own activity lines", () => {
    const lines = [{ core: "palworld", text: "a" }, { text: "wow" }, { core: "satisfactory", text: "b" }];
    expect(coreLines(lines, "palworld")).toEqual([{ core: "palworld", text: "a" }]);
  });

  it("is a sidebar row after WoW only when provisioned", () => {
    const ids = (cores: CoreView[]) => pages(cores).map((p) => p.id);
    expect(ids([wow])).toEqual(["status", "core:wow", "settings", "activity", "about"]);
    expect(ids([wow, core({ is: "allow" })])).toEqual(["status", "core:wow", "core:palworld", "settings", "activity", "about"]);
  });
});

describe("a fetched core's activity lines", () => {
  it("name the core", () => {
    expect(describeCoreEvent("Palworld", { event: "started", version: "0.5.0" })).toEqual({
      text: "Palworld started (version 0.5.0).",
      level: "info",
    });
    expect(describeCoreEvent("Palworld", { event: "event", kind: "pushed", row: "w1", message: "World 1 pushed" })).toEqual({
      text: "Palworld: World 1 pushed",
      level: "info",
    });
    expect(describeCoreEvent("Palworld", { event: "event", kind: "error", row: null, message: "403: no grant" })?.level).toBe("error");
    expect(describeCoreEvent("Palworld", { event: "status", rows: [] })).toBeNull();
    expect(describeCoreEvent("Palworld", { event: "passFinished", ok: true })).toBeNull();
    expect(describeCoreEvent("Palworld", { event: "stopped", message: "this core needs a newer Ryna", fix: null })?.text).toBe(
      "Palworld stopped: this core needs a newer Ryna",
    );
    expect(describeCoreEvent("Palworld", { event: "exited", message: "it exited (exit status: 3)" })?.text).toContain(
      "Starting it again within a minute",
    );
    expect(megabytes(1536 * 1024)).toBe("1.5 MB");
  });
});

describe("the header and Status count every running core", () => {
  const base: HeaderStatus = {
    signedIn: true,
    watching: true,
    lastRun: null,
    lastError: null,
    deviceKeyError: null,
    accounts: [],
  };

  it("counts WoW's loop and each running fetched core", () => {
    expect(runningCores({ watching: true, cores: [wow, core({ is: "running" })] })).toBe(2);
    expect(runningCores({ watching: false, cores: [wow, core({ is: "running" })] })).toBe(1);
    expect(runningCores({ watching: true, cores: [wow, core({ is: "allow" })] })).toBe(1);
  });

  it("isn't Paused while a fetched core runs", () => {
    expect(headerState({ ...base, watching: false, cores: [wow, core({ is: "running" })] }).state).toBe("connected");
    expect(headerState({ ...base, watching: false, cores: [wow, core({ is: "allow" })] }).state).toBe("paused");
  });

  it("names the core an error belongs to", () => {
    expect(
      headerState({ ...base, cores: [wow, core({ is: "stopped", message: "this core needs a newer Ryna", fix: null })] }),
    ).toEqual({ state: "problem", text: "Problem", detail: "Palworld: this core needs a newer Ryna", chip: "error" });
    expect(headerState({ ...base, lastError: "Couldn't reach the API", cores: [wow, core({ is: "running" })] }).detail).toBe(
      "World of Warcraft: Forever: Couldn't reach the API",
    );
    // WoW alone: the words as before.
    expect(headerState({ ...base, lastError: "Couldn't reach the API", cores: [wow] }).detail).toBe("Couldn't reach the API");
    expect(sidecarProblem([core({ is: "running" }, { lastError: "403" })])).toBe("Palworld: 403");
    expect(sidecarProblem([core({ is: "allow" })])).toBeNull();
  });
});
