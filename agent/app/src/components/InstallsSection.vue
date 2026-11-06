<script setup lang="ts">
import { ref } from "vue";
import { api, errorText } from "../api";
import { flavorName } from "../describe";
import { chooseGameFolder, log, refresh, status } from "../store";

const installing = ref(false);
const result = ref<{ ok: boolean; text: string } | null>(null);

async function installAddon(): Promise<void> {
  installing.value = true;
  result.value = null;
  try {
    const done = await api.installAddon();
    const text = `Installed the addon in ${done.map((d) => d.path).join(", ")}. Restart the game or /reload to load it.`;
    result.value = { ok: true, text };
    log(text);
  } catch (e) {
    result.value = { ok: false, text: errorText(e) };
    log(`Couldn't install the addon: ${errorText(e)}`, "error");
  } finally {
    installing.value = false;
    await refresh();
  }
}
</script>

<template>
  <section v-if="status">
    <h2>Game</h2>
    <ul v-if="status.installs.length" class="rows">
      <li v-for="i in status.installs" :key="i.path">
        <div>
          <strong>{{ flavorName(i.flavor) }}</strong>
          <div class="path">{{ i.path }}</div>
        </div>
        <span v-if="i.hasAddon" class="chip pushed">Addon installed</span>
        <span v-else-if="i.forever" class="chip waiting">No addon yet</span>
      </li>
    </ul>
    <p v-else class="muted">
      The game wasn't found in the usual place.
      <button type="button" @click="chooseGameFolder()">Choose folder…</button>
    </p>
    <p class="row-actions">
      <button type="button" :disabled="installing || !status.installs.length" @click="installAddon()">
        {{ installing ? "Installing…" : "Install addon" }}
      </button>
      <button v-if="status.installs.length" type="button" class="quiet" @click="chooseGameFolder()">
        Choose another folder…
      </button>
    </p>
    <p v-if="result" class="banner" :class="{ error: !result.ok }">{{ result.text }}</p>
  </section>
</template>
