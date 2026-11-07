// The window's tabs, which one is open, and how the arrow keys move between them. Pure, apart
// from the storage it's handed, so the tests can hold it.

export const TABS = [
  { id: "status", label: "Status" },
  { id: "game", label: "Game" },
  { id: "accounts", label: "Accounts" },
  { id: "settings", label: "Settings" },
  { id: "activity", label: "Activity" },
  { id: "about", label: "About" },
] as const;

export type TabId = (typeof TABS)[number]["id"];

export const DEFAULT_TAB: TabId = "status";
export const TAB_KEY = "ryna.tab";

/** The subset of Storage this needs (localStorage in the window, a fake in the tests). */
export type TabStorage = Pick<Storage, "getItem" | "setItem">;

export function isTab(v: unknown): v is TabId {
  return TABS.some((t) => t.id === v);
}

/** The tab to open with: the one last chosen, or Status when storage is empty, odd or throws. */
export function loadTab(storage: TabStorage | null | undefined): TabId {
  try {
    const v = storage?.getItem(TAB_KEY);
    return isTab(v) ? v : DEFAULT_TAB;
  } catch {
    return DEFAULT_TAB;
  }
}

/** Remember the chosen tab; a storage that throws (private mode, blocked) only forgets it. */
export function saveTab(storage: TabStorage | null | undefined, id: TabId): void {
  try {
    storage?.setItem(TAB_KEY, id);
  } catch {
    // Nothing to do: the tab still opens, it just isn't remembered.
  }
}

/** Where a key moves the selection from `current`, or null for a key that doesn't move it. */
export function tabForKey(current: TabId, key: string): TabId | null {
  const i = TABS.findIndex((t) => t.id === current);
  const last = TABS.length - 1;
  let next: number;
  switch (key) {
    case "ArrowRight":
      next = i >= last ? 0 : i + 1;
      break;
    case "ArrowLeft":
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
  return TABS[next]?.id ?? null;
}
