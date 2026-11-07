// node --test scripts/: the release scripts' own tests.
import assert from "node:assert/strict";
import { test } from "node:test";
import { manifest } from "./latest-json.mjs";
import { agree, lockVersion, tocVersion, workspaceVersion } from "./versions.mjs";

test("reads each place's version", () => {
  assert.equal(tocVersion("## Interface: 16001\r\n## Version: 0.2.0\r\n## SavedVariables: RynaDB\n"), "0.2.0");
  assert.equal(tocVersion("## Title: Ryna\n"), null);
  assert.equal(
    workspaceVersion('[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.2.0"\nedition = "2021"\n\n[profile.release]\nversion = "9"\n'),
    "0.2.0",
  );
  const lock = '[[package]]\nname = "ryna-wow"\nversion = "0.2.0"\n\n[[package]]\nname = "ryna-wow-app"\nversion = "0.1.0"\n';
  assert.equal(lockVersion(lock, "ryna-wow"), "0.2.0");
  assert.equal(lockVersion(lock, "ryna-wow-app"), "0.1.0");
  assert.equal(lockVersion(lock, "ryna-wow-core"), null);
});

test("refuses versions that disagree, or a tag that doesn't match", () => {
  const ok = { a: "0.2.0", b: "0.2.0" };
  assert.deepEqual(agree(ok, "v0.2.0"), { version: "0.2.0", problems: [] });
  assert.deepEqual(agree(ok, null), { version: "0.2.0", problems: [] });
  const off = agree({ a: "0.2.0", "addon/Ryna/Ryna.toc": "0.1.0" }, "v0.2.0");
  assert.equal(off.version, null);
  assert.match(off.problems[0], /disagree[\s\S]*0\.1\.0  addon\/Ryna\/Ryna\.toc/);
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
  assert.deepEqual(Object.keys(m.platforms).sort(), ["darwin-aarch64", "darwin-x86_64", "windows-x86_64", "windows-x86_64-nsis"]);
  assert.equal(m.version, "0.2.0");
  assert.equal(m.platforms["darwin-aarch64"].signature, "sig of Ryna_0.2.0_aarch64.app.tar.gz");
  assert.equal(
    m.platforms["windows-x86_64"].url,
    "https://github.com/MaddisonM79/ryna_agent/releases/download/v0.2.0/Ryna_0.2.0_x64-setup.exe",
  );
});

test("no signatures, no manifest", () => {
  const m = manifest({ files: ["Ryna_0.2.0_x64-setup.exe"], sig: () => "", version: "0.2.0", tag: "v0.2.0", repo: "o/r", date: "d" });
  assert.equal(m, null);
});
