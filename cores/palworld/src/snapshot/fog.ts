/**
 * The world map's fog of war, from LocalData.sav, as a small grayscale PNG.
 *
 * `WorldMapUISaveDataMap.MainMap.MaskTextureData` is a square RGBA texture
 * (1024 x 1024 today) whose alpha is the fog: 255 is unexplored, anything less
 * is revealed (0 inside the explored area, a soft ramp at its edge). The colour
 * channels are something else (a few small, changing blobs), not exploration.
 * The PNG is white where explored, black where not, downsampled so the portal
 * can overlay it on the map without shipping 4 MB every push.
 */

import { deflateSync } from "node:zlib";
import { dict, isDict, list, pyRound, type Dict } from "./util.js";

const PNG_SIZE = 512;

export type Grid = { side: number; cells: Uint8Array };

export function maskBytes(local: Dict | null): Uint8Array | null {
  for (const e of list(dict(local?.["SaveData"])["WorldMapUISaveDataMap"])) {
    if (isDict(e) && e["key"] === "MainMap") {
      const data = dict(e["value"])["MaskTextureData"];
      return typeof data === "string" ? Buffer.from(data, "base64") : null;
    }
  }
  return null;
}

/** One byte per pixel: 1 explored (alpha below 255), 0 not, from an RGBA mask. */
export function exploredGrid(mask: Uint8Array): Grid {
  const pixels = mask.byteLength >> 2;
  const side = Math.floor(Math.sqrt(pixels));
  if (side * side !== pixels) throw new Error(`fog mask is ${mask.byteLength} bytes, not a square RGBA texture`);
  const cells = new Uint8Array(pixels);
  for (let i = 0; i < pixels; i++) cells[i] = mask[i * 4 + 3]! < 255 ? 1 : 0;
  return { side, cells };
}

export function exploredPct(grid: Grid): number {
  let n = 0;
  for (const c of grid.cells) n += c;
  return pyRound((100 * n) / grid.cells.length, 2);
}

function chunk(kind: string, body: Uint8Array): Buffer {
  const head = Buffer.alloc(4);
  head.writeUInt32BE(body.byteLength);
  const kb = Buffer.from(kind, "latin1");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([kb, body])));
  return Buffer.concat([head, kb, body, crc]);
}

let CRC_TABLE: Uint32Array | null = null;
function crc32(buf: Uint8Array): number {
  if (!CRC_TABLE) {
    CRC_TABLE = new Uint32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      CRC_TABLE[n] = c >>> 0;
    }
  }
  let c = 0xffffffff;
  for (let i = 0; i < buf.byteLength; i++) c = CRC_TABLE[(c ^ buf[i]!) & 0xff]! ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** A size x size 8-bit grayscale PNG: each pixel is the explored share of its block. */
export function png(grid: Grid, size = PNG_SIZE): Buffer {
  const rows = blockRows(grid, size);
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0);
  header.writeUInt32BE(size, 4);
  header[8] = 8; // bit depth
  header[9] = 0; // grayscale
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(Buffer.concat(rows), { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

/** PNG rows (filter byte first), each pixel the explored share of its block, as the Python `block_rows`. */
function blockRows({ side, cells }: Grid, size: number): Buffer[] {
  const step = side / size;
  const rows: Buffer[] = [];
  for (let y = 0; y < size; y++) {
    const row = Buffer.alloc(size + 1);
    row[0] = 0; // filter: none
    const y0 = Math.trunc(y * step);
    const y1 = Math.max(Math.trunc((y + 1) * step), y0 + 1);
    for (let x = 0; x < size; x++) {
      const x0 = Math.trunc(x * step);
      const x1 = Math.max(Math.trunc((x + 1) * step), x0 + 1);
      let n = 0;
      let hit = 0;
      for (let yy = y0; yy < y1; yy++) {
        const base = yy * side;
        for (let xx = x0; xx < x1; xx++) {
          n++;
          hit += cells[base + xx]!;
        }
      }
      row[x + 1] = pyRound((255 * hit) / n);
    }
    rows.push(row);
  }
  return rows;
}
