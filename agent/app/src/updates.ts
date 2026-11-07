// Updates: a check at launch and every 6 hours, the banner's "Update and restart", and the
// Settings button. The work is Rust's (src-tauri/src/updates.rs); this keeps what to show.
import { ref } from "vue";
import { api, errorText, onUpdateProgress, type UpdateCheck, type UpdateProgress } from "./api";
import { checkLog, UPDATE_INTERVAL_MS, type UpdatePhase } from "./updateWords";

export const updateCheck = ref<UpdateCheck | null>(null);
export const updatePhase = ref<UpdatePhase>("idle");
export const updateProgress = ref<UpdateProgress | null>(null);
export const updateError = ref<string | null>(null);
/** The version "Later" put off; a newer one, or a check from Settings, shows the banner again. */
export const laterFor = ref<string | null>(null);

type Log = (text: string, level?: "info" | "error") => void;
let log: Log = () => {};

export async function checkForUpdates(manual: boolean): Promise<void> {
  if (updatePhase.value === "checking" || updatePhase.value === "downloading") return;
  updatePhase.value = "checking";
  updateError.value = null;
  try {
    const check = await api.checkForUpdate();
    updateCheck.value = check;
    if (manual) laterFor.value = null;
    updatePhase.value = "idle";
    const text = checkLog(check, manual);
    if (text) log(text);
  } catch (e) {
    updatePhase.value = "error";
    updateError.value = errorText(e);
    log(`Couldn't check for updates: ${updateError.value}`, "error");
  }
}

export async function installUpdate(): Promise<void> {
  const version = updateCheck.value?.available?.version;
  updatePhase.value = "downloading";
  updateProgress.value = null;
  updateError.value = null;
  log(`Downloading Ryna ${version ?? "update"}; it restarts when it's installed.`);
  try {
    // On success this doesn't come back: the new version starts in its place.
    await api.installUpdate();
  } catch (e) {
    updatePhase.value = "error";
    updateError.value = errorText(e);
    log(`The update didn't install: ${updateError.value}`, "error");
  }
}

export function later(): void {
  const version = updateCheck.value?.available?.version ?? null;
  laterFor.value = version;
  if (version) log(`Ryna ${version} put off until later.`);
}

let started = false;

export async function startUpdates(logTo: Log): Promise<void> {
  if (started) return;
  started = true;
  log = logTo;
  await onUpdateProgress((p) => {
    updateProgress.value = p;
  });
  void checkForUpdates(false);
  setInterval(() => void checkForUpdates(false), UPDATE_INTERVAL_MS);
}
