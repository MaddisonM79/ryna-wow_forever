import { readFileSync } from "node:fs";
import { deflateSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import { readSave } from "../src/decode.js";
import { SaveFormatError, decompress } from "../src/sav.js";

const FIXTURES = new URL("./fixtures/", import.meta.url);
// Real saves from the Mac build, scrubbed (scripts/scrub.ts): every world-made GUID is a stand-in. The player
// save is rewrapped as uncompressed Oodle blocks ("PlM"), LocalData as zlib ("PlZ"), since nothing here can
// compress Oodle; the GVAS inside each is the real one, byte for byte apart from the GUIDs.
const PLAYER_SAV = readFileSync(new URL("Players/00000000000000000000000000000001.sav", FIXTURES));
const LOCAL_SAV = readFileSync(new URL("LocalData.sav", FIXTURES));

function plz(raw: Uint8Array, double = false): Uint8Array {
  let body = deflateSync(raw);
  if (double) body = deflateSync(body);
  const out = new Uint8Array(12 + body.byteLength);
  const dv = new DataView(out.buffer);
  dv.setUint32(0, raw.byteLength, true);
  dv.setUint32(4, body.byteLength, true);
  out.set([0x50, 0x6c, 0x5a, double ? 0x32 : 0x31], 8);
  out.set(body, 12);
  return out;
}

const gvas = (n: number) => Uint8Array.from([0x47, 0x56, 0x41, 0x53, ...new Array(n).fill(0)]);

describe("decompress", () => {
  it("reads single and double zlib saves", () => {
    const raw = gvas(8);
    expect(Array.from(decompress(plz(raw)))).toEqual(Array.from(raw));
    expect(Array.from(decompress(plz(raw, true)))).toEqual(Array.from(raw));
  });

  it("refuses a truncated or foreign file", () => {
    expect(() => decompress(new Uint8Array([1, 2, 3]))).toThrow(SaveFormatError);
    const foreign = new Uint8Array(16);
    foreign.set([4, 0, 0, 0, 4, 0, 0, 0, 0x58, 0x59, 0x5a, 0x31, 0x47, 0x56, 0x41, 0x53]);
    expect(() => decompress(foreign)).toThrow(/unknown save magic/);
    expect(() => decompress(plz(Uint8Array.from([0x4e, 0x4f, 0x50, 0x45, 0, 0, 0, 0, 0, 0, 0, 0])))).toThrow(/not GVAS/);
  });

  it("refuses a length mismatch from a half-written save", () => {
    const lying = plz(gvas(8));
    new DataView(lying.buffer).setUint32(0, 99, true);
    expect(() => decompress(lying)).toThrow(/header says 99/);
  });

  it("refuses a half-written save, Oodle or zlib, as a bad save rather than a crash", () => {
    for (const share of [0.3, 0.6, 0.9, 0.99]) {
      expect(() => decompress(PLAYER_SAV.subarray(0, Math.floor(PLAYER_SAV.byteLength * share)))).toThrow(SaveFormatError);
      expect(() => decompress(PLAYER_SAV.subarray(0, Math.floor(PLAYER_SAV.byteLength * share)))).toThrow(/truncated/);
    }
    const garbled = plz(gvas(8));
    garbled.fill(0xff, 12);
    expect(() => decompress(garbled)).toThrow(SaveFormatError);
    expect(() => decompress(garbled)).toThrow(/could not decompress/);
    // An Oodle body whose block header ooz refuses.
    const oodle = Uint8Array.from(PLAYER_SAV);
    oodle.fill(0xff, 12, 14);
    expect(() => decompress(oodle)).toThrow(/could not decompress/);
  });

  it("reads an Oodle player save", () => {
    const raw = decompress(PLAYER_SAV);
    expect(raw.byteLength).toBe(27462);
  });
});

describe("readSave", () => {
  it("reads a real player save into plain values", () => {
    const save = readSave(PLAYER_SAV);
    const sd = save["SaveData"] as Record<string, unknown>;
    expect(sd["IndividualId"]).toEqual({ PlayerUId: "00000000-0000-0000-0000-000000000001", InstanceId: "fd5f4107-59f3-3ec7-20a6-9f9842cef9c2" });
    expect(sd["TechnologyPoint"]).toBe(16);
    expect(sd["PlayerPlatform"]).toBe("EPalPlayerPlatform::Mac");
  });

  it("reads the max-friendship map's PalInstanceID keys (an unhinted key misaligns the rest of the file)", () => {
    const sd = readSave(LOCAL_SAV)["SaveData"] as Record<string, unknown>;
    expect(sd["Local_MaxFriendshipPalIds"]).toEqual([
      { key: { PlayerUId: "00000000-0000-0000-0000-000000000001", InstanceId: "bceb24f1-748c-bad9-1b1d-86837701aeb0", DebugName: "" }, value: 10 },
    ]);
    expect(sd["TrackingQuestId"]).toBe("Main_DefeatDesertBoss");
    expect((sd["Local_OtomoLoadoutSaveData"] as { LoadoutPals: unknown[] }[])[0]!.LoadoutPals).toHaveLength(5);
  });
});
