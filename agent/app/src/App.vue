<script setup lang="ts">
import { onMounted } from "vue";
import { api } from "./api";
import { loadError, start, status } from "./store";
import AppHeader from "./components/AppHeader.vue";
import InstallsSection from "./components/InstallsSection.vue";
import AccountsSection from "./components/AccountsSection.vue";
import ActivityLog from "./components/ActivityLog.vue";
import SettingsSection from "./components/SettingsSection.vue";

onMounted(() => {
  void start();
});
</script>

<template>
  <AppHeader />
  <main>
    <p v-if="loadError" class="banner error">Couldn't read the app's status: {{ loadError }}</p>
    <template v-if="status">
      <InstallsSection />
      <AccountsSection />
      <ActivityLog />
      <SettingsSection />
    </template>
    <p v-else-if="!loadError" class="muted">Loading…</p>
  </main>
  <footer>
    <span class="muted">Ryna {{ status?.appVersion ?? "" }} · agent {{ status?.agentVersion ?? "" }}</span>
    <button class="link" type="button" @click="api.openPortal()">Open ryna.io/wow</button>
  </footer>
</template>
