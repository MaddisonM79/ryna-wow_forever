<script setup lang="ts">
// "Check for updates", the version it's checking from, and what the last check found.
// The same updates.ts state the banner reads; on Status and About.
import { computed } from "vue";
import { status } from "../store";
import { checkButton, checkResult } from "../updateWords";
import { checkForUpdates, updateCheck, updateError, updatePhase } from "../updates";

withDefaults(defineProps<{ showVersion?: boolean }>(), { showVersion: true });
const button = computed(() => checkButton(updateCheck.value, updatePhase.value));
const result = computed(() => checkResult(updateCheck.value, updatePhase.value, updateError.value));
</script>

<template>
  <span class="update-control">
    <span v-if="showVersion" class="muted">Ryna {{ status?.appVersion ?? "" }}</span>
    <span v-if="result && updatePhase !== 'checking'" class="chip" :class="result.chip" role="status">{{ result.text }}</span>
    <button type="button" class="quiet" :disabled="button.disabled" @click="checkForUpdates(true)">
      {{ button.label }}
    </button>
  </span>
</template>
