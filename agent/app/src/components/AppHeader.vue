<script setup lang="ts">
import { computed } from "vue";
import { api } from "../api";
import { runningCores } from "../coreWords";
import { headerState, signInNote } from "../describe";
import { cancelSignIn, login, loginBusy, loginError, signIn, signOut, status } from "../store";

const who = computed(() => status.value?.signedInAs ?? "you");
/** The chip is the agent's state (Connected, Syncing…, Problem, Paused, Signed out), not the rule. */
const head = computed(() => (status.value ? headerState(status.value) : null));
</script>

<template>
  <header>
    <div class="who">
      <strong class="brand">ryna</strong>
      <span v-if="status?.signedIn">Signed in as <strong>{{ who }}</strong></span>
      <template v-if="head">
        <span class="chip" :class="head.chip" role="status">{{ head.text }}</span>
        <span v-if="head.detail" class="error-text detail">{{ head.detail }}</span>
        <span v-else-if="status && status.cores.length > 1 && head.state !== 'signedOut'" class="muted">
          {{ runningCores(status) }} of {{ status.cores.length }} running
        </span>
      </template>
    </div>
    <div class="actions">
      <template v-if="status?.signedIn">
        <button type="button" @click="api.syncNow()">Sync now</button>
        <button type="button" class="quiet" @click="signOut()">Sign out</button>
      </template>
      <button
        v-else-if="status && !login"
        type="button"
        class="primary"
        :disabled="loginBusy || !status.clientIdIssued"
        @click="signIn()"
      >
        Sign in
      </button>
    </div>
  </header>
  <section v-if="login" class="signin">
    <p>
      Check that the sign-in page shows this code, then allow it:
      <code class="code">{{ login.userCode }}</code>
    </p>
    <p>
      <button type="button" class="primary" @click="api.openSignInPage()">Open sign-in page</button>
      <button type="button" class="quiet" @click="cancelSignIn()">Cancel</button>
      <span class="muted">Waiting for you to allow it…</span>
    </p>
  </section>
  <p v-if="status && !status.signedIn && !status.clientIdIssued" class="banner">
    This build can't sign in yet: its sign-in client hasn't been issued. Developers can set
    <code>RYNA_CLIENT_ID</code>.
  </p>
  <p v-if="status?.keychainError" class="banner error">
    Couldn't read the sign-in from the system keychain: {{ status.keychainError }}
  </p>
  <p v-if="status && signInNote(status)" class="banner">{{ signInNote(status) }}</p>
  <p v-if="loginError" class="banner error">{{ loginError }}</p>
</template>
