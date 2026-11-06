// The window's one store: the status from Rust, kept fresh by the agent's events.
import { ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import {
  api,
  errorText,
  onAgentEvent,
  onLoginFinished,
  onWatchState,
  type AgentEvent,
  type LoginCode,
  type Phase,
  type Status,
} from "./api";
import { describe } from "./describe";

export interface LogLine {
  id: number;
  at: number;
  text: string;
  level: "info" | "error";
}

const LOG_SIZE = 50;

export const status = ref<Status | null>(null);
export const loadError = ref<string | null>(null);
export const activity = ref<LogLine[]>([]);
export const login = ref<LoginCode | null>(null);
export const loginBusy = ref(false);
export const loginError = ref<string | null>(null);
/** Ticks every 30 s so "5 minutes ago" moves on its own. */
export const now = ref(Date.now() / 1000);

let nextId = 1;

export function log(text: string, level: LogLine["level"] = "info"): void {
  activity.value = [{ id: nextId++, at: Date.now() / 1000, text, level }, ...activity.value].slice(0, LOG_SIZE);
}

export async function refresh(): Promise<void> {
  try {
    const s = await api.status();
    status.value = s;
    loadError.value = null;
    if (s.login && !login.value) login.value = s.login;
  } catch (e) {
    loadError.value = errorText(e);
  }
}

function setPhase(path: string, phase: Phase): void {
  const row = status.value?.accounts.find((a) => a.path === path);
  if (row) row.phase = phase;
}

let refreshTimer: ReturnType<typeof setTimeout> | null = null;
/** Several events arrive together at the end of a pass; read the status once after them. */
function refreshSoon(): void {
  if (refreshTimer) clearTimeout(refreshTimer);
  refreshTimer = setTimeout(() => {
    refreshTimer = null;
    void refresh();
  }, 300);
}

let lastSignedIn: boolean | null = null;

function onEvent(e: AgentEvent): void {
  if (e.event === "signedIn") {
    // Said once per change, not every pass.
    if (e.signedIn === lastSignedIn) return;
    lastSignedIn = e.signedIn;
  }
  const text = describe(e);
  const failed = e.event === "error" || e.event === "fightFailed" || (e.event === "heartbeat" && !e.ok);
  if (text) log(text, failed ? "error" : "info");
  switch (e.event) {
    case "pushing":
      setPhase(e.path, "pushing");
      break;
    case "pushed":
      setPhase(e.path, "pushed");
      refreshSoon();
      break;
    case "error":
      if (e.path) setPhase(e.path, "error");
      refreshSoon();
      break;
    case "installFound":
    case "accountFileChanged":
    case "passFinished":
    case "combatLogFound":
    case "fightUploaded":
    case "fightFailed":
      refreshSoon();
      break;
  }
}

export async function signIn(): Promise<void> {
  loginBusy.value = true;
  loginError.value = null;
  try {
    login.value = await api.startLogin();
    log("Waiting for you to allow the sign-in in your browser…");
  } catch (e) {
    loginError.value = errorText(e);
    loginBusy.value = false;
  }
}

/** Stop a sign-in that's waiting; `login-finished` (cancelled) tidies up after it. */
export async function cancelSignIn(): Promise<void> {
  try {
    await api.cancelLogin();
  } catch (e) {
    log(`Couldn't cancel the sign-in: ${errorText(e)}`, "error");
  }
  login.value = null;
  loginBusy.value = false;
}

export async function signOut(): Promise<void> {
  try {
    await api.logout();
    lastSignedIn = null;
    log("Signed out on this computer. Nothing syncs until you sign in again.");
  } catch (e) {
    log(`Couldn't sign out: ${errorText(e)}`, "error");
  }
  await refresh();
}

export async function start(): Promise<void> {
  await Promise.all([
    onAgentEvent(onEvent),
    onWatchState((watching) => {
      if (status.value) status.value.watching = watching;
    }),
    onLoginFinished((r) => {
      login.value = null;
      loginBusy.value = false;
      if (r.ok) {
        loginError.value = null;
        log("Signed in. Syncing starts now.");
      } else if (r.cancelled) {
        loginError.value = null;
        log("Sign-in cancelled.");
      } else {
        loginError.value = r.error ?? "The sign-in didn't finish.";
        log(`Sign-in didn't finish: ${loginError.value}`, "error");
      }
      void refresh();
    }),
  ]);
  setInterval(() => {
    now.value = Date.now() / 1000;
  }, 30_000);
  await refresh();
}

/** Point the agent at a folder, or back at the usual places with null. */
export async function setGameFolder(path: string | null): Promise<void> {
  try {
    await api.setInstallOverride(path);
    log(path ? `Looking for the game in ${path}` : "Looking for the game in the usual places.");
  } catch (e) {
    log(errorText(e), "error");
  }
  await refresh();
}

/** "Send fight digests" and "Keep raw logs on ryna.io". */
export async function setCombatSettings(digests: boolean, raw: boolean): Promise<void> {
  try {
    await api.setCombatSettings(digests, raw);
    log(
      `Fights: ${digests ? "sending digests" : "summaries only, no digests"}${raw ? ", keeping raw logs on ryna.io" : ""}.`,
    );
  } catch (e) {
    log(errorText(e), "error");
  }
  await refresh();
}

export async function chooseGameFolder(): Promise<void> {
  const picked = await open({ directory: true, multiple: false, title: "Choose the World of Warcraft folder" });
  if (typeof picked === "string") await setGameFolder(picked);
}
