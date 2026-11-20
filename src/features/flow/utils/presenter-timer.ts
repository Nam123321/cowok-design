// @story 06.4
// @implements AC-02, AC-03
import type { PresenterTimerSnapshot } from "../../../lib/validations/presentation-presenter";

const MS_PER_SECOND = 1000;
const SECONDS_PER_MINUTE = 60;
const SECONDS_PER_HOUR = 3600;
const PAD_WIDTH = 2;

export interface PresenterTimerState {
  isRunning: boolean;
  accumulatedMs: number;
  startedAtPerfMs: number | null;
}

export function getMonotonicNowMs(): number {
  if (typeof globalThis.performance?.now === "function") {
    return globalThis.performance.now();
  }
  return Date.now();
}

export function createInitialTimerState(): PresenterTimerState {
  return { isRunning: false, accumulatedMs: 0, startedAtPerfMs: null };
}

export function computeTimerElapsedMs(
  state: PresenterTimerState,
  nowPerfMs: number = getMonotonicNowMs()
): number {
  if (!state.isRunning || state.startedAtPerfMs === null) {
    return state.accumulatedMs;
  }
  return state.accumulatedMs + Math.max(0, nowPerfMs - state.startedAtPerfMs);
}

export function applyTimerCommand(
  state: PresenterTimerState,
  action: "start" | "pause" | "reset",
  nowPerfMs: number = getMonotonicNowMs()
): PresenterTimerState {
  if (action === "start") {
    if (state.isRunning) return state;
    return {
      isRunning: true,
      accumulatedMs: state.accumulatedMs,
      startedAtPerfMs: nowPerfMs,
    };
  }
  if (action === "pause") {
    if (!state.isRunning) return state;
    return {
      isRunning: false,
      accumulatedMs: computeTimerElapsedMs(state, nowPerfMs),
      startedAtPerfMs: null,
    };
  }
  return createInitialTimerState();
}

export function buildTimerSnapshot(
  state: PresenterTimerState,
  nowPerfMs: number = getMonotonicNowMs()
): PresenterTimerSnapshot {
  const elapsed = computeTimerElapsedMs(state, nowPerfMs);
  return {
    accumulatedMs: state.accumulatedMs,
    elapsedAtSyncMs: elapsed,
    elapsedMs: elapsed,
    isRunning: state.isRunning,
    syncedAtMs: Math.max(0, Math.round(nowPerfMs)),
  };
}

export function computeClientDisplayMs(
  snapshot: PresenterTimerSnapshot,
  receivedPerfMs: number,
  nowPerfMs: number = getMonotonicNowMs()
): number {
  if (!snapshot.isRunning) {
    return snapshot.elapsedAtSyncMs;
  }
  return snapshot.elapsedAtSyncMs + Math.max(0, nowPerfMs - receivedPerfMs);
}

function padTwo(val: number): string {
  return String(val).padStart(PAD_WIDTH, "0");
}

export function formatStopwatchMmSs(totalMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(totalMs / MS_PER_SECOND));
  const hours = Math.floor(totalSeconds / SECONDS_PER_HOUR);
  const mins = Math.floor((totalSeconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
  const secs = totalSeconds % SECONDS_PER_MINUTE;
  if (hours > 0) {
    return `${padTwo(hours)}:${padTwo(mins)}:${padTwo(secs)}`;
  }
  return `${padTwo(mins)}:${padTwo(secs)}`;
}
