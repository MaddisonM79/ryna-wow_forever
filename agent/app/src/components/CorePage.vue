<script setup lang="ts">
// A core's page: a tab row (Game, Accounts, Combat, Mods, Addon settings while the Settings mod is
// on, Settings) over the core's sections. The
// WoW core's sections are the window's own; a fetched core's page is SidecarPage (its state, its
// rows, its activity and its switch), with no tabs until it has something to put in them. The open tab is remembered per core.
import { computed, nextTick, ref, watch } from "vue";
import type { CoreView } from "../api";
import { settingsModOn } from "../settingsWords";
import { status } from "../store";
import { coreTabForKey, coreTabs, DEFAULT_CORE_TAB, loadCoreTab, saveCoreTab, windowStorage, type CoreTabId } from "../tabs";
import InstallsSection from "./InstallsSection.vue";
import AccountsSection from "./AccountsSection.vue";
import CombatSection from "./CombatSection.vue";
import ModsSection from "./ModsSection.vue";
import CoreSettings from "./CoreSettings.vue";
import AddonSettingsSection from "./AddonSettingsSection.vue";
import SidecarPage from "./SidecarPage.vue";

const props = defineProps<{ core: CoreView }>();

const tabs = computed(() => coreTabs(settingsModOn(status.value)));
const chosen = ref<CoreTabId>(loadCoreTab(windowStorage(), props.core.id));
watch(chosen, (id) => saveCoreTab(windowStorage(), props.core.id, id));
/** The open tab: the one chosen, unless it isn't shown (the Settings mod turned off). */
const tab = computed<CoreTabId>({
  get: () => (tabs.value.some((t) => t.id === chosen.value) ? chosen.value : DEFAULT_CORE_TAB),
  set: (id) => {
    chosen.value = id;
  },
});

function tabId(t: CoreTabId): string {
  return `tab-${props.core.id}-${t}`;
}

/** Arrow keys, Home and End move the selection and the focus with it (automatic activation). */
async function onKey(e: KeyboardEvent): Promise<void> {
  const next = coreTabForKey(tab.value, e.key, tabs.value);
  if (!next) return;
  e.preventDefault();
  tab.value = next;
  await nextTick();
  document.getElementById(tabId(next))?.focus();
}
</script>

<template>
  <div class="core-page">
  <template v-if="core.id === 'wow'">
    <div class="tabs" role="tablist" :aria-label="core.name" @keydown="onKey">
      <button
        v-for="t in tabs"
        :id="tabId(t.id)"
        :key="t.id"
        type="button"
        role="tab"
        :aria-selected="tab === t.id"
        :aria-controls="`panel-${core.id}-${t.id}`"
        :tabindex="tab === t.id ? 0 : -1"
        @click="tab = t.id"
      >
        {{ t.label }}
      </button>
    </div>
    <div
      v-for="t in tabs"
      v-show="tab === t.id"
      :id="`panel-${core.id}-${t.id}`"
      :key="t.id"
      class="tabpanel"
      :class="t.id"
      role="tabpanel"
      :aria-labelledby="tabId(t.id)"
      tabindex="0"
    >
      <InstallsSection v-if="t.id === 'game'" />
      <AccountsSection v-else-if="t.id === 'accounts'" />
      <CombatSection v-else-if="t.id === 'combat'" />
      <ModsSection v-else-if="t.id === 'mods'" />
      <AddonSettingsSection v-else-if="t.id === 'addonSettings'" :visible="tab === 'addonSettings'" />
      <CoreSettings v-else-if="t.id === 'settings'" />
    </div>
  </template>
  <SidecarPage v-else :core="core" />
  </div>
</template>
