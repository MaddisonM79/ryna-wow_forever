<script setup lang="ts">
// About: what this is, its versions, where it lives, who made it and what it's made with.
import { ref } from "vue";
import { api, errorText } from "../api";
import { copyText } from "../clipboard";
import { status } from "../store";
import UpdateControl from "./UpdateControl.vue";

const REPO = "https://github.com/MaddisonM79/ryna-wow_forever";

// The window has no command to open an arbitrary URL (only the portal and the sign-in page), so
// these are shown as text to select or copy.
const LINKS = [
  { label: "Source on GitHub", url: REPO },
  { label: "Releases", url: `${REPO}/releases` },
  { label: "README", url: `${REPO}#readme` },
];

const LICENSE_URL = `${REPO}/blob/main/LICENSE`;

// What the app is built on (agent/app/package.json, src-tauri/Cargo.toml, agent/core/Cargo.toml).
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
  <section v-if="status">
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
        <dt>Addon bundled</dt>
        <dd>{{ status.addonBundled ?? "None in this build" }}</dd>
      </div>
      <div>
        <dt>Updates</dt>
        <dd><UpdateControl /></dd>
      </div>
    </dl>
  </section>

  <section>
    <h2>Links</h2>
    <ul class="rows links">
      <li>
        <span>ryna.io/wow</span>
        <button class="link" type="button" @click="api.openPortal()">Open in browser</button>
      </li>
      <li v-for="l in LINKS" :key="l.url">
        <span>
          {{ l.label }}
          <span class="url">{{ l.url }}</span>
        </span>
        <span class="actions">
          <button class="link" type="button" @click="open(l.url)">Open</button>
          <button type="button" class="quiet" @click="copy(l.url)">
            {{ copied === l.url ? "Copied" : "Copy link" }}
          </button>
        </span>
      </li>
    </ul>
    <p v-if="copyFailed" class="muted">Couldn't copy it here: select the link and copy it instead.</p>
    <p v-if="openFailed" class="muted">Couldn't open the browser: {{ openFailed }}</p>
  </section>

  <section>
    <h2>Credits</h2>
    <p>Made by Maddison Markham, with Nyx (Claude).</p>
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
</template>
