/** The fixture scrubber: what it changes, what it keeps, and that a scrubbed save still reads. */

import { describe, expect, it } from "vitest";
import { guidAt } from "../src/gvas/uuid.js";
import { decompress } from "../src/sav.js";
import { guidBytes, guidsIn, isShared, key32, namesIn, placeholder, scrubBytes, scrubNames, scrubText, standIn, wrapStored } from "../scripts/scrub.js";

const G = "8c7b148a-43b1-4ab2-493c-3ab1d97826a5";

describe("scrub", () => {
  it("writes a GUID back as the bytes the reader made it from", () => {
    expect(guidAt(guidBytes(key32(G)), 0)).toBe(G);
  });

  it("replaces each GUID in every spelling with one stand-in, and keeps every world's shared ids", () => {
    const k = key32(G);
    const s = standIn(k);
    expect(s).toMatch(/^[0-9a-f]{32}$/);
    expect(s).not.toBe(k);
    const text = `${G} ${G.toUpperCase()} ${k} 8C7B148A-43B14AB2-493C3AB1-D97826A5 00000000-0000-0000-0000-000000000001`;
    const out = scrubText(text, guidsIn(text));
    expect(out).not.toMatch(/8c7b148a/i);
    const [a, b, c, d, e] = out.split(" ");
    expect([key32(a!), key32(b!), c, key32(d!)]).toEqual([s, s, s, s]);
    expect([a!.length, b, d!.split("-").length]).toEqual([36, b!.toUpperCase(), 4]);
    expect(e).toBe("00000000-0000-0000-0000-000000000001");
    expect(isShared(key32(e!))).toBe(true);
  });

  it("replaces names in either FString encoding without changing a length", () => {
    const names = namesIn({ player_name: "Somebody", guild_name: "Unnamed Guild", base: { name: "新規生成拠点テンプレート名3(仮)" }, NickName: "Pal" });
    expect([...names.keys()]).toEqual(["Somebody", "Pal"]);
    expect(placeholder("Somebody")).toHaveLength(8);
    const bytes = Buffer.concat([Buffer.from("Somebody", "latin1"), Buffer.from("Somebody", "utf16le")]);
    const out = Buffer.from(scrubBytes(bytes, new Set(), names));
    expect(out.length).toBe(bytes.length);
    expect(out.includes(Buffer.from("Somebody"))).toBe(false);
    expect(out.includes(Buffer.from("Somebody", "utf16le"))).toBe(false);
    expect(scrubNames({ player_name: "Somebody", other: "Somebody" }, names)).toEqual({ player_name: placeholder("Somebody"), other: "Somebody" });
  });

  it("wraps GVAS as uncompressed Oodle blocks that read back unchanged, across a block boundary", () => {
    const gvas = new Uint8Array(0x40000 + 100);
    gvas.set([0x47, 0x56, 0x41, 0x53]);
    for (let i = 4; i < gvas.length; i++) gvas[i] = i % 251;
    expect(Buffer.compare(Buffer.from(decompress(wrapStored(gvas))), Buffer.from(gvas))).toBe(0);
  });
});
