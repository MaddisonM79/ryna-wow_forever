<script setup lang="ts">
// The at-a-glance tab: four cards side by side (account, game, combat, updates), each a few short
// rows and one number where a number is the point, so the window's width is used and nothing scrolls.
import { computed } from "vue";
import { accountsLine, ago, combatState, FIGHTS_LABEL, gameLine, lastFightLine, when } from "../describe";
import { now, status } from "../store";
import UpdateControl from "./UpdateControl.vue";

const game = computed(() => (status.value ? gameLine(status.value.installs, status.value.addonBundled) : null));
const accounts = computed(() => (status.value ? accountsLine(status.value.accounts, now.value) : null));
const combat = computed(() => (status.value ? combatState(status.value.combat) : null));
const install = computed(() => status.value?.installs[0] ?? null);
/** The addon's state without the game's name in front, which the card already says. */
const addonText = computed(() => game.value?.text.replace(/^World of Warcraft: Forever · /, "") ?? "");
</script>

<template>
  <div v-if="status" class="cards">
    <section class="card">
      <h2>Account</h2>
      <p class="lead">
        <strong v-if="status.signedIn">{{ status.signedInAs ?? "Signed in" }}</strong>
        <span v-else class="chip waiting">Not signed in</span>
      </p>
      <dl class="glance">
        <div>
          <dt>Syncing</dt>
          <dd>
            <span v-if="status.signedIn" class="chip" :class="status.watching ? 'pushed' : 'waiting'">
              {{ status.watching ? "While open" : "Paused" }}
            </span>
            <span v-else class="muted">Sign in first</span>
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
        <div v-if="accounts">
          <dt>Accounts</dt>
          <dd><span class="chip" :class="accounts.chip">{{ accounts.text }}</span></dd>
        </div>
      </dl>
    </section>

    <section class="card">
      <h2>Game</h2>
      <p class="lead">
        <strong v-if="install">World of Warcraft: Forever</strong>
        <span v-else class="chip waiting">Not found</span>
      </p>
      <dl class="glance">
        <div v-if="install">
          <dt>Folder</dt>
          <dd><span class="path">{{ install.path }}</span></dd>
        </div>
        <div v-if="game">
          <dt>Addon</dt>
          <dd><span class="chip" :class="game.chip">{{ addonText }}</span></dd>
        </div>
      </dl>
    </section>

    <section class="card">
      <h2>Combat</h2>
      <p class="lead">
        <span class="big">{{ status.combat.fightsTonight }}</span>
        <span class="muted">{{ FIGHTS_LABEL.toLowerCase() }}</span>
      </p>
      <dl class="glance">
        <div v-if="combat">
          <dt>Log</dt>
          <dd class="stack">
            <span class="chip" :class="combat.chip">{{ combat.text.split(" · ")[0] }}</span>
            <span v-if="combat.text.includes(' · ')" class="path">{{ combat.text.split(" · ").slice(1).join(" · ") }}</span>
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
      </dl>
    </section>

    <section class="card">
      <h2>Updates</h2>
      <p class="lead">
        <strong>Ryna {{ status.appVersion }}</strong>
        <span class="muted">· agent {{ status.agentVersion }}</span>
      </p>
      <UpdateControl :show-version="false" />
    </section>
  </div>
</template>
