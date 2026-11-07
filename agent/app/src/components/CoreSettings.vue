<script setup lang="ts">
// The core's own settings: where the game is, what of its fights goes up, and the submission ID
// the census traces sightings to.
import { onMounted, ref, watch } from "vue";
import { api, errorText } from "../api";
import { copyText } from "../clipboard";
import { chooseGameFolder, setCombatSettings, setGameFolder, status } from "../store";

const submissionId = ref<string | null>(null);
const idError = ref<string | null>(null);
const copied = ref<"yes" | "no" | null>(null);

async function loadId(): Promise<void> {
  try {
    submissionId.value = await api.submissionId();
    idError.value = null;
  } catch (e) {
    submissionId.value = null;
    idError.value = errorText(e);
  }
}

async function copyId(): Promise<void> {
  if (!submissionId.value) return;
  copied.value = (await copyText(submissionId.value)) ? "yes" : "no";
}

onMounted(loadId);
// Read again once the keychain gives it (or stops giving it).
watch(
  () => status.value?.deviceKeyError ?? null,
  () => void loadId(),
);
</script>

<template>
  <section v-if="status">
    <h2>Settings</h2>
    <div class="setting">
      <span>
        Game folder
        <span class="path">{{ status.installOverride ?? "The usual places" }}</span>
      </span>
      <span class="row-actions">
        <button type="button" class="quiet" @click="chooseGameFolder()">Choose…</button>
        <button v-if="status.installOverride" type="button" class="quiet" @click="setGameFolder(null)">
          Use the usual places
        </button>
      </span>
    </div>
    <label class="setting">
      <input
        type="checkbox"
        :checked="status.sendDigests"
        @change="setCombatSettings(($event.target as HTMLInputElement).checked, status.keepRaw)"
      />
      <span>
        Send fight digests
        <span class="muted">Each fight's hits, heals and deaths go up beside its totals, so ryna.io can merge it with a groupmate's log.</span>
      </span>
    </label>
    <label class="setting">
      <input
        type="checkbox"
        :checked="status.keepRaw"
        @change="setCombatSettings(status.sendDigests, ($event.target as HTMLInputElement).checked)"
      />
      <span>
        Keep raw logs on ryna.io
        <span class="muted">Each fight's own combat log lines are stored too, for 90 days.</span>
      </span>
    </label>
    <h2 class="sub">Census</h2>
    <div class="setting">
      <span>
        Submission ID
        <code v-if="submissionId" class="submission-id">{{ submissionId }}</code>
        <span v-else-if="idError" class="error-text">Unavailable: see Activity</span>
        <span v-else class="muted">Reading…</span>
        <span class="muted">
          Sightings you contribute are traced to this ID, not to you. While you're signed in they're also attributed to your account.
        </span>
      </span>
      <span class="row-actions">
        <button type="button" class="quiet" :disabled="!submissionId" @click="copyId()">Copy</button>
        <span v-if="copied === 'yes'" class="muted" role="status">Copied</span>
        <span v-else-if="copied === 'no'" class="muted" role="status">Select it to copy</span>
      </span>
    </div>
  </section>
</template>
