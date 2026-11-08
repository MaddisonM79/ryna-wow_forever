<script setup lang="ts">
// The Settings mod's tab (shown while the mod is on): every addon's settings on the account, copying
// them from one character to another, and backups to ryna.io. All of it works on files with the
// game closed; while the game runs every write is greyed out. Nothing is ever said in the game.
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  api,
  errorText,
  onSettingsProgress,
  type BackupSummary,
  type CopyResult,
  type DiffRow,
  type InventoryView,
  type RestoreReport,
  type SettingsView,
} from "../api";
import { when } from "../describe";
import {
  backupDoneLine,
  backupWhen,
  charName,
  copyableAddons,
  copyableGameFiles,
  copyDoneLine,
  copyPreviewLines,
  diffText,
  GAME_FILES,
  GAME_RUNNING,
  inventoryLabel,
  progressText,
  restoreDoneLine,
  restorePreviewLines,
  scopeText,
  sizeText,
  STORED_ENCRYPTED,
} from "../settingsWords";
import { log, status } from "../store";

const props = defineProps<{ visible: boolean }>();

const view = ref<SettingsView | null>(null);
const loadError = ref<string | null>(null);
const picked = ref(0);
const inv = computed<InventoryView | null>(() => view.value?.inventories[picked.value] ?? null);
const gameRunning = ref(false);
/** Something is writing or sending: one thing at a time. */
const busy = ref<"copy" | "backup" | "restore" | "delete" | null>(null);
const progress = ref<string | null>(null);

async function load(): Promise<void> {
  try {
    view.value = await api.settingsView();
    gameRunning.value = view.value.gameRunning;
    loadError.value = null;
    if (picked.value >= view.value.inventories.length) picked.value = 0;
  } catch (e) {
    loadError.value = errorText(e);
  }
}

// The game's state, every few seconds while the tab is open.
let poll: ReturnType<typeof setInterval> | null = null;
function startPoll(): void {
  stopPoll();
  poll = setInterval(async () => {
    try {
      gameRunning.value = await api.settingsGameRunning();
    } catch {
      // Keep the last answer.
    }
  }, 5000);
}
function stopPoll(): void {
  if (poll) clearInterval(poll);
  poll = null;
}

let unlisten: UnlistenFn | null = null;
onMounted(async () => {
  unlisten = await onSettingsProgress((p) => (progress.value = progressText(p)));
});
onUnmounted(() => {
  stopPoll();
  unlisten?.();
});
watch(
  () => props.visible,
  (shown) => {
    if (shown) {
      void load();
      void loadBackups();
      startPoll();
    } else {
      stopPoll();
    }
  },
  { immediate: true },
);

// --- Copy from another character ---

const from = ref("");
const to = ref("");
const items = ref<string[]>([]);
const diffRows = ref<DiffRow[]>([]);
const preview = ref<CopyResult | null>(null);
const copyResult = ref<{ ok: boolean; lines: string[] } | null>(null);

const characters = computed(() => inv.value?.characters ?? []);
const fromChar = computed(() => characters.value.find((c) => charName(c) === from.value) ?? null);
const toChar = computed(() => characters.value.find((c) => charName(c) === to.value) ?? null);
const addonChoices = computed(() => (inv.value && fromChar.value ? copyableAddons(inv.value, fromChar.value) : []));
const gameChoices = computed(() =>
  inv.value && fromChar.value
    ? copyableGameFiles(inv.value, fromChar.value).map((name) => GAME_FILES.find((g) => g.name === name)!)
    : [],
);

function diffOf(item: string): string | null {
  const row = diffRows.value.find((r) => r.item === item);
  return row && fromChar.value && toChar.value ? diffText(row, fromChar.value, toChar.value) : null;
}

// A new source ticks every addon it has, and no game file.
watch(from, () => {
  items.value = addonChoices.value.map((a) => a.addon);
});
watch([from, to, items], () => {
  preview.value = null;
});
watch([from, to], async () => {
  diffRows.value = [];
  if (!inv.value || !from.value || !to.value || from.value === to.value) return;
  try {
    diffRows.value = await api.settingsDiff(inv.value.install, inv.value.account, from.value, to.value);
  } catch {
    // The diff is a nicety; the preview says what would happen.
  }
});

const canPreview = computed(
  () => !!inv.value && !!from.value && !!to.value && from.value !== to.value && items.value.length > 0 && busy.value === null,
);

async function previewCopy(): Promise<void> {
  if (!inv.value) return;
  copyResult.value = null;
  try {
    preview.value = await api.settingsCopy(inv.value.install, inv.value.account, from.value, to.value, items.value, true);
  } catch (e) {
    copyResult.value = { ok: false, lines: [errorText(e)] };
  }
}

async function confirmCopy(): Promise<void> {
  if (!inv.value || !preview.value) return;
  busy.value = "copy";
  try {
    const done = await api.settingsCopy(inv.value.install, inv.value.account, from.value, to.value, items.value, false);
    const line = copyDoneLine(done.plan, done.applied?.written.length ?? 0, done.applied?.backups.length ?? 0);
    log(line);
    copyResult.value = { ok: true, lines: [line] };
    preview.value = null;
    await load();
  } catch (e) {
    copyResult.value = { ok: false, lines: [errorText(e)] };
    log(`Couldn't copy settings: ${errorText(e)}`, "error");
  } finally {
    busy.value = null;
  }
}

// --- Backups ---

const incAddons = ref(true);
const incGame = ref(false);
const backupResult = ref<{ ok: boolean; lines: string[] } | null>(null);
const backups = ref<BackupSummary[] | null>(null);
const backupsError = ref<string | null>(null);
const restoring = ref<{ backup: BackupSummary; report: RestoreReport } | null>(null);
const deleteArmed = ref<string | null>(null);
const signedIn = computed(() => status.value?.signedIn ?? false);

async function loadBackups(): Promise<void> {
  if (!signedIn.value) {
    backups.value = null;
    return;
  }
  try {
    backups.value = await api.settingsBackups();
    backupsError.value = null;
  } catch (e) {
    backupsError.value = errorText(e);
  }
}
watch(signedIn, () => void loadBackups());

async function backUpNow(): Promise<void> {
  if (!inv.value) return;
  busy.value = "backup";
  backupResult.value = null;
  progress.value = null;
  try {
    const done = await api.settingsBackup(inv.value.install, inv.value.account, incAddons.value, incGame.value);
    const line = backupDoneLine(done);
    log(line);
    backupResult.value = { ok: true, lines: [line] };
    await loadBackups();
  } catch (e) {
    backupResult.value = { ok: false, lines: [`Couldn't back up: ${errorText(e)}`] };
    log(`Couldn't back up settings: ${errorText(e)}`, "error");
  } finally {
    busy.value = null;
    progress.value = null;
  }
}

async function startRestore(b: BackupSummary): Promise<void> {
  if (!inv.value) return;
  busy.value = "restore";
  backupResult.value = null;
  try {
    const report = await api.settingsRestore(inv.value.install, b.id, inv.value.account, true);
    restoring.value = { backup: b, report };
  } catch (e) {
    backupResult.value = { ok: false, lines: [`Couldn't open that backup: ${errorText(e)}`] };
  } finally {
    busy.value = null;
    progress.value = null;
  }
}

async function confirmRestore(): Promise<void> {
  if (!inv.value || !restoring.value) return;
  busy.value = "restore";
  try {
    const report = await api.settingsRestore(inv.value.install, restoring.value.backup.id, inv.value.account, false);
    const line = restoreDoneLine(report);
    log(line);
    backupResult.value = { ok: true, lines: [line] };
    restoring.value = null;
    await load();
  } catch (e) {
    backupResult.value = { ok: false, lines: [`Couldn't restore: ${errorText(e)}`] };
    log(`Couldn't restore settings: ${errorText(e)}`, "error");
  } finally {
    busy.value = null;
    progress.value = null;
  }
}

async function deleteBackup(b: BackupSummary): Promise<void> {
  if (deleteArmed.value !== b.id) {
    deleteArmed.value = b.id;
    return;
  }
  busy.value = "delete";
  deleteArmed.value = null;
  try {
    await api.settingsDelete(b.id);
    log(`Deleted the backup from ${backupWhen(b.createdAt)}.`);
    await loadBackups();
  } catch (e) {
    backupResult.value = { ok: false, lines: [`Couldn't delete it: ${errorText(e)}`] };
  } finally {
    busy.value = null;
  }
}

/** The game's files, by character, for the inventory. */
const gameFilesByChar = computed(() => {
  const rows: { who: string; files: string[] }[] = [];
  for (const c of inv.value?.characterFolders ?? []) {
    const files = (inv.value?.gameFiles ?? [])
      .filter((f) => f.character && charName(f.character) === charName(c))
      .map((f) => GAME_FILES.find((g) => f.path.endsWith(`/${g.name}`))?.label ?? f.path);
    if (files.length) rows.push({ who: charName(c), files });
  }
  return rows;
});
</script>

<template>
  <section>
    <h2>Addon settings</h2>
    <p v-if="gameRunning" class="banner" role="status">{{ GAME_RUNNING }}</p>
    <p v-if="loadError" class="banner error">{{ loadError }}</p>
    <p v-for="(p, n) in view?.problems ?? []" :key="n" class="banner error">{{ p }}</p>
    <p v-if="view && !view.inventories.length" class="muted">
      No game account folders found yet. Ryna looks where the addon goes (see Game); log in to the game once to make one.
    </p>
    <template v-if="inv">
      <label v-if="view && view.inventories.length > 1" class="setting">
        <span>
          Account
          <select v-model.number="picked">
            <option v-for="(i, n) in view.inventories" :key="i.install + i.account" :value="n">
              {{ inventoryLabel(i, new Set(view.inventories.map((x) => x.install)).size > 1) }}
            </option>
          </select>
        </span>
      </label>
      <p v-else class="muted">Account {{ inv.account }}</p>
      <table v-if="inv.addons.length">
        <thead>
          <tr>
            <th>Addon</th>
            <th>Kept</th>
            <th>Size</th>
            <th>Last written</th>
            <th>Characters</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="a in inv.addons" :key="a.addon">
            <td>
              {{ a.title ?? a.addon }}
              <span v-if="!a.installed" class="muted">(not installed)</span>
            </td>
            <td>{{ scopeText(a.scope) }}<span v-if="a.acedb" class="muted"> · AceDB</span></td>
            <td>{{ sizeText(a.bytes) }}</td>
            <td>{{ a.modified ? when(a.modified) : "" }}</td>
            <td>
              <span v-if="a.characters.length">{{ a.characters.map(charName).join(", ") }}</span>
              <span v-else class="muted">Every character</span>
            </td>
          </tr>
        </tbody>
      </table>
      <p v-else class="muted">No addon has saved any settings on this account yet.</p>
      <h2 class="sub">The game's own files</h2>
      <ul v-if="gameFilesByChar.length" class="rows">
        <li v-for="g in gameFilesByChar" :key="g.who">
          <span>{{ g.who }}</span>
          <span class="muted">{{ g.files.join(", ") }}</span>
        </li>
      </ul>
      <p v-else class="muted">None yet.</p>
    </template>
  </section>

  <section v-if="inv">
    <h2>Copy from another character</h2>
    <p class="muted">Makes one character's addon settings another's, on this account. Faction doesn't matter.</p>
    <div class="setting">
      <label>
        From
        <select v-model="from" :disabled="busy !== null">
          <option value="" disabled>Choose…</option>
          <option v-for="c in characters" :key="charName(c)" :value="charName(c)">{{ charName(c) }}</option>
        </select>
      </label>
      <label>
        To
        <select v-model="to" :disabled="busy !== null">
          <option value="" disabled>Choose…</option>
          <option v-for="c in characters" :key="charName(c)" :value="charName(c)" :disabled="charName(c) === from">
            {{ charName(c) }}
          </option>
        </select>
      </label>
    </div>
    <template v-if="fromChar">
      <p v-if="!addonChoices.length && !gameChoices.length" class="muted">{{ from }} has no settings of its own to copy.</p>
      <label v-for="a in addonChoices" :key="a.addon" class="setting">
        <input v-model="items" type="checkbox" :value="a.addon" :disabled="busy !== null" />
        <span>
          {{ a.title ?? a.addon }}
          <span class="muted">
            {{ a.acedb ? "Its account file gets the target's entries" : "The character's own file is copied" }}<template v-if="diffOf(a.addon)"> · {{ diffOf(a.addon) }}</template>
          </span>
        </span>
      </label>
      <label v-for="g in gameChoices" :key="g.name" class="setting">
        <input v-model="items" type="checkbox" :value="g.name" :disabled="busy !== null" />
        <span>
          {{ g.label }}
          <span class="muted">{{ g.name }}<template v-if="diffOf(g.name)"> · {{ diffOf(g.name) }}</template></span>
        </span>
      </label>
      <p class="row-actions">
        <button type="button" class="quiet" :disabled="!canPreview" @click="previewCopy()">Preview</button>
      </p>
    </template>
    <div v-if="preview" class="banner">
      <p v-if="!preview.plan.writes.length">Nothing to write: {{ to }} already has these.</p>
      <template v-else>
        <p>These files will be written. Each one replaced is kept beside it as a .ryna.bak, until the next copy or restore.</p>
        <ul class="preview">
          <li v-for="(line, n) in copyPreviewLines(preview.plan)" :key="n" class="path">{{ line }}</li>
        </ul>
        <p class="row-actions">
          <button type="button" class="primary" :disabled="gameRunning || busy !== null" @click="confirmCopy()">
            {{ busy === "copy" ? "Copying…" : `Copy to ${to}` }}
          </button>
          <button type="button" class="quiet" :disabled="busy !== null" @click="preview = null">Cancel</button>
          <span v-if="gameRunning" class="muted">Close the game first.</span>
        </p>
      </template>
    </div>
    <p v-if="copyResult" class="banner" :class="{ error: !copyResult.ok }">
      <template v-for="(line, n) in copyResult.lines" :key="n"><br v-if="n > 0" />{{ line }}</template>
    </p>
  </section>

  <section v-if="inv">
    <h2>Backups</h2>
    <p class="muted">{{ STORED_ENCRYPTED }}</p>
    <label class="setting">
      <input v-model="incAddons" type="checkbox" :disabled="busy !== null" />
      <span>Addon settings</span>
    </label>
    <label class="setting">
      <input v-model="incGame" type="checkbox" :disabled="busy !== null" />
      <span>
        The game's own files
        <span class="muted">Key bindings, macros, chat windows, layout, game options and the addon list, per character.</span>
      </span>
    </label>
    <p class="row-actions">
      <button
        type="button"
        class="primary"
        :disabled="!signedIn || busy !== null || (!incAddons && !incGame)"
        @click="backUpNow()"
      >
        {{ busy === "backup" ? "Backing up…" : "Back up now" }}
      </button>
      <span v-if="!signedIn" class="muted">Sign in to back up to ryna.io.</span>
      <span v-if="progress" class="muted" role="status">{{ progress }}</span>
    </p>
    <p v-if="backupResult" class="banner" :class="{ error: !backupResult.ok }">
      <template v-for="(line, n) in backupResult.lines" :key="n"><br v-if="n > 0" />{{ line }}</template>
    </p>
    <p v-if="backupsError" class="banner error">Couldn't list the backups: {{ backupsError }}</p>
    <ul v-if="backups && backups.length" class="rows">
      <li v-for="b in backups" :key="b.id">
        <span>
          {{ backupWhen(b.createdAt) }}
          <span class="muted">{{ b.machine ?? "" }} · {{ b.files }} files · {{ sizeText(b.bytes) }}</span>
        </span>
        <span class="row-actions">
          <button type="button" class="quiet" :disabled="busy !== null || gameRunning" @click="startRestore(b)">Restore…</button>
          <button type="button" class="quiet" :disabled="busy !== null" @click="deleteBackup(b)">
            {{ deleteArmed === b.id ? "Delete for good?" : "Delete" }}
          </button>
        </span>
      </li>
    </ul>
    <p v-else-if="backups" class="muted">No backups yet.</p>
    <div v-if="restoring" class="banner">
      <p>
        Restoring the backup from {{ backupWhen(restoring.backup.createdAt) }} into {{ restoring.report.account }} writes these
        files. Each one replaced is kept beside it as a .ryna.bak.
      </p>
      <ul class="preview">
        <li v-for="(line, n) in restorePreviewLines(restoring.report)" :key="n" class="path">{{ line }}</li>
      </ul>
      <p class="row-actions">
        <button type="button" class="primary" :disabled="gameRunning || busy !== null" @click="confirmRestore()">
          {{ busy === "restore" ? "Restoring…" : "Restore" }}
        </button>
        <button type="button" class="quiet" :disabled="busy !== null" @click="restoring = null">Cancel</button>
        <span v-if="gameRunning" class="muted">Close the game first.</span>
      </p>
    </div>
  </section>
</template>
