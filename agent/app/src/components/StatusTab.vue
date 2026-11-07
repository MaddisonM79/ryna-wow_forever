<script setup lang="ts">
// The at-a-glance page: the agent's card (account, state, last sync, version, updates) and one
// card per core, side by side, each a few short rows, so the window's width is used and nothing
// scrolls.
import { computed } from "vue";
import { ago, headerState, when } from "../describe";
import { now, status } from "../store";
import type { PageId } from "../tabs";
import { corePage } from "../tabs";
import CoreCard from "./CoreCard.vue";
import UpdateControl from "./UpdateControl.vue";

const emit = defineEmits<{ open: [page: PageId] }>();
/** The same state as the header's chip. */
const head = computed(() => (status.value ? headerState(status.value) : null));
</script>

<template>
  <div v-if="status" class="cards">
    <section class="card">
      <h2>Agent</h2>
      <p class="lead">
        <strong v-if="status.signedIn">{{ status.signedInAs ?? "Signed in" }}</strong>
        <span v-else class="muted">Not signed in</span>
      </p>
      <dl class="glance">
        <div v-if="head">
          <dt>State</dt>
          <dd>
            <span class="chip" :class="head.chip">{{ head.text }}</span>
            <span v-if="head.detail" class="error-text">{{ head.detail }}</span>
          </dd>
        </div>
        <div>
          <dt>Last sync</dt>
          <dd>
            <span v-if="status.lastRun" :title="when(status.lastRun.at)">{{ ago(status.lastRun.at, now) }}</span>
            <span v-else class="muted">Not yet</span>
          </dd>
        </div>
        <div>
          <dt>Version</dt>
          <dd>
            <span>Ryna {{ status.appVersion }}</span>
            <span class="muted">· agent {{ status.agentVersion }}</span>
          </dd>
        </div>
        <div>
          <dt>Updates</dt>
          <dd><UpdateControl :show-version="false" /></dd>
        </div>
      </dl>
    </section>

    <CoreCard v-for="c in status.cores" :key="c.id" :core="c" @open="(id) => emit('open', corePage(id))" />
  </div>
</template>
