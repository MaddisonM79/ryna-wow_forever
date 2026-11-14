// The release's one version, read from every place that carries it. Exported for the tests;
// check-versions.mjs is the command.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

/** `## Version: x.y.z` from a TOC. */
export function tocVersion(text) {
  const m = /^##\s*Version:\s*(\S+)\s*$/m.exec(text.replace(/^﻿/, ""));
  return m ? m[1] : null;
}

/** `version = "…"` in Cargo.toml's `[workspace.package]` (every crate takes it from there). */
export function workspaceVersion(text) {
  const section = /^\[workspace\.package\]\s*$([\s\S]*?)(?=^\[|$(?![\s\S]))/m.exec(text);
  const m = section && /^version\s*=\s*"([^"]+)"/m.exec(section[1]);
  return m ? m[1] : null;
}

/** `text` matched literally inside a RegExp: every character a pattern treats specially, escaped. */
export function escapeRegExp(text) {
  return text.replace(/[\\^$.*+?()[\]{}|\/-]/g, "\\$&");
}

/** The version Cargo.lock records for a package of ours. */
export function lockVersion(text, name) {
  const re = new RegExp(`^\\[\\[package\\]\\]\\s*\\nname = "${escapeRegExp(name)}"\\s*\\nversion = "([^"]+)"`, "m");
  const m = re.exec(text);
  return m ? m[1] : null;
}

export const CRATES = ["ryna-agent", "ryna", "ryna-wow-core", "ryna-wow-app"];

/** Every place's version, by a name to report it under. */
export function readVersions(root) {
  const read = (p) => readFileSync(join(root, p), "utf8");
  const found = {
    "Cargo.toml [workspace.package]": workspaceVersion(read("Cargo.toml")),
    "agent/app/package.json": JSON.parse(read("agent/app/package.json")).version ?? null,
    "cores/wow/addon/Ryna/Ryna.toc": tocVersion(read("cores/wow/addon/Ryna/Ryna.toc")),
    // A core's manifest version is what its heartbeat reports; bundled cores take the agent's.
    "cores/wow/core.json": JSON.parse(read("cores/wow/core.json")).version ?? null,
    // A fetched core's package is on the same release, named for the same version (13.3), and
    // its bundle stamps its own package.json's version into every snapshot.
    "cores/palworld/core.json": JSON.parse(read("cores/palworld/core.json")).version ?? null,
    "cores/palworld/package.json": JSON.parse(read("cores/palworld/package.json")).version ?? null,
  };
  // Each bundled mod (mods/wow/<id>/mod.json, `kind: "addon"` or `kind: "agent"`) versions with the
  // agent, and so does each addon folder it installs (its TOC; an agent mod has none).
  for (const [where, version] of modVersions(root)) found[where] = version;
  // An addon reads its version from its TOC in game. A copy written into its Lua has to agree too.
  for (const [where, version] of luaVersions(root)) found[where] = version;
  // tauri.conf.json falls back to the crate's version; if it ever names one, it must agree.
  const tauri = JSON.parse(read("agent/app/src-tauri/tauri.conf.json"));
  if (tauri.version !== undefined) found["agent/app/src-tauri/tauri.conf.json"] = tauri.version;
  const lock = read("Cargo.lock");
  for (const c of CRATES) found[`Cargo.lock ${c}`] = lockVersion(lock, c);
  return found;
}

/** Every bundled mod's `mod.json` version and its addon folders' TOC versions, by where they are. */
export function modVersions(root) {
  const out = [];
  const mods = join(root, "mods", "wow");
  if (!existsSync(mods)) return out;
  for (const id of readdirSync(mods).sort()) {
    const manifest = join("mods", "wow", id, "mod.json");
    if (!existsSync(join(root, manifest))) continue;
    const m = JSON.parse(readFileSync(join(root, manifest), "utf8"));
    out.push([manifest, m.version ?? null]);
    for (const folder of m.addons ?? []) {
      const toc = join("mods", "wow", id, "addon", folder, `${folder}.toc`);
      out.push([toc, existsSync(join(root, toc)) ? tocVersion(readFileSync(join(root, toc), "utf8")) : null]);
    }
  }
  return out;
}

/**
 * Every `ns.VERSION = "…"` written into an addon's own Lua (the core's and each mod's addon
 * folders, not their tests), by where it is. None is the healthy answer: the addons read their TOC.
 * One that exists is a second copy, and the core's sat at 0.3.2 while its TOC said 0.3.9.
 */
export function luaVersions(root) {
  const out = [];
  const walk = (dir) => {
    if (!existsSync(join(root, dir))) return;
    for (const e of readdirSync(join(root, dir), { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))) {
      const p = join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith(".lua")) {
        const m = /^[ \t]*ns\.VERSION\s*=\s*["']([^"']*)["']/m.exec(readFileSync(join(root, p), "utf8"));
        if (m) out.push([p, m[1] || null]);
      }
    }
  };
  walk(join("cores", "wow", "addon"));
  const mods = join("mods", "wow");
  if (existsSync(join(root, mods))) for (const id of readdirSync(join(root, mods)).sort()) walk(join(mods, id, "addon"));
  return out;
}

/**
 * The one version they agree on, checked against the tag when there is one.
 * @returns {{ version: string | null, problems: string[] }}
 */
export function agree(found, tag) {
  const problems = [];
  for (const [where, v] of Object.entries(found)) if (!v) problems.push(`${where}: no version found`);
  const values = [...new Set(Object.values(found).filter(Boolean))];
  if (values.length > 1) {
    problems.push(
      `the versions disagree:\n${Object.entries(found)
        .map(([w, v]) => `  ${v ?? "(none)"}  ${w}`)
        .join("\n")}`,
    );
  }
  const version = values.length === 1 ? values[0] : null;
  if (tag && version && tag !== `v${version}`) problems.push(`the tag is ${tag} but every file says ${version} (tag v${version})`);
  if (version && !/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) problems.push(`${version} isn't x.y.z (or x.y.z-pre)`);
  return { version: problems.length ? null : version, problems };
}
