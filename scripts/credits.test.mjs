// credits.json is the one list of people thanked; the About tab imports it, and this holds the
// addon's Credits.lua, CREDITS.md and the README to it. From the repository root: node --test scripts/
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(join(ROOT, p), "utf8");
const credits = JSON.parse(read("credits.json"));

/** `text` as it's written inside a double-quoted Lua string: backslashes first, then quotes. */
const luaQuoted = (text) => text.replace(/\\/g, "\\\\").replace(/"/g, '\\"');

test("a credits line is quoted for Lua the way Lua reads it back", () => {
  assert.equal(luaQuoted('say "hi"'), 'say \\"hi\\"');
  assert.equal(luaQuoted("a\\b"), "a\\\\b");
  assert.equal(luaQuoted('\\"'), '\\\\\\"');
});

test("credits.json has names, roles and lines", () => {
  assert.ok(credits.length >= 1);
  for (const c of credits) {
    assert.match(c.name, /\S/);
    assert.match(c.role, /\S/);
    assert.match(c.line, /\S/);
  }
});

test("CREDITS.md and the addon's Credits.lua carry every name and line", () => {
  const md = read("CREDITS.md");
  const lua = read("cores/wow/addon/Ryna/Credits.lua");
  for (const c of credits) {
    assert.ok(md.includes(c.name), `CREDITS.md lacks ${c.name}`);
    assert.ok(md.includes(c.line), `CREDITS.md lacks ${c.name}'s line`);
    assert.ok(lua.includes(c.name), `Credits.lua lacks ${c.name}`);
    // The game's fonts have no beer glyph: the Lua copy says Cheers! instead.
    const luaLine = luaQuoted(c.line.replace("🍻", "Cheers!"));
    assert.ok(lua.includes(luaLine), `Credits.lua lacks ${c.name}'s line`);
  }
});

test("the README points at CREDITS.md", () => {
  assert.ok(read("README.md").includes("CREDITS.md"));
});
