// Writes app-icon.png (1024x1024), the placeholder the app's icons are made from, with no
// dependencies: the PNG is encoded by hand with Node's zlib. `pnpm icon` runs this and then
// `tauri icon app-icon.png`, which writes src-tauri/icons/ (commit those; CI doesn't make them;
// delete the android/, ios/ and Square*/StoreLogo files it also writes, which a desktop build never reads).
// Replace app-icon.png with real artwork and run `pnpm tauri icon app-icon.png` the same way.
import { writeFileSync } from "node:fs";
import { deflateSync } from "node:zlib";

const SIZE = 1024;
const BG = [75, 91, 215]; // the app's accent
const FG = [255, 255, 255];

function crc32(buf) {
  let c = ~0;
  for (const b of buf) {
    c ^= b;
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return ~c >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

/** Coverage of pixel (x, y) by a rounded square, a ring and a dot, with 4x4 supersampling. */
function shade(x, y) {
  let bg = 0;
  let fg = 0;
  const c = SIZE / 2;
  const r = SIZE * 0.2; // corner radius
  const inset = SIZE * 0.06;
  for (let i = 0; i < 4; i++) {
    for (let j = 0; j < 4; j++) {
      const px = x + (i + 0.5) / 4;
      const py = y + (j + 0.5) / 4;
      const qx = Math.max(Math.abs(px - c) - (c - inset - r), 0);
      const qy = Math.max(Math.abs(py - c) - (c - inset - r), 0);
      if (Math.hypot(qx, qy) > r) continue;
      bg++;
      const d = Math.hypot(px - c, py - c);
      if ((d > SIZE * 0.24 && d < SIZE * 0.31) || d < SIZE * 0.09) fg++;
    }
  }
  return [bg / 16, fg / 16];
}

const raw = Buffer.alloc((SIZE * 4 + 1) * SIZE);
let o = 0;
for (let y = 0; y < SIZE; y++) {
  raw[o++] = 0; // filter: none
  for (let x = 0; x < SIZE; x++) {
    const [a, f] = shade(x, y);
    const mix = (k) => Math.round(BG[k] * (1 - f / Math.max(a, 1e-9)) + FG[k] * (f / Math.max(a, 1e-9)));
    raw[o++] = a ? mix(0) : 0;
    raw[o++] = a ? mix(1) : 0;
    raw[o++] = a ? mix(2) : 0;
    raw[o++] = Math.round(a * 255);
  }
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);
writeFileSync(new URL("../app-icon.png", import.meta.url), png);
console.log(`app-icon.png: ${SIZE}x${SIZE}, ${png.length} bytes`);
