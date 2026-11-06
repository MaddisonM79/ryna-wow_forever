<script setup lang="ts">
import { activity } from "../store";

function time(at: number): string {
  return new Date(at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
</script>

<template>
  <section>
    <h2>Activity</h2>
    <p v-if="!activity.length" class="muted">Nothing yet this session.</p>
    <ol v-else class="log">
      <li v-for="line in activity" :key="line.id" :class="{ 'error-text': line.level === 'error' }">
        <time class="muted">{{ time(line.at) }}</time>
        <span>{{ line.text }}</span>
      </li>
    </ol>
  </section>
</template>
