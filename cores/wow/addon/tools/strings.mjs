#!/usr/bin/env node
// Checks the addons' translatable strings (no dependencies; Node 18+): the core addon
// (cores/wow/addon/Ryna) and every WoW mod's addon (mods/wow/*/addon/*). From the repository root:
//
//   node cores/wow/addon/tools/strings.mjs [folder...]              report and exit 1 when anything is off
//   node cores/wow/addon/tools/strings.mjs --templates [folder...]  also rewrite the other Locales/*.lua from
//                                                         enUS.lua, keeping every translation already in them
//
// With no folder it checks them all; a folder is one addon's (holding its .toc and Locales/).
// A key is used where the code says L["..."] or N("...") (string literals, joined with `..` for
// long ones; L[x] with anything else is a lookup of an N(...) key and is skipped). Each addon lists
// the keys it uses in its own Locales/enUS.lua, except those the core's already has: a mod's L falls
// back to the core's (Ryna.L). It reports keys an addon uses that neither list has, keys in an
// enUS.lua that nothing uses (the core's counts its mods' uses), a mod repeating a core key, and,
// per locale template, keys missing from it or no longer in enUS.lua. Every list empty: exit 0.

import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const CORE = join(ROOT, "cores", "wow", "addon", "Ryna");
const MODS = join(ROOT, "mods", "wow");
const BASE = "enUS";

// Locale code (as GetLocale() reports it) -> language. The order is the TOC's.
export const TEMPLATES = {
  deDE: "German",
  frFR: "French",
  esES: "Spanish (Spain)",
  esMX: "Spanish (Latin America)",
  itIT: "Italian",
  ptBR: "Portuguese (Brazil)",
  ruRU: "Russian",
  koKR: "Korean",
  zhCN: "Chinese (Simplified)",
  zhTW: "Chinese (Traditional)",
};

// --- A Lua 5.1 tokenizer: names, strings (decoded), numbers and punctuation, comments skipped.

function longBracket(src, i) {
  // At "[": returns the level of a long bracket ([[, [=[, ...) or -1.
  let j = i + 1;
  while (src[j] === "=") j++;
  return src[j] === "[" ? j - i - 1 : -1;
}

const ESCAPES = { n: "\n", t: "\t", r: "\r", a: "\x07", b: "\b", f: "\f", v: "\v", "\\": "\\", '"': '"', "'": "'" };

export function tokenize(src) {
  const tokens = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (/\s/.test(c)) { i++; continue; }
    if (src.startsWith("--", i)) {
      if (src[i + 2] === "[" && longBracket(src, i + 2) >= 0) {
        const close = "]" + "=".repeat(longBracket(src, i + 2)) + "]";
        const end = src.indexOf(close, i);
        i = end < 0 ? src.length : end + close.length;
      } else {
        const end = src.indexOf("\n", i);
        i = end < 0 ? src.length : end + 1;
      }
      continue;
    }
    const start = i;
    if (c === '"' || c === "'") {
      let value = "";
      i++;
      while (i < src.length && src[i] !== c) {
        if (src[i] === "\\") {
          const e = src[i + 1];
          if (/\d/.test(e)) {
            const m = /^\d{1,3}/.exec(src.slice(i + 1));
            value += String.fromCharCode(Number(m[0]));
            i += 1 + m[0].length;
            continue;
          }
          value += e === "\n" ? "\n" : (ESCAPES[e] ?? e);
          i += 2;
          continue;
        }
        if (src[i] === "\n") throw new Error(`unfinished string at offset ${start}`);
        value += src[i++];
      }
      i++;
      tokens.push({ kind: "string", value, start, end: i });
      continue;
    }
    if (c === "[" && longBracket(src, i) >= 0) {
      const level = longBracket(src, i);
      const close = "]" + "=".repeat(level) + "]";
      const end = src.indexOf(close, i);
      let value = src.slice(i + level + 2, end);
      if (value.startsWith("\n")) value = value.slice(1);
      i = end + close.length;
      tokens.push({ kind: "string", value, start, end: i });
      continue;
    }
    if (/[A-Za-z_]/.test(c)) {
      const m = /^[A-Za-z_][A-Za-z0-9_]*/.exec(src.slice(i));
      i += m[0].length;
      tokens.push({ kind: "name", value: m[0], start, end: i });
      continue;
    }
    if (/\d/.test(c)) {
      const m = /^(0x[0-9a-fA-F]+|\d+(\.\d*)?([eE][+-]?\d+)?)/.exec(src.slice(i));
      i += m[0].length;
      tokens.push({ kind: "number", value: m[0], start, end: i });
      continue;
    }
    const op = ["...", "..", "==", "~=", "<=", ">="].find((o) => src.startsWith(o, i)) ?? c;
    i += op.length;
    tokens.push({ kind: "op", value: op, start, end: i });
  }
  return tokens;
}

// From tokens[i], string ('..' string)* closed by `close`: { key, end (index of close) } or null.
function literalKey(tokens, i, close) {
  if (tokens[i]?.kind !== "string") return null;
  let key = tokens[i].value;
  let j = i + 1;
  while (tokens[j]?.value === ".." && tokens[j + 1]?.kind === "string") {
    key += tokens[j + 1].value;
    j += 2;
  }
  return tokens[j]?.kind === "op" && tokens[j].value === close ? { key, end: j } : null;
}

// Every L["..."] and N("...") key in one file, with its line.
export function usedKeys(src) {
  const tokens = tokenize(src);
  const out = [];
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    const prev = tokens[i - 1];
    if (t.kind !== "name" || (prev && (prev.value === "." || prev.value === ":"))) continue;
    let found = null;
    if (t.value === "L" && tokens[i + 1]?.value === "[") found = literalKey(tokens, i + 2, "]");
    if (t.value === "N" && tokens[i + 1]?.value === "(") found = literalKey(tokens, i + 2, ")");
    if (found) out.push({ key: found.key, line: src.slice(0, t.start).split("\n").length });
  }
  return out;
}

// The entries of a locale file: L[<literal>] = <value>, with offsets of the value's source text.
export function localeEntries(src) {
  const tokens = tokenize(src);
  const out = [];
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (t.kind !== "name" || t.value !== "L" || tokens[i + 1]?.value !== "[") continue;
    const found = literalKey(tokens, i + 2, "]");
    if (!found || tokens[found.end + 1]?.value !== "=") continue;
    const v = found.end + 2;
    let last = v;
    while (tokens[last + 1]?.value === ".." && tokens[last + 2]) last += 2;
    out.push({
      key: found.key,
      valueStart: tokens[v].start,
      valueEnd: tokens[last].end,
      value: src.slice(tokens[v].start, tokens[last].end),
    });
    i = last;
  }
  return out;
}

function luaFiles(dir, locales = join(dir, "Locales")) {
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (p !== locales) out.push(...luaFiles(p, locales));
    } else if (entry.name.endsWith(".lua")) {
      out.push(p);
    }
  }
  return out.sort();
}

// The addon folders: the core's, then each mod's (mods/wow/<id>/addon/<Folder>/ with a Locales/enUS.lua).
export function addonFolders() {
  const out = [CORE];
  if (!existsSync(MODS)) return out;
  for (const mod of readdirSync(MODS, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name).sort()) {
    const addons = join(MODS, mod, "addon");
    if (!existsSync(addons)) continue;
    for (const a of readdirSync(addons, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name).sort()) {
      if (existsSync(join(addons, a, "Locales", `${BASE}.lua`))) out.push(join(addons, a));
    }
  }
  return out;
}

/** An addon's title for the template headers: "Ryna", "Ryna Chronicle". */
function title(dir) {
  return basename(dir).replace(/_/g, " ");
}

function templateHeader(code, language, name, isMod) {
  return `-- ${language} (${code}) strings for ${name}. Loaded on every client, applied only when the game runs
-- in ${code}: GetLocale() is "${code}".
--
-- How to translate:
--   - Each line is L["English"] = nil. Replace nil with your text in double quotes:
--       L["Run census now"] = "..."
--     A line left as nil shows the English, so a partial translation is fine.
--   - Keep every %s and %d, in the same order (WoW's string.format can't reorder them).
--   - Keep "||" as it is: it prints one "|" in chat. Inside a command (/ryna debug on||off||toggle,
--     /ryna census stop, /who, /reload) keep the words in English: they are what the player types.
--   - Plurals: a pair like "%d player" / "%d players" is used for 1 / everything else.
--     For a language with more forms, use the wording that reads best for most numbers.
--   - Keys are the English text and must stay exactly as they are; only change the values.
--   - Test: set the client's language (Battle.net -> Settings -> Game Settings -> Text Language,
--     or SET textLocale "${code}" in WTF/Config.wtf, with that language installed), log in, then
--     /ryna, /ryna status and /ryna help.
-- This file is generated from enUS.lua by cores/wow/addon/tools/strings.mjs --templates, which keeps the
-- translations already here; run it after enUS.lua changes.

local _, ns = ...
${isMod ? "if ns.off then return end\n" : ""}if ns.Locale() ~= "${code}" then return end
local L = ns.L
`;
}

// Rewrites one template from enUS.lua's text: the same keys, layout and comments, with each value
// taken from the existing template (when it has a translation) or nil.
function writeTemplate(dir, code, baseSrc, baseEntries) {
  const path = join(dir, "Locales", `${code}.lua`);
  let existing = new Map();
  try {
    for (const e of localeEntries(readFileSync(path, "utf8"))) existing.set(e.key, e.value);
  } catch {
    existing = new Map();
  }
  const bodyStart = baseSrc.indexOf("\n", baseSrc.indexOf("local L = ns.L")) + 1;
  let body = "";
  let at = bodyStart;
  for (const e of baseEntries) {
    body += baseSrc.slice(at, e.valueStart);
    const value = existing.get(e.key);
    body += value && value !== "nil" ? value : "nil";
    at = e.valueEnd;
  }
  body += baseSrc.slice(at);
  writeFileSync(path, templateHeader(code, TEMPLATES[code], title(dir), dir !== CORE) + body);
}

function usedIn(dir) {
  const used = new Map();
  for (const file of luaFiles(dir)) {
    for (const u of usedKeys(readFileSync(file, "utf8"))) {
      if (!used.has(u.key)) used.set(u.key, `${relative(ROOT, file)}:${u.line}`);
    }
  }
  return used;
}

function baseOf(dir) {
  const src = readFileSync(join(dir, "Locales", `${BASE}.lua`), "utf8");
  return { src, entries: localeEntries(src) };
}

// One addon's problems. `coreKeys`: the core's enUS keys (for a mod); `modUses`: every key the mods
// use (for the core, whose strings they may share).
function check(dir, { coreKeys, modUses, writeTemplates }) {
  const isMod = dir !== CORE;
  const name = relative(ROOT, dir);
  const used = usedIn(dir);
  const { src: baseSrc, entries: baseEntries } = baseOf(dir);
  const base = new Set(baseEntries.map((e) => e.key));
  const problems = [];

  const dup = baseEntries.map((e) => e.key).filter((k, i, all) => all.indexOf(k) !== i);
  for (const k of dup) problems.push(`${name}: enUS.lua lists a key twice: ${JSON.stringify(k)}`);
  for (const e of baseEntries) {
    if (e.value !== "true") problems.push(`${name}: enUS.lua: ${JSON.stringify(e.key)} should be = true`);
    if (isMod && coreKeys.has(e.key)) {
      problems.push(`${name}: enUS.lua repeats a key the core's has (leave it to the core's): ${JSON.stringify(e.key)}`);
    }
  }
  const missing = [...used.keys()].filter((k) => !base.has(k) && !(isMod && coreKeys.has(k)));
  const unused = [...base].filter((k) => !used.has(k) && !(!isMod && modUses.has(k)));
  for (const k of missing) problems.push(`${name}: missing from enUS.lua (${used.get(k)}): ${JSON.stringify(k)}`);
  for (const k of unused) problems.push(`${name}: in enUS.lua but unused: ${JSON.stringify(k)}`);

  for (const code of Object.keys(TEMPLATES)) {
    if (writeTemplates) writeTemplate(dir, code, baseSrc, baseEntries);
    let keys;
    try {
      keys = new Set(localeEntries(readFileSync(join(dir, "Locales", `${code}.lua`), "utf8")).map((e) => e.key));
    } catch {
      problems.push(`${name}: ${code}.lua is missing (node cores/wow/addon/tools/strings.mjs --templates writes it)`);
      continue;
    }
    for (const k of base) if (!keys.has(k)) problems.push(`${name}: ${code}.lua lacks ${JSON.stringify(k)}`);
    for (const k of keys) {
      if (!base.has(k)) problems.push(`${name}: ${code}.lua has a key enUS.lua doesn't: ${JSON.stringify(k)}`);
    }
  }
  console.log(`${name}: ${used.size} keys used, ${base.size} in enUS.lua, ${Object.keys(TEMPLATES).length} templates; `
    + `missing ${missing.length}, unused ${unused.length}`);
  return problems;
}

function main() {
  const args = process.argv.slice(2);
  const writeTemplates = args.includes("--templates");
  const all = addonFolders();
  const named = args.filter((a) => !a.startsWith("--")).map((a) => resolve(a));
  for (const d of named) {
    if (!all.includes(d)) {
      console.log(`not an addon folder this checks: ${d} (the core's, or mods/wow/<id>/addon/<Folder>)`);
      process.exit(1);
    }
  }
  const coreKeys = new Set(baseOf(CORE).entries.map((e) => e.key));
  const modUses = new Set();
  for (const d of all.filter((d) => d !== CORE)) for (const k of usedIn(d).keys()) modUses.add(k);
  const problems = [];
  for (const dir of named.length ? named : all) problems.push(...check(dir, { coreKeys, modUses, writeTemplates }));
  if (problems.length) {
    for (const p of problems) console.log(`  ${p}`);
    process.exit(1);
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
