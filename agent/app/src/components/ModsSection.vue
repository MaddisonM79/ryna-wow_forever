<script setup lang="ts">
// The core's Mods tab: a toggle per mod, and its state in each install the addon goes into.
import { computed, ref } from "vue";
import { api, errorText, type InstallView, type ModView } from "../api";
import { ACTS_IN_GAME, installedLines, modAction, modRow, modToggledLines } from "../describe";
import { watchGameClosing } from "../gameWatch";
import { log, refresh, status } from "../store";

/** The installs a mod goes into: the ones the addon goes into (Forever, or a chosen folder). */
function canInstall(i: InstallView): boolean {
  return i.forever || status.value?.installOverride != null;
}

const modInstalls = computed(() => status.value?.installs.filter(canInstall) ?? []);
/** The mod whose toggle is being applied, if any (one at a time). */
const toggling = ref<string | null>(null);
/** What the last toggle said, by mod id. */
const modResults = ref<Record<string, { ok: boolean; lines: string[] }>>({});
// "The game is running: /reload ..." goes once the game has closed.
watchGameClosing(modResults);

function modResultOf(id: string): { ok: boolean; lines: string[] }[] {
  const r = modResults.value[id];
  return r ? [r] : [];
}

/** The install being written by a row's Update or Install button, if any. */
const installing = ref<string | null>(null);

/** The row's Update / Install button: the same install as the Game tab's (the addon and every mod that's on). */
async function installFor(m: ModView, path: string): Promise<void> {
  installing.value = path;
  delete modResults.value[m.id];
  try {
    const done = await api.installAddon(path);
    const lines = installedLines(done);
    for (const line of lines) log(line);
    modResults.value[m.id] = { ok: true, lines };
  } catch (e) {
    modResults.value[m.id] = { ok: false, lines: [errorText(e)] };
    log(`Couldn't install into ${path}: ${errorText(e)}`, "error");
  } finally {
    installing.value = null;
    await refresh();
  }
}

async function toggleMod(m: ModView, on: boolean): Promise<void> {
  toggling.value = m.id;
  delete modResults.value[m.id];
  try {
    const done = await api.setModEnabled(m.id, on);
    const lines = modToggledLines(done);
    for (const line of lines) log(line);
    // The first line ("Settings: on.") is the checkbox itself; an agent mod has nothing after it,
    // and an empty banner drew as a blank pill (2026-11-08).
    const rest = lines.slice(1);
    if (rest.length) modResults.value[m.id] = { ok: true, lines: rest };
  } catch (e) {
    modResults.value[m.id] = { ok: false, lines: [errorText(e)] };
    log(`Couldn't turn ${m.name} ${on ? "on" : "off"}: ${errorText(e)}`, "error");
  } finally {
    toggling.value = null;
    await refresh();
  }
}
</script>

<template>
  <section v-if="status">
    <h2>Mods</h2>
    <ul v-if="status.mods.length && modInstalls.length" class="rows mods">
      <li v-for="m in status.mods" :key="m.id" class="mod">
        <label class="setting">
          <input
            type="checkbox"
            :checked="m.on"
            :disabled="toggling !== null"
            @change="toggleMod(m, ($event.target as HTMLInputElement).checked)"
          />
          <span>
            {{ m.name }}
            <span class="muted">{{ m.summary }}</span>
            <span v-if="m.actsInGame" class="acts-in-game">{{ ACTS_IN_GAME }}</span>
          </span>
        </label>
        <p v-for="i in modInstalls" :key="i.path" class="mod-state">
          <span class="chip" :class="modRow(m, i.path).chip">{{ modRow(m, i.path).text }}</span>
          <button
            v-if="modAction(m, i.path)"
            type="button"
            class="primary"
            :disabled="installing !== null || toggling !== null"
            @click="installFor(m, i.path)"
          >
            {{ installing === i.path ? "Installing…" : modAction(m, i.path)!.label }}
          </button>
          <span v-if="modInstalls.length > 1" class="path">{{ i.path }}</span>
        </p>
        <p v-for="r in modResultOf(m.id).filter((r) => r.lines.length)" :key="m.id" class="banner" :class="{ error: !r.ok }">
          <template v-for="(line, n) in r.lines" :key="n">
            <br v-if="n > 0" />{{ line }}
          </template>
        </p>
      </li>
    </ul>
    <p v-else-if="!status.mods.length" class="muted">No mods for this game yet.</p>
    <p v-else class="muted">Mods go into the game folder with the addon. The game wasn't found yet: see Game.</p>
  </section>
</template>
