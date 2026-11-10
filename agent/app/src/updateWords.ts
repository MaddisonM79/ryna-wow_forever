// What the window says about updates. Pure, so the tests can hold it.
import type { Band, Support, UpdateCheck, UpdateProgress } from "./api";

export type UpdatePhase = "idle" | "checking" | "downloading" | "error";

/** How often a running app asks GitHub Releases, after the check at launch. */
export const UPDATE_INTERVAL_MS = 6 * 60 * 60 * 1000;

/** The Settings button: its words and whether it can be pressed. */
export function checkButton(check: UpdateCheck | null, phase: UpdatePhase): { label: string; disabled: boolean } {
  if (check && !check.configured) return { label: "Updates aren't set up in this build", disabled: true };
  if (phase === "checking") return { label: "Checking…", disabled: true };
  if (phase === "downloading") return { label: "Updating…", disabled: true };
  return { label: "Check for updates", disabled: false };
}

/** The banner's line, or null when there's nothing to offer (or it was put off with Later). */
export function bannerText(check: UpdateCheck | null, laterFor: string | null): string | null {
  const v = check?.available?.version;
  if (!v || v === laterFor) return null;
  return `Ryna ${v} is available`;
}

/** Maddison's words (2026-11-10): the same sentence in the app, the game and the portal. */
export const REDUCED_SENTENCE =
  "This version of Ryna is getting old. Your characters still sync, but what you add to everyone's census and prices is paused until you update.";
export const UNSUPPORTED_SENTENCE =
  "This version of Ryna is no longer supported, so nothing syncs to ryna.io. Update to carry on.";

/** The sentence for a band that has one. */
export function bandSentence(band: Band): string | null {
  if (band === "reduced") return REDUCED_SENTENCE;
  if (band === "unsupported") return UNSUPPORTED_SENTENCE;
  return null;
}

/** The update banner: what it says, in which colour, and what its buttons do. */
export interface Banner {
  /** `update` (behind: "Ryna x.y.z is available"), `reduced` (amber) or `unsupported` (red). */
  tone: "update" | "reduced" | "unsupported";
  text: string;
  /** The policy's note, as a second line. */
  note: string | null;
  /** The updater has a newer release, so "Update and restart" can install it. */
  canInstall: boolean;
  /** "Later" can put it off (never when unsupported). */
  dismissable: boolean;
  /** What "Later" puts off: a newer release, or a new band, shows it again. */
  key: string;
  /** Where to read how to update, when the updater has nothing to install. */
  guide: string | null;
}

/** Links the app may open (its open_link command allows these). */
const GUIDE = /^https:\/\/ryna\.io(\/|#|$)/;

/**
 * The banner, or null when there's nothing to say (or it was put off with Later). The API's
 * judgment (`support`) decides the colour and words; with none, or one this app can't read, it's
 * "current" and the banner is the updater's alone. It never stops anything: the API refuses, the
 * app only says so.
 */
export function banner(check: UpdateCheck | null, laterFor: string | null, support: Support | null): Banner | null {
  const band: Band = support?.band ?? "current";
  const available = check?.available?.version ?? null;
  const note = support && band !== "current" ? (support.note ?? null) : null;
  const sentence = bandSentence(band);
  if (sentence) {
    const tone = band === "unsupported" ? "unsupported" : "reduced";
    const key = `${band}:${available ?? support?.perfect ?? ""}`;
    if (tone === "reduced" && key === laterFor) return null;
    const guide = !available && support?.update && GUIDE.test(support.update) ? support.update : null;
    return { tone, text: sentence, note, canInstall: !!available, dismissable: tone === "reduced", key, guide };
  }
  const text = bannerText(check, laterFor);
  if (!text || !available) return null;
  return { tone: "update", text, note, canInstall: true, dismissable: true, key: available, guide: null };
}

/** What the activity log says when the API's judgment of this version changes; null for nothing. */
export function supportLog(support: Support, changed: boolean): string | null {
  if (!changed) return null;
  const sentence = bandSentence(support.band);
  if (!sentence) return null;
  return support.note ? `${sentence} (${support.note})` : sentence;
}

/** What a check adds to the activity log; an automatic check that found nothing adds nothing. */
export function checkLog(check: UpdateCheck, manual: boolean): string | null {
  if (!check.configured) return manual ? "Updates aren't set up in this build." : null;
  if (check.available) return `Ryna ${check.available.version} is available (this is ${check.current}).`;
  return manual ? `Ryna ${check.current} is up to date.` : null;
}

function mb(bytes: number): string {
  return (bytes / 1_000_000).toFixed(1);
}

export function progressText(p: UpdateProgress | null): string {
  if (!p) return "Downloading…";
  if (p.total && p.total > 0) {
    const pct = Math.min(100, Math.floor((p.downloaded / p.total) * 100));
    return `Downloading ${mb(p.downloaded)} of ${mb(p.total)} MB (${pct}%)`;
  }
  return `Downloading ${mb(p.downloaded)} MB`;
}

/** What the last check found, for beside the button: null before the first one finishes. */
export function checkResult(
  check: UpdateCheck | null,
  phase: UpdatePhase,
  error: string | null,
): { text: string; chip: "pushed" | "waiting" | "update" | "error" } | null {
  if (phase === "error" && error) return { text: `Couldn't check: ${error}`, chip: "error" };
  if (phase === "checking") return { text: "Checking…", chip: "waiting" };
  if (!check) return null;
  if (!check.configured) return { text: "Updates aren't set up in this build", chip: "waiting" };
  if (check.available) return { text: `Ryna ${check.available.version} is available`, chip: "update" };
  return { text: "Up to date", chip: "pushed" };
}
