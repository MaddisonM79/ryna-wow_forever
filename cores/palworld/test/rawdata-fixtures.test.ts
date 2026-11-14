import { describe, expect, it } from "vitest";
import { FArchiveReader } from "../src/gvas/reader.js";
import { TYPE_HINTS } from "../src/gvas/hints.js";
import { CUSTOM_PROPERTIES } from "../src/rawdata/index.js";
import { b64, plain, type RawFixture } from "./rawdata-fixtures.js";
import raw from "./fixtures/rawdata.json" with { type: "json" };

const fixtures = raw as Record<string, RawFixture>;

/**
 * Each custom property decoder replays the bytes a real save gave it (scripts/capture-rawdata.ts)
 * and lands on the same value, consuming exactly those bytes. Before this, 14 of the 16 decoders
 * had no byte-level test (the 2026-11-03 review); a game update that moves a field now fails here
 * instead of in a snapshot nobody reads closely.
 */
describe("RawData decoders against a real save", () => {
  it.each(Object.entries(fixtures))("%s", (path, f) => {
    const decode = CUSTOM_PROPERTIES[path];
    expect(decode, `${path} is no longer a custom property`).toBeDefined();
    const reader = new FArchiveReader(b64(f.bytes), TYPE_HINTS, CUSTOM_PROPERTIES);
    const node = decode!(reader, f.typeName, f.size, path);
    expect(plain(node)).toEqual(f.decoded);
    expect(reader.eof(), "the decoder left bytes unread").toBe(true);
  });

  /** Whole-array properties too big to carry; their element decoders are tested in rawdata.test.ts. */
  const TOO_BIG = new Set([".worldSaveData.MapObjectSaveData", ".worldSaveData.WorkSaveData"]);

  it("covers every decoder, or names the one it can't", () => {
    const missing = Object.keys(CUSTOM_PROPERTIES).filter((p) => !fixtures[p] && !TOO_BIG.has(p));
    expect(missing, "rerun scripts/capture-rawdata.ts, or add the path to TOO_BIG with a reason").toEqual([]);
  });
});
