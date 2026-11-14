/**
 * The multipart push and the heartbeat to `<api>/v1/palworld/agent/*`, with the agent's access
 * token that came with this pass's `sync` (serve.ts). The token is a header and nothing else: it is
 * never logged, never put in an error message and never written anywhere.
 */

import { randomUUID } from "node:crypto";
import type { Dict } from "../snapshot/util.js";
import type { RawFiles } from "./snapshot-world.js";

/** Ingest didn't take the push: a non-JSON answer, an HTTP error, or no answer. The pass logs it and carries on. */
export class PushError extends Error {}

/**
 * No answer from the API at all: DNS, a refused connection, a timeout. `reason` is the short cause
 * (`ENOTFOUND`, `ECONNREFUSED`, `timed out`), never anything from the request.
 */
export class UnreachableError extends PushError {
  constructor(
    message: string,
    readonly reason: string,
    options?: ErrorOptions,
  ) {
    super(message, options);
  }
}

/** fetch's own failure, shortly: the system's code when it has one ("fetch failed" says nothing). */
function shortCause(e: unknown): string {
  if (e instanceof Error && e.name === "TimeoutError") return "timed out";
  const cause: unknown = e instanceof Error ? e.cause : undefined;
  if (cause instanceof Error) {
    const code = (cause as NodeJS.ErrnoException).code;
    return typeof code === "string" && code ? code : cause.message;
  }
  return e instanceof Error ? e.message : String(e);
}

/**
 * The API refused the token (401) or the caller (403). Nothing this core does fixes it: the agent
 * decides whether that means signing in again. `message` is the API's own `error.message`.
 */
export class AuthError extends PushError {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

export type Fetch = typeof fetch;

/** Where and as whom a pass talks to the API. */
export type Api = {
  /** The API's origin, from `hello` (`https://api.ryna.io`). */
  url: string;
  /** The agent's access token, from this pass's `sync`. */
  token: string;
  /** `ryna-agent/<v> palworld/<v>`: the User-Agent. */
  userAgent: string;
  fetch?: Fetch;
  /** Aborted by `stop`. */
  signal?: AbortSignal;
};

/** The request body: the snapshot as a JSON field, then every file as a "file" part named for ingest. */
export function multipart(snapshot: Dict, raw: RawFiles): { body: Buffer; contentType: string } {
  const boundary = `palworld-${randomUUID().replace(/-/g, "")}`;
  const parts: Buffer[] = [];
  const part = (headers: string, body: Uint8Array) => {
    parts.push(Buffer.from(`--${boundary}\r\n${headers}\r\n\r\n`), Buffer.from(body), Buffer.from("\r\n"));
  };
  part('Content-Disposition: form-data; name="snapshot"\r\nContent-Type: application/json', Buffer.from(toJson(snapshot)));
  for (const [name, data] of Object.entries(raw)) {
    part(`Content-Disposition: form-data; name="file"; filename="${name}"\r\nContent-Type: application/octet-stream`, data);
  }
  parts.push(Buffer.from(`--${boundary}--\r\n`));
  return { body: Buffer.concat(parts), contentType: `multipart/form-data; boundary=${boundary}` };
}

/** The snapshot as JSON. A DateTime's ticks can still be a bigint in the tree; nothing the snapshot keeps is, but never let one crash a push. */
const toJson = (snapshot: unknown): string => JSON.stringify(snapshot, (_k, v: unknown) => (typeof v === "bigint" ? Number(v) : v));

/** The API's `error.message` from an envelope, if the body is one. */
function apiMessage(text: string): string | null {
  try {
    const body: unknown = JSON.parse(text);
    if (body !== null && typeof body === "object" && "error" in body) {
      const error: unknown = body.error;
      if (error !== null && typeof error === "object" && "message" in error && typeof error.message === "string" && error.message) return error.message;
    }
  } catch {
    // not JSON: the caller says what status it was
  }
  return null;
}

/**
 * `text` with the token taken out. Every message here quotes the API's answer, and an answer can
 * quote the request back; these messages are logged, kept in status.json and sent in the heartbeat.
 */
const clean = (text: string, api: Api): string => (api.token ? text.split(api.token).join("[token]") : text);

/** A 401 or 403 as an AuthError carrying the API's words; anything else passes. */
async function refusedAuth(res: Response, kind: string, api: Api): Promise<void> {
  if (res.status !== 401 && res.status !== 403) return;
  const text = await res.text().catch(() => "");
  throw new AuthError(clean(apiMessage(text) ?? `${kind} answered ${res.status}`, api), res.status);
}

async function send(api: Api, kind: "ingest" | "heartbeat", init: RequestInit, timeoutMs: number): Promise<Response> {
  const signals = [AbortSignal.timeout(timeoutMs), ...(api.signal ? [api.signal] : [])];
  try {
    return await (api.fetch ?? fetch)(`${api.url.replace(/\/+$/, "")}/v1/palworld/agent/${kind}`, {
      ...init,
      headers: { ...(init.headers as Record<string, string>), Authorization: `Bearer ${api.token}`, "User-Agent": api.userAgent },
      redirect: "follow",
      signal: AbortSignal.any(signals),
    });
  } catch (e) {
    // fetch's own words (a DNS failure, a timeout, an abort); they never carry the headers.
    throw new UnreachableError(clean(`${kind} unreachable: ${e instanceof Error ? e.message : String(e)}`, api), clean(shortCause(e), api), { cause: e });
  }
}

/** What a pass reports after itself: status.json's content plus a log tail. */
export type HeartbeatBody = { agent: string; version: string; lastRun: Dict; worlds: Dict; history: Dict[]; log: string };

/** Tell the API how the pass went. A failure here is the caller's to log, never to raise past the pass. */
export async function heartbeat(body: HeartbeatBody, api: Api): Promise<Dict> {
  const res = await send(api, "heartbeat", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) }, 30_000);
  await refusedAuth(res, "heartbeat", api);
  const text = clean(await res.text(), api);
  if (!res.ok) throw new PushError(`heartbeat answered ${res.status}: ${text.slice(0, 300)}`);
  let answer: Dict;
  try {
    answer = JSON.parse(text) as Dict;
  } catch {
    throw new PushError(`heartbeat answered ${res.status} from ${res.url}: ${JSON.stringify(text.slice(0, 200))}`);
  }
  if (!answer["ok"]) throw new PushError(`heartbeat refused: ${JSON.stringify(answer["error"])}`);
  return answer;
}

export async function push(snapshot: Dict, raw: RawFiles, api: Api): Promise<Dict> {
  const { body, contentType } = multipart(snapshot, raw);
  const res = await send(api, "ingest", { method: "POST", headers: { "Content-Type": contentType }, body }, 120_000);
  await refusedAuth(res, "ingest", api);
  const kind = (res.headers.get("content-type") ?? "").split(";")[0]!.trim();
  const answer = Buffer.from(clean(Buffer.from(await res.arrayBuffer()).toString("latin1"), api), "latin1");
  if (!res.ok) throw new PushError(`ingest answered ${res.status}: ${answer.subarray(0, 500).toString("utf8")}`);
  if (kind === "application/json") {
    try {
      return JSON.parse(answer.toString("utf8")) as Dict;
    } catch {
      // fall through: not the envelope
    }
  }
  // A login page or a proxy's answer: a 200 that isn't the envelope.
  throw new PushError(`ingest answered ${res.status} ${kind} from ${res.url}: ${JSON.stringify(answer.subarray(0, 200).toString("latin1"))}`);
}
