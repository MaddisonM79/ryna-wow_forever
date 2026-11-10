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

/** One row of a core's status card (`ryna_agent::core::StatusRow`). */
export interface StatusRow {
  label: string;
  value: string;
  chip: "pushed" | "waiting" | "update" | "error" | null;
}

/** A core's card, as the core lays it out through the agent's interface (not drawn yet). */
export interface CoreView {
  id: string;
  name: string;
  version: string;
  rows: StatusRow[];
}

/** One install's copy of a mod (the core's `ModInstall`). */
export interface ModInstall {
  /** The install (an `InstallView.path`). */
  install: string;
  installed: boolean;
  /** Its addon folder's `## Version:`, when it's there and says. */
  version: string | null;
  /** How many characters the game has it turned off for (its own AddOns list). */
  offInGame: number;
}

/** A WoW mod (the core's `ModStatus`). */
export interface ModView {
  id: string;
  name: string;
  summary: string;
  /** `addon` (folders in the game) or `agent` (the app's own work: nothing to install). */
  kind: "addon" | "agent" | string;
  /** It does something in the game for the player: said under its toggle, every time. */
  actsInGame: boolean;
  defaultOn: boolean;
  on: boolean;
  /** The version this app would install. */
  bundled: string;
  addons: string[];
  installs: ModInstall[];
}

/** What happened to one mod folder (the core's `ModChange`). */
export type ModChange =
  | { change: "installed"; id: string; name: string; folder: string; path: string; version: string | null }
  | { change: "removed"; id: string; name: string; folder: string; path: string };

/** What `set_mod_enabled` did. */
export interface ModToggleResult {
  id: string;
  name: string;
  on: boolean;
  changes: ModChange[];
  /** Installs without the addon yet: the mod goes in with it. */
  waitingForAddon: string[];
  gameRunning: boolean;
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

/** Where this version stands against the API's marks (the core's `support::Band`). */
export type Band = "current" | "behind" | "reduced" | "unsupported";

/** What the API last said about this version (the core's `support::Support`). */
export interface Support {
  band: Band;
  /** The app's version, as judged. */
  version: string | null;
  /** The oldest core addon judged with it (an install's TOC). */
  addon: string | null;
  channel: string | null;
  minimum: string | null;
  preferred: string | null;
  /** The newest release on the channel. */
  perfect: string | null;
  /** The policy's one line for people. */
  note: string | null;
  /** Where to read how to update. */
  update: string | null;
}

/** That judgment as kept (the core's `support::KnownSupport`). */
export interface KnownSupport {
  support: Support;
  /** Unix seconds. */
  receivedAt: number;
  agent: string;
  pooled: boolean | null;
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
  /** The sign-in lasts only until a restart: Linux with no Secret Service running. */
  signInUntilReboot: boolean;
  /** The device key couldn't be read or kept (the keychain's words): the header shows Problem. */
  deviceKeyError: string | null;
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
  /** What the API last said about this version, or null when it hasn't (then: current). */
  support: KnownSupport | null;
  /** Each core's card through the core interface (stage 1: carried, not drawn). */
  cores: CoreView[];
  /** The WoW core's mods. */
  mods: ModView[];
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
  /** What the API said about this version this pass; `changed` when the band moved. */
  | { event: "support"; support: Support; receivedAt: number; changed: boolean }
  /** Reduced mode began: `label` synced, its contributions to the pools paused. Said once. */
  | { event: "contributionsPaused"; label: string }
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
  /** Mod folders installed (on mods) or removed (off ones) with it. */
  mods: ModChange[];
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

/** The Settings mod's types (the core's `settings::inventory`, `copy`, `backup`). */
export interface CharacterRef {
  name: string;
  realm: string;
}

export type SettingsScope = "account" | "character" | "both";

export interface SettingsFileView {
  /** Relative to the account's folder: `SavedVariables/ElvUI.lua`, `Stormcall/Aerin/config-cache.wtf`. */
  path: string;
  character: CharacterRef | null;
  bytes: number;
  /** Unix seconds. */
  modified: number | null;
}

export interface AddonSettingsView {
  addon: string;
  title: string | null;
  installed: boolean;
  accountVariables: string[];
  characterVariables: string[];
  scope: SettingsScope;
  files: SettingsFileView[];
  characters: CharacterRef[];
  /** Its account file is AceDB: a copy edits it for the target character. */
  acedb: boolean;
  bytes: number;
  modified: number | null;
}

export interface InventoryView {
  install: string;
  flavor: string | null;
  account: string;
  accountKey: string;
  characters: CharacterRef[];
  characterFolders: CharacterRef[];
  addons: AddonSettingsView[];
  gameFiles: SettingsFileView[];
}

/** Whether the game is running; `unknown` (why it can't be told) counts as running. */
export interface GameState {
  running: boolean;
  unknown: string | null;
}

export interface SettingsView {
  on: boolean;
  inventories: InventoryView[];
  game: GameState;
  problems: string[];
}

export interface DiffRow {
  item: string;
  gameFile: boolean;
  difference: "same" | "differ" | "onlyA" | "onlyB";
}

export type PlannedWrite = {
  item: string;
  path: string;
  replaces: boolean;
  bytes: number;
} & ({ how: "file"; from: string } | { how: "aceDb"; variables: string[] });

export interface CopyPlan {
  from: CharacterRef;
  to: CharacterRef;
  writes: PlannedWrite[];
  skipped: { item: string; reason: string }[];
  newFolder: boolean;
}

export interface CopyResult {
  plan: CopyPlan;
  /** Null for a preview. */
  applied: { written: string[]; backups: string[] } | null;
  gameRunning: boolean;
}

export interface BackupDone {
  id: string;
  createdAt: number | string | null;
  bytes: number;
  files: number;
  parts: number;
}

export interface BackupSummary {
  id: string;
  createdAt: number | string | null;
  bytes: number;
  machine: string | null;
  flavor: string | null;
  account: string | null;
  files: number;
}

export interface RestoreReport {
  id: string;
  account: string;
  files: { path: string; bytes: number; replaces: boolean }[];
  written: boolean;
  backups: string[];
}

export interface SettingsProgress {
  stage: "building" | "uploading" | "completing" | "downloading" | "checking" | "writing" | string;
  done: number;
  total: number;
}

export const api = {
  status: () => invoke<Status>("get_status"),
  /** This computer's submission ID, grouped (`a1b2-c3d4-…`); rejects with the keychain's words. */
  submissionId: () => invoke<string>("get_submission_id"),
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
  /** Turns a mod on or off, and installs or removes its folders. */
  setModEnabled: (id: string, on: boolean) => invoke<ModToggleResult>("set_mod_enabled", { id, on }),
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  openPortal: () => invoke<void>("open_portal"),
  /** Opens one of the project's own pages (GitHub, ryna.io) in the browser; anything else is refused by the command. */
  openLink: (url: string) => invoke<void>("open_link", { url }),
  checkForUpdate: () => invoke<UpdateCheck>("check_for_update"),
  /** Resolves only if it failed to install: on success the app restarts into the new version. */
  installUpdate: () => invoke<void>("install_update"),
  settingsView: () => invoke<SettingsView>("settings_view"),
  settingsGameRunning: () => invoke<GameState>("settings_game_running"),
  settingsDiff: (install: string, account: string, a: string, b: string) =>
    invoke<DiffRow[]>("settings_diff", { install, account, a, b }),
  /** A preview when `preview`, else written (refused while the game runs). */
  settingsCopy: (install: string, account: string, from: string, to: string, items: string[], preview: boolean) =>
    invoke<CopyResult>("settings_copy", { install, account, from, to, items, preview }),
  settingsBackup: (install: string, account: string, addons: boolean, game: boolean) =>
    invoke<BackupDone>("settings_backup", { install, account, addons, game }),
  settingsBackups: () => invoke<BackupSummary[]>("settings_backups"),
  /** Only into `account`, and only when the backup was made from it. */
  settingsRestore: (install: string, id: string, account: string, preview: boolean) =>
    invoke<RestoreReport>("settings_restore", { install, id, account, preview }),
  settingsDelete: (id: string) => invoke<void>("settings_delete", { id }),
};

export function onSettingsProgress(f: (p: SettingsProgress) => void): Promise<UnlistenFn> {
  return listen<SettingsProgress>("settings-progress", (e) => f(e.payload));
}

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
