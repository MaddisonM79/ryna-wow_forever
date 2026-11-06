<script setup lang="ts">
import type { Phase } from "../api";
import { ago, when } from "../describe";
import { now, status } from "../store";

const PHASE_WORDS: Record<Phase, string> = {
  waiting: "Waiting",
  pushing: "Pushing",
  pushed: "Pushed",
  error: "Error",
};
</script>

<template>
  <section v-if="status">
    <h2>Accounts</h2>
    <p v-if="!status.accounts.length" class="muted">
      No saved data yet. Install the addon, log in to the game, then log out or type /reload.
    </p>
    <table v-else>
      <thead>
        <tr>
          <th>Account</th>
          <th>Last saved</th>
          <th>Last pushed</th>
          <th>Characters</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="a in status.accounts" :key="a.path">
          <td>
            <strong>{{ a.label }}</strong>
            <div v-if="a.lastError" class="path error-text">{{ a.lastError }}</div>
          </td>
          <td :title="when(a.savedAt)">{{ ago(a.savedAt, now) }}</td>
          <td :title="when(a.pushedAt)">{{ ago(a.pushedAt, now) }}</td>
          <td>{{ a.characters ?? "–" }}</td>
          <td><span class="chip" :class="a.phase">{{ PHASE_WORDS[a.phase] }}</span></td>
        </tr>
      </tbody>
    </table>
  </section>
</template>
