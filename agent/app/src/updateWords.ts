// What the window says about updates. Pure, so the tests can hold it.
import type { UpdateCheck, UpdateProgress } from "./api";

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
