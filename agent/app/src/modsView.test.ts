// The Mods tab's structure, rendered to HTML on the server renderer (no DOM needed): one grid with
// the same cells in every row, one button for the whole card, and one place for the last result.
import { describe as group, expect, it } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import type { InstallResult, InstallView, ModView } from "./api";
import { installSummary, modsAction, pathPieces, type ModsResult } from "./describe";
import ModsView from "./components/ModsView.vue";

const AT = "/Applications/World of Warcraft/_forever_";
const ADDONS = `${AT}/Interface/AddOns`;
const INSTALL: InstallView = {
  path: AT,
  flavor: "_forever_",
  forever: true,
  hasAddon: true,
  addonVersion: "0.3.9",
  addon: "older",
};

function mod(id: string, version: string | null, opts: Partial<ModView> = {}): ModView {
  return {
    id,
    name: id[0]!.toUpperCase() + id.slice(1),
    summary: `What ${id} does.`,
    kind: "addon",
    actsInGame: false,
    defaultOn: true,
    on: true,
    bundled: "0.4.0",
    addons: [`Ryna_${id}`],
    installs: [{ install: AT, installed: version !== null, version, offInGame: 0 }],
    ...opts,
  };
}

const SETTINGS = mod("settings", null, { kind: "agent", installs: [], addons: [] });
const OLD = [mod("chronicle", "0.3.9"), mod("automation", "0.3.9", { actsInGame: true }), mod("prices", "0.3.9"), SETTINGS];

async function render(mods: ModView[], result: ModsResult | null = null, installs = [INSTALL]): Promise<string> {
  const app = createSSRApp({
    render: () => h(ModsView, { mods, installs, result, toggling: null, installing: null }),
  });
  // Without the renderer's fragment markers, which say nothing about the structure. Removed until
  // none are left, so taking one out can't leave another behind (CodeQL alert 19).
  let html = await renderToString(app);
  for (let before = ""; before !== html; ) {
    before = html;
    html = html.replace(/<!--[^]*?-->/g, "");
  }
  return html;
}

function count(html: string, re: RegExp): number {
  return html.match(re)?.length ?? 0;
}

/** The rows of the grid, each as its own HTML. */
function rows(html: string): string[] {
  return html.split('<li class="mod">').slice(1);
}

group("the Mods tab's grid", () => {
  it("gives every row the same three cells, a state even where there's nothing to do", async () => {
    const html = await render(OLD);
    const all = rows(html);
    expect(all).toHaveLength(4);
    for (const row of all) {
      expect(count(row, /class="mod-switch"/g)).toBe(1);
      expect(count(row, /class="mod-text"/g)).toBe(1);
      expect(count(row, /class="mod-status"/g)).toBe(1);
      expect(count(row, /class="[^"]*\bchip\b/g)).toBe(1);
      expect(row).not.toContain("<button");
    }
    // Settings has no update and still has its state where the others have theirs.
    expect(all[3]).toContain("On · runs in the app");
  });

  it("offers the one install every row's update was, once, in the card's head", async () => {
    const html = await render(OLD);
    expect(count(html, /<button/g)).toBe(1);
    const head = html.slice(0, html.indexOf('class="mods-grid"'));
    expect(head).toContain("Update all to 0.4.0");
  });

  it("offers nothing when everything is current", async () => {
    const html = await render([mod("chronicle", "0.4.0"), SETTINGS]);
    expect(html).not.toContain("<button");
    expect(rows(html)).toHaveLength(2);
  });

  it("keeps the words that were there", async () => {
    const html = await render(OLD);
    expect(html).toContain("This mod acts in the game for you.");
    expect(html).toContain("On · 0.3.9 installed, 0.4.0 available");
  });
});

group("the Mods tab's result", () => {
  const installed: ModsResult = {
    ok: true,
    summary: "Updated the addon and 3 mods to 0.4.0",
    lines: [`Installed addon 0.4.0 into ${ADDONS}/Ryna`, `Installed the Chronicle mod 0.4.0 into ${ADDONS}/Ryna_Chronicle`],
  };

  it("shows once for the card, above the rows and never inside one", async () => {
    const html = await render(OLD, installed);
    expect(count(html, /class="[^"]*\bmods-result[ "]/g)).toBe(1);
    expect(html.indexOf("mods-result")).toBeLessThan(html.indexOf('class="mods-grid"'));
    for (const row of rows(html)) expect(row).not.toContain("banner");
    expect(html).toContain("Updated the addon and 3 mods to 0.4.0");
    expect(html).toContain("Dismiss");
  });

  it("puts each line on its own, and lets a path wrap between folders", async () => {
    const html = await render(OLD, installed);
    const list = html.slice(html.indexOf("<ul>"), html.indexOf("</ul>"));
    expect(count(list, /<li>/g)).toBe(2);
    expect(list).toContain("World of Warcraft/<wbr>_forever_/<wbr>Interface/");
  });

  it("shows an error in the same place", async () => {
    const html = await render(OLD, { ok: false, summary: "Couldn't install the addon.", lines: ["Permission denied"] });
    expect(html).toMatch(/class="error banner mods-result" role="alert"/);
    for (const row of rows(html)) expect(row).not.toContain("Permission denied");
  });
});

group("the card's words", () => {
  it("names the card's button for what it does", () => {
    expect(modsAction(OLD, AT)).toEqual({ label: "Update all to 0.4.0", kind: "update" });
    expect(modsAction([mod("prices", null), ...OLD], AT)).toEqual({ label: "Install the addon", kind: "install" });
    expect(modsAction([mod("chronicle", "0.3.9"), mod("prices", "0.3.9", { bundled: "0.4.1" })], AT)?.label).toBe("Update all");
    expect(modsAction([mod("chronicle", "0.4.0"), mod("prices", "0.3.9", { on: false }), SETTINGS], AT)).toBeNull();
  });

  it("sums an install up in one line", () => {
    const change = (name: string) =>
      ({ change: "installed", id: name, name, folder: `Ryna_${name}`, path: `${ADDONS}/Ryna_${name}`, version: "0.4.0" }) as const;
    const r: InstallResult = {
      version: "0.4.0",
      installed: [{ install: AT, path: `${ADDONS}/Ryna` }],
      mods: [change("Chronicle"), change("Automation"), change("Prices")],
      gameRunning: false,
    };
    expect(installSummary(r, true)).toBe("Updated the addon and 3 mods to 0.4.0");
    expect(installSummary(r, false)).toBe("Installed the addon and 3 mods (0.4.0)");
    expect(installSummary({ ...r, mods: [] }, true)).toBe("Updated the addon to 0.4.0");
    const removed = { change: "removed", id: "prices", name: "Prices", folder: "Ryna_Prices", path: "/p" } as const;
    expect(installSummary({ ...r, mods: [change("Chronicle"), removed] }, true)).toBe(
      "Updated the addon and 1 mod to 0.4.0, and removed 1 mod that's off",
    );
  });

  it("cuts a line into pieces that end at each slash, and loses nothing", () => {
    const line = `Installed addon 0.4.0 into ${ADDONS}/Ryna`;
    expect(pathPieces(line).join("")).toBe(line);
    expect(pathPieces("/a/b")).toEqual(["/", "a/", "b"]);
    expect(pathPieces("no path")).toEqual(["no path"]);
  });
});
