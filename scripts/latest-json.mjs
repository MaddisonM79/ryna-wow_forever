#!/usr/bin/env node
// Writes the updater's manifest (latest.json) from the release's signed updater artifacts:
//
//   node scripts/latest-json.mjs <dir> <version> <tag> <owner/repo>
//
// Each artifact in <dir> with a `.sig` beside it becomes a platform:
//   Ryna_<v>_aarch64.app.tar.gz  darwin-aarch64
//   Ryna_<v>_x64.app.tar.gz      darwin-x86_64
//   Ryna_<v>_x64-setup.exe       windows-x86_64 (and windows-x86_64-nsis)
//   Ryna_<v>_amd64.AppImage      linux-x86_64
// Its URL is the asset on the tag's release, which is what the app downloads. Writes nothing
// (and says so) when there are no signatures, which is a build without the signing key.
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const PLATFORMS = [
  { re: /_aarch64\.app\.tar\.gz$/, keys: ["darwin-aarch64"] },
  { re: /_x64\.app\.tar\.gz$/, keys: ["darwin-x86_64"] },
  { re: /_x64-setup\.exe$/, keys: ["windows-x86_64", "windows-x86_64-nsis"] },
  { re: /_amd64\.AppImage$/, keys: ["linux-x86_64"] },
];

/**
 * @param {{ files: string[], sig: (name: string) => string, version: string, tag: string, repo: string, date: string }} input
 * @returns {object | null}
 */
export function manifest({ files, sig, version, tag, repo, date }) {
  const platforms = {};
  for (const name of files) {
    if (!files.includes(`${name}.sig`)) continue;
    const p = PLATFORMS.find((x) => x.re.test(name));
    if (!p) continue;
    const entry = {
      signature: sig(name).trim(),
      url: `https://github.com/${repo}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`,
    };
    for (const k of p.keys) {
      if (platforms[k]) throw new Error(`two artifacts for ${k}: ${platforms[k].url} and ${entry.url}`);
      platforms[k] = entry;
    }
  }
  if (!Object.keys(platforms).length) return null;
  return {
    version,
    notes: `See https://github.com/${repo}/releases/tag/${tag}`,
    pub_date: date,
    platforms,
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [dir, version, tag, repo] = process.argv.slice(2);
  if (!dir || !version || !tag || !repo) {
    console.error("usage: latest-json.mjs <dir> <version> <tag> <owner/repo>");
    process.exit(2);
  }
  const files = existsSync(dir) ? readdirSync(dir) : [];
  const m = manifest({
    files,
    sig: (n) => readFileSync(join(dir, `${n}.sig`), "utf8"),
    version,
    tag,
    repo,
    date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
  });
  if (!m) {
    console.log("No signed updater artifacts: no latest.json (the release can't update anyone).");
  } else {
    writeFileSync(join(dir, "latest.json"), `${JSON.stringify(m, null, 2)}\n`);
    console.log(`latest.json: ${Object.keys(m.platforms).join(", ")}`);
  }
}
