<script setup lang="ts">
import { ref } from "vue";
import { copyText, logText } from "../clipboard";
import { activity, clearActivity } from "../store";

function time(at: number): string {
  return new Date(at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

const copied = ref<"yes" | "no" | null>(null);

async function copyLog(): Promise<void> {
  copied.value = (await copyText(logText(activity.value, time))) ? "yes" : "no";
}
</script>

<template>
  <section class="activity">
    <div class="section-head">
      <h2>Activity</h2>
      <span class="row-actions">
        <span v-if="copied === 'no'" class="muted">Couldn't copy here</span>
        <button type="button" class="quiet" :disabled="!activity.length" @click="copyLog()">
          {{ copied === "yes" ? "Copied" : "Copy" }}
        </button>
        <button type="button" class="quiet" :disabled="!activity.length" @click="clearActivity(); copied = null">
          Clear
        </button>
      </span>
    </div>
    <p v-if="!activity.length" class="muted">Nothing yet this session.</p>
    <ol v-else class="log">
      <li v-for="line in activity" :key="line.id" :class="{ 'error-text': line.level === 'error' }">
        <time class="muted">{{ time(line.at) }}</time>
        <span>{{ line.text }}</span>
      </li>
    </ol>
  </section>
</template>
