import { describe, expect, it } from "vitest";
import { DEFAULT_TAB, loadTab, saveTab, TAB_KEY, tabForKey, type TabStorage } from "./tabs";

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

describe("tab persistence", () => {
  it("opens on Status with nothing stored, an unknown tab, or no storage", () => {
    expect(DEFAULT_TAB).toBe("status");
    expect(loadTab(memory())).toBe("status");
    const s = memory();
    s.data.set(TAB_KEY, "charts");
    expect(loadTab(s)).toBe("status");
    expect(loadTab(null)).toBe("status");
  });

  it("remembers the chosen tab", () => {
    const s = memory();
    saveTab(s, "activity");
    expect(s.data.get(TAB_KEY)).toBe("activity");
    expect(loadTab(s)).toBe("activity");
  });

  it("survives a storage that throws", () => {
    expect(loadTab(throwing)).toBe("status");
    expect(() => saveTab(throwing, "about")).not.toThrow();
  });
});

describe("tab keys", () => {
  it("moves with the arrows, wrapping at the ends", () => {
    expect(tabForKey("status", "ArrowRight")).toBe("game");
    expect(tabForKey("game", "ArrowLeft")).toBe("status");
    expect(tabForKey("status", "ArrowLeft")).toBe("about");
    expect(tabForKey("about", "ArrowRight")).toBe("status");
  });

  it("jumps with Home and End, and ignores other keys", () => {
    expect(tabForKey("settings", "Home")).toBe("status");
    expect(tabForKey("settings", "End")).toBe("about");
    expect(tabForKey("settings", "Enter")).toBeNull();
    expect(tabForKey("settings", "ArrowDown")).toBeNull();
  });
});
