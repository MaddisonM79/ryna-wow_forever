<script setup lang="ts">
// The core's Mods tab: a toggle per mod, and its state in each install the addon goes into.
// ModsView draws it; this holds the state and does the work.
import { computed, ref } from "vue";
import { api, errorText, type InstallView, type ModView } from "../api";
import { installedLines, installSummary, modToggledLines, type ModsResult } from "../describe";
import { watchGameClosing } from "../gameWatch";
import { log, refresh, status } from "../store";
import ModsView from "./ModsView.vue";

/** The installs a mod goes into: the ones the addon goes into (Forever, or a chosen folder). */
function canInstall(i: InstallView): boolean {
  return i.forever || status.value?.installOverride != null;
}

const modInstalls = computed(() => status.value?.installs.filter(canInstall) ?? []);
/** The mod whose toggle is being applied, if any (one at a time). */
const toggling = ref<string | null>(null);
/** The install being written by the card's Update or Install button, if any. */
const installing = ref<string | null>(null);
/** What the last toggle or install said: one result for the whole card, under `card`. */
const results = ref<Record<string, ModsResult>>({});
// "The game is running: /reload ..." goes once the game has closed.
watchGameClosing(results);
const result = computed(() => results.value.card ?? null);

function show(r: ModsResult | null): void {
  if (r) results.value = { card: r };
  else results.value = {};
}

/** The card's Update / Install button: the same install as the Game tab's (the addon and every mod that's on). */
async function installFor(path: string, update: boolean): Promise<void> {
  installing.value = path;
  show(null);
  try {
    const done = await api.installAddon(path);
    const lines = installedLines(done);
    for (const line of lines) log(line);
    show({ ok: true, summary: installSummary(done, update), lines });
  } catch (e) {
    show({ ok: false, summary: "Couldn't install the addon.", lines: [errorText(e)] });
    log(`Couldn't install into ${path}: ${errorText(e)}`, "error");
  } finally {
    installing.value = null;
    await refresh();
  }
}

async function toggleMod(m: ModView, on: boolean): Promise<void> {
  toggling.value = m.id;
  show(null);
  try {
    const done = await api.setModEnabled(m.id, on);
    const lines = modToggledLines(done);
    for (const line of lines) log(line);
    // The first line ("Settings: on.") heads the result; an agent mod has nothing after it, and
    // the checkbox already says that much, so it shows no result (an empty one drew as a blank
    // pill, 2026-11-08).
    const [head = "", ...rest] = lines;
    if (rest.length) show({ ok: true, summary: head, lines: rest });
  } catch (e) {
    show({ ok: false, summary: `Couldn't turn ${m.name} ${on ? "on" : "off"}.`, lines: [errorText(e)] });
    log(`Couldn't turn ${m.name} ${on ? "on" : "off"}: ${errorText(e)}`, "error");
  } finally {
    toggling.value = null;
    await refresh();
  }
}
</script>

<template>
  <ModsView
    v-if="status"
    :mods="status.mods"
    :installs="modInstalls"
    :result="result"
    :toggling="toggling"
    :installing="installing"
    @toggle="toggleMod"
    @install="installFor"
    @dismiss="show(null)"
  />
</template>
