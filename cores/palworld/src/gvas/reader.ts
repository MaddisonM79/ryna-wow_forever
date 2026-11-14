/**
 * A port of palworld-save-tools' FArchiveReader (oMaN-Rod's fork, the one the
 * Python agent pins): the Unreal property stream, read into the same typed
 * tree the Python library builds ({type, value, id, ...} per property), so the
 * agent's unwrap and snapshot code port line for line and the Python output is
 * the oracle for parity.
 *
 * Read-only: the writer half of the library isn't needed to push snapshots.
 */

import { guidAt } from "./uuid.js";

/** A property node as the library dumps it. Loose on purpose: the shape is the Python dict's. */
export type Node = Record<string, unknown>;

/**
 * Decodes one custom property (a Palworld `RawData` blob or a whole section) in
 * place of the generic reader. Called with the property's type, size and path,
 * exactly as the Python `decode(reader, type_name, size, path)` functions are.
 */
export type CustomReader = (reader: FArchiveReader, typeName: string, size: number, path: string) => Node;

const ascii = new TextDecoder("ascii");
const MAX_SAFE = BigInt(Number.MAX_SAFE_INTEGER);
const safe = (v: bigint): number | bigint => (v <= MAX_SAFE && v >= -MAX_SAFE ? Number(v) : v);
const utf16 = new TextDecoder("utf-16le");

export class FArchiveReader {
  private pos = 0;
  private readonly dv: DataView;
  readonly size: number;

  constructor(
    readonly data: Uint8Array,
    readonly typeHints: Record<string, string> = {},
    readonly customProperties: Record<string, CustomReader> = {},
  ) {
    this.dv = new DataView(data.buffer, data.byteOffset, data.byteLength);
    this.size = data.byteLength;
  }

  /** A reader over another byte string with the same hints and decoders (a RawData blob). */
  internalCopy(data: Uint8Array): FArchiveReader {
    return new FArchiveReader(data, this.typeHints, this.customProperties);
  }

  getTypeOr(path: string, fallback: string): string {
    return this.typeHints[path] ?? fallback;
  }

  tell(): number {
    return this.pos;
  }

  seek(pos: number): void {
    this.pos = pos;
  }

  eof(): boolean {
    return this.pos >= this.size;
  }

  read(n: number): Uint8Array {
    const out = this.data.subarray(this.pos, this.pos + n);
    this.pos += n;
    return out;
  }

  readToEnd(): Uint8Array {
    return this.read(this.size - this.pos);
  }

  bool(): boolean {
    return this.byte() > 0;
  }

  /** An FString: i32 length (negative for UTF-16), the bytes, a terminator. */
  fstring(): string {
    const size = this.i32();
    if (size === 0) return "";
    if (size < 0) {
      const n = -size;
      const bytes = this.read(n * 2).subarray(0, n * 2 - 2);
      return utf16.decode(bytes);
    }
    const bytes = this.read(size).subarray(0, size - 1);
    // The library decodes strictly as ASCII, and a byte above 0x7f is how a
    // misaligned read first shows itself (an unhinted map key), so keep that check.
    for (let i = 0; i < bytes.length; i++) {
      if (bytes[i]! > 0x7f) {
        throw new Error(`Error decoding ascii string of length ${size}: ${Buffer.from(bytes.subarray(0, 64)).toString("latin1")}`);
      }
    }
    return ascii.decode(bytes);
  }

  i16(): number {
    const v = this.dv.getInt16(this.pos, true);
    this.pos += 2;
    return v;
  }

  u16(): number {
    const v = this.dv.getUint16(this.pos, true);
    this.pos += 2;
    return v;
  }

  i32(): number {
    const v = this.dv.getInt32(this.pos, true);
    this.pos += 4;
    return v;
  }

  u32(): number {
    const v = this.dv.getUint32(this.pos, true);
    this.pos += 4;
    return v;
  }

  /**
   * A Number when it fits one exactly, else a bigint: counts and the game clocks
   * are small, but a DateTime's ticks (100 ns since year 1) pass 2^53, and the
   * Python agent prints their microseconds exactly.
   */
  i64(): number | bigint {
    const v = this.dv.getBigInt64(this.pos, true);
    this.pos += 8;
    return safe(v);
  }

  u64(): number | bigint {
    const v = this.dv.getBigUint64(this.pos, true);
    this.pos += 8;
    return safe(v);
  }

  float(): number {
    const v = this.dv.getFloat32(this.pos, true);
    this.pos += 4;
    return v;
  }

  double(): number {
    const v = this.dv.getFloat64(this.pos, true);
    this.pos += 8;
    return v;
  }

  byte(): number {
    return this.data[this.pos++]!;
  }

  byteList(n: number): Uint8Array {
    return this.read(n);
  }

  skip(n: number): void {
    this.pos += n;
  }

  guid(): string {
    const s = guidAt(this.data, this.pos);
    this.pos += 16;
    return s;
  }

  optionalGuid(): string | null {
    return this.data[this.pos++] ? this.guid() : null;
  }

  tarray<T>(item: (r: FArchiveReader) => T): T[] {
    const count = this.u32();
    const out: T[] = [];
    for (let i = 0; i < count; i++) out.push(item(this));
    return out;
  }

  /** Named properties up to the "None" terminator. */
  propertiesUntilEnd(path = ""): Node {
    const properties: Node = {};
    for (;;) {
      const name = this.fstring();
      if (name === "None") break;
      const typeName = this.fstring();
      const size = Number(this.u64());
      properties[name] = this.property(typeName, size, `${path}.${name}`);
    }
    return properties;
  }

  /**
   * One property's node. A custom decoder registered for `path` replaces the
   * generic read, unless this is the decoder itself re-entering for the
   * underlying bytes (`nestedCallerPath === path`).
   */
  property(typeName: string, size: number, path: string, nestedCallerPath = ""): Node {
    let value: Node;
    const custom = this.customProperties[path];
    if (custom && (path !== nestedCallerPath || nestedCallerPath === "")) {
      value = custom(this, typeName, size, path);
      value["custom_type"] = path;
    } else {
      const handler = DISPATCH[typeName];
      if (!handler) throw new Error(`Unknown type: ${typeName} (${path})`);
      value = handler(this, size, path);
    }
    value["type"] = typeName;
    return value;
  }

  /** A map key or value, which carries no header of its own. */
  propValue(typeName: string, structTypeName: string | null, path: string): unknown {
    switch (typeName) {
      case "StructProperty":
        return this.structValue(structTypeName!, path);
      case "EnumProperty":
      case "NameProperty":
      case "StrProperty":
        return this.fstring();
      case "IntProperty":
        return this.i32();
      case "Int64Property":
        return this.i64();
      case "BoolProperty":
        return this.bool();
      case "UInt32Property":
        return this.u32();
      default:
        throw new Error(`Unknown property value type: ${typeName} (${path})`);
    }
  }

  struct(path: string): Node {
    const structType = this.fstring();
    const structId = this.guid();
    const id = this.optionalGuid();
    const value = this.structValue(structType, path);
    return { struct_type: structType, struct_id: structId, id, value };
  }

  structValue(structType: string, path = ""): unknown {
    switch (structType) {
      case "Vector":
        return this.vectorDict();
      case "DateTime":
        return this.u64();
      case "Guid":
        return this.guid();
      case "Quat":
        return this.quatDict();
      case "LinearColor":
        return { r: this.float(), g: this.float(), b: this.float(), a: this.float() };
      case "Color":
        return { b: this.byte(), g: this.byte(), r: this.byte(), a: this.byte() };
      default:
        return this.propertiesUntilEnd(path);
    }
  }

  arrayProperty(arrayType: string, size: number, path: string): Node {
    const count = this.u32();
    if (arrayType === "StructProperty") {
      const propName = this.fstring();
      const propType = this.fstring();
      this.u64();
      const typeName = this.fstring();
      const id = this.guid();
      this.skip(1);
      const values: unknown[] = [];
      for (let i = 0; i < count; i++) values.push(this.structValue(typeName, `${path}.${propName}`));
      return { prop_name: propName, prop_type: propType, values, type_name: typeName, id };
    }
    return { values: this.arrayValue(arrayType, count, size, path) };
  }

  arrayValue(arrayType: string, count: number, size: number, path: string): unknown {
    let item: () => unknown;
    switch (arrayType) {
      case "EnumProperty":
      case "NameProperty":
        item = () => this.fstring();
        break;
      case "Guid":
        item = () => this.guid();
        break;
      case "ByteProperty":
        if (size === count) return this.byteList(count);
        throw new Error("Labelled ByteProperty not implemented");
      default:
        throw new Error(`Unknown array type: ${arrayType} (${path})`);
    }
    const values: unknown[] = [];
    for (let i = 0; i < count; i++) values.push(item());
    return values;
  }

  compressedShortRotator(): [number, number, number] {
    const pitch = this.bool() ? this.u16() : 0;
    const yaw = this.bool() ? this.u16() : 0;
    const roll = this.bool() ? this.u16() : 0;
    return [pitch * (360 / 65536), yaw * (360 / 65536), roll * (360 / 65536)];
  }

  serializeInt(componentBitCount: number): bigint {
    const bytes = Uint8Array.from(this.read((componentBitCount + 7) >> 3));
    if (componentBitCount % 8 !== 0) bytes[bytes.length - 1]! &= (1 << componentBitCount % 8) - 1;
    let v = 0n;
    for (let i = bytes.length - 1; i >= 0; i--) v = (v << 8n) | BigInt(bytes[i]!);
    return v;
  }

  packedVector(scaleFactor: number): [number, number, number] {
    const info = this.u32();
    const bits = info & 63;
    const extra = info >>> 6;
    if (bits > 0) {
      const sign = 1n << BigInt(bits - 1);
      const unsign = (x: bigint) => Number((x & (sign - 1n)) - (x & sign));
      const x = unsign(this.serializeInt(bits));
      const y = unsign(this.serializeInt(bits));
      const z = unsign(this.serializeInt(bits));
      return extra ? [x / scaleFactor, y / scaleFactor, z / scaleFactor] : [x, y, z];
    }
    return extra ? this.vector() : [this.float(), this.float(), this.float()];
  }

  vector(): [number, number, number] {
    return [this.double(), this.double(), this.double()];
  }

  vectorDict(): { x: number; y: number; z: number } {
    return { x: this.double(), y: this.double(), z: this.double() };
  }

  quat(): [number, number, number, number] {
    return [this.double(), this.double(), this.double(), this.double()];
  }

  quatDict(): { x: number; y: number; z: number; w: number } {
    return { x: this.double(), y: this.double(), z: this.double(), w: this.double() };
  }

  ftransform(): { rotation: unknown; translation: unknown; scale3d: unknown } {
    return { rotation: this.quatDict(), translation: this.vectorDict(), scale3d: this.vectorDict() };
  }
}

type Handler = (r: FArchiveReader, size: number, path: string) => Node;

const withId = (read: (r: FArchiveReader) => unknown): Handler => (r) => {
  const id = r.optionalGuid();
  return { id, value: read(r) };
};

/** The generic property readers, by Unreal type name. */
const DISPATCH: Record<string, Handler> = {
  StructProperty: (r, _size, path) => r.struct(path),
  IntProperty: withId((r) => r.i32()),
  UInt16Property: withId((r) => r.u16()),
  UInt32Property: withId((r) => r.u32()),
  UInt64Property: withId((r) => r.u64()),
  Int64Property: withId((r) => r.i64()),
  FixedPoint64Property: withId((r) => r.i32()),
  FloatProperty: withId((r) => r.float()),
  StrProperty: withId((r) => r.fstring()),
  NameProperty: withId((r) => r.fstring()),
  EnumProperty: (r) => {
    const type = r.fstring();
    const id = r.optionalGuid();
    return { id, value: { type, value: r.fstring() } };
  },
  BoolProperty: (r) => {
    const value = r.bool();
    return { value, id: r.optionalGuid() };
  },
  ByteProperty: (r) => {
    const type = r.fstring();
    const id = r.optionalGuid();
    return { id, value: { type, value: type === "None" ? r.byte() : r.fstring() } };
  },
  ArrayProperty: (r, size, path) => {
    const arrayType = r.fstring();
    const id = r.optionalGuid();
    return { array_type: arrayType, id, value: r.arrayProperty(arrayType, size - 4, path) };
  },
  MapProperty: (r, _size, path) => {
    const keyType = r.fstring();
    const valueType = r.fstring();
    const id = r.optionalGuid();
    r.u32();
    const count = r.u32();
    const keyPath = `${path}.Key`;
    const valuePath = `${path}.Value`;
    const keyStructType = keyType === "StructProperty" ? r.getTypeOr(keyPath, "Guid") : null;
    const valueStructType = valueType === "StructProperty" ? r.getTypeOr(valuePath, "StructProperty") : null;
    const values: Node[] = [];
    for (let i = 0; i < count; i++) {
      const key = r.propValue(keyType, keyStructType, keyPath);
      const value = r.propValue(valueType, valueStructType, valuePath);
      values.push({ key, value });
    }
    return { key_type: keyType, value_type: valueType, key_struct_type: keyStructType, value_struct_type: valueStructType, id, value: values };
  },
  SetProperty: (r, _size, path) => {
    const setType = r.fstring();
    const id = r.optionalGuid();
    r.u32();
    const count = r.u32();
    let structType: string | null = null;
    let value: unknown[];
    if (setType === "StructProperty") {
      structType = r.getTypeOr(`${path}.StructProperty`, "StructProperty");
      value = [];
      for (let i = 0; i < count; i++) value.push(r.structValue(structType, `${path}.StructProperty`));
    } else {
      value = [];
      for (let i = 0; i < count; i++) value.push(r.propertiesUntilEnd());
    }
    return { set_type: setType, id, struct_type: structType, value };
  },
};
