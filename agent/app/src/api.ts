// The Rust side's commands and events (src-tauri/src/commands.rs, agent.rs), typed.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Phase = "waiting" | "pushing" | "pushed" | "error";
/** What fixes an error (the core's `Fix`). */
export type Fix = "signIn" | "update" | "network" | "other";

/** How an install's addon compares with the bundled one (the core's `AddonState`). */
export type AddonState = "notInstalled" | "current" | "older" | "newer" | "unknown";

/** The core's `InstallStatus`. */
export interface InstallView {
  path: string;
  flavor: string | null;
  forever: boolean;
  hasAddon: boolean;
  /** The installed addon's `## Version:`, when it's there and says. */
  addonVersion: string | null;
  /** How that compares with `Status.addonBundled`. */
  addon: AddonState;
}

/** The core's `AccountStatus`, plus where the loop has it. */
export interface AccountView {
  path: string;
  install: string;
  key: string;
  label: string;
  savedAt: number | null;
  pushedAt: number | null;
  characters: number | null;
  lastError: string | null;
  phase: Phase;
}

export interface LoginCode {
  userCode: string;
  verificationUri: string;
  verificationUriComplete: string | null;
  link: string;
  /** Unix seconds. */
  expiresAt: number;
  interval: number;
}

export interface LastRun {
  at: number;
  ok: boolean;
  error: string | null;
  fix: Fix | null;
  /** The words to show for the failure. */
  advice: string | null;
}

/** A fight that went up (the core's `UploadedFight`). */
export interface UploadedFight {
  /** Unix seconds. */
  at: number;
  key: string;
  kind: "boss" | "trash" | "world" | string;
  name: string | null;
  zone: string | null;
  durationMs: number;
  players: number;
}

/** The combat log's side (the core's `CombatStatus`). */
export interface CombatStatus {
  active: boolean;
  advanced: boolean | null;
  /** The newest log's file name. */
  file: string | null;
  /** Unix seconds. */
  lastEventAt: number | null;
  /** Fights waiting to upload, plus encounters still open. */
  pending: number;
  /** Fights uploaded in the last 12 hours. */
  fightsTonight: number;
  lastFight: UploadedFight | null;
  lastError: string | null;
}

export interface Status {
  agentVersion: string;
  appVersion: string;
  api: string;
  issuer: string;
  clientIdIssued: boolean;
  signedIn: boolean;
  signedInAs: string | null;
  keychainError: string | null;
  watching: boolean;
  installs: InstallView[];
  /** The bundled addon's version, when this build carries one that says. */
  addonBundled: string | null;
  accounts: AccountView[];
  lastError: string | null;
  lastRun: LastRun | null;
  installOverride: string | null;
  login: LoginCode | null;
  stateDir: string | null;
  combat: CombatStatus;
  /** "Send fight digests". */
  sendDigests: boolean;
  /** "Keep raw logs on ryna.io". */
  keepRaw: boolean;
  autostart: boolean;
}

/** The core's `WatchEvent`, as serialised (`{"event": "pushed", ...}`). */
export type AgentEvent =
  | { event: "signedIn"; signedIn: boolean }
  | { event: "installFound"; path: string; flavor: string | null }
  | { event: "accountFileChanged"; label: string; path: string; savedAt: number }
  | { event: "pushing"; label: string; path: string }
  | {
      event: "pushed";
      label: string;
      path: string;
      characters: number;
      result: Record<string, unknown>;
    }
  | {
      event: "error";
      label: string | null;
      path: string | null;
      /** The error itself, for the curious. */
      message: string;
      fix: Fix;
      /** The words to show: what fixes it, or for "other" the error itself. */
      advice: string;
    }
  | { event: "heartbeat"; ok: boolean; error: string | null }
  | { event: "passFinished"; ok: boolean; nextInSecs: number }
  | { event: "combatLogFound"; file: string; path: string }
  | {
      event: "fightQueued";
      key: string;
      kind: string;
      zone: string | null;
      name: string | null;
      durationMs: number;
      players: number;
    }
  | {
      event: "fightUploaded";
      key: string;
      kind: string;
      zone: string | null;
      name: string | null;
      durationMs: number;
      players: number;
      id: unknown;
      merged: boolean;
    }
  | {
      event: "fightFailed";
      key: string;
      /** "Zephras Isle, 3 min, 2 players", or "Fights" for the queue as a whole. */
      fight: string;
      message: string;
      fix: Fix;
      advice: string;
    };

export interface LoginFinished {
  ok: boolean;
  /** Ended by `cancelLogin`. */
  cancelled: boolean;
  error: string | null;
  fix: Fix | null;
}

export interface Installed {
  /** The install it went into (an `InstallView.path`). */
  install: string;
  /** The addon's folder there. */
  path: string;
}

/** What `install_addon` did. */
export interface InstallResult {
  version: string | null;
  installed: Installed[];
  /** The game was running, so it needs a /reload to load the new copy. */
  gameRunning: boolean;
}

/** What an update check found (updates.rs `UpdateCheck`). */
export interface UpdateCheck {
  /** False in a build without an updater key: it never checks. */
  configured: boolean;
  current: string;
  available: { version: string; notes: string | null; date: string | null } | null;
}

/** While an update downloads (`update-progress`). */
export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export const api = {
  status: () => invoke<Status>("get_status"),
  startLogin: () => invoke<LoginCode>("start_login"),
  cancelLogin: () => invoke<void>("cancel_login"),
  openSignInPage: () => invoke<void>("open_sign_in_page"),
  logout: () => invoke<void>("logout"),
  startWatch: () => invoke<void>("start_watch"),
  stopWatch: () => invoke<void>("stop_watch"),
  syncNow: () => invoke<void>("sync_now"),
  setInstallOverride: (path: string | null) => invoke<void>("set_install_override", { path }),
  setCombatSettings: (digests: boolean, raw: boolean) =>
    invoke<void>("set_combat_settings", { digests, raw }),
  /** Into the install at `path`, or every install it belongs in when null. */
  installAddon: (path: string | null = null) => invoke<InstallResult>("install_addon", { path }),
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  openPortal: () => invoke<void>("open_portal"),
  checkForUpdate: () => invoke<UpdateCheck>("check_for_update"),
  /** Resolves only if it failed to install: on success the app restarts into the new version. */
  installUpdate: () => invoke<void>("install_update"),
};

export function onAgentEvent(f: (e: AgentEvent) => void): Promise<UnlistenFn> {
  return listen<AgentEvent>("agent-event", (e) => f(e.payload));
}

export function onWatchState(f: (watching: boolean) => void): Promise<UnlistenFn> {
  return listen<boolean>("watch-state", (e) => f(e.payload));
}

export function onLoginFinished(f: (r: LoginFinished) => void): Promise<UnlistenFn> {
  return listen<LoginFinished>("login-finished", (e) => f(e.payload));
}

export function onUpdateProgress(f: (p: UpdateProgress) => void): Promise<UnlistenFn> {
  return listen<UpdateProgress>("update-progress", (e) => f(e.payload));
}

/** An error from `invoke` (the commands reject with a plain string). */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
