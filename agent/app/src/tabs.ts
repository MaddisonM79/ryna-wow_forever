// The window's pages (the sidebar: Status, one per core, Settings, Activity, About), the tabs
// inside a core's page, which are open, and how the arrow keys move between them. Pure, apart
// from the storage it's handed, so the tests can hold it.

/** The agent's own pages, in sidebar order around the cores. */
export const AGENT_PAGES = {
  before: [{ id: "status", label: "Status" }],
  after: [
    { id: "settings", label: "Settings" },
    { id: "activity", label: "Activity" },
    { id: "about", label: "About" },
  ],
} as const;

export type AgentPageId = (typeof AGENT_PAGES.before)[number]["id"] | (typeof AGENT_PAGES.after)[number]["id"];

/** A core's page id: `core:<id>`, so a second core is a row in the sidebar, not code. */
export type CorePageId = `core:${string}`;
export type PageId = AgentPageId | CorePageId;

export interface Page {
  id: PageId;
  label: string;
  /** The core's id for a core's page; null for the agent's own. */
  core: string | null;
}

/** What a page needs to know about a core: `Status.cores[n]`'s id and name. */
export interface CoreRef {
  id: string;
  name: string;
}

export const DEFAULT_PAGE: PageId = "status";
export const PAGE_KEY = "ryna.tab";

/** The sidebar: Status, the cores in the agent's order, then Settings, Activity, About. */
export function pages(cores: readonly CoreRef[]): Page[] {
  return [
    ...AGENT_PAGES.before.map((p) => ({ id: p.id, label: p.label, core: null })),
    ...cores.map((c) => ({ id: corePage(c.id), label: c.name, core: c.id })),
    ...AGENT_PAGES.after.map((p) => ({ id: p.id, label: p.label, core: null })),
  ];
}

export function corePage(coreId: string): CorePageId {
  return `core:${coreId}`;
}

/** The core a page belongs to, or null for the agent's own pages. */
export function coreOf(page: PageId): string | null {
  return page.startsWith("core:") ? page.slice("core:".length) : null;
}

/** The tabs inside a core's page. */
export const CORE_TABS = [
  { id: "game", label: "Game" },
  { id: "accounts", label: "Accounts" },
  { id: "combat", label: "Combat" },
  { id: "mods", label: "Mods" },
  // The Settings mod's own tab: shown only while that mod is on (coreTabs).
  { id: "addonSettings", label: "Addon settings" },
  { id: "settings", label: "Settings" },
] as const;

export type CoreTabId = (typeof CORE_TABS)[number]["id"];
export type CoreTab = (typeof CORE_TABS)[number];

/** The tabs a core's page shows: all of them, less "Addon settings" while the Settings mod is off. */
export function coreTabs(settingsModOn: boolean): CoreTab[] {
  return CORE_TABS.filter((t) => settingsModOn || t.id !== "addonSettings");
}
export const DEFAULT_CORE_TAB: CoreTabId = "game";

/** Where a core's open tab is remembered: one slot per core. */
export function coreTabKey(coreId: string): string {
  return `ryna.coreTab.${coreId}`;
}

/** The subset of Storage this needs (localStorage in the window, a fake in the tests). */
export type TabStorage = Pick<Storage, "getItem" | "setItem">;

export function isPage(v: unknown, cores: readonly CoreRef[]): v is PageId {
  return pages(cores).some((p) => p.id === v);
}

export function isCoreTab(v: unknown): v is CoreTabId {
  return CORE_TABS.some((t) => t.id === v);
}

function read(storage: TabStorage | null | undefined, key: string): string | null {
  try {
    return storage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

function write(storage: TabStorage | null | undefined, key: string, value: string): void {
  try {
    storage?.setItem(key, value);
  } catch {
    // Nothing to do: the page still opens, it just isn't remembered.
  }
}

/**
 * The page to open with: the one last chosen, or Status when storage is empty, odd, throws, or
 * names a core the agent no longer has (or one of the old flat tabs, "game" and "accounts").
 */
export function loadPage(storage: TabStorage | null | undefined, cores: readonly CoreRef[]): PageId {
  const v = read(storage, PAGE_KEY);
  return isPage(v, cores) ? v : DEFAULT_PAGE;
}

/** Remember the chosen page; a storage that throws (private mode, blocked) only forgets it. */
export function savePage(storage: TabStorage | null | undefined, id: PageId): void {
  write(storage, PAGE_KEY, id);
}

/** The tab a core's page opens on: the one last chosen there, else Game. */
export function loadCoreTab(storage: TabStorage | null | undefined, coreId: string): CoreTabId {
  const v = read(storage, coreTabKey(coreId));
  return isCoreTab(v) ? v : DEFAULT_CORE_TAB;
}

export function saveCoreTab(storage: TabStorage | null | undefined, coreId: string, tab: CoreTabId): void {
  write(storage, coreTabKey(coreId), tab);
}

/**
 * Where a key moves the selection in a list of ids, or null for a key that doesn't move it. The
 * sidebar is vertical (Up and Down), a tab row horizontal (Left and Right); Home and End work in
 * both, and the ends wrap.
 */
export function moveInList<T extends string>(
  ids: readonly T[],
  current: T,
  key: string,
  orientation: "vertical" | "horizontal",
): T | null {
  const i = ids.indexOf(current);
  const last = ids.length - 1;
  if (last < 0) return null;
  const forward = orientation === "vertical" ? "ArrowDown" : "ArrowRight";
  const back = orientation === "vertical" ? "ArrowUp" : "ArrowLeft";
  let next: number;
  switch (key) {
    case forward:
      next = i >= last ? 0 : i + 1;
      break;
    case back:
      next = i <= 0 ? last : i - 1;
      break;
    case "Home":
      next = 0;
      break;
    case "End":
      next = last;
      break;
    default:
      return null;
  }
  return ids[next] ?? null;
}

/** The sidebar's keys: Up, Down, Home and End over the pages. */
export function pageForKey(current: PageId, key: string, cores: readonly CoreRef[]): PageId | null {
  return moveInList(
    pages(cores).map((p) => p.id),
    current,
    key,
    "vertical",
  );
}

/** A core's tab row's keys: Left, Right, Home and End over its tabs (the ones it shows). */
export function coreTabForKey(
  current: CoreTabId,
  key: string,
  tabs: readonly CoreTab[] = CORE_TABS,
): CoreTabId | null {
  return moveInList(
    tabs.map((t) => t.id),
    current,
    key,
    "horizontal",
  );
}

/** The window's localStorage, or null where reading it throws (a blocked or private webview). */
export function windowStorage(): TabStorage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}
