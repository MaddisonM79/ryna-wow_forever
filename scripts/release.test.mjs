// node --test scripts/: the release scripts' own tests.
import assert from "node:assert/strict";
import { test } from "node:test";
import { manifest } from "./latest-json.mjs";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { NODE_BUILDS, NODE_VERSION, PACKAGE_FILES, TARGETS, buildPackage, bundleNodeMajor, nodeTarball, packageName, shasumOf } from "./core-package.mjs";
import { agree, escapeRegExp, lockVersion, luaVersions, modVersions, readVersions, tocVersion, workspaceVersion } from "./versions.mjs";

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
  // A name is matched literally, whatever characters it has: no wildcard, class or escape in it.
  assert.equal(lockVersion(lock, "ryn."), null);
  assert.equal(lockVersion(lock, "ryna-wow-.*"), null);
  assert.equal(lockVersion('[[package]]\nname = "a\\\\b+[c]"\nversion = "1.2.3"\n', "a\\\\b+[c]"), "1.2.3");
  assert.equal(escapeRegExp("a.b*c\\d[e]-f/g"), "a\\.b\\*c\\\\d\\[e\\]\\-f\\/g");
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

test("this repository's versions include Chronicle's, the Settings mod's and the Palworld core's", () => {
  const found = readVersions(join(import.meta.dirname, ".."));
  assert.ok("cores/palworld/core.json" in found);
  assert.ok("cores/palworld/package.json" in found);
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

test("a core package is named for its core, the release's version and the target", () => {
  assert.deepEqual(TARGETS, ["aarch64-apple-darwin", "x86_64-apple-darwin"]);
  assert.equal(packageName("palworld", "0.5.0", "aarch64-apple-darwin"), "ryna-core-palworld-0.5.0-aarch64-apple-darwin.tar.gz");
  assert.equal(packageName("palworld", "0.5.0-rc.1", "x86_64-apple-darwin"), "ryna-core-palworld-0.5.0-rc.1-x86_64-apple-darwin.tar.gz");
  assert.throws(() => packageName("palworld", "0.5.0", "x86_64-pc-windows-msvc"), /no package for/);
  assert.throws(() => packageName("Pal World", "0.5.0", "aarch64-apple-darwin"), /bad core id/);
  assert.throws(() => packageName("palworld", "v0.5", "aarch64-apple-darwin"), /bad version/);
  assert.equal(nodeTarball("aarch64-apple-darwin", "26.13.1"), "node-v26.13.1-darwin-arm64.tar.gz");
  assert.equal(nodeTarball("x86_64-apple-darwin", "26.13.1"), "node-v26.13.1-darwin-x64.tar.gz");
});

test("the Node it carries is the major the bundle is built for, pinned by hash", () => {
  const pkg = JSON.parse(readFileSync(join(import.meta.dirname, "..", "cores", "palworld", "package.json"), "utf8"));
  assert.equal(bundleNodeMajor(pkg.scripts.build), Number(NODE_VERSION.split(".")[0]));
  for (const t of TARGETS) assert.match(NODE_BUILDS[t].sha256, /^[0-9a-f]{64}$/);
  const shasums = `${"a".repeat(64)}  node-v1.0.0-darwin-arm64.tar.gz\n${"b".repeat(64)}  node-v1.0.0-darwin-arm64.tar.xz\n`;
  assert.equal(shasumOf(shasums, "node-v1.0.0-darwin-arm64.tar.gz"), "a".repeat(64));
  assert.equal(shasumOf(shasums, "node-v1.0.0-darwin-x64.tar.gz"), null);
});

test("a core package is flat: node, the bundle, the manifest and the licences, regular files only", () => {
  const root = mkdtempSync(join(tmpdir(), "ryna-core-pkg-"));
  writeFileSync(join(root, "LICENSE"), "MIT\n");
  const core = join(root, "cores", "palworld");
  mkdirSync(join(core, "dist"), { recursive: true });
  mkdirSync(join(core, "node_modules", "ooz-wasm"), { recursive: true });
  writeFileSync(join(core, "core.json"), JSON.stringify({ id: "palworld", version: "0.5.0" }));
  writeFileSync(join(core, "package.json"), JSON.stringify({ version: "0.5.0", scripts: { build: `esbuild --target=node${NODE_VERSION.split(".")[0]}` } }));
  writeFileSync(join(core, "NOTICE.md"), "notice\n");
  writeFileSync(join(core, "dist", "agent.mjs"), "console.log(1)\n");
  writeFileSync(join(core, "node_modules", "ooz-wasm", "LICENSE"), "GPL\n");
  // A stand-in for Node's tarball, with the symlinks the real one has (npm, npx): they stay out.
  const prefix = `node-v${NODE_VERSION}-darwin-arm64`;
  const nodeDir = join(root, "nodetar", prefix);
  mkdirSync(join(nodeDir, "bin"), { recursive: true });
  writeFileSync(join(nodeDir, "bin", "node"), "#!/bin/sh\n");
  writeFileSync(join(nodeDir, "LICENSE"), "Node's licence\n");
  symlinkSync("../lib/node_modules/npm/bin/npm-cli.js", join(nodeDir, "bin", "npm"));
  const nodeTar = join(root, "node.tar.gz");
  execFileSync("tar", ["-czf", nodeTar, "-C", join(root, "nodetar"), prefix]);

  const out = buildPackage({ root, core, target: "aarch64-apple-darwin", nodeTar, outDir: join(root, "out") });
  assert.ok(out.endsWith("/ryna-core-palworld-0.5.0-aarch64-apple-darwin.tar.gz"));
  const names = execFileSync("tar", ["-tzf", out], { encoding: "utf8" }).trim().split("\n");
  assert.deepEqual(names, PACKAGE_FILES);
  const listing = execFileSync("tar", ["-tvzf", out], { encoding: "utf8" }).trim().split("\n");
  assert.ok(listing.every((l) => l.startsWith("-")), listing.join("\n"));
  assert.ok(listing[0].startsWith("-rwx"), "node is executable");
  const unpacked = join(root, "unpacked");
  mkdirSync(unpacked);
  execFileSync("tar", ["-xzf", out, "-C", unpacked]);
  assert.equal(readFileSync(join(unpacked, "LICENSE-node"), "utf8"), "Node's licence\n");
  assert.equal(readFileSync(join(unpacked, "LICENSE-ooz-wasm"), "utf8"), "GPL\n");

  // A manifest and package.json that disagree, or a bundle for another Node, make no package.
  writeFileSync(join(core, "package.json"), JSON.stringify({ version: "0.4.9", scripts: { build: "esbuild --target=node26" } }));
  assert.throws(() => buildPackage({ root, core, target: "aarch64-apple-darwin", nodeTar, outDir: join(root, "out") }), /core.json says 0.5.0/);
  writeFileSync(join(core, "package.json"), JSON.stringify({ version: "0.5.0", scripts: { build: "esbuild --target=node18" } }));
  assert.throws(() => buildPackage({ root, core, target: "aarch64-apple-darwin", nodeTar, outDir: join(root, "out") }), /targets node18/);
});

test("a signed core package is not an updater platform", () => {
  const files = ["ryna-core-palworld-0.5.0-aarch64-apple-darwin.tar.gz", "ryna-core-palworld-0.5.0-aarch64-apple-darwin.tar.gz.sig", "Ryna_0.5.0_x64.app.tar.gz", "Ryna_0.5.0_x64.app.tar.gz.sig"];
  const m = manifest({ files, sig: (n) => `sig of ${n}`, version: "0.5.0", tag: "v0.5.0", repo: "o/r", date: "d" });
  assert.deepEqual(Object.keys(m.platforms), ["darwin-x86_64"]);
});
