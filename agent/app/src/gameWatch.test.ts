import { afterEach, beforeEach, describe as group, expect, it, vi } from "vitest";
import { effectScope, nextTick, ref } from "vue";

// The app's game check, answered by the test.
const running = { value: true, fail: false, calls: 0 };
vi.mock("./api", () => ({
  api: {
    settingsGameRunning: async () => {
      running.calls++;
      if (running.fail) throw new Error("no answer");
      return { running: running.value, unknown: null };
    },
  },
}));

import { RUNNING_CHANGE, RUNNING_INSTALL, gameClosedLines, waitsOnGame } from "./describe";
import { watchGameClosing, type Results } from "./gameWatch";

const INSTALLED = "Installed the Prices mod 0.3.8 into /wow/_forever_/Interface/AddOns/Ryna_Prices";

group("the game-running note", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    running.value = true;
    running.fail = false;
    running.calls = 0;
  });
  afterEach(() => vi.useRealTimers());

  it("rewrites the lines once the game has closed", () => {
    expect(waitsOnGame([INSTALLED, RUNNING_CHANGE])).toBe(true);
    expect(waitsOnGame([INSTALLED])).toBe(false);
    expect(gameClosedLines([INSTALLED, RUNNING_CHANGE])).toEqual([
      INSTALLED,
      "The game picks it up at the next login or /reload.",
    ]);
    expect(gameClosedLines(["Installed 0.3.8. The game loads it at the next login or /reload.", RUNNING_INSTALL])).toEqual([
      "Installed 0.3.8. The game loads it at the next login or /reload.",
    ]);
  });

  it("asks while a note says the game runs, and drops the note when it has closed", async () => {
    const scope = effectScope();
    const results = ref<Results>({});
    scope.run(() => watchGameClosing(results, 1000));
    await vi.advanceTimersByTimeAsync(3000);
    expect(running.calls).toBe(0); // nothing shown: nothing asked

    results.value.prices = { ok: true, lines: [INSTALLED, RUNNING_CHANGE] };
    await nextTick();
    await vi.advanceTimersByTimeAsync(2000);
    expect(running.calls).toBe(2);
    expect(results.value.prices.lines).toContain(RUNNING_CHANGE); // still running

    running.fail = true;
    await vi.advanceTimersByTimeAsync(1000);
    expect(results.value.prices.lines).toContain(RUNNING_CHANGE); // no answer: unchanged

    running.fail = false;
    running.value = false;
    await vi.advanceTimersByTimeAsync(1000);
    expect(results.value.prices.lines).toEqual([INSTALLED, "The game picks it up at the next login or /reload."]);
    const asked = running.calls;
    await vi.advanceTimersByTimeAsync(5000);
    expect(running.calls).toBe(asked); // done: no more asking
    scope.stop();
  });

  it("stops asking when the tab goes away", async () => {
    const scope = effectScope();
    const results = ref<Results>({ a: { ok: true, lines: [RUNNING_INSTALL] } });
    scope.run(() => watchGameClosing(results, 1000));
    await vi.advanceTimersByTimeAsync(1000);
    expect(running.calls).toBe(1);
    scope.stop();
    await vi.advanceTimersByTimeAsync(5000);
    expect(running.calls).toBe(1);
  });
});
