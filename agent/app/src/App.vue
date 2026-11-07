<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { api } from "./api";
import { loadError, start, status } from "./store";
import { loadPage, pages, savePage, windowStorage, type PageId } from "./tabs";
import AppHeader from "./components/AppHeader.vue";
import Sidebar from "./components/Sidebar.vue";
import StatusTab from "./components/StatusTab.vue";
import CorePage from "./components/CorePage.vue";
import ActivityLog from "./components/ActivityLog.vue";
import SettingsSection from "./components/SettingsSection.vue";
import AboutTab from "./components/AboutTab.vue";
import UpdateBanner from "./components/UpdateBanner.vue";

/** The cores the agent has, as the sidebar needs them (none until the status has loaded). */
const cores = computed(() => status.value?.cores.map((c) => ({ id: c.id, name: c.name })) ?? []);
const sidebar = computed(() => pages(cores.value));

const page = ref<PageId>(loadPage(windowStorage(), []));
watch(page, (id) => savePage(windowStorage(), id));
// The remembered page may be a core's, which isn't known until the first status: re-read it then.
let restored = false;
watch(cores, (list) => {
  if (restored || !list.length) return;
  restored = true;
  page.value = loadPage(windowStorage(), list);
});

/** The page's element id for the content's `aria-labelledby` (ids may hold a colon). */
function panelId(id: PageId): string {
  return `panel-${id}`;
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
    <div v-if="status" class="shell">
      <Sidebar :pages="sidebar" :cores="cores" :current="page" @select="(id) => (page = id)" />
      <div class="content">
        <template v-for="p in sidebar" :key="p.id">
          <CorePage v-if="p.core" v-show="page === p.id" :core="status.cores.find((c) => c.id === p.core)!" />
          <div
            v-else
            v-show="page === p.id"
            :id="panelId(p.id)"
            class="tabpanel"
            :class="p.id"
            tabindex="0"
          >
            <StatusTab v-if="p.id === 'status'" @open="(id) => (page = id)" />
            <SettingsSection v-else-if="p.id === 'settings'" />
            <ActivityLog v-else-if="p.id === 'activity'" />
            <AboutTab v-else-if="p.id === 'about'" />
          </div>
        </template>
      </div>
    </div>
    <p v-else-if="!loadError" class="muted">Loading…</p>
  </main>
  <footer>
    <span class="muted">ryna {{ status?.appVersion ?? "" }} · agent {{ status?.agentVersion ?? "" }}</span>
    <button class="link" type="button" @click="api.openPortal()">Open ryna.io/wow</button>
  </footer>
</template>
