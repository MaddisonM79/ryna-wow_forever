/**
 * One pass, what a `sync` asks for: find the worlds, wait for each save to settle, snapshot the
 * changed ones, push them, heartbeat. The old launchd agent's `run`, with the token handed in
 * (never stored) and what happened reported as events and status rows instead of a notification.
 */

import { hostname } from "node:os";
import { basename } from "node:path";
import { AuthError, PushError, UnreachableError, heartbeat as realHeartbeat, push as realPush, type Api, type HeartbeatBody } from "./agent/push.js";
import { snapshotWorld as realSnapshotWorld, type RawFiles } from "./agent/snapshot-world.js";
import { Run, log, logTail } from "./agent/status.js";
import { SaveFormatError } from "./sav.js";
import type { Dict } from "./snapshot/util.js";
import { VERSION } from "./version.js";
import { fingerprint, loadState, readFiles, saveState, settled, worlds, type Clock } from "./worlds.js";

/** 13.4's event kinds. `row` is the world's id (its save folder's name). */
export type EventKind = "waiting" | "pushing" | "pushed" | "error" | "info";
export type CoreEvent = { kind: EventKind; row?: string; message: string };
/** The host's chip styles. */
export type Chip = "pushed" | "waiting" | "update" | "error";
export type StatusRow = { label: string; value: string; chip: Chip };

/** Everything a pass touches outside its own logic, so the tests can stand in for the disk and the API. */
export type Deps = {
  worlds: () => string[];
  settled: (world: string, signal?: AbortSignal) => Promise<string[] | null>;
  readFiles: (files: string[]) => RawFiles;
  snapshotWorld: (worldId: string, raw: RawFiles) => { snap: Dict; files: RawFiles };
  push: (snap: Dict, raw: RawFiles, api: Api) => Promise<Dict>;
  heartbeat: (body: HeartbeatBody, api: Api) => Promise<unknown>;
  fingerprint: (raw: RawFiles) => string;
  statePath: string;
  statusPath: string;
  version: string;
  /** Seconds since the epoch, for "saved 2 min ago". */
  now: () => number;
  /**
   * The heartbeat's `agent`: this computer's host name, as the launchd agent sent it. The API keys a
   * person's agents by it (palworld_agents), so it names the machine, never a version.
   */
  host: () => string;
};

/** The real ones, under the save root and the state folder given. */
export const defaults = (paths: { root: string; statePath: string; statusPath: string; clock?: Clock }): Deps => ({
  worlds: () => worlds(paths.root),
  settled: (w, signal) => settled(w, 4, paths.clock, signal),
  readFiles,
  snapshotWorld: realSnapshotWorld,
  push: realPush,
  heartbeat: realHeartbeat,
  fingerprint,
  statePath: paths.statePath,
  statusPath: paths.statusPath,
  version: VERSION,
  now: () => Date.now() / 1000,
  host: hostname,
});

export type PassInput = { api: Api; force: boolean; emit: (e: CoreEvent) => void };
export type PassResult = { ok: boolean; rows: StatusRow[] };

/** A failure that ends one world's push and lets the pass go on: a bad save, a refused push, or a file that went away. */
const isExpected = (e: unknown): boolean => e instanceof SaveFormatError || e instanceof PushError || (e instanceof Error && typeof (e as NodeJS.ErrnoException).code === "string");
const describe = (e: unknown): string => (e instanceof Error ? e.message : String(e));
const isAbort = (e: unknown, signal?: AbortSignal): boolean => signal?.aborted === true && (e === signal.reason || (e instanceof Error && e.name === "AbortError") || e instanceof PushError);

/** "just now", "4 min ago", "3 h ago", "2 days ago". */
export function ago(iso: unknown, now: number): string | null {
  const t = typeof iso === "string" ? Date.parse(iso) / 1000 : NaN;
  if (!Number.isFinite(t)) return null;
  const s = Math.max(0, now - t);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 48 * 3600) return `${Math.floor(s / 3600)} h ago`;
  return `${Math.floor(s / 86400)} days ago`;
}

/** A world's name for a person: the save's own, else its id's first eight characters. */
const labelOf = (worldId: string, name: unknown): string => (typeof name === "string" && name.trim() ? name : `World ${worldId.slice(0, 8)}`);

/**
 * The status card: one row per world found this pass. `update` means this core can't read the save
 * or the API no longer takes its snapshot: a newer Ryna is the fix.
 */
export function rows(found: readonly string[], status: Record<string, Dict>, chips: Record<string, Chip>, now: number): StatusRow[] {
  if (!found.length) return [{ label: "Palworld", value: "no worlds found", chip: "waiting" }];
  return found.map((world) => {
    const id = basename(world);
    const w = status[id] ?? {};
    const saved = ago(w["savedAt"], now);
    const label = labelOf(id, w["name"]);
    const result = w["result"];
    if (result === "skipped" && w["note"] === "still saving") return { label, value: "saving now, waiting", chip: "waiting" };
    if (result === "failed") return { label, value: `${saved ? `saved ${saved}, ` : ""}not pushed: ${String(w["error"] ?? "failed").slice(0, 120)}`, chip: chips[id] ?? "error" };
    if (w["pushedAt"]) return { label, value: `saved ${saved ?? "at an unknown time"}, pushed`, chip: "pushed" };
    return { label, value: saved ? `saved ${saved}, not pushed yet` : "not pushed yet", chip: "waiting" };
  });
}

/** One pass. Rejects only when `stop` aborted it or something unexpected broke (after recording it). */
export async function pass(input: PassInput, deps: Deps): Promise<PassResult> {
  const { api, force, emit } = input;
  const signal = api.signal;
  const state = loadState(deps.statePath);
  const st = new Run(deps.version, state, deps.statusPath);
  const chips: Record<string, Chip> = {};
  let found: string[] = [];
  let failures = 0;
  let refused: AuthError | null = null;
  // The first push this pass that got no answer at all: said once, after the pass.
  let unreachable: UnreachableError | null = null;
  let current: string | null = null;

  const fail = (worldId: string, label: string, why: string, chip: Chip): void => {
    failures++;
    chips[worldId] = chip;
    log(`${worldId}: ${why}`);
    st.failed(worldId, why);
    emit({ kind: "error", row: worldId, message: `${label}: ${why}` });
  };

  try {
    found = deps.worlds();
    if (!found.length) {
      log("no worlds found under the save folder");
      st.finish(false, "no worlds found (has macOS allowed Ryna to read Palworld's saves?)");
      const bad = sayApi(await beat(st, deps, api), null, emit);
      return { ok: !bad, rows: rows(found, st.worlds, chips, deps.now()) };
    }
    for (const world of found) {
      signal?.throwIfAborted();
      const worldId = basename(world);
      current = worldId;
      let label = labelOf(worldId, st.worlds[worldId]?.["name"]);
      const started = Date.now();
      let snap: Dict;
      let fp: string;
      let result: Dict;
      try {
        // Inside the try: a file that goes away while settled() stats it fails this world only.
        const files = await deps.settled(world, signal);
        if (files === null) {
          log(`${worldId}: still saving, waiting for the next pass`);
          st.skipped(worldId, "still saving");
          emit({ kind: "waiting", row: worldId, message: `${label} is still saving; it goes at the next pass` });
          continue;
        }
        const raw = deps.readFiles(files);
        fp = deps.fingerprint(raw);
        if (!force && (state[worldId] as Dict | undefined)?.["fingerprint"] === fp) {
          st.skipped(worldId);
          continue;
        }
        const built = deps.snapshotWorld(worldId, raw);
        snap = built.snap;
        label = labelOf(worldId, (snap["world"] as Dict | undefined)?.["name"]);
        emit({ kind: "pushing", row: worldId, message: `${label}: pushing` });
        result = await deps.push(snap, built.files, api);
      } catch (e) {
        if (isAbort(e, signal)) throw e;
        if (e instanceof AuthError) {
          // The token, not this world: no other world would get through either, nor the heartbeat.
          refused = e;
          break;
        }
        if (e instanceof UnreachableError) unreachable ??= e;
        if (!isExpected(e)) throw e;
        fail(worldId, label, describe(e), e instanceof SaveFormatError ? "update" : "error");
        continue;
      }
      if (!result["ok"]) {
        const error = (result["error"] ?? {}) as Dict;
        const why = typeof error["message"] === "string" ? `refused: ${error["message"]}` : `refused: ${JSON.stringify(result["error"])}`;
        fail(worldId, label, why, error["code"] === "PALWORLD_SCHEMA_UNSUPPORTED" ? "update" : "error");
        continue;
      }
      const w = (snap["world"] ?? {}) as Dict;
      state[worldId] = { fingerprint: fp, pushedAt: new Date().toISOString(), savedAt: w["savedAt"] ?? null };
      saveState(state, deps.statePath);
      const seconds = (Date.now() - started) / 1000;
      st.pushed(worldId, snap, result as { data?: unknown }, seconds);
      const data = (result["data"] ?? {}) as Dict;
      const how = data["upgraded"] ? " (stored again for this version)" : data["duplicate"] ? " (already stored)" : "";
      log(`${worldId} (${label}): pushed ${(snap["pals"] as unknown[] | undefined)?.length ?? 0} pals in ${seconds.toFixed(0)}s: ${JSON.stringify(result["data"])}`);
      emit({ kind: "pushed", row: worldId, message: `${label} pushed${how}` });
    }
    current = null;
    if (refused) {
      log(`the API refused this pass's token (${refused.status}): ${refused.message}`);
      st.finish(false, refused.message);
      emit({ kind: "error", message: refused.message });
      return { ok: false, rows: rows(found, st.worlds, chips, deps.now()) };
    }
    st.finish(!failures, failures ? `${failures} world(s) failed` : null);
    const bad = sayApi(await beat(st, deps, api), unreachable, emit);
    return { ok: !failures && !bad, rows: rows(found, st.worlds, chips, deps.now()) };
  } catch (e) {
    if (isAbort(e, signal)) throw e;
    // Anything unexpected still ends the pass (its stack goes to the log); status.json records it first.
    const error = e instanceof Error ? `${e.name}: ${e.message}` : String(e);
    if (current) st.failed(current, error);
    st.finish(false, error);
    await beat(st, deps, api);
    throw e;
  }
}

/** What the agent says when the API never answered: the window's words, then the short cause. */
export const unreachableMessage = (e: UnreachableError): string => `Couldn't reach ryna.io${e.reason ? ` (${e.reason})` : ""}`;

/**
 * One pass-level error event for the API refusing the token or not answering at all (a push's or
 * the heartbeat's), so the window doesn't show a pass that reached nobody as fine. True if it said one.
 */
function sayApi(fromBeat: AuthError | UnreachableError | null, fromPush: UnreachableError | null, emit: (e: CoreEvent) => void): boolean {
  const e = fromBeat ?? fromPush;
  if (!e) return false;
  emit({ kind: "error", message: e instanceof UnreachableError ? unreachableMessage(e) : e.message });
  return true;
}

/**
 * Tell the API how the pass went. Advisory: most failures are one log line, never the pass's outcome.
 * Two are returned: the API refusing the token, and no answer at all. With every save unchanged the
 * heartbeat is the only call a pass makes, so it's where a lost sign-in or a lost network shows.
 */
async function beat(st: Run, deps: Deps, api: Api): Promise<AuthError | UnreachableError | null> {
  if (!st.final) return null;
  try {
    await deps.heartbeat(
      {
        // At most 100 characters, the API's limit. The versions go in `version` (the core's) and
        // the User-Agent; the heartbeat has no other field for the agent's own.
        agent: (deps.host().trim() || "unknown host").slice(0, 100),
        version: deps.version,
        lastRun: st.final["lastRun"] as Dict,
        worlds: st.final["worlds"] as Dict,
        history: st.final["history"] as Dict[],
        log: logTail(),
      },
      api,
    );
    return null;
  } catch (e) {
    if (api.signal?.aborted) return null;
    log(`heartbeat failed: ${describe(e)}`);
    return e instanceof AuthError || e instanceof UnreachableError ? e : null;
  }
}
