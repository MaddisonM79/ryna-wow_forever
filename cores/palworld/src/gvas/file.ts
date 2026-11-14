/** The GVAS container: a header, the properties, and a 4-byte trailer. */

import { FArchiveReader, type CustomReader, type Node } from "./reader.js";

type GvasHeader = {
  magic: number;
  saveGameVersion: number;
  packageFileVersionUe4: number;
  packageFileVersionUe5: number;
  engineVersionMajor: number;
  engineVersionMinor: number;
  engineVersionPatch: number;
  engineVersionChangelist: number;
  engineVersionBranch: string;
  customVersionFormat: number;
  customVersions: [string, number][];
  saveGameClassName: string;
};

function readHeader(r: FArchiveReader): GvasHeader {
  const magic = r.i32();
  if (magic !== 0x53415647) throw new Error("invalid magic");
  const saveGameVersion = r.i32();
  if (saveGameVersion !== 3) throw new Error(`expected save game version 3, got ${saveGameVersion}`);
  const packageFileVersionUe4 = r.i32();
  const packageFileVersionUe5 = r.i32();
  const engineVersionMajor = r.u16();
  const engineVersionMinor = r.u16();
  const engineVersionPatch = r.u16();
  const engineVersionChangelist = r.u32();
  const engineVersionBranch = r.fstring();
  const customVersionFormat = r.i32();
  if (customVersionFormat !== 3) throw new Error(`expected custom version format 3, got ${customVersionFormat}`);
  const customVersions = r.tarray((x): [string, number] => [x.guid(), x.i32()]);
  const saveGameClassName = r.fstring();
  return {
    magic, saveGameVersion, packageFileVersionUe4, packageFileVersionUe5, engineVersionMajor, engineVersionMinor,
    engineVersionPatch, engineVersionChangelist, engineVersionBranch, customVersionFormat, customVersions, saveGameClassName,
  };
}

export type GvasFile = { header: GvasHeader; properties: Node; trailer: Uint8Array };

/** The whole file as the typed tree. `trailer` is 4 zero bytes when everything parsed. */
export function readGvas(data: Uint8Array, typeHints: Record<string, string> = {}, customProperties: Record<string, CustomReader> = {}): GvasFile {
  const r = new FArchiveReader(data, typeHints, customProperties);
  const header = readHeader(r);
  const properties = r.propertiesUntilEnd();
  const trailer = r.readToEnd();
  return { header, properties, trailer };
}
