/** `serve`, the sidecar interface (13.4), driven the way the agent drives it: lines in, lines out. */

import { mkdirSync, mkdtempSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { PassThrough } from "node:stream";
import { beforeEach, describe, expect, it } from "vitest";
import { heartbeat as realHeartbeat, push as realPush } from "../src/agent/push.js";
import { snapshotWorld, type RawFiles } from "../src/agent/snapshot-world.js";
import { defaults, type Deps } from "../src/pass.js";
import { serve, userAgent, type Hello } from "../src/serve.js";
import type { Dict } from "../src/snapshot/util.js";
import { VERSION } from "../src/version.js";
import { realClock, saveFiles, settled } from "../src/worlds.js";
import { META, OPTIONS, PLAYER_SAVE, WORLD } from "./fixtures.js";

const TOKEN = "eyJhbGciOiJFZERTQSJ9.secret-access-token.sig";
const API = "https://api.example.test";
const PARSED: Record<string, Dict> = { level: WORLD, meta: META, options: OPTIONS, player: PLAYER_SAVE };
const fakeParse = (data: Uint8Array): Dict => PARSED[Buffer.from(data).toString()]!;

let tmp: string;
let stateDir: string;
let saves: string;
beforeEach(() => {
  tmp = mkdtempSync(join(tmpdir(), "palworld-serve-"));
  stateDir = join(tmp, "state");
  saves = join(tmp, "SaveGames");
  process.env["PALWORLD_APP"] = join(tmp, "Missing.app");
  const world = join(saves, "user", "0123456789ABCDEF0123456789ABCDEF");
  mkdirSync(join(world, "Players"), { recursive: true });
  writeFileSync(join(world, "Level.sav"), "level");
  writeFileSync(join(world, "LevelMeta.sav"), "meta");
  writeFileSync(join(world, "WorldOption.sav"), "options");
  writeFileSync(join(world, "Players", "00000000000000000000000000000001.sav"), "player");
});

type Call = { url: string; auth: string; userAgent: string; body: string };

/** A fake API: answers each call with `answer(kind)`, and keeps every call. */
function fakeApi(answer: (kind: "ingest" | "heartbeat") => Response | Promise<Response> = () => new Response('{"ok":true,"data":{}}', { headers: { "content-type": "application/json" } })) {
  const calls: Call[] = [];
  const fetch = (async (url: unknown, init?: RequestInit) => {
    const h = init!.headers as Record<string, string>;
    calls.push({ url: String(url), auth: h["Authorization"]!, userAgent: h["User-Agent"]!, body: Buffer.isBuffer(init!.body) ? init!.body.toString("latin1") : String(init!.body) });
    return answer(String(url).endsWith("/ingest") ? "ingest" : "heartbeat");
  }) as typeof globalThis.fetch;
  return { calls, fetch };
}

/** The pass's real code under a temp save folder and state folder, with the parser and the network stood in for. */
const depsWith = (fetch: typeof globalThis.fetch, over: Partial<Deps> = {}) => (hello: Hello): Deps => ({
  ...defaults({ root: hello.gameFolder!, statePath: join(hello.stateDir, "state.json"), statusPath: join(hello.stateDir, "status.json") }),
  settled: async (w) => saveFiles(w),
  host: () => "test-mac",
  snapshotWorld: (id: string, raw: RawFiles) => snapshotWorld(id, raw, fakeParse),
  push: (s, r, a) => realPush(s, r, { ...a, fetch }),
  heartbeat: (b, a) => realHeartbeat(b, { ...a, fetch }),
  ...over,
});

/** A running sidecar: write lines to it, read what it wrote. */
function start(deps: (h: Hello) => Deps) {
  const input = new PassThrough();
  const out: string[] = [];
  const err: string[] = [];
  const waiters: (() => void)[] = [];
  const exited = serve({ input, out: (l) => (out.push(l), waiters.splice(0).forEach((w) => w())), err: (l) => void err.push(l), deps });
  const send = (msg: unknown) => input.write(`${JSON.stringify(msg)}\n`);
  const lines = () => out.map((l) => JSON.parse(l) as Dict);
  /** Until `n` lines of `type` have been written. */
  const until = async (type: string, n = 1): Promise<Dict[]> => {
    while (lines().filter((l) => l["type"] === type).length < n) await new Promise<void>((r) => waiters.push(r));
    return lines();
  };
  const hello = (over: Dict = {}) => send({ type: "hello", protocol: 1, api: API, stateDir, agent: "ryna-agent/0.5.0", gameFolder: saves, ...over });
  return { input, out, err, exited, send, lines, until, hello };
}

/** Every file under `dir`, read as text. */
function everyFile(dir: string): string {
  return readdirSync(dir).map((n) => (statSync(join(dir, n)).isDirectory() ? everyFile(join(dir, n)) : readFileSync(join(dir, n), "latin1"))).join("\n");
}

describe("serve", () => {
  it("answers hello, runs a pass per sync, and stops: the whole transcript", async () => {
    const api = fakeApi();
    const s = start(depsWith(api.fetch));
    s.hello();
    s.send({ type: "sync", token: TOKEN, force: false });
    await s.until("done");
    s.send({ type: "stop" });
    expect(await s.exited).toBe(0);
    const lines = s.lines();
    expect(lines.map((l) => l["type"])).toEqual(["ready", "event", "event", "status", "done"]);
    expect(lines[0]).toEqual({ type: "ready", protocol: 1, core: "palworld", version: VERSION });
    expect(lines[1]).toEqual({ type: "event", event: { kind: "pushing", row: "0123456789ABCDEF0123456789ABCDEF", message: "Test Islands: pushing" } });
    expect(lines[2]).toEqual({ type: "event", event: { kind: "pushed", row: "0123456789ABCDEF0123456789ABCDEF", message: "Test Islands pushed" } });
    expect(lines[3]).toEqual({ type: "status", rows: [{ label: "Test Islands", value: expect.stringMatching(/^saved .*, pushed$/), chip: "pushed" }] });
    expect(lines[4]).toEqual({ type: "done", ok: true });
    // The push and the heartbeat went to hello's API with the sync's token, as ryna-agent/<v> palworld/<v>.
    expect(api.calls.map((c) => [c.url, c.auth, c.userAgent])).toEqual([
      [`${API}/v1/palworld/agent/ingest`, `Bearer ${TOKEN}`, `ryna-agent/0.5.0 palworld/${VERSION}`],
      [`${API}/v1/palworld/agent/heartbeat`, `Bearer ${TOKEN}`, `ryna-agent/0.5.0 palworld/${VERSION}`],
    ]);
    // The heartbeat names the host, as the launchd agent did, and carries the core's version.
    expect((JSON.parse(api.calls[1]!.body) as Dict)["agent"]).toBe("test-mac");
    expect((JSON.parse(api.calls[1]!.body) as Dict)["version"]).toBe(VERSION);
    // State lives in hello's folder.
    expect(readdirSync(stateDir).sort()).toEqual(["state.json", "status.json"]);
  });

  it("writes only protocol lines to stdout, one JSON object each, and logs to stderr", async () => {
    const s = start(depsWith(fakeApi().fetch));
    s.hello();
    s.send({ type: "sync", token: TOKEN, force: true });
    await s.until("done");
    s.input.write("this isn't json\n");
    s.send({ type: "mystery" });
    s.input.end();
    await s.exited;
    for (const line of s.out) {
      expect(line.endsWith("\n")).toBe(true);
      expect(line.slice(0, -1)).not.toContain("\n");
      expect(["ready", "event", "status", "done", "error"]).toContain((JSON.parse(line) as Dict)["type"]);
    }
    const err = s.err.join("");
    expect(err).toMatch(/pushed \d+ pals/);
    expect(err).toContain("ignored a line that isn't JSON");
    expect(err).toContain('ignored a message of type "mystery"');
  });

  it("queues a sync that arrives during a pass (one), and runs it with the newest token", async () => {
    let release!: () => void;
    const gate = new Promise<void>((r) => (release = r));
    let first = true;
    const api = fakeApi(async (kind) => {
      if (kind === "ingest" && first) {
        first = false;
        await gate;
      }
      return new Response('{"ok":true,"data":{}}', { headers: { "content-type": "application/json" } });
    });
    const s = start(depsWith(api.fetch));
    s.hello();
    s.send({ type: "sync", token: "token-1", force: false });
    await s.until("event"); // the first pass is pushing
    s.send({ type: "sync", token: "token-2", force: false });
    s.send({ type: "sync", token: "token-3", force: true });
    release();
    await s.until("done", 2);
    s.send({ type: "stop" });
    await s.exited;
    expect(s.lines().filter((l) => l["type"] === "done")).toHaveLength(2);
    // The second pass is the queued one: the newest token, and forced (one of the folded syncs asked).
    expect(api.calls.map((c) => [c.url.split("/").at(-1), c.auth])).toEqual([
      ["ingest", "Bearer token-1"],
      ["heartbeat", "Bearer token-1"],
      ["ingest", "Bearer token-3"],
      ["heartbeat", "Bearer token-3"],
    ]);
  });

  it("stops when stdin closes, and a stop during a pass ends it at once", async () => {
    const quiet = start(depsWith(fakeApi().fetch));
    quiet.hello();
    await quiet.until("ready");
    quiet.input.end();
    expect(await quiet.exited).toBe(0);

    // A world still being saved: the pass is in its settle wait (a real sleep, minutes long).
    const busy = start(depsWith(fakeApi().fetch, { settled: (w, signal) => settled(w, 4, { now: () => Date.now() / 1000 + 1e9 * 0, sleep: (_s, sig) => realClock.sleep(600, sig) }, signal) }));
    writeFileSync(join(saves, "user", "0123456789ABCDEF0123456789ABCDEF", "Level.sav"), "level"); // just written
    busy.hello();
    busy.send({ type: "sync", token: TOKEN, force: false });
    await new Promise((r) => setTimeout(r, 50));
    const t0 = Date.now();
    busy.send({ type: "stop" });
    expect(await busy.exited).toBe(0);
    expect(Date.now() - t0).toBeLessThan(2000);
    expect(busy.lines().map((l) => l["type"])).toEqual(["ready"]);
  });

  it("makes a 401 an error event with the API's words, then done not ok", async () => {
    const api = fakeApi(() => new Response(JSON.stringify({ ok: false, error: { code: "UNAUTHENTICATED", message: "the access token has expired" } }), { status: 401, headers: { "content-type": "application/json" } }));
    const s = start(depsWith(api.fetch));
    s.hello();
    s.send({ type: "sync", token: TOKEN, force: false });
    await s.until("done");
    s.input.end();
    await s.exited;
    const lines = s.lines();
    expect(lines.at(-3)).toEqual({ type: "event", event: { kind: "error", message: "the access token has expired" } });
    expect(lines.at(-1)).toEqual({ type: "done", ok: false });
    expect(api.calls.map((c) => c.url.split("/").at(-1))).toEqual(["ingest"]); // no heartbeat on a refused token
  });

  it("never lets the token reach stdout, stderr or the state folder, even when the API repeats it", async () => {
    // A misbehaving API that quotes the token back in its error, and a push that fails with it in the message.
    const api = fakeApi((kind) =>
      kind === "ingest"
        ? new Response(`upstream said: Bearer ${TOKEN}`, { status: 502 })
        : new Response(JSON.stringify({ ok: false, error: { message: `bad token ${TOKEN}` } }), { status: 401, headers: { "content-type": "application/json" } }),
    );
    const s = start(depsWith(api.fetch));
    s.hello();
    s.send({ type: "sync", token: TOKEN, force: true });
    await s.until("done");
    s.send({ type: "sync", token: TOKEN, force: true });
    await s.until("done", 2);
    s.input.end();
    await s.exited;
    const everything = [s.out.join(""), s.err.join(""), everyFile(stateDir)].join("\n");
    expect(everything).toContain("[token]"); // it was there to leak, and was redacted
    expect(everything).not.toContain(TOKEN);
    expect(everything).not.toContain("secret-access-token");
  });

  it("ends a pass that reached nobody with an error event and done not ok", async () => {
    const offline = (async () => {
      throw new TypeError("fetch failed", { cause: Object.assign(new Error("connect ECONNREFUSED"), { code: "ECONNREFUSED" }) });
    }) as typeof fetch;
    const s = start(depsWith(offline));
    s.hello();
    s.send({ type: "sync", token: TOKEN, force: false });
    await s.until("done");
    s.input.end();
    await s.exited;
    const lines = s.lines();
    expect(lines.at(-3)).toEqual({ type: "event", event: { kind: "error", message: "Couldn't reach ryna.io (ECONNREFUSED)" } });
    expect(lines.at(-2)).toMatchObject({ type: "status", rows: [{ chip: "error" }] });
    expect(lines.at(-1)).toEqual({ type: "done", ok: false });
    expect(s.out.join("") + s.err.join("")).not.toContain(TOKEN);
  });

  it("refuses a protocol it doesn't speak, with an error, and exits", async () => {
    const s = start(depsWith(fakeApi().fetch));
    s.hello({ protocol: 2 });
    expect(await s.exited).toBe(1);
    expect(s.lines()).toEqual([{ type: "error", message: "this Palworld core speaks protocol 1, and Ryna sent 2", fix: "Update Ryna, or let it fetch the Palworld core again." }]);
  });

  it("answers a sync before hello, or one with no token, with done not ok", async () => {
    const s = start(depsWith(fakeApi().fetch));
    s.send({ type: "sync", token: TOKEN, force: false });
    await s.until("done");
    s.hello();
    s.send({ type: "sync", force: false });
    await s.until("done", 2);
    s.input.end();
    await s.exited;
    expect(s.lines().map((l) => (l["type"] === "event" ? (l["event"] as Dict)["message"] : l["type"]))).toEqual([
      "Ryna asked for a pass before saying hello",
      "done",
      "ready",
      "Ryna sent no access token with this pass",
      "done",
    ]);
  });

  it("names itself after the agent in its User-Agent", () => {
    expect(userAgent("ryna-agent/0.5.0")).toBe(`ryna-agent/0.5.0 palworld/${VERSION}`);
    expect(userAgent("x".repeat(200))).toHaveLength(100);
  });
});
