<script setup lang="ts">
// The Mods tab as drawn (ModsSection holds the state and does the work): one grid for every mod,
// so the switches, the words and the states line up in columns; the one button the mods share in
// the card's head; and the last result for the whole card under it, never inside a row.
import { computed } from "vue";
import type { InstallView, ModView } from "../api";
import { ACTS_IN_GAME, flavorName, modRow, modsAction, pathPieces, type ModsResult } from "../describe";

const props = defineProps<{
  mods: ModView[];
  /** The installs a mod goes into. */
  installs: InstallView[];
  result: ModsResult | null;
  /** The mod whose toggle is being applied, if any. */
  toggling: string | null;
  /** The install being written, if any. */
  installing: string | null;
}>();

const emit = defineEmits<{
  toggle: [m: ModView, on: boolean];
  install: [path: string, update: boolean];
  dismiss: [];
}>();

/** The card's buttons: one per install that has something to install or update. */
const actions = computed(() =>
  props.installs.flatMap((i) => {
    const a = props.mods.length ? modsAction(props.mods, i.path) : null;
    return a ? [{ path: i.path, flavor: i.flavor, ...a }] : [];
  }),
);
</script>

<template>
  <section class="mods-card">
    <div class="section-head">
      <h2>Mods</h2>
      <span v-if="actions.length" class="row-actions">
        <button
          v-for="a in actions"
          :key="a.path"
          type="button"
          class="primary"
          :title="a.path"
          :disabled="installing !== null || toggling !== null"
          @click="emit('install', a.path, a.kind === 'update')"
        >
          {{ installing === a.path ? "Installing…" : a.label
          }}{{ installs.length > 1 ? ` · ${flavorName(a.flavor)}` : "" }}
        </button>
      </span>
    </div>
    <div v-if="result" class="banner mods-result" :class="{ error: !result.ok }" :role="result.ok ? 'status' : 'alert'">
      <div class="mods-result-text">
        <strong>{{ result.summary }}</strong>
        <ul v-if="result.lines.length">
          <li v-for="(line, n) in result.lines" :key="n">
            <template v-for="(piece, k) in pathPieces(line)" :key="k"><wbr v-if="k > 0" />{{ piece }}</template>
          </li>
        </ul>
      </div>
      <button type="button" class="quiet small" @click="emit('dismiss')">Dismiss</button>
    </div>
    <ul v-if="mods.length && installs.length" class="mods-grid">
      <li v-for="m in mods" :key="m.id" class="mod">
        <span class="mod-switch">
          <input
            :id="`mod-${m.id}`"
            type="checkbox"
            :checked="m.on"
            :disabled="toggling !== null"
            @change="emit('toggle', m, ($event.target as HTMLInputElement).checked)"
          />
        </span>
        <label class="mod-text" :for="`mod-${m.id}`">
          {{ m.name }}
          <span class="muted">{{ m.summary }}</span>
          <span v-if="m.actsInGame" class="acts-in-game">{{ ACTS_IN_GAME }}</span>
        </label>
        <div class="mod-status">
          <div v-for="i in installs" :key="i.path" class="mod-state">
            <span class="chip" :class="modRow(m, i.path).chip">{{ modRow(m, i.path).text }}</span>
            <span v-if="installs.length > 1" class="path">{{ i.path }}</span>
          </div>
        </div>
      </li>
    </ul>
    <p v-else-if="!mods.length" class="muted">No mods for this game yet.</p>
    <p v-else class="muted">Mods go into the game folder with the addon. The game wasn't found yet: see Game.</p>
  </section>
</template>
