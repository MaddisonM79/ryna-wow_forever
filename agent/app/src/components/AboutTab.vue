<script setup lang="ts">
// About: what this is, its versions, where it lives, who made it and what it's made with.
import { ref } from "vue";
import { api, errorText } from "../api";
import { copyText } from "../clipboard";
import { status } from "../store";
import UpdateControl from "./UpdateControl.vue";
import credits from "../../../../credits.json";

const REPO = "https://github.com/MaddisonM79/ryna_agent";

// The window has no command to open an arbitrary URL (only the portal and the sign-in page), so
// these are shown as text to select or copy.
const LINKS = [
  { label: "Source on GitHub", url: REPO },
  { label: "Releases", url: `${REPO}/releases` },
  { label: "README", url: `${REPO}#readme` },
];

const LICENSE_URL = `${REPO}/blob/main/LICENSE`;

// What the app is built on (agent/app/package.json, src-tauri/Cargo.toml, agent/host/Cargo.toml, cores/wow/core/Cargo.toml).
const THIRD_PARTY = [
  { name: "Tauri 2", what: "the window and tray, with its updater, autostart, opener and dialog plugins", licence: "MIT or Apache-2.0" },
  { name: "Vue 3", what: "this window's interface", licence: "MIT" },
  { name: "reqwest", what: "talking to ryna.io (with rustls)", licence: "MIT or Apache-2.0" },
  { name: "serde", what: "reading and writing JSON", licence: "MIT or Apache-2.0" },
  { name: "keyring", what: "keeping the sign-in in the system keychain", licence: "MIT or Apache-2.0" },
  { name: "flate2", what: "compressing fight digests", licence: "MIT or Apache-2.0" },
  { name: "sha2", what: "hashing account folder names before they leave", licence: "MIT or Apache-2.0" },
];

const copied = ref<string | null>(null);
const copyFailed = ref<string | null>(null);

const openFailed = ref<string | null>(null);
async function open(url: string): Promise<void> {
  try {
    await api.openLink(url);
    openFailed.value = null;
  } catch (e) {
    openFailed.value = errorText(e);
  }
}

async function copy(url: string): Promise<void> {
  const ok = await copyText(url);
  copied.value = ok ? url : null;
  copyFailed.value = ok ? null : url;
}
</script>

<template>
  <div class="cards">
  <section v-if="status" class="card">
    <h2>About</h2>
    <p>
      <strong>Ryna</strong>, the companion app for World of Warcraft: Forever. It sends what the
      Ryna addon saves, and your fights, to ryna.io.
    </p>
    <dl class="glance">
      <div>
        <dt>App</dt>
        <dd>Ryna {{ status.appVersion }}</dd>
      </div>
      <div>
        <dt>Agent</dt>
        <dd>{{ status.agentVersion }}</dd>
      </div>
      <div>
        <dt>Addon</dt>
        <dd>{{ status.addonBundled ?? "None in this build" }}</dd>
      </div>
      <div>
        <dt>Updates</dt>
        <dd><UpdateControl /></dd>
      </div>
    </dl>
  </section>

  <section class="card">
    <h2>Links</h2>
    <ul class="rows links">
      <li>
        <span>ryna.io/wow</span>
        <button class="link" type="button" @click="api.openPortal()">Open in browser</button>
      </li>
      <li v-for="l in LINKS" :key="l.url">
        <button class="link" type="button" :title="l.url" @click="open(l.url)">{{ l.label }}</button>
        <button type="button" class="quiet small" :title="`Copy ${l.url}`" @click="copy(l.url)">
          {{ copied === l.url ? "Copied" : "Copy" }}
        </button>
      </li>
    </ul>
    <p v-if="copyFailed" class="muted">Couldn't copy it here: select the link and copy it instead.</p>
    <p v-if="openFailed" class="muted">Couldn't open the browser: {{ openFailed }}</p>
  </section>

  <section class="card wide">
    <h2>Credits</h2>
    <p>Made by MaddisonM79, with Nyx (Claude).</p>
    <ul v-if="credits.length" class="credits">
      <li v-for="c in credits" :key="c.name">
        <strong>{{ c.name }}</strong> <span class="muted">· {{ c.role }}</span>
        <div class="muted">{{ c.line }}</div>
      </li>
    </ul>
    <p>
      MIT licence: <button class="link" type="button" @click="open(LICENSE_URL)">{{ LICENSE_URL }}</button>
    </p>
    <p class="muted">
      Not affiliated with Blizzard Entertainment. World of Warcraft is a trademark of Blizzard
      Entertainment, Inc.
    </p>
    <p class="muted">
      Privacy: only what the addon saved, your fights (summaries, plus digests unless you turn them
      off) and a heartbeat (versions, host name, OS, install folders) leave this computer; raw logs
      only if you turn that on. The data is never sold or used for advertising.
    </p>
    <h2 class="sub">Third-party</h2>
    <ul class="third-party">
      <li v-for="t in THIRD_PARTY" :key="t.name">
        <strong>{{ t.name }}</strong> <span class="muted">· {{ t.what }} · {{ t.licence }}</span>
      </li>
    </ul>
  </section>
  </div>
</template>
