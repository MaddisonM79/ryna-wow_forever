<script setup lang="ts">
// The core's own settings: where the game is, and what of its fights goes up.
import { chooseGameFolder, setCombatSettings, setGameFolder, status } from "../store";
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
  </section>
</template>
