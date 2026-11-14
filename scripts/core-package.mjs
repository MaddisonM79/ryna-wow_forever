#!/usr/bin/env node
// Builds a fetched core's package (docs/ryna-agent-cores-mods.md 13.3): today the Palworld core's,
// `ryna-core-palworld-<version>-<target>.tar.gz`, one per macOS target. Flat: every file at the
// archive's root, no folders, no symlinks.
//
//   node scripts/core-package.mjs [--out DIR] [--target T]... [--cache DIR]
//
// Run `pnpm install && pnpm build` in cores/palworld first (the package takes dist/agent.mjs).
// Without --target it builds both. It downloads Node's official build for each target from
// nodejs.org (cached in --cache, default node_modules/.cache/ryna-node), checks it against the
// hash pinned here and against Node's published SHASUMS256.txt, and takes its `bin/node` as it is:
// still signed by the Node.js project, never re-signed. It signs nothing itself: release.yml signs
// each package with the updater's key (`tauri signer sign`), so a local run makes the same,
// unsigned, package for development.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** The Node the package carries. Its major must match the bundle's esbuild `--target` (cores/palworld/package.json). */
export const NODE_VERSION = "26.13.1";

/** Each target's Node build, and its sha256 as SHASUMS256.txt published it for NODE_VERSION. */
export const NODE_BUILDS = {
  "aarch64-apple-darwin": { platform: "darwin-arm64", sha256: "aec2514ea1ceb2ba52455e9ec65986f6749a06dd759e5db2a72e6889668c9735" },
  "x86_64-apple-darwin": { platform: "darwin-x64", sha256: "82bb96078a24ca50b9c9a38f2379e6be6d87171fcd400fc14c8934b3bce96e2f" },
};

export const TARGETS = Object.keys(NODE_BUILDS);

/** What's in the package, in order: the runtime, the bundle, the manifest, then the licences. */
export const PACKAGE_FILES = ["node", "agent.mjs", "core.json", "LICENSE", "NOTICE.md", "LICENSE-ooz-wasm", "LICENSE-node"];

/** `ryna-core-<id>-<version>-<target>.tar.gz` (13.3: the host builds the URL from its own version and target). */
export function packageName(id, version, target) {
  if (!/^[a-z0-9-]+$/.test(id)) throw new Error(`bad core id ${id}`);
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) throw new Error(`bad version ${version}`);
  if (!TARGETS.includes(target)) throw new Error(`no package for ${target} (only ${TARGETS.join(", ")})`);
  return `ryna-core-${id}-${version}-${target}.tar.gz`;
}

/** The Node tarball's name for a target. */
export const nodeTarball = (target, version = NODE_VERSION) => `node-v${version}-${NODE_BUILDS[target].platform}.tar.gz`;

/** The sha256 SHASUMS256.txt gives a file, or null. */
export function shasumOf(shasums, file) {
  for (const line of shasums.split("\n")) {
    const m = /^([0-9a-f]{64})\s+\*?(\S+)\s*$/.exec(line);
    if (m && m[2] === file) return m[1];
  }
  return null;
}

/** The esbuild `--target=nodeNN` major in a build script. */
export function bundleNodeMajor(buildScript) {
  const m = /--target=node(\d+)/.exec(buildScript);
  return m ? Number(m[1]) : null;
}

const sha256 = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");

/**
 * The package for one target, from a Node tarball already checked. Returns the archive's path.
 * `core` is cores/<id>: its core.json, NOTICE.md, dist/agent.mjs and node_modules/ooz-wasm/LICENSE.
 */
export function buildPackage({ root, core, target, nodeTar, outDir, nodeVersion = NODE_VERSION }) {
  const manifest = JSON.parse(readFileSync(join(core, "core.json"), "utf8"));
  const pkg = JSON.parse(readFileSync(join(core, "package.json"), "utf8"));
  if (manifest.version !== pkg.version) throw new Error(`core.json says ${manifest.version} and package.json ${pkg.version}`);
  const major = bundleNodeMajor(pkg.scripts?.build ?? "");
  if (major !== Number(nodeVersion.split(".")[0])) throw new Error(`the bundle targets node${major} but the package carries Node ${nodeVersion}`);
  const bundle = join(core, "dist", "agent.mjs");
  if (!existsSync(bundle)) throw new Error(`${bundle} is missing: run pnpm install && pnpm build in ${core}`);

  const stage = mkdtempSync(join(tmpdir(), "ryna-core-"));
  try {
    // Take only the binary and the licence out of Node's tarball; npm and npx are symlinks, left behind.
    const prefix = `node-v${nodeVersion}-${NODE_BUILDS[target].platform}`;
    const unpack = join(stage, ".node");
    mkdirSync(unpack);
    execFileSync("tar", ["-xzf", nodeTar, "-C", unpack, `${prefix}/bin/node`, `${prefix}/LICENSE`]);
    copyFileSync(join(unpack, prefix, "bin", "node"), join(stage, "node"));
    chmodSync(join(stage, "node"), 0o755);
    copyFileSync(join(unpack, prefix, "LICENSE"), join(stage, "LICENSE-node"));
    rmSync(unpack, { recursive: true });
    copyFileSync(bundle, join(stage, "agent.mjs"));
    copyFileSync(join(core, "core.json"), join(stage, "core.json"));
    copyFileSync(join(root, "LICENSE"), join(stage, "LICENSE"));
    copyFileSync(join(core, "NOTICE.md"), join(stage, "NOTICE.md"));
    copyFileSync(join(core, "node_modules", "ooz-wasm", "LICENSE"), join(stage, "LICENSE-ooz-wasm"));

    mkdirSync(outDir, { recursive: true });
    const out = resolve(outDir, packageName(manifest.id, manifest.version, target));
    // Named one by one: no "./" entry, no folder, and nothing else that happened to be there.
    // COPYFILE_DISABLE keeps macOS's tar from adding ._ files.
    execFileSync("tar", ["-czf", out, ...ownerFlags(), "-C", stage, ...PACKAGE_FILES], { env: { ...process.env, COPYFILE_DISABLE: "1" } });
    checkArchive(out);
    return out;
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }
}

/** Owner and group 0 with no names, so an archive built on someone's Mac doesn't carry their user name. */
function ownerFlags() {
  const bsd = /bsdtar/.test(execFileSync("tar", ["--version"], { encoding: "utf8" }));
  return bsd ? ["--uid", "0", "--gid", "0", "--uname", "", "--gname", ""] : ["--owner=0", "--group=0", "--numeric-owner"];
}

/** The archive holds exactly PACKAGE_FILES, at its root, every one a regular file. */
export function checkArchive(path) {
  const listing = execFileSync("tar", ["-tvzf", path], { encoding: "utf8" }).trim().split("\n");
  const names = execFileSync("tar", ["-tzf", path], { encoding: "utf8" }).trim().split("\n");
  if (JSON.stringify(names) !== JSON.stringify(PACKAGE_FILES)) throw new Error(`${path} holds ${names.join(", ")}, not ${PACKAGE_FILES.join(", ")}`);
  for (const line of listing) if (!line.startsWith("-")) throw new Error(`${path}: not a regular file: ${line}`);
}

async function fetchTo(url, path) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url} answered ${res.status}`);
  writeFileSync(path, Buffer.from(await res.arrayBuffer()));
}

/** Node's tarball for a target, downloaded once and checked against the pinned hash and Node's SHASUMS256.txt. */
export async function nodeFor(target, cache) {
  const file = nodeTarball(target);
  const base = `https://nodejs.org/dist/v${NODE_VERSION}`;
  mkdirSync(cache, { recursive: true });
  const shasumsPath = join(cache, `SHASUMS256-v${NODE_VERSION}.txt`);
  if (!existsSync(shasumsPath)) await fetchTo(`${base}/SHASUMS256.txt`, shasumsPath);
  const published = shasumOf(readFileSync(shasumsPath, "utf8"), file);
  const pinned = NODE_BUILDS[target].sha256;
  if (published !== pinned) throw new Error(`SHASUMS256.txt gives ${file} as ${published}, but this script pins ${pinned}`);
  const path = join(cache, file);
  if (!existsSync(path) || sha256(path) !== pinned) await fetchTo(`${base}/${file}`, path);
  const got = sha256(path);
  if (got !== pinned) {
    rmSync(path, { force: true });
    throw new Error(`${file} is ${got}, not the published ${pinned}`);
  }
  return path;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const args = process.argv.slice(2);
  const values = (flag) => args.flatMap((a, i) => (a === flag && args[i + 1] ? [args[i + 1]] : []));
  const outDir = values("--out")[0] ?? join(root, "dist-core");
  const cache = values("--cache")[0] ?? join(root, "node_modules", ".cache", "ryna-node");
  const targets = values("--target").length ? values("--target") : TARGETS;
  const core = join(root, "cores", "palworld");
  try {
    for (const target of targets) {
      if (!TARGETS.includes(target)) throw new Error(`no package for ${target} (only ${TARGETS.join(", ")})`);
      const nodeTar = await nodeFor(target, cache);
      const out = buildPackage({ root, core, target, nodeTar, outDir });
      console.log(`${out}  ${sha256(out)}`);
    }
  } catch (e) {
    console.error(`::error::${e instanceof Error ? e.message : String(e)}`);
    process.exit(1);
  }
}
