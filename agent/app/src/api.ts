// The Rust side's commands and events (src-tauri/src/commands.rs, agent.rs), typed.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Phase = "waiting" | "pushing" | "pushed" | "error";
/** What fixes an error (the core's `Failure`). */
export type Failure = "signIn" | "update" | "other";

export interface InstallView {
  path: string;
  flavor: string | null;
  forever: boolean;
  hasAddon: boolean;
}

export interface AccountView {
  path: string;
  install: string;
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
  link: string;
  expiresIn: number;
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
  accounts: AccountView[];
  lastError: string | null;
  lastRun: { at: number; ok: boolean; error: string | null } | null;
  installOverride: string | null;
  login: LoginCode | null;
  stateDir: string | null;
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
      message: string;
      failure: Failure;
    }
  | { event: "heartbeat"; ok: boolean; error: string | null }
  | { event: "passFinished"; ok: boolean; nextInSecs: number };

export interface LoginFinished {
  ok: boolean;
  error: string | null;
}

export interface Installed {
  path: string;
}

export const api = {
  status: () => invoke<Status>("get_status"),
  startLogin: () => invoke<LoginCode>("start_login"),
  openSignInPage: () => invoke<void>("open_sign_in_page"),
  logout: () => invoke<void>("logout"),
  startWatch: () => invoke<void>("start_watch"),
  stopWatch: () => invoke<void>("stop_watch"),
  syncNow: () => invoke<void>("sync_now"),
  setInstallOverride: (path: string | null) => invoke<void>("set_install_override", { path }),
  installAddon: () => invoke<Installed[]>("install_addon"),
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  openPortal: () => invoke<void>("open_portal"),
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

/** An error from `invoke` (the commands reject with a plain string). */
export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
