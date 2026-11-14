/**
 * Captures one example of each custom property (src/rawdata/index.ts) from a real Level.sav into
 * test/fixtures/rawdata.json: the bytes the decoder consumed and what it made of them. The fixture
 * test (test/rawdata-fixtures.test.ts) replays each one, so a change to a decoder, a type hint or
 * the reader shows as a diff against a save that is known to parse.
 *
 *   pnpm exec tsx scripts/capture-rawdata.ts [path/to/Level.sav]      (in cores/palworld)
 *
 * Without a path it takes the newest world under the game's save folder. Rerun it after a game
 * update changes a blob's layout, and commit the result with the decoder change. Whole-array
 * properties bigger than MAX_BYTES (the map objects, the works) are left out: their element
 * decoders have their own byte tests in test/rawdata.test.ts.
 *
 * This repository is public: what it writes is scrubbed first (scripts/scrub.ts). Names become
 * placeholders and every world-made GUID a stand-in, in the bytes and the decoded value alike, and
 * the scrubbed bytes are decoded again to prove they still give the scrubbed value. Read the diff
 * before committing it all the same: a name under a key scrub.ts doesn't know would get through.
 */

import { readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { readGvas } from "../src/gvas/file.js";
import { TYPE_HINTS } from "../src/gvas/hints.js";
import { FArchiveReader, type CustomReader } from "../src/gvas/reader.js";
import { CUSTOM_PROPERTIES } from "../src/rawdata/index.js";
import { decompress } from "../src/sav.js";
import { SAVE_ROOT, worlds } from "../src/worlds.js";
import { b64, plain, type RawFixture } from "../test/rawdata-fixtures.js";
import { guidsIn, namesIn, scrubBytes, scrubNames, scrubText } from "./scrub.js";

const MAX_BYTES = 32 * 1024;
/** How many occurrences of a path to look at before settling on the richest one that fits. */
const LOOK_AT = 200;

const level =
  process.argv[2] ??
  (() => {
    const found = worlds(SAVE_ROOT);
    if (!found.length) throw new Error(`no worlds under ${SAVE_ROOT}`);
    return join(found.reduce((a, b) => (statSync(join(b, "Level.sav")).mtimeMs > statSync(join(a, "Level.sav")).mtimeMs ? b : a)), "Level.sav");
  })();
const savedAt = statSync(level).mtime.toISOString();

const captured: Record<string, RawFixture> = {};
const seen: Record<string, number> = {};
const wrapped: Record<string, CustomReader> = {};
for (const [path, real] of Object.entries(CUSTOM_PROPERTIES)) {
  wrapped[path] = (reader, typeName, size, p) => {
    const start = reader.tell();
    const node = real(reader, typeName, size, p);
    const end = reader.tell();
    seen[path] = (seen[path] ?? 0) + 1;
    if (seen[path] <= LOOK_AT && end - start <= MAX_BYTES) {
      const decoded = plain(node);
      const richness = JSON.stringify(decoded).length;
      const have = captured[path];
      if (!have || richness > JSON.stringify(have.decoded).length) {
        captured[path] = { savedAt, typeName, size, bytes: Buffer.from(reader.data.subarray(start, end)).toString("base64"), decoded };
      }
    }
    return node;
  };
}

readGvas(decompress(readFileSync(level)), TYPE_HINTS, wrapped);

const out = join(import.meta.dirname, "..", "test", "fixtures", "rawdata.json");
const ordered = Object.fromEntries(Object.keys(CUSTOM_PROPERTIES).filter((p) => captured[p]).map((p) => [p, captured[p]!]));

// Scrub: one set of GUIDs and names across every entry, so ids that join still join.
const known = guidsIn(JSON.stringify(ordered));
const names = new Map<string, string>();
for (const f of Object.values(ordered)) namesIn(f.decoded, names);
const scrubB64 = (s: string): string => Buffer.from(scrubBytes(b64(s), known, names)).toString("base64");
const scrubBlobs = (v: unknown): unknown => {
  if (Array.isArray(v)) return v.map(scrubBlobs);
  if (v && typeof v === "object") {
    const o = v as Record<string, unknown>;
    if (typeof o["$bytes"] === "string" && Object.keys(o).length === 1) return { $bytes: scrubB64(o["$bytes"]) };
    return Object.fromEntries(Object.entries(o).map(([k, x]) => [k, scrubBlobs(x)]));
  }
  return v;
};
const scrubbed = JSON.parse(
  scrubText(JSON.stringify(Object.fromEntries(Object.entries(ordered).map(([p, f]) => [p, { ...f, bytes: scrubB64(f.bytes), decoded: scrubBlobs(scrubNames(f.decoded, names)) }]))), known),
) as Record<string, RawFixture>;
// The scrubbed bytes must still decode to the scrubbed value, consuming every byte.
for (const [path, f] of Object.entries(scrubbed)) {
  const reader = new FArchiveReader(b64(f.bytes), TYPE_HINTS, CUSTOM_PROPERTIES);
  const again = JSON.stringify(plain(CUSTOM_PROPERTIES[path]!(reader, f.typeName, f.size, path)));
  if (again !== JSON.stringify(f.decoded) || !reader.eof()) throw new Error(`${path}: the scrubbed bytes no longer decode to the scrubbed value; fix scrub.ts before committing anything`);
}
writeFileSync(out, JSON.stringify(scrubbed, null, 1) + "\n");
console.log(`scrubbed ${names.size} name(s) and ${known.size} GUID(s)`);
for (const path of Object.keys(CUSTOM_PROPERTIES)) {
  const f = captured[path];
  console.log(f ? `${path}: ${Buffer.from(f.bytes, "base64").byteLength} bytes (seen ${seen[path]})` : `${path}: not captured (seen ${seen[path] ?? 0}, all over ${MAX_BYTES} bytes)`);
}
console.log(`wrote ${out} from ${level} (${savedAt})`);
