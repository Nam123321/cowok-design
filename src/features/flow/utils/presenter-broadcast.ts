// @story 06.4
// @implements AC-01, AC-03, AC-04
import {
  PresenterBroadcastEventSchema,
  PresenterTelemetryEventSchema,
  TauriPresenterIpcEventSchema,
  type PresenterBroadcastEvent,
  type PresenterTelemetryEvent,
  type PresenterTelemetryMetricName,
  type PresenterTelemetryRejectReason,
  type PresenterTransportMode,
} from "../../../lib/validations/presentation-presenter";

const SECRET_BYTES_LEN = 32;
const HEX_RADIX = 16;
const HEX_PAD = 2;
const MAX_TELEMETRY_BUFFER = 200;
const MAX_COMMAND_DEDUPE_CACHE = 128;
const POPUP_WIDTH = 1280;
const POPUP_HEIGHT = 800;
const WINDOW_LABEL_SUFFIX_MAX = 72;
const CHANNEL_SUFFIX_MAX = 128;

export const PRESENTER_TELEMETRY_EVENT_NAME = "cowok:presenter-telemetry";
export const TAURI_PRESENTER_EVENT_CHANNEL = "presenter:sync-event";

interface TauriInternalsLike {
  invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  transformCallback?: (cb: (payload: unknown) => void, once?: boolean) => number;
}

const telemetryBuffer: PresenterTelemetryEvent[] = [];

export function getPresenterTelemetryBuffer(): readonly PresenterTelemetryEvent[] {
  return telemetryBuffer;
}

export function clearPresenterTelemetryBuffer(): void {
  telemetryBuffer.length = 0;
}

export function generatePresenterSecret(): string {
  if (typeof globalThis.crypto?.getRandomValues !== "function") {
    throw new Error("CSPRNG globalThis.crypto.getRandomValues is required for presenter sessionSecret");
  }
  const bytes = new Uint8Array(SECRET_BYTES_LEN);
  globalThis.crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(HEX_RADIX).padStart(HEX_PAD, "0")).join("");
}

export function generatePresenterUuid(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  return generatePresenterSecret().slice(0, SECRET_BYTES_LEN);
}

export function timingSafeEqualString(a: string, b: string): boolean {
  const enc = new TextEncoder();
  const bytesA = enc.encode(a);
  const bytesB = enc.encode(b);
  const maxLen = Math.max(bytesA.length, bytesB.length);
  let diff = bytesA.length ^ bytesB.length;
  for (let i = 0; i < maxLen; i += 1) {
    const ba = i < bytesA.length ? bytesA[i]! : 0;
    const bb = i < bytesB.length ? bytesB[i]! : 0;
    diff |= ba ^ bb;
  }
  return diff === 0 && bytesA.length > 0;
}

export function buildPresenterChannelName(deckId: string, sessionId?: string): string {
  const rawSuffix = sessionId ? `${deckId}:${sessionId}` : deckId;
  const sanitized = rawSuffix.replace(/[^A-Za-z0-9_.:-]/g, "-").slice(0, CHANNEL_SUFFIX_MAX) || "deck";
  return `cowok-presenter:${sanitized}`;
}

export function buildPresenterWindowLabel(deckId: string): string {
  const sanitized = deckId.replace(/[^A-Za-z0-9_.:-]/g, "-").slice(0, WINDOW_LABEL_SUFFIX_MAX) || "deck";
  return `presenter-${sanitized}`;
}

export function buildPresenterPopupUrl(deckId: string, sessionId: string): string {
  const encodedDeck = encodeURIComponent(deckId);
  const encodedSession = encodeURIComponent(sessionId);
  return `/presenter/${encodedDeck}?sessionId=${encodedSession}`;
}

export function isTauriPresenterRuntime(): boolean {
  if (typeof window === "undefined") return false;
  const win = window as Window & { __TAURI_INTERNALS__?: TauriInternalsLike };
  return Boolean(win.__TAURI_INTERNALS__);
}

export function resolvePresenterTransportMode(): PresenterTransportMode {
  return isTauriPresenterRuntime() ? "tauri_ipc" : "broadcast_channel";
}

export function openPresenterPopupSync(deckId: string, sessionId: string): Window | null {
  if (typeof window === "undefined" || typeof window.open !== "function") {
    return null;
  }
  const url = buildPresenterPopupUrl(deckId, sessionId);
  const target = `cowok-presenter-${deckId}`;
  const features = `popup=yes,width=${POPUP_WIDTH},height=${POPUP_HEIGHT},resizable=yes`;
  const win = window.open(url, target, features);
  if (!win || win.closed) return null;
  return win;
}

export function emitPresenterTelemetry(opts: {
  metric: PresenterTelemetryMetricName;
  value: number;
  deckId: string;
  sessionId: string;
  transportMode: PresenterTransportMode;
  rejectReason?: PresenterTelemetryRejectReason;
}): PresenterTelemetryEvent {
  const validated = PresenterTelemetryEventSchema.parse({
    ...opts,
    recordedAtMs: Date.now(),
  });
  if (telemetryBuffer.length >= MAX_TELEMETRY_BUFFER) {
    telemetryBuffer.shift();
  }
  telemetryBuffer.push(validated);
  if (typeof window !== "undefined" && typeof window.dispatchEvent === "function") {
    window.dispatchEvent(new CustomEvent(PRESENTER_TELEMETRY_EVENT_NAME, { detail: validated }));
  }
  return validated;
}

export interface PresenterTransportAdapter {
  mode: PresenterTransportMode;
  channelName: string;
  send: (event: PresenterBroadcastEvent) => void;
  subscribe: (onRawMessage: (raw: unknown) => void) => () => void;
  close: () => void;
}

function dispatchValidatedTauriPayload(raw: unknown, listeners: Set<(raw: unknown) => void>): void {
  const candidate =
    raw && typeof raw === "object" && "payload" in raw && !("windowLabel" in raw)
      ? (raw as { payload: unknown }).payload
      : raw;
  const parsed = TauriPresenterIpcEventSchema.safeParse(candidate);
  if (!parsed.success) return;
  listeners.forEach((fn) => fn(parsed.data.payload));
}

function registerTauriNativeListener(
  tauri: TauriInternalsLike | undefined,
  listeners: Set<(raw: unknown) => void>
): () => void {
  if (!tauri?.invoke || !tauri?.transformCallback) return () => {};
  let unlistenPromise: Promise<unknown> | null = null;
  try {
    const callbackId = tauri.transformCallback((ev) => dispatchValidatedTauriPayload(ev, listeners), false);
    unlistenPromise = tauri
      .invoke("plugin:event|listen", {
        event: TAURI_PRESENTER_EVENT_CHANNEL,
        target: { kind: "Any" },
        handler: callbackId,
      })
      .catch(() => undefined);
  } catch {
    return () => {};
  }
  return () => {
    void unlistenPromise?.then((eventId) => {
      if (typeof eventId === "number") {
        void tauri.invoke?.("plugin:event|unlisten", { event: TAURI_PRESENTER_EVENT_CHANNEL, eventId }).catch(() => undefined);
      }
    });
  };
}

function createTauriTransportAdapter(
  deckId: string,
  channelName: string
): PresenterTransportAdapter {
  const win = window as Window & { __TAURI_INTERNALS__?: TauriInternalsLike };
  const tauri = win.__TAURI_INTERNALS__;
  const windowLabel = buildPresenterWindowLabel(deckId);
  const listeners = new Set<(raw: unknown) => void>();
  const bc = typeof BroadcastChannel !== "undefined" ? new BroadcastChannel(channelName) : null;
  const unlistenNative = registerTauriNativeListener(tauri, listeners);
  const onWindowIpc = (ev: Event) => dispatchValidatedTauriPayload((ev as CustomEvent<unknown>).detail, listeners);
  window.addEventListener(TAURI_PRESENTER_EVENT_CHANNEL, onWindowIpc);
  if (bc) {
    bc.onmessage = (ev) => listeners.forEach((fn) => fn(ev.data));
  }

  return {
    mode: "tauri_ipc",
    channelName,
    send: (event) => {
      const ipcEnvelope = TauriPresenterIpcEventSchema.parse({ windowLabel, payload: event });
      void tauri?.invoke?.("plugin:event|emit", { event: TAURI_PRESENTER_EVENT_CHANNEL, payload: ipcEnvelope })?.catch?.(() => undefined);
      bc?.postMessage(event);
    },
    subscribe: (onRawMessage) => {
      listeners.add(onRawMessage);
      return () => listeners.delete(onRawMessage);
    },
    close: () => {
      unlistenNative();
      window.removeEventListener(TAURI_PRESENTER_EVENT_CHANNEL, onWindowIpc);
      bc?.close();
      listeners.clear();
    },
  };
}

export function createPresenterTransportAdapter(
  deckId: string,
  channelName: string = buildPresenterChannelName(deckId)
): PresenterTransportAdapter {
  if (isTauriPresenterRuntime()) {
    return createTauriTransportAdapter(deckId, channelName);
  }
  const bc = typeof BroadcastChannel !== "undefined" ? new BroadcastChannel(channelName) : null;
  const listeners = new Set<(raw: unknown) => void>();
  if (bc) {
    bc.onmessage = (ev) => listeners.forEach((fn) => fn(ev.data));
  }
  return {
    mode: "broadcast_channel",
    channelName,
    send: (event) => bc?.postMessage(event),
    subscribe: (onRawMessage) => {
      listeners.add(onRawMessage);
      return () => listeners.delete(onRawMessage);
    },
    close: () => {
      bc?.close();
      listeners.clear();
    },
  };
}

export interface IncomingEnvelopeValidationState {
  expectedOrigin: string;
  expectedDeckId: string;
  expectedSessionId: string;
  expectedSecret: string;
  expectedEpoch: number;
  expectedSenderRole: "host" | "presenter";
  activeSenderInstanceId?: string | null;
  lastAcceptedSeqByInstance: Map<string, number>;
  processedCommandKeys?: string[];
}

export type EnvelopeValidationResult =
  | { ok: true; event: PresenterBroadcastEvent }
  | { ok: false; reason: PresenterTelemetryRejectReason };

function isDuplicateCommand(
  cache: string[] | undefined,
  senderInstanceId: string,
  commandId: string | undefined
): boolean {
  if (!cache || !commandId) return false;
  const key = `${senderInstanceId}:${commandId}`;
  if (cache.includes(key)) return true;
  if (cache.length >= MAX_COMMAND_DEDUPE_CACHE) {
    cache.shift();
  }
  cache.push(key);
  return false;
}

export function validateIncomingPresenterEvent(
  raw: unknown,
  state: IncomingEnvelopeValidationState
): EnvelopeValidationResult {
  const parsed = PresenterBroadcastEventSchema.safeParse(raw);
  if (!parsed.success) {
    return { ok: false, reason: "zod_invalid" };
  }
  const ev = parsed.data;
  if (ev.senderOrigin !== state.expectedOrigin) {
    return { ok: false, reason: "origin_mismatch" };
  }
  if (ev.deckId !== state.expectedDeckId || ev.sessionId !== state.expectedSessionId) {
    return { ok: false, reason: "deck_mismatch" };
  }
  if (!timingSafeEqualString(ev.sessionSecret, state.expectedSecret)) {
    return { ok: false, reason: "secret_mismatch" };
  }
  if (ev.senderRole !== state.expectedSenderRole || ev.epoch !== state.expectedEpoch) {
    return { ok: false, reason: "secret_mismatch" };
  }
  if (state.activeSenderInstanceId && ev.senderInstanceId !== state.activeSenderInstanceId) {
    return { ok: false, reason: "stale_instance" };
  }
  const prevSeq = state.lastAcceptedSeqByInstance.get(ev.senderInstanceId) ?? 0;
  if (ev.seq <= prevSeq) {
    return { ok: false, reason: "stale_seq" };
  }
  if (isDuplicateCommand(state.processedCommandKeys, ev.senderInstanceId, ev.commandId)) {
    return { ok: false, reason: "duplicate_command" };
  }
  state.lastAcceptedSeqByInstance.set(ev.senderInstanceId, ev.seq);
  return { ok: true, event: ev };
}
