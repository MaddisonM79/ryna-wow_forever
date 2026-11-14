/**
 * `serve`: the sidecar interface (docs/ryna-agent-cores-mods.md 13.4). Line-delimited JSON, one
 * object a line, UTF-8.
 *
 *   in  (stdin):  hello {protocol, api, stateDir, agent, gameFolder}, sync {token, force}, stop
 *   out (stdout): ready, event {event: {kind, row?, message}}, status {rows}, done {ok}, error {message, fix?}
 *
 * stdout carries those lines and nothing else; every log line goes to stderr. Stdin closing is
 * `stop`. A `sync` during a pass is queued (one: later ones fold into it, with the newest token and
 * `force` if any asked) and run when the pass ends; each pass ends with one `done`.
 *
 * The token comes with each `sync` and is held for that pass only. It is never written to stdout,
 * stderr or the state folder: no line ever echoes input, and every line out passes through
 * `redact`, which replaces any token this process has been given.
 */

import { mkdirSync } from "node:fs";
import { isAbsolute, join } from "node:path";
import { createInterface } from "node:readline";
import type { Readable } from "node:stream";
import { log, setLogSink } from "./agent/status.js";
import { defaults, pass, type CoreEvent, type Deps, type StatusRow } from "./pass.js";
import { VERSION } from "./version.js";
import { SAVE_ROOT } from "./worlds.js";

export const PROTOCOL = 1;
export const CORE_ID = "palworld";

export type Hello = { api: string; stateDir: string; agent: string; gameFolder: string | null };

type Out =
  | { type: "ready"; protocol: number; core: string; version: string }
  | { type: "event"; event: CoreEvent }
  | { type: "status"; rows: StatusRow[] }
  | { type: "done"; ok: boolean }
  | { type: "error"; message: string; fix?: string };

export type ServeIO = {
  input: Readable;
  /** One protocol line (already ending in \n). */
  out: (line: string) => void;
  /** One log line (already ending in \n). */
  err: (line: string) => void;
  /** The pass's dependencies for this hello; the tests stand in for the disk and the API here. */
  deps?: (hello: Hello) => Deps;
};

/** `ryna-agent/<v> palworld/<v>`: the User-Agent on every call. (The heartbeat's `agent` is the host name: pass.ts.) */
export function userAgent(agent: string): string {
  const host = agent.trim() || "ryna-agent";
  return `${host} ${CORE_ID}/${VERSION}`.slice(0, 100);
}

const isRecord = (v: unknown): v is Record<string, unknown> => v !== null && typeof v === "object" && !Array.isArray(v);

/** Run the sidecar until `stop` or stdin closes. Resolves with the exit code. */
export async function serve(io: ServeIO): Promise<number> {
  const tokens = new Set<string>();
  const redact = (text: string): string => {
    let out = text;
    for (const t of tokens) if (t) out = out.split(t).join("[token]");
    return out;
  };
  const send = (msg: Out): void => io.out(`${redact(JSON.stringify(msg))}\n`);
  setLogSink((line) => io.err(redact(line)));

  const stopper = new AbortController();
  let hello: Hello | null = null;
  let deps: Deps | null = null;
  let pending: { token: string; force: boolean } | null = null;
  let running: Promise<void> | null = null;
  let exitCode = 0;

  let finish!: () => void;
  const finished = new Promise<void>((resolve) => (finish = resolve));
  let stopping = false;
  const stop = async (code = 0): Promise<void> => {
    if (stopping) return;
    stopping = true;
    exitCode = code;
    pending = null;
    stopper.abort(new Error("stopped"));
    rl.close();
    await running;
    finish();
  };

  const onePass = async (job: { token: string; force: boolean }): Promise<void> => {
    const h = hello!;
    try {
      const result = await pass(
        {
          api: { url: h.api, token: job.token, userAgent: userAgent(h.agent), signal: stopper.signal },
          force: job.force,
          emit: (event) => send({ type: "event", event }),
        },
        deps!,
      );
      send({ type: "status", rows: result.rows });
      send({ type: "done", ok: result.ok });
    } catch (e) {
      if (stopper.signal.aborted) return;
      log(`the pass failed: ${e instanceof Error ? (e.stack ?? e.message) : String(e)}`);
      send({ type: "event", event: { kind: "error", message: `the pass failed: ${e instanceof Error ? e.message : String(e)}` } });
      send({ type: "done", ok: false });
    }
  };

  const drain = async (first: { token: string; force: boolean }): Promise<void> => {
    let job: { token: string; force: boolean } | null = first;
    while (job && !stopping) {
      await onePass(job);
      job = pending;
      pending = null;
    }
    running = null;
  };

  const onHello = (msg: Record<string, unknown>): void => {
    if (hello) return log("ignored a second hello");
    if (msg["protocol"] !== PROTOCOL) {
      send({ type: "error", message: `this Palworld core speaks protocol ${PROTOCOL}, and Ryna sent ${JSON.stringify(msg["protocol"] ?? null)}`, fix: "Update Ryna, or let it fetch the Palworld core again." });
      void stop(1);
      return;
    }
    const api = msg["api"];
    const stateDir = msg["stateDir"];
    if (typeof api !== "string" || !/^https?:\/\//.test(api) || typeof stateDir !== "string" || !isAbsolute(stateDir)) {
      send({ type: "error", message: "Ryna's hello named no API address or no absolute state folder", fix: "Update Ryna." });
      void stop(1);
      return;
    }
    const gameFolder = typeof msg["gameFolder"] === "string" && msg["gameFolder"] ? msg["gameFolder"] : null;
    hello = { api, stateDir, agent: typeof msg["agent"] === "string" ? msg["agent"] : "ryna-agent", gameFolder };
    try {
      mkdirSync(stateDir, { recursive: true });
    } catch (e) {
      send({ type: "error", message: `can't make the state folder: ${e instanceof Error ? e.message : String(e)}` });
      void stop(1);
      return;
    }
    deps = io.deps?.(hello) ?? defaults({ root: gameFolder ?? SAVE_ROOT, statePath: join(stateDir, "state.json"), statusPath: join(stateDir, "status.json") });
    send({ type: "ready", protocol: PROTOCOL, core: CORE_ID, version: VERSION });
  };

  const onSync = (msg: Record<string, unknown>): void => {
    const token = msg["token"];
    if (typeof token === "string" && token) tokens.add(token);
    if (!hello) {
      log("a sync came before hello");
      send({ type: "event", event: { kind: "error", message: "Ryna asked for a pass before saying hello" } });
      send({ type: "done", ok: false });
      return;
    }
    if (typeof token !== "string" || !token) {
      send({ type: "event", event: { kind: "error", message: "Ryna sent no access token with this pass" } });
      send({ type: "done", ok: false });
      return;
    }
    const job = { token, force: msg["force"] === true };
    if (running) {
      pending = { token, force: job.force || (pending?.force ?? false) };
      return;
    }
    running = drain(job);
  };

  const rl = createInterface({ input: io.input, crlfDelay: Infinity });
  rl.on("line", (line) => {
    if (stopping || !line.trim()) return;
    let msg: unknown;
    try {
      msg = JSON.parse(line);
    } catch {
      // Never echo the line: it may hold a token.
      return log("ignored a line that isn't JSON");
    }
    if (!isRecord(msg)) return log("ignored a line that isn't a JSON object");
    switch (msg["type"]) {
      case "hello":
        return onHello(msg);
      case "sync":
        return onSync(msg);
      case "stop":
        return void stop(0);
      default:
        return log(`ignored a message of type ${JSON.stringify(String(msg["type"]).slice(0, 40))}`);
    }
  });
  rl.on("close", () => void stop(exitCode));

  await finished;
  return exitCode;
}
