<script setup lang="ts">
import { computed, ref } from "vue";
import { api, errorText } from "../api";
import { installedLines, installedMessage } from "../describe";
import { log, refresh, status } from "../store";
import { banner, progressText } from "../updateWords";
import {
  checkForUpdates,
  installUpdate,
  later,
  laterFor,
  updateCheck,
  updateError,
  updatePhase,
  updateProgress,
} from "../updates";

// The updater's newest release and the API's judgment of this version (current, behind, reduced,
// unsupported): one banner, coloured by the judgment.
// When the addon in the game folder is what's old (the app isn't), it says so and its button
// installs this app's addon there: the app's own updater has nothing to offer for that.
const b = computed(() =>
  banner(
    updateCheck.value,
    laterFor.value,
    status.value?.support?.support ?? null,
    status.value ? { installs: status.value.installs, bundled: status.value.addonBundled } : null,
  ),
);
const downloading = computed(() => updatePhase.value === "downloading");
const fixing = ref(false);
/** What the addon's button last did: shown until the API's next judgment takes the banner away. */
const fixed = ref<{ ok: boolean; lines: string[] } | null>(null);
const busy = computed(() => downloading.value || updatePhase.value === "checking" || fixing.value);

/** Installs this app's addon into every game folder it belongs in, then asks the API again. */
async function fixAddon(action: "install" | "sync"): Promise<void> {
  fixing.value = true;
  fixed.value = null;
  try {
    if (action === "install") {
      const done = await api.installAddon();
      for (const line of installedLines(done)) log(line);
      fixed.value = { ok: true, lines: installedMessage(done) };
      await refresh();
    }
    // The judgment comes with the heartbeat a pass ends on.
    await api.syncNow();
  } catch (e) {
    fixed.value = { ok: false, lines: [errorText(e)] };
    log(`Couldn't update the addon: ${errorText(e)}`, "error");
  } finally {
    fixing.value = false;
  }
}

async function openGuide(url: string): Promise<void> {
  try {
    await api.openLink(url);
  } catch (e) {
    log(`Couldn't open the update steps: ${errorText(e)}`, "error");
  }
}
</script>

<template>
  <div v-if="b" class="banner update" :class="b.tone" :role="b.tone === 'update' ? 'status' : 'alert'">
    <span>
      <strong>{{ b.text }}</strong>
      <span v-if="downloading" class="muted"> {{ progressText(updateProgress) }}</span>
      <span v-else-if="updatePhase === 'error' && updateError" class="muted"> {{ updateError }}</span>
      <span v-if="b.note" class="banner-note">{{ b.note }}</span>
      <span v-if="b.addon && fixed" class="banner-note">{{ fixed.lines.join(" ") }}</span>
    </span>
    <span class="row-actions">
      <button v-if="b.addon" type="button" class="primary" :disabled="busy" @click="fixAddon(b.addon.action)">
        {{ fixing ? "Working…" : b.addon.action === "install" ? `Update the addon to ${b.addon.to}` : "Sync now" }}
      </button>
      <button v-else-if="b.canInstall" type="button" class="primary" :disabled="busy" @click="installUpdate()">
        {{ downloading ? "Updating…" : "Update and restart" }}
      </button>
      <template v-else>
        <button type="button" class="primary" :disabled="busy" @click="checkForUpdates(true)">Check for updates</button>
        <button v-if="b.guide" type="button" class="quiet" @click="openGuide(b.guide)">How to update</button>
      </template>
      <button v-if="b.dismissable" type="button" class="quiet" :disabled="busy" @click="later(b.key)">Later</button>
    </span>
  </div>
</template>
