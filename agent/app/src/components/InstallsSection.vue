<script setup lang="ts">
import { computed, ref } from "vue";
import { api, errorText, type InstallView, type ModView } from "../api";
import {
  ACTS_IN_GAME,
  addonRow,
  flavorName,
  installedLines,
  installedMessage,
  modRow,
  modToggledLines,
} from "../describe";
import { chooseGameFolder, log, refresh, status } from "../store";

/** The install whose addon is being copied, if any (one at a time). */
const installing = ref<string | null>(null);
/** What the last install said, by install path. */
const results = ref<Record<string, { ok: boolean; lines: string[] }>>({});

/** The addon goes into Forever installs, or anything in a folder that was chosen. */
function canInstall(i: InstallView): boolean {
  return i.forever || status.value?.installOverride != null;
}

/** The row's last result, as a list of none or one (for the template's v-for). */
function resultOf(path: string): { ok: boolean; lines: string[] }[] {
  const r = results.value[path];
  return r ? [r] : [];
}

/** The installs a mod goes into: the ones the addon goes into. */
const modInstalls = computed(() => status.value?.installs.filter(canInstall) ?? []);
/** The mod whose toggle is being applied, if any (one at a time). */
const toggling = ref<string | null>(null);
/** What the last toggle said, by mod id. */
const modResults = ref<Record<string, { ok: boolean; lines: string[] }>>({});

function modResultOf(id: string): { ok: boolean; lines: string[] }[] {
  const r = modResults.value[id];
  return r ? [r] : [];
}

async function toggleMod(m: ModView, on: boolean): Promise<void> {
  toggling.value = m.id;
  delete modResults.value[m.id];
  try {
    const done = await api.setModEnabled(m.id, on);
    const lines = modToggledLines(done);
    for (const line of lines) log(line);
    modResults.value[m.id] = { ok: true, lines: lines.slice(1) };
  } catch (e) {
    modResults.value[m.id] = { ok: false, lines: [errorText(e)] };
    log(`Couldn't turn ${m.name} ${on ? "on" : "off"}: ${errorText(e)}`, "error");
  } finally {
    toggling.value = null;
    await refresh();
  }
}

async function installAddon(path: string): Promise<void> {
  installing.value = path;
  delete results.value[path];
  try {
    const done = await api.installAddon(path);
    for (const line of installedLines(done)) log(line);
    for (const d of done.installed) results.value[d.install] = { ok: true, lines: installedMessage(done) };
  } catch (e) {
    results.value[path] = { ok: false, lines: [errorText(e)] };
    log(`Couldn't install the addon: ${errorText(e)}`, "error");
  } finally {
    installing.value = null;
    await refresh();
  }
}
</script>

<template>
  <section v-if="status">
    <h2>Game</h2>
    <ul v-if="status.installs.length" class="rows">
      <li v-for="i in status.installs" :key="i.path" class="install">
        <div class="install-row">
          <div>
            <strong>{{ flavorName(i.flavor) }}</strong>
            <div class="path">{{ i.path }}</div>
          </div>
          <template v-if="canInstall(i)">
            <span class="chip" :class="addonRow(i, status.addonBundled).chip">
              {{ addonRow(i, status.addonBundled).text }}
            </span>
            <button
              v-if="addonRow(i, status.addonBundled).action"
              type="button"
              :class="{ primary: addonRow(i, status.addonBundled).primary }"
              :disabled="installing !== null"
              @click="installAddon(i.path)"
            >
              {{ installing === i.path ? "Installing…" : addonRow(i, status.addonBundled).action }}
            </button>
          </template>
          <span v-else class="chip waiting" title="The addon is for World of Warcraft: Forever">
            {{ i.hasAddon ? addonRow(i, status.addonBundled).text : "Not Forever: no addon here" }}
          </span>
        </div>
        <p v-for="r in resultOf(i.path)" :key="i.path" class="banner" :class="{ error: !r.ok }">
          <template v-for="(line, n) in r.lines" :key="n">
            <br v-if="n > 0" />{{ line }}
          </template>
        </p>
      </li>
    </ul>
    <p v-else class="muted">
      The game wasn't found in the usual place.
      <button type="button" @click="chooseGameFolder()">Choose folder…</button>
    </p>
    <template v-if="status.mods.length && modInstalls.length">
      <h3>Mods</h3>
      <ul class="rows mods">
        <li v-for="m in status.mods" :key="m.id" class="mod">
          <label class="setting">
            <input
              type="checkbox"
              :checked="m.on"
              :disabled="toggling !== null || installing !== null"
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
            <span v-if="modInstalls.length > 1" class="path">{{ i.path }}</span>
          </p>
          <p v-for="r in modResultOf(m.id)" :key="m.id" class="banner" :class="{ error: !r.ok }">
            <template v-for="(line, n) in r.lines" :key="n">
              <br v-if="n > 0" />{{ line }}
            </template>
          </p>
        </li>
      </ul>
    </template>
    <p v-if="status.installs.length && !status.addonBundled" class="muted">
      This build doesn't carry the addon, so it can't install it.
    </p>
    <p v-if="status.installs.length" class="row-actions">
      <button type="button" class="quiet" @click="chooseGameFolder()">Choose another folder…</button>
    </p>
  </section>
</template>
