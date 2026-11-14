/**
 * status.json: what the last passes did, kept in the core's state folder for the
 * heartbeat, which sends the same content to the portal's Agent page.
 *
 * state.json stays the agent's own memory (fingerprints); this file is only for
 * reading. It is rewritten whole after every run, through a temp file and a
 * rename, so a reader never sees half of one. Writing it must never break a
 * run: every failure here is logged and swallowed.
 *
 *     {version, lastRun{startedAt, finishedAt, ok, error?},
 *      worlds{<id>: {name, savedAt, pushedAt, result, summary, events, alertsOpened, durationS, error?}},
 *      history[{at, ok, pushed, skipped, failed}]}   newest last, at most HISTORY runs
 */

import { closeSync, fsyncSync, mkdirSync, openSync, readFileSync, renameSync, unlinkSync, writeSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import type { Dict } from "../snapshot/util.js";

export const HISTORY = 50;
// Details of the last accepted push. A skipped or failed world keeps them, so the app still shows them.
export const PUSH_FIELDS = ["name", "savedAt", "pushedAt", "summary", "events", "alertsOpened", "durationS"] as const;

const nowIso = (): string => new Date().toISOString().replace(/\.\d{3}Z$/, "Z");

function load(path: string): Dict {
  try {
    const data: unknown = JSON.parse(readFileSync(path, "utf8"));
    return data !== null && typeof data === "object" && !Array.isArray(data) ? (data as Dict) : {};
  } catch {
    return {};
  }
}

/** Write through a temp file in the same folder and rename it over `path`; a failure leaves the old file. */
export function writeAtomic(path: string, data: unknown, serialise: (d: unknown) => string = (d) => JSON.stringify(d, null, 2)): void {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = join(dirname(path), `.${basename(path)}.${process.pid}.${Date.now()}.tmp`);
  const fd = openSync(tmp, "w");
  try {
    writeSync(fd, serialise(data));
    fsyncSync(fd);
    closeSync(fd);
    renameSync(tmp, path);
  } catch (e) {
    try {
      closeSync(fd);
    } catch {
      // already closed
    }
    try {
      unlinkSync(tmp);
    } catch {
      // nothing to remove
    }
    throw e;
  }
}

const stamp = (): string => {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
};

/** How much of this process's log the heartbeat carries. */
export const TAIL_LINES = 80;
export const TAIL_BYTES = 32 * 1024;
const tail: string[] = [];

/** Where log lines go. stderr, always: stdout is the sidecar protocol's alone (serve.ts). */
let sink: (line: string) => void = (line) => void process.stderr.write(line);

/** For serve.ts, which passes every line through its token redaction first. */
export function setLogSink(write: (line: string) => void): void {
  sink = write;
}

/** The core's own log line: local time, like Python's `isoformat(timespec="seconds")`. To stderr, and kept for the heartbeat. */
export function log(msg: string): void {
  const line = `${stamp()} ${msg}\n`;
  tail.push(line);
  if (tail.length > TAIL_LINES) tail.splice(0, tail.length - TAIL_LINES);
  sink(line);
}

/** The last lines this process logged, newest last, at most TAIL_LINES and TAIL_BYTES (whole lines). */
export function logTail(): string {
  let text = tail.join("");
  while (Buffer.byteLength(text) > TAIL_BYTES && text.includes("\n")) text = text.slice(text.indexOf("\n") + 1);
  return text;
}

type PushAnswer = { data?: unknown };

/** One run's status, built up world by world and written once at the end. */
export class Run {
  readonly path: string;
  readonly started = nowIso();
  worlds: Record<string, Dict> = {};
  history: Dict[] = [];
  results: Record<string, string> = {};
  /** What finish() built, whether or not it could be written: the heartbeat sends it. */
  final: Dict | null = null;

  constructor(
    readonly version: string,
    state: Dict,
    path: string,
  ) {
    this.path = path;
    this.advisory("_seed", () => this.seed(state));
  }

  /** Status is advisory: log and swallow anything that goes wrong keeping it. */
  private advisory<T>(name: string, fn: () => T): T | undefined {
    try {
      return fn();
    } catch (e) {
      log(`status.json: ${name} failed: ${e instanceof Error ? `${e.name}: ${e.message}` : String(e)}`);
      return undefined;
    }
  }

  private seed(state: Dict): void {
    const prev = load(this.path);
    const old = prev["worlds"];
    // Seed each world from the last status, or failing that from state.json's last push.
    for (const [wid, s] of Object.entries(state)) {
      if (s !== null && typeof s === "object" && !Array.isArray(s)) this.worlds[wid] = { name: null, savedAt: s["savedAt"] ?? null, pushedAt: s["pushedAt"] ?? null };
    }
    if (old !== null && typeof old === "object" && !Array.isArray(old)) {
      for (const [wid, w] of Object.entries(old)) if (w !== null && typeof w === "object" && !Array.isArray(w)) this.worlds[wid] = { ...w };
    }
    const hist = prev["history"];
    this.history = Array.isArray(hist) ? hist.filter((h): h is Dict => h !== null && typeof h === "object" && !Array.isArray(h)) : [];
  }

  private kept(worldId: string): Dict {
    const w = this.worlds[worldId] ?? {};
    return Object.fromEntries(Object.entries(w).filter(([k]) => (PUSH_FIELDS as readonly string[]).includes(k)));
  }

  skipped(worldId: string, note?: string): void {
    this.advisory("skipped", () => {
      this.worlds[worldId] = { ...this.kept(worldId), result: "skipped", ...(note ? { note } : {}) };
      this.results[worldId] = "skipped";
    });
  }

  failed(worldId: string, error: string): void {
    this.advisory("failed", () => {
      this.worlds[worldId] = { ...this.kept(worldId), result: "failed", error };
      this.results[worldId] = "failed";
    });
  }

  pushed(worldId: string, snap: Dict, answer: PushAnswer, durationS: number): void {
    this.advisory("pushed", () => {
      const data = (answer.data !== null && typeof answer.data === "object" ? answer.data : {}) as Dict;
      const result = data["upgraded"] ? "upgraded" : data["duplicate"] ? "duplicate" : "pushed";
      const world = (snap["world"] ?? {}) as Dict;
      this.worlds[worldId] = {
        name: world["name"] ?? null,
        savedAt: data["savedAt"] ?? world["savedAt"] ?? null,
        pushedAt: nowIso(),
        result,
        summary: data["summary"] !== null && typeof data["summary"] === "object" ? data["summary"] : {},
        events: data["events"] ?? 0,
        alertsOpened: data["alertsOpened"] ?? 0,
        durationS: Math.round(durationS * 10) / 10,
      };
      this.results[worldId] = result;
    });
  }

  /** Write status.json. Never throws: the run's own outcome must not depend on it. */
  finish(ok: boolean, error?: string | null): Dict | undefined {
    return this.advisory("finish", () => {
      const finished = nowIso();
      const counts = { pushed: 0, skipped: 0, failed: 0 };
      for (const r of Object.values(this.results)) counts[r === "skipped" ? "skipped" : r === "failed" ? "failed" : "pushed"]++;
      const last: Dict = { startedAt: this.started, finishedAt: finished, ok };
      if (error) last["error"] = error;
      this.history = [...this.history, { at: finished, ok, ...counts }].slice(-HISTORY);
      const data: Dict = { version: this.version, lastRun: last, worlds: this.worlds, history: this.history };
      this.final = data;
      writeAtomic(this.path, data);
      return data;
    });
  }
}
