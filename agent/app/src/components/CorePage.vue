<script setup lang="ts">
// A core's page: a tab row (Game, Accounts, Combat, Mods, Settings) over the core's sections. The
// WoW core's sections are the window's own; another core shows the rows it lays out through the
// agent's interface until it has sections of its own. The open tab is remembered per core.
import { nextTick, ref, watch } from "vue";
import type { CoreView } from "../api";
import { CORE_TABS, coreTabForKey, loadCoreTab, saveCoreTab, windowStorage, type CoreTabId } from "../tabs";
import InstallsSection from "./InstallsSection.vue";
import AccountsSection from "./AccountsSection.vue";
import CombatSection from "./CombatSection.vue";
import ModsSection from "./ModsSection.vue";
import CoreSettings from "./CoreSettings.vue";

const props = defineProps<{ core: CoreView }>();

const tab = ref<CoreTabId>(loadCoreTab(windowStorage(), props.core.id));
watch(tab, (id) => saveCoreTab(windowStorage(), props.core.id, id));

function tabId(t: CoreTabId): string {
  return `tab-${props.core.id}-${t}`;
}

/** Arrow keys, Home and End move the selection and the focus with it (automatic activation). */
async function onKey(e: KeyboardEvent): Promise<void> {
  const next = coreTabForKey(tab.value, e.key);
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
        v-for="t in CORE_TABS"
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
      v-for="t in CORE_TABS"
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
      <CoreSettings v-else-if="t.id === 'settings'" />
    </div>
  </template>
  <div v-else class="tabpanel">
    <section>
      <h2>{{ core.name }}</h2>
      <dl class="glance">
        <div v-for="row in core.rows" :key="row.label">
          <dt>{{ row.label }}</dt>
          <dd>
            <span v-if="row.chip" class="chip" :class="row.chip">{{ row.value }}</span>
            <span v-else>{{ row.value }}</span>
          </dd>
        </div>
      </dl>
      <p v-if="!core.rows.length" class="muted">Nothing to show yet.</p>
    </section>
  </div>
  </div>
</template>
