import { defineConfig } from "vitest/config";

export default defineConfig({
  // The saves' local timestamps are read in the machine's zone; the fixtures were written in Chicago.
  // 30 s, not vitest's 5: the parser tests over real saves are slow on a busy machine.
  test: { include: ["test/**/*.test.ts"], env: { TZ: "America/Chicago" }, testTimeout: 30_000 },
});
