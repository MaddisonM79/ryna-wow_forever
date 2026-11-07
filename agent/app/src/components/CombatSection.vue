<script setup lang="ts">
// The core's Combat tab: the combat log's state and file, what it saw last, the fights waiting to
// go up, fights in the last 12 hours and the last one: what the Status card and the heartbeat know.
import { computed } from "vue";
import { ago, combatState, FIGHTS_LABEL, lastFightLine, when } from "../describe";
import { now, status } from "../store";

const combat = computed(() => (status.value ? combatState(status.value.combat) : null));
</script>

<template>
  <section v-if="status">
    <h2>Combat</h2>
    <p v-if="!status.combat.file" class="muted">No combat log yet. Type /combatlog in the game to start one.</p>
    <dl v-else class="glance">
      <div v-if="combat">
        <dt>Log</dt>
        <dd><span class="chip" :class="combat.chip">{{ combat.text.split(" · ")[0] }}</span></dd>
      </div>
      <div>
        <dt>File</dt>
        <dd><span class="path">{{ status.combat.file }}</span></dd>
      </div>
      <div>
        <dt>Advanced logging</dt>
        <dd>
          <span v-if="status.combat.advanced === null" class="muted">Not seen yet</span>
          <span v-else class="chip" :class="status.combat.advanced ? 'pushed' : 'waiting'">
            {{ status.combat.advanced ? "On" : "Off" }}
          </span>
        </dd>
      </div>
      <div>
        <dt>Last event</dt>
        <dd>
          <span v-if="status.combat.lastEventAt" :title="when(status.combat.lastEventAt)">
            {{ ago(status.combat.lastEventAt, now) }}
          </span>
          <span v-else class="muted">None yet</span>
        </dd>
      </div>
      <div>
        <dt>Waiting to upload</dt>
        <dd>{{ status.combat.pending }}</dd>
      </div>
      <div>
        <dt>{{ FIGHTS_LABEL }}</dt>
        <dd><strong>{{ status.combat.fightsTonight }}</strong></dd>
      </div>
      <div>
        <dt>Last fight</dt>
        <dd>
          <span v-if="status.combat.lastFight" :title="when(status.combat.lastFight.at)">
            {{ lastFightLine(status.combat.lastFight, now) }}
          </span>
          <span v-else class="muted">None yet</span>
        </dd>
      </div>
      <div v-if="status.combat.lastError">
        <dt>Error</dt>
        <dd class="error-text">{{ status.combat.lastError }}</dd>
      </div>
    </dl>
  </section>
</template>
