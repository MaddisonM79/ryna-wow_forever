// Agent events in plain words, for the activity log. Null means "not worth a line".
import type { AgentEvent, InstallResult, InstallView } from "./api";

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
    case "error":
      // The core's words for it (what fixes it, or the error itself).
      return e.label ? `${e.label}: ${e.advice}` : e.advice;
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

/** What an install's row says about the addon, and what its button does. */
export interface AddonRow {
  text: string;
  /** The chip's style: ok, waiting, or update (amber). */
  chip: "pushed" | "waiting" | "update";
  /** The button's words; null when there's nothing to install from. */
  action: string | null;
  primary: boolean;
}

/** One line and one button per install, from the core's comparison with the bundled addon. */
export function addonRow(i: InstallView, bundled: string | null): AddonRow {
  const action = (words: string): string | null => (bundled ? words : null);
  switch (i.addon) {
    case "notInstalled":
      return {
        text: "Addon not installed",
        chip: "waiting",
        action: action(`Install addon ${bundled}`),
        primary: true,
      };
    case "current":
      return {
        text: `Addon ${i.addonVersion} installed`,
        chip: "pushed",
        action: action("Reinstall"),
        primary: false,
      };
    case "older":
      return {
        text: `Addon ${i.addonVersion} installed, ${bundled} available`,
        chip: "update",
        action: action(`Update to ${bundled}`),
        primary: true,
      };
    case "newer":
      return {
        text: `Addon ${i.addonVersion} installed (this app bundles ${bundled})`,
        chip: "pushed",
        action: action(`Reinstall ${bundled}`),
        primary: false,
      };
    case "unknown":
      // Installed, but one side names no version: an unversioned copy predates versioning.
      if (i.addonVersion) {
        return { text: `Addon ${i.addonVersion} installed`, chip: "pushed", action: null, primary: false };
      }
      return {
        text: bundled ? `Addon installed (version unknown), ${bundled} available` : "Addon installed",
        chip: bundled ? "update" : "pushed",
        action: action(`Update to ${bundled}`),
        primary: true,
      };
  }
}

/** The activity log's lines for an install: "Installed addon 0.2.0 into <folder>". */
export function installedLines(r: InstallResult): string[] {
  const what = r.version ? `addon ${r.version}` : "the addon";
  return r.installed.map((d) => `Installed ${what} into ${d.path}`);
}

/** What the row says after an install: what went in, and how the game picks it up. */
export function installedMessage(r: InstallResult): string[] {
  const lines = [
    `Installed ${r.version ?? "the addon"}. The game loads it at the next login or /reload.`,
  ];
  if (r.gameRunning) lines.push("The game is running: /reload or log out and in to load it.");
  return lines;
}
