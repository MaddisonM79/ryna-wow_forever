// A fetched core's words: the one panel its page shows above its rows (Allow, Getting…, Retry,
// not on this computer yet, stopped, off), its events as activity lines, and how many cores are
// running. Pure, so the tests hold every state's words. Kept apart from the components so the
// states can be restyled without touching what they say.
import type { CoreState, CoreView, SidecarEvent } from "./api";

/** What the panel's button does. */
export type PanelAction = "allow" | "retry" | "start";

export interface CorePanel {
  /** The state, for styling: one class per state. */
  state: CoreState["is"];
  /** `banner` tone: plain, `update` (amber) or `error` (red). */
  tone: "" | "update" | "error";
  title: string;
  text: string;
  /** 0..1 while fetching with a known size; null otherwise. */
  progress: number | null;
  action: { label: string; does: PanelAction } | null;
}

const PLATFORM_NAMES: Record<string, string> = { macos: "macOS", windows: "Windows", linux: "Linux" };

function list(names: string[]): string {
  if (names.length <= 1) return names[0] ?? "";
  return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/** "macOS", "macOS and Windows". */
export function platformWords(platforms: string[]): string {
  return list(platforms.map((p) => PLATFORM_NAMES[p] ?? p));
}

/** Bytes as MB with one decimal ("12.3 MB"). */
export function megabytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * The panel a core's page shows above its rows, or null when it's running and there is
 * nothing to say. The words are the window's; Maddison reviews a mockup of the three new states
 * (Allow, Getting…, not on this computer yet) before they ship.
 */
export function corePanel(core: Pick<CoreView, "name" | "state">): CorePanel | null {
  const { name, state } = core;
  switch (state.is) {
    case "running":
      return null;
    case "allow":
      return {
        state: state.is,
        tone: "update",
        title: `Allow ${name} on this computer`,
        text: `Your ryna.io account has ${name}. Allowing it is one approval in your browser, for every game your account has. Nothing of ${name} is downloaded or runs until then.`,
        progress: null,
        action: { label: `Allow ${name} on this computer`, does: "allow" },
      };
    case "fetching":
      return {
        state: state.is,
        tone: "",
        title: `Getting ${name}…`,
        text:
          state.total !== null && state.total > 0
            ? `${megabytes(state.done)} of ${megabytes(state.total)}`
            : state.done > 0
              ? `${megabytes(state.done)} so far`
              : "Starting the download",
        progress: state.total !== null && state.total > 0 ? Math.min(1, state.done / state.total) : null,
        action: null,
      };
    case "fetchFailed":
      return {
        state: state.is,
        tone: "error",
        title: `Couldn't get ${name}`,
        text: `${sentence(state.message)} Ryna tries again every hour.`,
        progress: null,
        action: { label: "Retry", does: "retry" },
      };
    case "notHere":
      return {
        state: state.is,
        tone: "",
        title: `${name} isn't on this computer yet`,
        text: `Your account has ${name}, but for now it runs on ${platformWords(state.platforms)} only.`,
        progress: null,
        action: null,
      };
    case "signedOut":
      return {
        state: state.is,
        tone: "",
        title: `Sign in to run ${name}`,
        text: "Nothing syncs until you sign in.",
        progress: null,
        action: null,
      };
    case "off":
      return {
        state: state.is,
        tone: "",
        title: `${name} is off on this computer`,
        text: "Turn it on in Settings below to start syncing.",
        progress: null,
        action: null,
      };
    case "stopped":
      return {
        state: state.is,
        tone: state.message ? "error" : "",
        title: `${name} stopped`,
        text: state.message
          ? [sentence(state.message), state.fix ? sentence(state.fix) : ""].filter(Boolean).join(" ")
          : "It isn't running.",
        progress: null,
        action: { label: "Start again", does: "start" },
      };
  }
}

/** A capital first letter and a full stop. */
function sentence(s: string): string {
  const t = s.trim();
  if (!t) return t;
  const cap = t[0]!.toUpperCase() + t.slice(1);
  return /[.!?…]$/.test(cap) ? cap : `${cap}.`;
}

/** A sidecar core's event as an activity line, named for its core; null for none. */
export function describeCoreEvent(name: string, e: SidecarEvent): { text: string; level: "info" | "error" } | null {
  switch (e.event) {
    case "started":
      return { text: `${name} started${e.version ? ` (version ${e.version})` : ""}.`, level: "info" };
    case "event":
      return e.message ? { text: `${name}: ${e.message}`, level: e.kind === "error" ? "error" : "info" } : null;
    case "status":
      return null;
    case "passFinished":
      return e.ok ? null : { text: `${name}: the last sync didn't finish.`, level: "error" };
    case "failed":
      return { text: `${name}: ${e.advice}`, level: "error" };
    case "exited":
      return { text: `${name} stopped unexpectedly (${e.message}). Starting it again within a minute.`, level: "error" };
    case "stopped":
      return {
        text: `${name} stopped: ${[e.message, e.fix].filter(Boolean).join(". ")}`,
        level: "error",
      };
  }
}

/** How many cores are running: WoW's loop, and each fetched core that is. */
export function runningCores(s: { watching: boolean; cores: Pick<CoreView, "kind" | "state">[] }): number {
  const sidecars = s.cores.filter((c) => c.kind === "sidecar" && c.state.is === "running").length;
  const wow = s.cores.some((c) => c.kind === "builtin") && s.watching ? 1 : 0;
  return wow + sidecars;
}

/** The first fetched core's failure, named: "Palworld: <words>". */
export function sidecarProblem(cores: Pick<CoreView, "kind" | "name" | "state" | "lastError">[]): string | null {
  for (const c of cores) {
    if (c.kind !== "sidecar") continue;
    if (c.state.is === "stopped" && c.state.message) return `${c.name}: ${c.state.message}`;
    if (c.state.is === "fetchFailed") return `${c.name}: couldn't get it`;
    if (c.state.is === "running" && c.lastError) return `${c.name}: ${c.lastError}`;
  }
  return null;
}

/** The activity lines about one fetched core, for its page. */
export function coreLines<L extends { core?: string }>(lines: L[], core: string): L[] {
  return lines.filter((l) => l.core === core);
}
