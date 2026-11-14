/** A .sav's top-level properties as plain values: what the snapshot code reads. */

import { CUSTOM_PROPERTIES } from "./rawdata/index.js";
import { readGvas } from "./gvas/file.js";
import { TYPE_HINTS } from "./gvas/hints.js";
import { unwrap, type Plain } from "./gvas/unwrap.js";
import { decompress } from "./sav.js";

export type SaveTree = { [k: string]: Plain };

export function readSave(data: Uint8Array): SaveTree {
  const gvas = readGvas(decompress(data), TYPE_HINTS, CUSTOM_PROPERTIES);
  return unwrap(gvas.properties) as SaveTree;
}
