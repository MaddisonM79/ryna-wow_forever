// The Settings mod's words and choices (the core's `settings`), pure so the tests can hold them.
import type {
  AddonSettingsView,
  BackupDone,
  CharacterRef,
  CopyPlan,
  DiffRow,
  GameState,
  InventoryView,
  ModView,
  PlannedWrite,
  RestoreReport,
  SettingsProgress,
  SettingsScope,
  Status,
} from "./api";

/** The mod's id (`mods/wow/settings/mod.json`). */
export const SETTINGS_MOD = "settings";

/** The one sentence about where backups go and how (docs/ryna-agent-cores-mods.md 7.2). */
export const STORED_ENCRYPTED = "Stored encrypted for you on ryna.io.";

/** What's said while the game runs, and every write is greyed out. */
export const GAME_RUNNING =
  "The game is running. Close it to copy or restore settings: it rewrites these files when you log out, so a change made now would be lost.";

/** The banner while writes are greyed out: the game running, or Ryna unable to tell. */
export function gameBanner(game: GameState | null | undefined): string | null {
  if (!game?.running) return null;
  return game.unknown
    ? `Ryna can't tell whether the game is running (${game.unknown}), so copies and restores are off until it can.`
    : GAME_RUNNING;
}

/** The game's own per-character files, as the core names them, and what each is. */
export const GAME_FILES: { name: string; label: string }[] = [
  { name: "config-cache.wtf", label: "Game options (CVars)" },
  { name: "bindings-cache.wtf", label: "Key bindings" },
  { name: "macros-cache.txt", label: "Macros" },
  { name: "chat-cache.txt", label: "Chat windows" },
  { name: "layout-local.txt", label: "Window layout" },
  { name: "AddOns.txt", label: "Addon list" },
];

/** Whether the Settings mod is on (its tab shows only then). */
export function settingsModOn(status: Status | null | undefined): boolean {
  return status?.mods.some((m: ModView) => m.id === SETTINGS_MOD && m.on) ?? false;
}

/** `Name-Realm`, as the core writes a character. */
export function charName(c: CharacterRef): string {
  return `${c.name}-${c.realm}`;
}

export function sameChar(a: CharacterRef, b: CharacterRef): boolean {
  return a.name.toLowerCase() === b.name.toLowerCase() && a.realm.toLowerCase() === b.realm.toLowerCase();
}

export function sizeText(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function scopeText(scope: SettingsScope): string {
  switch (scope) {
    case "account":
      return "Account";
    case "character":
      return "Per character";
    default:
      return "Account and per character";
  }
}

/** The account's label in a picker: its folder, and the install when there's more than one. */
export function inventoryLabel(inv: InventoryView, many: boolean): string {
  return many ? `${inv.account} (${inv.install})` : inv.account;
}

/**
 * The addons a copy from `from` has something to take: its own file, or an AceDB file that names
 * it. These are the boxes the "Copy from…" flow ticks to begin with.
 */
export function copyableAddons(inv: InventoryView, from: CharacterRef): AddonSettingsView[] {
  return inv.addons.filter((a) => a.characters.some((c) => sameChar(c, from)));
}

/** The game files `from` has. */
export function copyableGameFiles(inv: InventoryView, from: CharacterRef): string[] {
  return GAME_FILES.map((g) => g.name).filter((name) =>
    inv.gameFiles.some((f) => f.character && sameChar(f.character, from) && f.path.endsWith(`/${name}`)),
  );
}

function writeLine(w: PlannedWrite, plan: CopyPlan): string {
  const how =
    w.how === "file"
      ? `copied from ${charName(plan.from)}`
      : `${charName(plan.to)}'s entries made from ${charName(plan.from)}'s`;
  const kept = w.replaces ? `; the file there is kept as ${w.path.split("/").pop()}.ryna.bak` : "";
  return `${w.path} (${w.item}: ${how}${kept})`;
}

/** The preview: every file the copy writes, and what it leaves and why. */
export function copyPreviewLines(plan: CopyPlan): string[] {
  const lines = plan.writes.map((w) => writeLine(w, plan));
  if (plan.newFolder) lines.unshift(`${charName(plan.to)} has no folder yet: it's made, and the game uses it at their first login.`);
  for (const s of plan.skipped) lines.push(`Left ${s.item}: ${s.reason}.`);
  return lines;
}

/** What a finished copy says. */
export function copyDoneLine(plan: CopyPlan, written: number, backups: number): string {
  const kept = backups ? `, ${backups === 1 ? "1 replaced file" : `${backups} replaced files`} kept as .ryna.bak` : "";
  return `Copied ${charName(plan.from)}'s settings to ${charName(plan.to)}: ${written === 1 ? "1 file" : `${written} files`} written${kept}.`;
}

export function diffText(row: DiffRow, a: CharacterRef, b: CharacterRef): string {
  switch (row.difference) {
    case "same":
      return "Same";
    case "differ":
      return "Different";
    case "onlyA":
      return `Only ${charName(a)}`;
    default:
      return `Only ${charName(b)}`;
  }
}

export function progressText(p: SettingsProgress): string {
  switch (p.stage) {
    case "building":
      return "Packing the files…";
    case "uploading":
      return p.total > 1 ? `Uploading part ${p.done + 1} of ${p.total}…` : "Uploading…";
    case "completing":
      return "Finishing…";
    case "downloading":
      return "Downloading…";
    case "checking":
      return "Checking the backup…";
    case "writing":
      return `Writing file ${p.done + 1} of ${p.total}…`;
    default:
      return "Working…";
  }
}

export function backupDoneLine(d: BackupDone): string {
  return `Backed up ${d.files === 1 ? "1 file" : `${d.files} files`} (${sizeText(d.bytes)}). ${STORED_ENCRYPTED}`;
}

/** A backup's time, from the API's `createdAt` (unix seconds, milliseconds, or ISO text). */
export function backupWhen(createdAt: number | string | null): string {
  if (createdAt === null) return "";
  const ms = typeof createdAt === "number" ? (createdAt < 1e12 ? createdAt * 1000 : createdAt) : Date.parse(createdAt);
  return Number.isNaN(ms) ? String(createdAt) : new Date(ms).toLocaleString();
}

export function restorePreviewLines(r: RestoreReport): string[] {
  return r.files.map(
    (f) => `${f.path} (${sizeText(f.bytes)})${f.replaces ? `; the file there is kept as ${f.path.split("/").pop()}.ryna.bak` : ""}`,
  );
}

export function restoreDoneLine(r: RestoreReport): string {
  const n = r.files.length;
  const kept = r.backups.length ? `, ${r.backups.length} kept as .ryna.bak` : "";
  return `Restored ${n === 1 ? "1 file" : `${n} files`} into ${r.account}${kept}.`;
}
