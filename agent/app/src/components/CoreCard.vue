<script setup lang="ts">
// One core's card on Status: the game and its addon, the accounts, the fights, and a button to its
// page. The WoW core's card reads the fields the window already has for it; any other core's is
// drawn from the rows it lays out through the agent's interface (`CoreView.rows`).
import { computed } from "vue";
import type { CoreView } from "../api";
import { corePanel } from "../coreWords";
import { accountsLine, combatState, FIGHTS_LABEL, gameLine, lastFightLine, when } from "../describe";
import { now, status } from "../store";

const props = defineProps<{ core: CoreView }>();
defineEmits<{ open: [id: string] }>();

const wow = computed(() => props.core.id === "wow" && status.value ? status.value : null);
const game = computed(() => (wow.value ? gameLine(wow.value.installs, wow.value.addonBundled) : null));
const accounts = computed(() => (wow.value ? accountsLine(wow.value.accounts, now.value) : null));
const combat = computed(() => (wow.value ? combatState(wow.value.combat) : null));
const install = computed(() => wow.value?.installs.find((i) => i.forever) ?? wow.value?.installs[0] ?? null);
/** The addon's state without the game's name in front, which the card already says. */
/** A fetched core's state panel, said in one chip on the card. */
const panel = computed(() => (props.core.id === "wow" ? null : corePanel(props.core)));
const addonText = computed(() => game.value?.text.replace(/^World of Warcraft: Forever · /, "") ?? "");
</script>

<template>
  <section class="card">
    <div class="section-head">
      <h2>{{ core.name }}</h2>
      <button type="button" class="quiet small" @click="$emit('open', core.id)">Open page</button>
    </div>
    <template v-if="wow">
      <p class="lead">
        <strong v-if="install">Found</strong>
        <span v-else class="chip waiting">Not found</span>
        <span v-if="install" class="path">{{ install.path }}</span>
      </p>
      <dl class="glance">
        <div v-if="game">
          <dt>Addon</dt>
          <dd><span class="chip" :class="game.chip">{{ addonText }}</span></dd>
        </div>
        <div v-if="accounts">
          <dt>Accounts</dt>
          <dd><span class="chip" :class="accounts.chip">{{ accounts.text }}</span></dd>
        </div>
        <div v-if="combat">
          <dt>Log</dt>
          <dd class="stack">
            <span class="chip" :class="combat.chip">{{ combat.text.split(" · ")[0] }}</span>
            <span v-if="combat.text.includes(' · ')" class="path">{{ combat.text.split(" · ").slice(1).join(" · ") }}</span>
          </dd>
        </div>
        <div>
          <dt>Fights</dt>
          <dd>
            <strong>{{ wow.combat.fightsTonight }}</strong>
            <span class="muted">{{ FIGHTS_LABEL.replace(/^Fights, /, "") }}</span>
          </dd>
        </div>
        <div>
          <dt>Last fight</dt>
          <dd>
            <span v-if="wow.combat.lastFight" :title="when(wow.combat.lastFight.at)">
              {{ lastFightLine(wow.combat.lastFight, now) }}
            </span>
            <span v-else class="muted">None yet</span>
          </dd>
        </div>
      </dl>
    </template>
    <p v-if="!wow && panel" class="lead core-state" :class="`state-${panel.state}`">
      <span class="chip" :class="panel.tone === 'error' ? 'error' : 'waiting'">{{ panel.title }}</span>
    </p>
    <dl v-if="!wow" class="glance">
      <div v-for="row in core.rows" :key="row.label">
        <dt>{{ row.label }}</dt>
        <dd>
          <span v-if="row.chip" class="chip" :class="row.chip">{{ row.value }}</span>
          <span v-else>{{ row.value }}</span>
        </dd>
      </div>
      <div v-if="!core.rows.length && !panel">
        <dt>Version</dt>
        <dd>{{ core.version }} <span class="muted">· nothing to show yet</span></dd>
      </div>
    </dl>
  </section>
</template>
