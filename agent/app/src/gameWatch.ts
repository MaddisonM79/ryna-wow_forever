// Results that say "the game is running: /reload ..." stay true only while it runs. While any shown
// result says so, ask the app every few seconds whether the game still runs (the same check the
// Addon settings tab polls), and once it has closed, rewrite those lines (describe.ts,
// gameClosedLines), so the note doesn't tell the player to /reload a game they already closed.
import { onScopeDispose, watch, type Ref } from "vue";
import { api } from "./api";
import { gameClosedLines, waitsOnGame } from "./describe";

export type Results = Record<string, { ok: boolean; lines: string[] }>;

export function watchGameClosing(results: Ref<Results>, every = 5000): void {
  let poll: ReturnType<typeof setInterval> | null = null;
  const stop = (): void => {
    if (poll) clearInterval(poll);
    poll = null;
  };
  const waiting = (): boolean => Object.values(results.value).some((r) => waitsOnGame(r.lines));
  async function check(): Promise<void> {
    let running: boolean;
    try {
      running = (await api.settingsGameRunning()).running;
    } catch {
      return; // Ask again next time.
    }
    if (running) return;
    for (const r of Object.values(results.value)) r.lines = gameClosedLines(r.lines);
    stop();
  }
  watch(
    results,
    () => {
      if (!waiting()) return stop();
      if (!poll) poll = setInterval(() => void check(), every);
    },
    { deep: true, immediate: true },
  );
  onScopeDispose(stop);
}
