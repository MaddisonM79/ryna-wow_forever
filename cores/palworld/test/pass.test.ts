/** One pass, the push and the heartbeat, with the disk and the API stood in for. Ported from the launchd agent's run tests. */

import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, unlinkSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthError, PushError, heartbeat, multipart, push, type Api, type HeartbeatBody } from "../src/agent/push.js";
import type { RawFiles } from "../src/agent/snapshot-world.js";
import { snapshotWorld } from "../src/agent/snapshot-world.js";
import * as status from "../src/agent/status.js";
import { snapshotArgs } from "../src/cli.js";
import { ago, defaults, pass, rows, type CoreEvent, type Deps } from "../src/pass.js";
import { SaveFormatError } from "../src/sav.js";
import type { Dict } from "../src/snapshot/util.js";
import { VERSION } from "../src/version.js";
import { SETTLE_SECONDS, fingerprint, readFiles, saveFiles, settled } from "../src/worlds.js";
import { META, OPTIONS, PLAYER_SAVE, WORLD } from "./fixtures.js";
import { RAW_NAME_RE } from "./wire.js";

const PLAYER = "00000000000000000000000000000001";
const API = "https://api.example.test";
const TOKEN = "at-test-token";
const api = (over: Partial<Api> = {}): Api => ({ url: API, token: TOKEN, userAgent: `ryna-agent/9.9.9 palworld/${VERSION}`, ...over });
// LocalData with a 512 x 512 fog mask explored across its top eighth, so the push carries fog.png.
const LOCAL: Dict = {
  SaveData: {
    WorldMapUISaveDataMap: [
      { key: "MainMap", value: { MaskTextureData: Buffer.concat([Buffer.alloc(512 * 64 * 4, 0), Buffer.from(Array(512 * 448).fill([0, 0, 0, 255]).flat())]).toString("base64") } },
    ],
  },
};

let tmp: string;
let stderr: string[];
beforeEach(() => {
  tmp = mkdtempSync(join(tmpdir(), "palworld-core-"));
  process.env["PALWORLD_APP"] = join(tmp, "Missing.app");
  stderr = [];
  status.setLogSink((line) => void stderr.push(line));
});
afterEach(() => {
  vi.restoreAllMocks();
});
const logged = (): string => stderr.join("");

/** The hand-made world as files on disk; each file's bytes pick the fixture the parser returns. */
const PARSED: Record<string, Dict> = { level: WORLD, meta: META, options: OPTIONS, player: PLAYER_SAVE, local: LOCAL };
const fakeParse = (data: Uint8Array): Dict => PARSED[Buffer.from(data).toString()]!;
const fakeSnapshotWorld = (worldId: string, raw: RawFiles) => snapshotWorld(worldId, raw, fakeParse);

/** A world folder with one Level.sav, found and settled at once, and state kept in tmp. */
function oneWorld(): { world: string; deps: Deps } {
  const world = join(tmp, "saves", "user", "W1");
  mkdirSync(world, { recursive: true });
  writeFileSync(join(world, "Level.sav"), "level");
  const base = defaults({ root: join(tmp, "saves"), statePath: join(tmp, "state.json"), statusPath: join(tmp, "status.json") });
  // The heartbeat is stubbed: no test reaches a real API.
  return { world, deps: { ...base, settled: async (w) => saveFiles(w), heartbeat: async () => ({}), push: async () => { throw new Error("a test pushed without stubbing push"); } } };
}

function savedWorld(): { world: string; deps: Deps } {
  const { world, deps } = oneWorld();
  writeFileSync(join(world, "LevelMeta.sav"), "meta");
  writeFileSync(join(world, "WorldOption.sav"), "options");
  mkdirSync(join(world, "Players"));
  writeFileSync(join(world, "Players", `${PLAYER}.sav`), "player");
  return { world, deps: { ...deps, snapshotWorld: fakeSnapshotWorld } };
}

/** A push that keeps each (snapshot, files) and answers `answer` (default: accepted). */
function keptPushes(answer?: Dict): { pushed: [Dict, RawFiles][]; push: Deps["push"] } {
  const pushed: [Dict, RawFiles][] = [];
  return { pushed, push: async (snap, raw) => (pushed.push([snap, raw]), answer ?? { ok: true, data: {} }) };
}

/** A pass, keeping its events. */
async function run(deps: Deps, force = false, a: Api = api()): Promise<{ ok: boolean; events: CoreEvent[]; rows: ReturnType<typeof rows> }> {
  const events: CoreEvent[] = [];
  const r = await pass({ api: a, force, emit: (e) => void events.push(e) }, deps);
  return { ...r, events };
}

const fetchAnswer = (body: string, contentType: string, statusCode = 200, url = `${API}/v1/palworld/agent/ingest`) => async () => {
  const r = new Response(body, { status: statusCode, headers: { "content-type": contentType } });
  Object.defineProperty(r, "url", { value: url });
  return r;
};

const readJson = (p: string): Dict => JSON.parse(readFileSync(p, "utf8")) as Dict;
const INGESTED: Dict = {
  ok: true,
  data: {
    worldId: "W1", savedAt: "2026-10-09T15:01:16.000Z", duplicate: false, events: 13, alertsOpened: 1,
    summary: { day: 50, playerLevel: 41, palCount: 276, speciesOwned: 82, money: 65784, baseCount: 2, bossesDefeated: 16 },
  },
};

describe("settled", () => {
  it("waits out a fresh save then returns the files", async () => {
    const f = join(tmp, "Level.sav");
    writeFileSync(f, "x");
    utimesSync(f, 1000, 1000);
    const clock = { t: 1005 }; // saved 5 s ago
    const slept: number[] = [];
    const files = await settled(tmp, 4, { now: () => clock.t, sleep: async (s) => void (slept.push(Math.round(s)), (clock.t += s)) });
    expect(files).toEqual([f]);
    expect(slept).toEqual([SETTLE_SECONDS - 5 + 1]);
  });

  it("gives up when the game keeps saving", async () => {
    const f = join(tmp, "Level.sav");
    writeFileSync(f, "x");
    utimesSync(f, 1000, 1000);
    const clock = { t: 1000 };
    const sleep = async (s: number) => {
      clock.t += s;
      utimesSync(f, clock.t, clock.t); // the game saved again meanwhile
    };
    expect(await settled(tmp, 3, { now: () => clock.t, sleep })).toBeNull();
  });

  it("fails with ENOENT when Level.sav went away after the world was listed", async () => {
    writeFileSync(join(tmp, "LevelMeta.sav"), "meta");
    const err = await settled(tmp, 1, { now: () => 1e12, sleep: async () => {} }).catch((e: unknown) => e);
    expect((err as NodeJS.ErrnoException).code).toBe("ENOENT");
  });
});

describe("snapshotArgs", () => {
  it("takes the output path whichever side of the flags it is on", () => {
    expect(snapshotArgs(["out.json", "--world", "0123ABCD"])).toEqual({ out: "out.json", world: "0123ABCD" });
    expect(snapshotArgs(["--world", "0123ABCD", "out.json"])).toEqual({ out: "out.json", world: "0123ABCD" });
    expect(snapshotArgs(["--root", "/x", "out.json"])).toEqual({ out: "out.json", root: "/x" });
    expect(snapshotArgs(["--world", "0123ABCD"])).toEqual({ world: "0123ABCD" });
  });
});

describe("push", () => {
  it("reads the JSON envelope, with the bearer and the User-Agent", async () => {
    const seen: RequestInit[] = [];
    const f = async (_u: unknown, init?: RequestInit) => (seen.push(init!), new Response('{"ok":true,"data":{"duplicate":true}}', { headers: { "content-type": "application/json; charset=utf-8" } }));
    expect(await push({ schema: 2 }, { "Level.sav": Buffer.from("x") }, api({ fetch: f as typeof fetch }))).toEqual({ ok: true, data: { duplicate: true } });
    const h = seen[0]!.headers as Record<string, string>;
    expect([h["Authorization"], h["User-Agent"]]).toEqual([`Bearer ${TOKEN}`, `ryna-agent/9.9.9 palworld/${VERSION}`]);
  });

  it("names a login page instead of crashing on it", async () => {
    const login = "https://login.example.test/";
    await expect(push({ schema: 2 }, { "Level.sav": Buffer.from("x") }, api({ fetch: fetchAnswer("<!doctype html><title>Sign in</title>", "text/html", 200, login) as typeof fetch }))).rejects.toThrow(
      `ingest answered 200 text/html from ${login}: "<!doctype html>`,
    );
  });

  it("refuses JSON it cannot parse, and names an HTTP error", async () => {
    await expect(push({ schema: 2 }, { "Level.sav": Buffer.from("x") }, api({ fetch: fetchAnswer("{not json", "application/json") as typeof fetch }))).rejects.toThrow("ingest answered 200 application/json");
    await expect(push({ schema: 2 }, { "Level.sav": Buffer.from("x") }, api({ fetch: fetchAnswer("nope", "text/plain", 502) as typeof fetch }))).rejects.toThrow(new PushError("ingest answered 502: nope"));
  });

  it("turns a 401 or 403 into an AuthError with the API's words", async () => {
    const body = JSON.stringify({ ok: false, error: { code: "UNAUTHENTICATED", message: "the token has expired" } });
    const err = await push({}, {}, api({ fetch: fetchAnswer(body, "application/json", 401) as typeof fetch })).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(AuthError);
    expect([(err as AuthError).message, (err as AuthError).status]).toEqual(["the token has expired", 401]);
    const bare = await heartbeat({ agent: "a", version: "1", lastRun: {}, worlds: {}, history: [], log: "" }, api({ fetch: fetchAnswer("Forbidden", "text/plain", 403) as typeof fetch })).catch((e: unknown) => e);
    expect([(bare as AuthError).message, bare instanceof AuthError]).toEqual(["heartbeat answered 403", true]);
  });

  it("never puts the token in an error, even when the network fails", async () => {
    const failing = async () => {
      throw new TypeError("fetch failed");
    };
    const err = await push({}, {}, api({ fetch: failing as typeof fetch })).catch((e: unknown) => e);
    expect(String((err as Error).message)).toBe("ingest unreachable: fetch failed");
    expect(String((err as Error).message)).not.toContain(TOKEN);
  });

  it("names its parts as ingest expects", () => {
    const { body, contentType } = multipart({ schema: 2, world: { id: "W1" } }, { "Level.sav": Buffer.from("level"), [`Players/${PLAYER}.sav`]: Buffer.from("player"), "fog.png": Buffer.from([0x89, 0x50]) });
    const boundary = contentType.split("boundary=")[1]!;
    const parts = body.toString("latin1").split(`--${boundary}`).slice(1, -1);
    const heads = parts.map((p) => p.split("\r\n\r\n")[0]!);
    expect(heads.map((h) => /name="(\w+)"/.exec(h)![1])).toEqual(["snapshot", "file", "file", "file"]);
    const names = heads.slice(1).map((h) => /filename="([^"]+)"/.exec(h)![1]!);
    expect(names).toEqual(["Level.sav", `Players/${PLAYER}.sav`, "fog.png"]);
    expect(names.every((n) => RAW_NAME_RE.test(n))).toBe(true);
    expect(heads[0]).toContain("Content-Type: application/json");
    expect(JSON.parse(parts[0]!.split("\r\n\r\n")[1]!.replace(/\r\n$/, ""))).toEqual({ schema: 2, world: { id: "W1" } });
  });
});

describe("pass", () => {
  it("pushes the bytes that were fingerprinted, with events and a row per world", async () => {
    const { world, deps } = savedWorld();
    const { pushed, push: kept } = keptPushes();
    const fp = (raw: RawFiles) => {
      const h = fingerprint(raw);
      writeFileSync(join(world, "Level.sav"), "newer"); // the game saves right after the hash
      return h;
    };
    const r = await run({ ...deps, push: kept, fingerprint: fp });
    expect(r.ok).toBe(true);
    const [snap, raw] = pushed[0]!;
    expect(Object.fromEntries(Object.entries(raw).map(([k, v]) => [k, Buffer.from(v).toString()] as [string, string]))).toEqual({ "Level.sav": "level", "LevelMeta.sav": "meta", "WorldOption.sav": "options", [`Players/${PLAYER}.sav`]: "player" });
    expect([(snap["world"] as Dict)["name"], ((snap["players"] as Dict[])[0] as Dict)["name"]]).toEqual(["Test Islands", "Tamer"]);
    expect((readJson(join(tmp, "state.json")) as Record<string, Dict>)["W1"]!["fingerprint"]).toBe(fingerprint(raw));
    expect(r.events).toEqual([
      { kind: "pushing", row: "W1", message: "Test Islands: pushing" },
      { kind: "pushed", row: "W1", message: "Test Islands pushed" },
    ]);
    expect(r.rows).toEqual([{ label: "Test Islands", value: expect.stringMatching(/^saved .*, pushed$/), chip: "pushed" }]);
  });

  it("logs a file that went away while settling, and goes on to the next world", async () => {
    const { world, deps } = savedWorld();
    const gone = join(tmp, "saves", "user", "W0");
    const { pushed, push: kept } = keptPushes();
    const settle = async (w: string) => {
      if (w === gone) throw Object.assign(new Error(`ENOENT: no such file or directory, stat '${w}/Level.sav'`), { code: "ENOENT" });
      return saveFiles(w);
    };
    const r = await run({ ...deps, worlds: () => [gone, world], settled: settle, push: kept });
    expect(r.ok).toBe(false);
    expect(logged()).toMatch(/W0: ENOENT: no such file or directory/);
    expect(pushed).toHaveLength(1);
    const worlds = readJson(deps.statusPath)["worlds"] as Record<string, Dict>;
    expect([worlds["W0"]!["result"], worlds["W1"]!["result"]]).toEqual(["failed", "pushed"]);
    expect(r.events[0]).toMatchObject({ kind: "error", row: "W0" });
  });

  it("fails only the world whose save the parser can't read, as an update, and goes on to the next", async () => {
    const { world, deps } = savedWorld();
    const other = join(tmp, "saves", "user", "W0");
    mkdirSync(other, { recursive: true });
    writeFileSync(join(other, "Level.sav"), "level");
    const { pushed, push: kept } = keptPushes();
    const unreadable = (): Dict => {
      throw new Error("Error decoding ascii string"); // what a game update looks like to the parser
    };
    const snap = (id: string, raw: RawFiles) => snapshotWorld(id, raw, id === "W0" ? unreadable : fakeParse);
    const r = await run({ ...deps, worlds: () => [other, world], snapshotWorld: snap, push: kept });
    expect(r.ok).toBe(false);
    expect(logged()).toContain("W0: Level.sav: Error decoding ascii string");
    expect(pushed.map(([s]) => (s["world"] as Dict)["id"])).toEqual(["W1"]);
    const s = readJson(deps.statusPath);
    expect((s["worlds"] as Record<string, Dict>)["W0"]).toEqual({ result: "failed", error: "Level.sav: Error decoding ascii string" });
    expect((s["lastRun"] as Dict)["error"]).toBe("1 world(s) failed");
    expect(r.rows[0]).toMatchObject({ label: "World W0", chip: "update" });
    // And a world read without a Level.sav fails as a bad save, not as a crash that ends the pass.
    expect(() => snapshotWorld("W0", {}, fakeParse)).toThrow(SaveFormatError);
  });

  it("says which game build is installed, or null, and which parser read it", async () => {
    const { deps } = savedWorld();
    const { pushed, push: kept } = keptPushes();
    await run({ ...deps, push: kept });
    expect((pushed[0]![0]["world"] as Dict)["gameBuild"]).toBeNull();
    const app = join(tmp, "Palworld.app");
    mkdirSync(join(app, "Contents"), { recursive: true });
    writeFileSync(
      join(app, "Contents", "Info.plist"),
      `<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleShortVersionString</key><string>1.0.8</string><key>CFBundleVersion</key><string>102999</string></dict></plist>`,
    );
    process.env["PALWORLD_APP"] = app;
    await run({ ...deps, push: kept }, true);
    expect((pushed[1]![0]["world"] as Dict)["gameBuild"]).toEqual({ version: "1.0.8", build: "102999" });
    expect(pushed[0]![0]["agent"]).toEqual({ version: VERSION, saveTools: "port of oMaN-Rod/palworld-save-tools@790e0bc" });
  });

  it("carries fog.png and the explored share from LocalData", async () => {
    const { world, deps } = savedWorld();
    writeFileSync(join(world, "LocalData.sav"), "local");
    const { pushed, push: kept } = keptPushes();
    expect((await run({ ...deps, push: kept })).ok).toBe(true);
    const [snap, files] = pushed[0]!;
    expect(Object.keys(files)).toEqual(["Level.sav", "LevelMeta.sav", "WorldOption.sav", "LocalData.sav", `Players/${PLAYER}.sav`, "fog.png"]);
    expect(Buffer.from(files["fog.png"]!).subarray(0, 8)).toEqual(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]));
    expect([snap["schema"], (snap["world"] as Dict)["id"], (((snap["players"] as Dict[])[0] as Dict)["local"] as Dict)["mapExploredPct"]]).toEqual([2, "W1", 12.5]);
  });

  it("keeps state only for an accepted push, skips an unchanged save quietly, and force pushes it", async () => {
    const { deps } = savedWorld();
    const refused = await run({ ...deps, push: keptPushes({ ok: false, error: { code: "PALWORLD_SCHEMA_UNSUPPORTED", message: "this snapshot schema isn't supported" } }).push });
    expect(refused.ok).toBe(false);
    expect(logged()).toContain("W1: refused: this snapshot schema isn't supported");
    expect(refused.rows[0]!.chip).toBe("update");
    expect(existsSync(join(tmp, "state.json"))).toBe(false);
    const { pushed, push: kept } = keptPushes();
    expect((await run({ ...deps, push: kept })).ok).toBe(true);
    expect(pushed).toHaveLength(1); // the refused save is tried again
    const state = readJson(join(tmp, "state.json")) as Record<string, Dict>;
    expect(Object.keys(state)).toEqual(["W1"]);
    expect(Object.keys(state["W1"]!).sort()).toEqual(["fingerprint", "pushedAt", "savedAt"]);
    const unchanged = await run({ ...deps, push: kept });
    expect([pushed.length, unchanged.events, unchanged.rows[0]!.chip]).toEqual([1, [], "pushed"]);
    expect((await run({ ...deps, push: kept }, true)).ok).toBe(true);
    expect(pushed).toHaveLength(2); // force pushes an unchanged save
  });

  it("pushes an unchanged save once for a new core version", async () => {
    const { deps } = savedWorld();
    const { pushed, push: kept } = keptPushes();
    await run({ ...deps, push: kept });
    await run({ ...deps, push: kept });
    expect(pushed).toHaveLength(1);
    const newer = { ...deps, push: kept, version: "99.0.0", fingerprint: (raw: RawFiles) => fingerprint(raw, "99.0.0") };
    await run(newer);
    expect(pushed).toHaveLength(2);
    await run(newer);
    expect(pushed).toHaveLength(2);
  });

  it("says a world still saving is waiting", async () => {
    const { deps } = oneWorld();
    const r = await run({ ...deps, settled: async () => null });
    expect(r.ok).toBe(true);
    expect((readJson(deps.statusPath)["worlds"] as Record<string, Dict>)["W1"]).toEqual({ result: "skipped", note: "still saving" });
    expect(r.events).toEqual([{ kind: "waiting", row: "W1", message: "World W1 is still saving; it goes at the next pass" }]);
    expect(r.rows).toEqual([{ label: "World W1", value: "saving now, waiting", chip: "waiting" }]);
  });

  it("stops at a refused token: one error event with the API's words, no heartbeat, and nothing else pushed", async () => {
    const { world, deps } = savedWorld();
    const second = join(tmp, "saves", "user", "W2");
    mkdirSync(second, { recursive: true });
    writeFileSync(join(second, "Level.sav"), "level");
    let pushes = 0;
    let beats = 0;
    const r = await run({
      ...deps,
      worlds: () => [world, second],
      push: async () => {
        pushes++;
        throw new AuthError("app:palworld:use is required", 403);
      },
      heartbeat: async () => void beats++,
    });
    expect([r.ok, pushes, beats]).toEqual([false, 1, 0]);
    expect(r.events.at(-1)).toEqual({ kind: "error", message: "app:palworld:use is required" });
    expect(existsSync(join(tmp, "state.json"))).toBe(false);
  });

  it("reports a heartbeat the API refuses, since with nothing to push that's the only call", async () => {
    const { deps } = savedWorld();
    await run({ ...deps, push: keptPushes().push });
    const r = await run({ ...deps, heartbeat: async () => { throw new AuthError("the token has expired", 401); } });
    expect(r.ok).toBe(false);
    expect(r.events).toEqual([{ kind: "error", message: "the token has expired" }]);
    // Any other heartbeat failure is a log line, not the pass's outcome.
    const down = await run({ ...deps, heartbeat: async () => { throw new PushError("heartbeat answered 502: nope"); } });
    expect([down.ok, down.events]).toEqual([true, []]);
    expect(logged()).toContain("heartbeat failed: heartbeat answered 502: nope");
  });

  it("says when the API couldn't be reached at all, once, and ends the pass not ok", async () => {
    const { deps } = savedWorld();
    const offline = (async () => {
      throw new TypeError("fetch failed", { cause: Object.assign(new Error("getaddrinfo ENOTFOUND api.example.test"), { code: "ENOTFOUND" }) });
    }) as typeof fetch;
    const r = await run({
      ...deps,
      push: (s, raw, a) => push(s, raw, { ...a, fetch: offline }),
      heartbeat: (b, a) => heartbeat(b, { ...a, fetch: offline }),
    });
    expect(r.ok).toBe(false);
    expect(r.events).toEqual([
      { kind: "pushing", row: "W1", message: "Test Islands: pushing" },
      { kind: "error", row: "W1", message: "Test Islands: ingest unreachable: fetch failed" },
      { kind: "error", message: "Couldn't reach ryna.io (ENOTFOUND)" },
    ]);
    expect(r.rows[0]!.chip).toBe("error"); // the world keeps its own row
    expect(JSON.stringify(r)).not.toContain(TOKEN);

    // Nothing changed to push: the heartbeat alone finds the network gone, and says so too.
    await run({ ...deps, push: keptPushes().push });
    const quiet = await run({ ...deps, heartbeat: (b, a) => heartbeat(b, { ...a, fetch: (async () => { throw new DOMException("The operation was aborted due to timeout", "TimeoutError"); }) as typeof fetch }) });
    expect([quiet.ok, quiet.events]).toEqual([false, [{ kind: "error", message: "Couldn't reach ryna.io (timed out)" }]]);
    expect(quiet.rows[0]!.chip).toBe("pushed");
  });

  it("records a crash, heartbeats, then rethrows it", async () => {
    const { deps } = savedWorld();
    const beats: HeartbeatBody[] = [];
    const boom = () => {
      throw new TypeError("Cannot read properties of undefined (reading 'get')");
    };
    await expect(run({ ...deps, snapshotWorld: boom, heartbeat: async (b) => void beats.push(b) })).rejects.toThrow(TypeError);
    const s = readJson(deps.statusPath);
    expect([(s["lastRun"] as Dict)["ok"], (s["lastRun"] as Dict)["error"]]).toEqual([false, "TypeError: Cannot read properties of undefined (reading 'get')"]);
    expect((beats[0]!.lastRun as Dict)["error"]).toBe("TypeError: Cannot read properties of undefined (reading 'get')");
  });

  it("has a row that says so when there are no worlds", async () => {
    const { deps } = oneWorld();
    const r = await run({ ...deps, worlds: () => [] });
    expect(r.rows).toEqual([{ label: "Palworld", value: "no worlds found", chip: "waiting" }]);
    expect((readJson(deps.statusPath)["lastRun"] as Dict)["ok"]).toBe(false);
  });
});

describe("status.json", () => {
  it("records a push: the world and the pass", async () => {
    const { deps } = savedWorld();
    await run({ ...deps, push: keptPushes(INGESTED).push });
    const s = readJson(deps.statusPath);
    expect(s["version"]).toBe(VERSION);
    const last = s["lastRun"] as Dict;
    expect([last["ok"], "error" in last]).toEqual([true, false]);
    const w = (s["worlds"] as Record<string, Dict>)["W1"]!;
    expect([w["name"], w["savedAt"], w["result"], w["events"], w["alertsOpened"]]).toEqual(["Test Islands", "2026-10-09T15:01:16.000Z", "pushed", 13, 1]);
    expect(s["history"]).toEqual([{ at: last["finishedAt"], ok: true, pushed: 1, skipped: 0, failed: 0 }]);
  });

  it.each([
    [{ duplicate: true }, "duplicate", "Test Islands pushed (already stored)"],
    [{ duplicate: true, upgraded: true }, "upgraded", "Test Islands pushed (stored again for this version)"],
  ])("gives a duplicate or upgraded answer its own result (%j)", async (flag, result, message) => {
    const { deps } = savedWorld();
    const r = await run({ ...deps, push: keptPushes({ ok: true, data: { ...(INGESTED["data"] as Dict), ...flag } }).push });
    expect((readJson(deps.statusPath)["worlds"] as Record<string, Dict>)["W1"]!["result"]).toBe(result);
    expect(r.events.at(-1)!.message).toBe(message);
  });

  it("keeps the last good push on a failed one and says why", async () => {
    const { deps } = savedWorld();
    await run({ ...deps, push: keptPushes(INGESTED).push });
    const pushed = (readJson(deps.statusPath)["worlds"] as Record<string, Dict>)["W1"]!;
    await run({ ...deps, push: (snap, raw, a) => push(snap, raw, { ...a, fetch: fetchAnswer("<html>", "text/html") as typeof fetch }) }, true);
    const w = (readJson(deps.statusPath)["worlds"] as Record<string, Dict>)["W1"]!;
    expect([w["result"], String(w["error"]).startsWith("ingest answered 200 text/html")]).toEqual(["failed", true]);
    for (const k of status.PUSH_FIELDS) expect(w[k]).toEqual(pushed[k]);
  });

  it("seeds a skipped world from state.json when the status file is new", async () => {
    const { deps } = savedWorld();
    const { push: kept } = keptPushes();
    await run({ ...deps, push: kept });
    unlinkSync(deps.statusPath);
    await run({ ...deps, push: kept });
    const w = (readJson(deps.statusPath)["worlds"] as Record<string, Dict>)["W1"];
    const state = (readJson(join(tmp, "state.json")) as Record<string, Dict>)["W1"]!;
    expect(w).toEqual({ name: null, savedAt: state["savedAt"], pushedAt: state["pushedAt"], result: "skipped" });
  });

  it("keeps the last fifty passes", async () => {
    const { deps } = oneWorld();
    for (let i = 0; i < status.HISTORY + 7; i++) await run({ ...deps, worlds: () => [] });
    expect((readJson(deps.statusPath)["history"] as Dict[]).length).toBe(50);
  });

  it("writes through a rename, and a failed write leaves the old file", () => {
    const path = join(tmp, "status.json");
    status.writeAtomic(path, { v: 1 });
    status.writeAtomic(path, { v: 2 });
    expect(() =>
      status.writeAtomic(path, { v: 3 }, () => {
        throw Object.assign(new Error("No space left on device"), { code: "ENOSPC" });
      }),
    ).toThrow(/No space left/);
    expect(readJson(path)).toEqual({ v: 2 });
  });

  it("never breaks the pass when it cannot be written", async () => {
    const { deps } = savedWorld();
    const dir = join(tmp, "status");
    mkdirSync(dir, { recursive: true });
    chmodSync(dir, 0o500); // read-only folder: the temp file can't be made
    try {
      expect((await run({ ...deps, statusPath: join(dir, "status.json"), push: keptPushes(INGESTED).push })).ok).toBe(true);
    } finally {
      chmodSync(dir, 0o700);
    }
    expect(logged()).toContain("status.json: finish failed");
  });
});

describe("the heartbeat", () => {
  it("follows every pass with the status, the host name, the core's version and this process's log tail", async () => {
    const { deps } = savedWorld();
    const beats: HeartbeatBody[] = [];
    for (let i = 0; i < 100; i++) status.log(`line ${i}`);
    await run({ ...deps, push: keptPushes(INGESTED).push, heartbeat: async (b) => void beats.push(b) });
    const b = beats[0]!;
    // `agent` is the host name, as the launchd agent sent it (the API keys agents by it); the version is the core's.
    expect([b.agent, b.version, (b.lastRun as Dict)["ok"], (b.worlds["W1"] as Dict)["result"]]).toEqual([deps.host(), VERSION, true, "pushed"]);
    const named: HeartbeatBody[] = [];
    await run({ ...deps, push: keptPushes(INGESTED).push, host: () => `  ${"m".repeat(150)} `, heartbeat: async (x) => void named.push(x) }, true);
    expect(named[0]!.agent).toBe("m".repeat(100));
    expect(b.log.split("\n").filter(Boolean)).toHaveLength(status.TAIL_LINES);
    expect(b.log).toMatch(/W1 \(Test Islands\): pushed .*\n$/);
  });

  it("posts JSON with the token and reads the envelope", async () => {
    const seen: { url: string; init: RequestInit }[] = [];
    const ok = await heartbeat(
      { agent: "a", version: "1", lastRun: {}, worlds: {}, history: [], log: "" },
      api({
        fetch: (async (url: unknown, init?: RequestInit) => {
          seen.push({ url: String(url), init: init! });
          return new Response('{"ok":true,"data":{"agent":"a"}}', { headers: { "content-type": "application/json" } });
        }) as typeof fetch,
      }),
    );
    expect(ok).toEqual({ ok: true, data: { agent: "a" } });
    expect(seen[0]!.url).toBe(`${API}/v1/palworld/agent/heartbeat`);
    expect((seen[0]!.init.headers as Record<string, string>)["Authorization"]).toBe(`Bearer ${TOKEN}`);
    await expect(heartbeat({ agent: "a", version: "1", lastRun: {}, worlds: {}, history: [], log: "" }, api({ fetch: fetchAnswer('{"ok":false,"error":{"code":"X"}}', "application/json") as typeof fetch }))).rejects.toThrow('heartbeat refused: {"code":"X"}');
  });
});

describe("rows", () => {
  it("says how long ago, in the window's words", () => {
    const now = Date.parse("2026-11-13T12:00:00Z") / 1000;
    expect([ago("2026-11-13T11:59:30Z", now), ago("2026-11-13T11:58:00Z", now), ago("2026-11-13T09:00:00Z", now), ago("2026-11-10T12:00:00Z", now), ago(null, now)]).toEqual([
      "just now",
      "2 min ago",
      "3 h ago",
      "3 days ago",
      null,
    ]);
  });

  it("readFiles names files as ingest does", () => {
    const { world } = savedWorld();
    expect(Object.keys(readFiles(saveFiles(world)))).toEqual(["Level.sav", "LevelMeta.sav", "WorldOption.sav", `Players/${PLAYER}.sav`]);
  });
});
