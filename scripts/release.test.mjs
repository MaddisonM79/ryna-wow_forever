// node --test scripts/: the release scripts' own tests.
import assert from "node:assert/strict";
import { test } from "node:test";
import { manifest } from "./latest-json.mjs";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { agree, lockVersion, luaVersions, modVersions, readVersions, tocVersion, workspaceVersion } from "./versions.mjs";

test("reads each place's version", () => {
  assert.equal(tocVersion("## Interface: 16001\r\n## Version: 0.2.0\r\n## SavedVariables: RynaDB\n"), "0.2.0");
  assert.equal(tocVersion("## Title: Ryna\n"), null);
  assert.equal(
    workspaceVersion('[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.2.0"\nedition = "2021"\n\n[profile.release]\nversion = "9"\n'),
    "0.2.0",
  );
  const lock = '[[package]]\nname = "ryna"\nversion = "0.2.0"\n\n[[package]]\nname = "ryna-wow-app"\nversion = "0.1.0"\n';
  assert.equal(lockVersion(lock, "ryna"), "0.2.0");
  assert.equal(lockVersion(lock, "ryna-wow-app"), "0.1.0");
  assert.equal(lockVersion(lock, "ryna-wow-core"), null);
});

test("reads each bundled mod's manifest and its addon folders' TOCs", () => {
  const root = mkdtempSync(join(tmpdir(), "ryna-versions-"));
  const mod = join(root, "mods", "wow", "chronicle");
  mkdirSync(join(mod, "addon", "Ryna_Chronicle"), { recursive: true });
  writeFileSync(join(mod, "mod.json"), JSON.stringify({ id: "chronicle", version: "0.3.0", addons: ["Ryna_Chronicle", "Ryna_Gone"] }));
  writeFileSync(join(mod, "addon", "Ryna_Chronicle", "Ryna_Chronicle.toc"), "## Version: 0.2.9\n");
  mkdirSync(join(root, "mods", "wow", "not-a-mod"));
  assert.deepEqual(modVersions(root), [
    [join("mods", "wow", "chronicle", "mod.json"), "0.3.0"],
    [join("mods", "wow", "chronicle", "addon", "Ryna_Chronicle", "Ryna_Chronicle.toc"), "0.2.9"],
    [join("mods", "wow", "chronicle", "addon", "Ryna_Gone", "Ryna_Gone.toc"), null],
  ]);
  assert.deepEqual(modVersions(join(root, "nowhere")), []);
});

test("a version written into an addon's Lua is found, and has to agree with the rest", () => {
  const root = mkdtempSync(join(tmpdir(), "ryna-versions-"));
  const core = join(root, "cores", "wow", "addon", "Ryna");
  const mod = join(root, "mods", "wow", "chronicle");
  mkdirSync(core, { recursive: true });
  mkdirSync(join(mod, "addon", "Ryna_Chronicle"), { recursive: true });
  mkdirSync(join(mod, "test"), { recursive: true });
  writeFileSync(join(core, "Core.lua"), 'local addonName, ns = ...\nns.NAME = addonName\nns.VERSION = "0.3.2"\n');
  // Read from the TOC, a table's own number, and a test's stub: none of them is a second copy.
  writeFileSync(join(core, "Mods.lua"), "R.VERSION = ns.VERSION\n");
  writeFileSync(join(mod, "addon", "Ryna_Chronicle", "Mod.lua"), 'ns.VERSION = R.Try(metadata, addonName, "Version")\nP.VERSION = 1\n');
  writeFileSync(join(mod, "test", "x_test.lua"), 'ns.VERSION = "test"\n');
  const where = join("cores", "wow", "addon", "Ryna", "Core.lua");
  assert.deepEqual(luaVersions(root), [[where, "0.3.2"]]);
  assert.deepEqual(luaVersions(join(root, "nowhere")), []);
  const off = agree({ "cores/wow/addon/Ryna/Ryna.toc": "0.3.9", [where]: "0.3.2" }, null);
  assert.equal(off.version, null);
  assert.match(off.problems[0], /disagree[\s\S]*0\.3\.2  .*Core\.lua/);
});

test("no addon in this repository carries its own copy of the version", () => {
  assert.deepEqual(luaVersions(join(import.meta.dirname, "..")), []);
});

test("this repository's versions include Chronicle's and the Settings mod's", () => {
  const found = readVersions(join(import.meta.dirname, ".."));
  assert.ok("mods/wow/chronicle/mod.json" in found);
  assert.ok("mods/wow/chronicle/addon/Ryna_Chronicle/Ryna_Chronicle.toc" in found);
  // An agent mod (no addon folder) has a manifest to agree and no TOC.
  assert.ok("mods/wow/settings/mod.json" in found);
  assert.ok(!Object.keys(found).some((k) => k.startsWith("mods/wow/settings/addon")));
});

test("refuses versions that disagree, or a tag that doesn't match", () => {
  const ok = { a: "0.2.0", b: "0.2.0" };
  assert.deepEqual(agree(ok, "v0.2.0"), { version: "0.2.0", problems: [] });
  assert.deepEqual(agree(ok, null), { version: "0.2.0", problems: [] });
  const off = agree({ a: "0.2.0", "cores/wow/addon/Ryna/Ryna.toc": "0.1.0" }, "v0.2.0");
  assert.equal(off.version, null);
  assert.match(off.problems[0], /disagree[\s\S]*0\.1\.0  cores\/wow\/addon\/Ryna\/Ryna\.toc/);
  assert.match(agree(ok, "v0.3.0").problems[0], /the tag is v0\.3\.0/);
  assert.match(agree({ a: null, b: "0.2.0" }, null).problems[0], /a: no version found/);
});

test("latest.json lists each signed artifact by platform", () => {
  const files = [
    "Ryna_0.2.0_aarch64.app.tar.gz",
    "Ryna_0.2.0_aarch64.app.tar.gz.sig",
    "Ryna_0.2.0_x64.app.tar.gz",
    "Ryna_0.2.0_x64.app.tar.gz.sig",
    "Ryna_0.2.0_x64-setup.exe",
    "Ryna_0.2.0_x64-setup.exe.sig",
    "Ryna_0.2.0_amd64.AppImage",
    "Ryna_0.2.0_amd64.AppImage.sig",
    "Ryna_0.2.0_aarch64.dmg",
    "Ryna-0.2.0.zip",
  ];
  const m = manifest({
    files,
    sig: (n) => `sig of ${n}\n`,
    version: "0.2.0",
    tag: "v0.2.0",
    repo: "MaddisonM79/ryna_agent",
    date: "2026-11-06T00:00:00Z",
  });
  assert.deepEqual(Object.keys(m.platforms).sort(), ["darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64", "windows-x86_64-nsis"]);
  assert.equal(m.version, "0.2.0");
  assert.equal(m.platforms["darwin-aarch64"].signature, "sig of Ryna_0.2.0_aarch64.app.tar.gz");
  assert.equal(
    m.platforms["windows-x86_64"].url,
    "https://github.com/MaddisonM79/ryna_agent/releases/download/v0.2.0/Ryna_0.2.0_x64-setup.exe",
  );
  assert.equal(
    m.platforms["linux-x86_64"].url,
    "https://github.com/MaddisonM79/ryna_agent/releases/download/v0.2.0/Ryna_0.2.0_amd64.AppImage",
  );
  assert.equal(m.platforms["linux-x86_64"].signature, "sig of Ryna_0.2.0_amd64.AppImage");
});

test("an unsigned AppImage (a build without the key) is not a platform", () => {
  const m = manifest({
    files: ["Ryna_0.2.0_amd64.AppImage", "Ryna_0.2.0_x64-setup.exe", "Ryna_0.2.0_x64-setup.exe.sig"],
    sig: (n) => `sig of ${n}`,
    version: "0.2.0",
    tag: "v0.2.0",
    repo: "o/r",
    date: "d",
  });
  assert.deepEqual(Object.keys(m.platforms).sort(), ["windows-x86_64", "windows-x86_64-nsis"]);
});

test("no signatures, no manifest", () => {
  const m = manifest({ files: ["Ryna_0.2.0_x64-setup.exe"], sig: () => "", version: "0.2.0", tag: "v0.2.0", repo: "o/r", date: "d" });
  assert.equal(m, null);
});
