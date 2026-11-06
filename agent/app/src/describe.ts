// Agent events in plain words, for the activity log. Null means "not worth a line".
import type { AgentEvent, Failure } from "./api";

const FLAVORS: Record<string, string> = {
  wowf: "World of Warcraft: Forever",
  wow: "World of Warcraft",
  wow_classic_era: "WoW Classic Era",
  wow_classic: "WoW Classic",
};

export function flavorName(flavor: string | null): string {
  if (!flavor) return "an unknown game";
  return FLAVORS[flavor] ?? flavor;
}

function plural(n: number, one: string, many = `${one}s`): string {
  return `${n.toLocaleString("en")} ${n === 1 ? one : many}`;
}

function num(v: unknown): number | null {
  return typeof v === "number" && Number.isFinite(v) ? v : null;
}

/** What the person can do about an error, in their words. */
export function errorWords(message: string, failure: Failure): string {
  if (failure === "signIn") return "Sign in again: the app's sign-in has lapsed.";
  if (failure === "update") return "Update the app and the addon: the server no longer takes this version.";
  if (/unreachable|failing after/i.test(message)) {
    return "Couldn't reach the API, retrying.";
  }
  return message;
}

export function describe(e: AgentEvent): string | null {
  switch (e.event) {
    case "signedIn":
      return e.signedIn ? null : "Not signed in: sign in to start syncing.";
    case "installFound":
      return `Found ${flavorName(e.flavor)} at ${e.path}`;
    case "accountFileChanged":
      return `${e.label}: the game saved new data.`;
    case "pushing":
      return `Pushing ${e.label}…`;
    case "pushed": {
      const parts = [plural(e.characters, "character")];
      const journal = e.result["journal"];
      if (journal && typeof journal === "object") {
        const j = journal as Record<string, unknown>;
        const stored = num(j["stored"]);
        const repeated = num(j["repeated"]);
        if (stored !== null && repeated !== null) {
          parts.push(plural(stored + repeated, "journal event"), `${repeated.toLocaleString("en")} repeated`);
        } else if (stored !== null) {
          parts.push(plural(stored, "journal event"));
        }
      }
      return `Pushed ${e.label}: ${parts.join(", ")}`;
    }
    case "error": {
      const words = errorWords(e.message, e.failure);
      return e.label ? `${e.label}: ${words}` : words;
    }
    case "heartbeat":
      return e.ok ? null : `Couldn't report in to the API${e.error ? `: ${e.error}` : ""}.`;
    case "passFinished":
      return null;
  }
}

/** "3 minutes ago", "just now", "never". */
export function ago(unixSecs: number | null, now = Date.now() / 1000): string {
  if (unixSecs === null) return "never";
  const s = Math.max(0, Math.round(now - unixSecs));
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${plural(m, "minute")} ago`;
  const h = Math.round(m / 60);
  if (h < 36) return `${plural(h, "hour")} ago`;
  return `${plural(Math.round(h / 24), "day")} ago`;
}

export function when(unixSecs: number | null): string {
  if (unixSecs === null) return "";
  return new Date(unixSecs * 1000).toLocaleString();
}
