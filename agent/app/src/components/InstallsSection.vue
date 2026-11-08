<script setup lang="ts">
// The core's Game tab: each install of the game, its addon, and the button that installs it.
import { ref } from "vue";
import { api, errorText, type InstallView } from "../api";
import { addonRow, flavorName, installedLines, installedMessage } from "../describe";
import { watchGameClosing } from "../gameWatch";
import { chooseGameFolder, log, refresh, status } from "../store";

/** The install whose addon is being copied, if any (one at a time). */
const installing = ref<string | null>(null);
/** What the last install said, by install path. */
const results = ref<Record<string, { ok: boolean; lines: string[] }>>({});
// "The game is running: /reload ..." goes once the game has closed.
watchGameClosing(results);

/** The addon goes into Forever installs, or anything in a folder that was chosen. */
function canInstall(i: InstallView): boolean {
  return i.forever || status.value?.installOverride != null;
}

/** The row's last result, as a list of none or one (for the template's v-for). */
function resultOf(path: string): { ok: boolean; lines: string[] }[] {
  const r = results.value[path];
  return r ? [r] : [];
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
    <p v-if="status.installs.length && !status.addonBundled" class="muted">
      This build doesn't carry the addon, so it can't install it.
    </p>
  </section>
</template>
