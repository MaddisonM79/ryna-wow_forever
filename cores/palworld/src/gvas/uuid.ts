/**
 * Unreal's FGuid on disk is four little-endian u32s. palworld-save-tools
 * prints it as "%08x-%04x-%04x-%04x-%04x%08x" over a particular byte shuffle,
 * and every id downstream (the API, D1, the portal) is that string, so the
 * port keeps the exact format.
 */

const HEX = Array.from({ length: 256 }, (_, i) => i.toString(16).padStart(2, "0"));

/** The 16 bytes at `at` as the library's guid string. */
export function guidAt(b: Uint8Array, at: number): string {
  const h = (i: number) => HEX[b[at + i]!]!;
  return (
    h(3) + h(2) + h(1) + h(0) + "-" +
    h(7) + h(6) + "-" +
    h(5) + h(4) + "-" +
    h(11) + h(10) + "-" +
    h(9) + h(8) + h(15) + h(14) + h(13) + h(12)
  );
}
