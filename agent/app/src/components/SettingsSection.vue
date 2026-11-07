<script setup lang="ts">
import { ref } from "vue";
import { api, errorText } from "../api";
import { chooseGameFolder, refresh, setCombatSettings, setGameFolder, status } from "../store";

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
    <p class="muted">Updates: Status and About have "Check for updates". A new Ryna brings the addon with it.</p>
  </section>
</template>
