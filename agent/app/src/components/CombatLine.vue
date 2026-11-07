<script setup lang="ts">
// The combat log's line: fights in the last 12 hours and the last one. Shared by Accounts and Status.
import { FIGHTS_LABEL, lastFightLine, when } from "../describe";
import { now, status } from "../store";
</script>

<template>
  <template v-if="status">
    <template v-if="status.combat.file">
      {{ FIGHTS_LABEL }}: <strong>{{ status.combat.fightsTonight }}</strong>
      <span v-if="status.combat.lastFight" :title="when(status.combat.lastFight.at)">
        · last: {{ lastFightLine(status.combat.lastFight, now) }}
      </span>
      <template v-if="status.combat.pending"> · {{ status.combat.pending }} waiting</template>
      <span v-if="status.combat.lastError" class="error-text"> · {{ status.combat.lastError }}</span>
    </template>
    <template v-else>No combat log yet. Type /combatlog in the game to start one.</template>
  </template>
</template>
