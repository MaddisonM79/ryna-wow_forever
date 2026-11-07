<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import { api } from "./api";
import { loadError, start, status } from "./store";
import { loadTab, saveTab, TABS, tabForKey, type TabId } from "./tabs";
import AppHeader from "./components/AppHeader.vue";
import StatusTab from "./components/StatusTab.vue";
import InstallsSection from "./components/InstallsSection.vue";
import AccountsSection from "./components/AccountsSection.vue";
import ActivityLog from "./components/ActivityLog.vue";
import SettingsSection from "./components/SettingsSection.vue";
import AboutTab from "./components/AboutTab.vue";
import UpdateBanner from "./components/UpdateBanner.vue";

/** localStorage, or null where reading it throws (a blocked or private webview). */
function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

const tab = ref<TabId>(loadTab(storage()));
watch(tab, (id) => saveTab(storage(), id));

/** Arrow keys, Home and End move the selection and the focus with it (automatic activation). */
async function onKey(e: KeyboardEvent): Promise<void> {
  const next = tabForKey(tab.value, e.key);
  if (!next) return;
  e.preventDefault();
  tab.value = next;
  await nextTick();
  document.getElementById(`tab-${next}`)?.focus();
}

onMounted(() => {
  void start();
});
</script>

<template>
  <AppHeader />
  <UpdateBanner />
  <main>
    <p v-if="loadError" class="banner error">Couldn't read the app's status: {{ loadError }}</p>
    <template v-if="status">
      <div class="tabs" role="tablist" aria-label="Ryna" @keydown="onKey">
        <button
          v-for="t in TABS"
          :id="`tab-${t.id}`"
          :key="t.id"
          type="button"
          role="tab"
          :aria-selected="tab === t.id"
          :aria-controls="`panel-${t.id}`"
          :tabindex="tab === t.id ? 0 : -1"
          @click="tab = t.id"
        >
          {{ t.label }}
        </button>
      </div>
      <div
        v-for="t in TABS"
        v-show="tab === t.id"
        :id="`panel-${t.id}`"
        :key="t.id"
        class="tabpanel"
        :class="t.id"
        role="tabpanel"
        :aria-labelledby="`tab-${t.id}`"
        tabindex="0"
      >
        <StatusTab v-if="t.id === 'status'" />
        <InstallsSection v-else-if="t.id === 'game'" />
        <AccountsSection v-else-if="t.id === 'accounts'" />
        <SettingsSection v-else-if="t.id === 'settings'" />
        <ActivityLog v-else-if="t.id === 'activity'" />
        <AboutTab v-else-if="t.id === 'about'" />
      </div>
    </template>
    <p v-else-if="!loadError" class="muted">Loading…</p>
  </main>
  <footer>
    <span class="muted">Ryna {{ status?.appVersion ?? "" }} · agent {{ status?.agentVersion ?? "" }}</span>
    <button class="link" type="button" @click="api.openPortal()">Open ryna.io/wow</button>
  </footer>
</template>
