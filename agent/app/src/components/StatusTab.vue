<script setup lang="ts">
// The at-a-glance tab: one "label · value" row for each thing worth knowing.
import { computed } from "vue";
import { accountsLine, ago, combatState, FIGHTS_LABEL, gameLine, lastFightLine, when } from "../describe";
import { now, status } from "../store";
import UpdateControl from "./UpdateControl.vue";

const game = computed(() => (status.value ? gameLine(status.value.installs, status.value.addonBundled) : null));
const accounts = computed(() => (status.value ? accountsLine(status.value.accounts, now.value) : null));
const combat = computed(() => (status.value ? combatState(status.value.combat) : null));
</script>

<template>
  <section v-if="status">
    <h2>Status</h2>
    <dl class="glance">
      <div>
        <dt>Sign-in</dt>
        <dd>
          <template v-if="status.signedIn">
            Signed in as <strong>{{ status.signedInAs ?? "you" }}</strong>
            <span class="chip" :class="status.watching ? 'pushed' : 'waiting'">
              {{ status.watching ? "Syncing while open" : "Not syncing" }}
            </span>
          </template>
          <span v-else class="chip waiting">Not signed in</span>
        </dd>
      </div>
      <div>
        <dt>Last sync</dt>
        <dd>
          <template v-if="status.lastRun">
            <span :title="when(status.lastRun.at)">{{ ago(status.lastRun.at, now) }}</span>
            <span v-if="!status.lastRun.ok" class="error-text">
              · {{ status.lastRun.advice ?? status.lastRun.error ?? "it didn't finish" }}
            </span>
          </template>
          <span v-else class="muted">Not yet</span>
        </dd>
      </div>
      <div v-if="game">
        <dt>Game</dt>
        <dd><span class="chip" :class="game.chip">{{ game.text }}</span></dd>
      </div>
      <div v-if="accounts">
        <dt>Accounts</dt>
        <dd><span class="chip" :class="accounts.chip">{{ accounts.text }}</span></dd>
      </div>
      <div v-if="combat">
        <dt>Combat log</dt>
        <dd><span class="chip" :class="combat.chip">{{ combat.text }}</span></dd>
      </div>
      <div>
        <dt>{{ FIGHTS_LABEL }}</dt>
        <dd>
          <strong>{{ status.combat.fightsTonight }}</strong>
        </dd>
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
      <div>
        <dt>Updates</dt>
        <dd><UpdateControl /></dd>
      </div>
    </dl>
  </section>
</template>
