<script setup lang="ts">
// The left side: Status, one row per core, then Settings, Activity, About. Under 700 px it is
// icons only, each with its name as a tooltip. The arrow keys, Home and End move the selection
// and the focus with it.
import { nextTick } from "vue";
import { pageForKey, type CoreRef, type Page, type PageId } from "../tabs";

const props = defineProps<{ pages: Page[]; cores: CoreRef[]; current: PageId }>();
const emit = defineEmits<{ select: [id: PageId] }>();

/** The sidebar's glyphs, drawn inline: no icon library. Every core shares the game glyph. */
function glyph(page: Page): "status" | "core" | "settings" | "activity" | "about" {
  if (page.core) return "core";
  return page.id as "status" | "settings" | "activity" | "about";
}

/** A page's element id in the DOM, for moving the focus (ids may hold a colon). */
function navId(id: PageId): string {
  return `nav-${id}`;
}

async function onKey(e: KeyboardEvent): Promise<void> {
  const next = pageForKey(props.current, e.key, props.cores);
  if (!next) return;
  e.preventDefault();
  emit("select", next);
  await nextTick();
  document.getElementById(navId(next))?.focus();
}
</script>

<template>
  <nav class="sidebar" role="navigation" aria-label="Ryna" @keydown="onKey">
    <button
      v-for="p in pages"
      :id="navId(p.id)"
      :key="p.id"
      type="button"
      :title="p.label"
      :aria-current="current === p.id ? 'page' : undefined"
      :tabindex="current === p.id ? 0 : -1"
      @click="emit('select', p.id)"
    >
      <svg class="glyph" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round">
        <template v-if="glyph(p) === 'status'">
          <circle cx="8" cy="8" r="6" />
          <circle cx="8" cy="8" r="2" fill="currentColor" stroke="none" />
        </template>
        <template v-else-if="glyph(p) === 'core'">
          <rect x="1.5" y="4.5" width="13" height="7" rx="3.5" />
          <path d="M5 6.5v3M3.5 8h3" />
          <circle cx="10.5" cy="7.3" r="0.8" fill="currentColor" stroke="none" />
          <circle cx="12.2" cy="8.9" r="0.8" fill="currentColor" stroke="none" />
        </template>
        <template v-else-if="glyph(p) === 'settings'">
          <path d="M2 4.5h12M2 8h12M2 11.5h12" />
          <circle cx="5.5" cy="4.5" r="1.6" fill="var(--bg)" />
          <circle cx="10.5" cy="8" r="1.6" fill="var(--bg)" />
          <circle cx="6.5" cy="11.5" r="1.6" fill="var(--bg)" />
        </template>
        <template v-else-if="glyph(p) === 'activity'">
          <path d="M2 4h12M2 8h9M2 12h11" />
        </template>
        <template v-else>
          <circle cx="8" cy="8" r="6" />
          <path d="M8 7.2v4" />
          <circle cx="8" cy="5" r="0.8" fill="currentColor" stroke="none" />
        </template>
      </svg>
      <span class="label">{{ p.label }}</span>
    </button>
  </nav>
</template>
