<script setup lang="ts">
// The agent's own settings: opening at login, and where updates are. Each game's settings are on
// its page.
import { ref } from "vue";
import { api, errorText } from "../api";
import { refresh, status } from "../store";

const autostartError = ref<string | null>(null);

async function toggleAutostart(e: Event): Promise<void> {
  const wanted = (e.target as HTMLInputElement).checked;
  autostartError.value = null;
  try {
    const on = await api.setAutostart(wanted);
    if (status.value) status.value.autostart = on;
  } catch (err) {
    autostartError.value = errorText(err);
    await refresh();
  }
}
</script>

<template>
  <section v-if="status">
    <h2>Settings</h2>
    <label class="setting">
      <input type="checkbox" :checked="status.autostart" @change="toggleAutostart" />
      <span>
        Open Ryna when I log in to this computer
        <span class="muted">It syncs only while it's open; quitting it stops everything.</span>
      </span>
    </label>
    <p v-if="autostartError" class="banner error">{{ autostartError }}</p>
    <p class="muted">Each game's own settings are on its page.</p>
    <p class="muted">Updates: Status and About have "Check for updates". A new Ryna carries a new addon: install it on the game's page.</p>
  </section>
</template>
