/**
 * Turn a Palworld .sav file into its GVAS bytes.
 *
 * A .sav is a 12-byte header and a compressed GVAS (Unreal property) blob:
 *
 *     u32 uncompressed length, u32 compressed length, "PlM" or "PlZ", u8 type
 *
 * `PlM` is Oodle (current builds, including the Mac App Store one), decoded by
 * ooz-wasm. `PlZ` is zlib (older saves), where type 0x32 means it was
 * compressed twice.
 */

import { inflateSync } from "node:zlib";
import { decompress as oodle } from "ooz-wasm";

const HEADER = 12;

export class SaveFormatError extends Error {}

export function decompress(data: Uint8Array): Uint8Array {
  if (data.byteLength < HEADER) throw new SaveFormatError("file is shorter than a save header");
  const dv = new DataView(data.buffer, data.byteOffset, data.byteLength);
  const ulen = dv.getUint32(0, true);
  const clen = dv.getUint32(4, true);
  const magic = String.fromCharCode(data[8]!, data[9]!, data[10]!);
  const kind = data[11]!;
  const body = data.subarray(HEADER);
  if (magic !== "PlM" && magic !== "PlZ") throw new SaveFormatError(`unknown save magic ${JSON.stringify(magic)}`);
  // A half-written file (the game was mid-save when we read it) is usually shorter than its header says.
  if (body.byteLength < clen) throw new SaveFormatError(`truncated: ${body.byteLength} of ${clen} compressed bytes`);
  let raw: Uint8Array;
  try {
    if (magic === "PlM") {
      // ooz hands back a view into WASM memory, valid until the next call: copy it out.
      raw = Uint8Array.from(oodle(body.subarray(0, clen), ulen));
    } else {
      raw = inflateSync(body);
      if (kind === 0x32) raw = inflateSync(raw);
    }
  } catch (e) {
    // Oodle and zlib throw plain errors on a damaged body; it's still a bad save, not a crash.
    throw new SaveFormatError(`could not decompress: ${e instanceof Error ? e.message : String(e)}`, { cause: e });
  }
  if (raw.byteLength !== ulen) {
    // A body that decompresses but to the wrong size (a damaged or half-written one) lands here.
    throw new SaveFormatError(`decompressed ${raw.byteLength} bytes, header says ${ulen}`);
  }
  if (String.fromCharCode(raw[0]!, raw[1]!, raw[2]!, raw[3]!) !== "GVAS") throw new SaveFormatError("decompressed data is not GVAS");
  return raw;
}
