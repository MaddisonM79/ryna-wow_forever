import { describe, expect, it } from "vitest";
import {
  coreOf,
  corePage,
  coreTabForKey,
  coreTabKey,
  DEFAULT_CORE_TAB,
  DEFAULT_PAGE,
  loadCoreTab,
  loadPage,
  PAGE_KEY,
  pageForKey,
  pages,
  saveCoreTab,
  savePage,
  type TabStorage,
} from "./tabs";

function memory(): TabStorage & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return {
    data,
    getItem: (k) => data.get(k) ?? null,
    setItem: (k, v) => void data.set(k, v),
  };
}

const throwing: TabStorage = {
  getItem: () => {
    throw new Error("blocked");
  },
  setItem: () => {
    throw new Error("blocked");
  },
};

const WOW = { id: "wow", name: "World of Warcraft: Forever" };
const PAL = { id: "palworld", name: "Palworld" };

describe("the sidebar's pages", () => {
  it("is Status, a row per core, then Settings, Activity, About", () => {
    expect(pages([WOW]).map((p) => p.id)).toEqual(["status", "core:wow", "settings", "activity", "about"]);
    expect(pages([WOW, PAL]).map((p) => p.label)).toEqual([
      "Status",
      "World of Warcraft: Forever",
      "Palworld",
      "Settings",
      "Activity",
      "About",
    ]);
    expect(pages([]).map((p) => p.id)).toEqual(["status", "settings", "activity", "about"]);
  });

  it("knows which core a page is", () => {
    expect(corePage("wow")).toBe("core:wow");
    expect(coreOf("core:wow")).toBe("wow");
    expect(coreOf("status")).toBeNull();
    expect(pages([WOW]).find((p) => p.id === "core:wow")?.core).toBe("wow");
  });
});

describe("page persistence", () => {
  it("opens on Status with nothing stored, an unknown page, or no storage", () => {
    expect(DEFAULT_PAGE).toBe("status");
    expect(loadPage(memory(), [WOW])).toBe("status");
    const s = memory();
    s.data.set(PAGE_KEY, "charts");
    expect(loadPage(s, [WOW])).toBe("status");
    expect(loadPage(null, [WOW])).toBe("status");
  });

  it("falls back to Status for a core the agent no longer has, and for the old flat tabs", () => {
    const s = memory();
    s.data.set(PAGE_KEY, "core:palworld");
    expect(loadPage(s, [WOW])).toBe("status");
    expect(loadPage(s, [WOW, PAL])).toBe("core:palworld");
    s.data.set(PAGE_KEY, "game");
    expect(loadPage(s, [WOW])).toBe("status");
    s.data.set(PAGE_KEY, "accounts");
    expect(loadPage(s, [WOW])).toBe("status");
  });

  it("remembers the chosen page", () => {
    const s = memory();
    savePage(s, "activity");
    expect(s.data.get(PAGE_KEY)).toBe("activity");
    expect(loadPage(s, [WOW])).toBe("activity");
    savePage(s, "core:wow");
    expect(loadPage(s, [WOW])).toBe("core:wow");
  });

  it("remembers each core's tab on its own", () => {
    const s = memory();
    expect(DEFAULT_CORE_TAB).toBe("game");
    expect(loadCoreTab(s, "wow")).toBe("game");
    saveCoreTab(s, "wow", "combat");
    expect(s.data.get(coreTabKey("wow"))).toBe("combat");
    expect(loadCoreTab(s, "wow")).toBe("combat");
    expect(loadCoreTab(s, "palworld")).toBe("game");
    s.data.set(coreTabKey("wow"), "charts");
    expect(loadCoreTab(s, "wow")).toBe("game");
  });

  it("survives a storage that throws", () => {
    expect(loadPage(throwing, [WOW])).toBe("status");
    expect(loadCoreTab(throwing, "wow")).toBe("game");
    expect(() => savePage(throwing, "about")).not.toThrow();
    expect(() => saveCoreTab(throwing, "wow", "mods")).not.toThrow();
  });
});

describe("the sidebar's keys", () => {
  it("moves up and down the pages, wrapping at the ends", () => {
    expect(pageForKey("status", "ArrowDown", [WOW])).toBe("core:wow");
    expect(pageForKey("core:wow", "ArrowUp", [WOW])).toBe("status");
    expect(pageForKey("status", "ArrowUp", [WOW])).toBe("about");
    expect(pageForKey("about", "ArrowDown", [WOW])).toBe("status");
  });

  it("jumps with Home and End, and ignores other keys", () => {
    expect(pageForKey("settings", "Home", [WOW])).toBe("status");
    expect(pageForKey("settings", "End", [WOW])).toBe("about");
    expect(pageForKey("settings", "Enter", [WOW])).toBeNull();
    expect(pageForKey("settings", "ArrowRight", [WOW])).toBeNull();
  });
});

describe("a core's tab keys", () => {
  it("moves left and right along the tabs, wrapping at the ends", () => {
    expect(coreTabForKey("game", "ArrowRight")).toBe("accounts");
    expect(coreTabForKey("accounts", "ArrowLeft")).toBe("game");
    expect(coreTabForKey("game", "ArrowLeft")).toBe("settings");
    expect(coreTabForKey("settings", "ArrowRight")).toBe("game");
  });

  it("jumps with Home and End, and ignores other keys", () => {
    expect(coreTabForKey("combat", "Home")).toBe("game");
    expect(coreTabForKey("combat", "End")).toBe("settings");
    expect(coreTabForKey("combat", "ArrowDown")).toBeNull();
  });
});
