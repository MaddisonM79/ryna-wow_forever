<script setup lang="ts">
import { computed } from "vue";
import { api, errorText } from "../api";
import { log, status } from "../store";
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
const b = computed(() => banner(updateCheck.value, laterFor.value, status.value?.support?.support ?? null));
const downloading = computed(() => updatePhase.value === "downloading");
const busy = computed(() => downloading.value || updatePhase.value === "checking");

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
    </span>
    <span class="row-actions">
      <button v-if="b.canInstall" type="button" class="primary" :disabled="busy" @click="installUpdate()">
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
