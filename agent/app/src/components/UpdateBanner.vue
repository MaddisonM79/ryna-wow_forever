<script setup lang="ts">
import { computed } from "vue";
import { bannerText, progressText } from "../updateWords";
import { installUpdate, later, laterFor, updateCheck, updateError, updatePhase, updateProgress } from "../updates";

const text = computed(() => bannerText(updateCheck.value, laterFor.value));
const busy = computed(() => updatePhase.value === "downloading");
</script>

<template>
  <div v-if="text" class="banner update" role="status">
    <span>
      <strong>{{ text }}</strong>
      <span v-if="busy" class="muted"> {{ progressText(updateProgress) }}</span>
      <span v-else-if="updatePhase === 'error' && updateError" class="muted"> {{ updateError }}</span>
    </span>
    <span class="row-actions">
      <button type="button" class="primary" :disabled="busy" @click="installUpdate()">
        {{ busy ? "Updating…" : "Update and restart" }}
      </button>
      <button type="button" class="quiet" :disabled="busy" @click="later()">Later</button>
    </span>
  </div>
</template>
