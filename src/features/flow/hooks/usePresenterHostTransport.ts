// @story 06.4
// @implements AC-01, AC-03, AC-04
import { useCallback, useEffect, useRef } from "react";
import { apiPresentationPresenter } from "../../../api/presentation-presenter";
import type {
  PresenterBlackoutMode,
  PresenterBroadcastEvent,
  PresenterConnectionState,
  PresenterSlideSnapshotDto,
  PresenterTransportMode,
} from "../../../lib/validations/presentation-presenter";
import {
  emitPresenterTelemetry,
  generatePresenterUuid,
} from "../utils/presenter-broadcast";
import {
  handleHostWindowHandshakeMessage,
  rotateHostSessionIdentity,
  validateHostIncomingBroadcast,
  type HostSequencerState,
} from "../utils/presenter-host-helpers";

const POPUP_CLOSED_POLL_MS = 1000;
const BEFOREUNLOAD_CLOSED_RECHECK_MS = 150;
const HEARTBEAT_PING_INTERVAL_MS = 5000;
const HEARTBEAT_TIMEOUT_MS = 15000;

export interface PresenterApiErrorState {
  status: number;
  code: string;
  message: string;
}

function startHostHeartbeatMonitor(opts: {
  effectiveDeckId: string;
  transportMode: PresenterTransportMode;
  seqState: HostSequencerState;
  getLastActivityMs: () => number;
  onDisconnect: () => void;
}): () => void {
  const closedTimer = window.setInterval(() => {
    if (opts.seqState.popupWindow?.closed) opts.onDisconnect();
  }, POPUP_CLOSED_POLL_MS);
  const pingTimer = window.setInterval(() => {
    if (Date.now() - opts.getLastActivityMs() > HEARTBEAT_TIMEOUT_MS) {
      opts.onDisconnect();
      return;
    }
    if (!opts.seqState.transport) return;
    opts.seqState.hostSeq += 1;
    opts.seqState.transport.send({
      type: "HEARTBEAT_PING",
      deckId: opts.effectiveDeckId,
      sessionId: opts.seqState.sessionId,
      sessionSecret: opts.seqState.sessionSecret,
      senderOrigin: window.location.origin,
      senderRole: "host",
      senderInstanceId: opts.seqState.hostInstanceId,
      epoch: opts.seqState.epoch,
      stateRevision: opts.seqState.stateRevision,
      seq: opts.seqState.hostSeq,
      sentAtMs: Date.now(),
      pingId: generatePresenterUuid(),
    });
  }, HEARTBEAT_PING_INTERVAL_MS);
  return () => {
    window.clearInterval(closedTimer);
    window.clearInterval(pingTimer);
  };
}

function subscribeHostSyncListeners(opts: {
  effectiveDeckId: string;
  transportMode: PresenterTransportMode;
  seqState: HostSequencerState;
  isHistoricalReadOnly: boolean;
  snapshotTag: string | null;
  connectionStateRef: React.MutableRefObject<PresenterConnectionState>;
  onAuthRef: React.MutableRefObject<() => void>;
  onCmdRef: React.MutableRefObject<(ev: PresenterBroadcastEvent) => void>;
  onDisconnectRef: React.MutableRefObject<() => void>;
}): () => void {
  let lastActivityMs = Date.now();
  const onWindowMessage = (ev: MessageEvent) =>
    handleHostWindowHandshakeMessage({
      ev, deckId: opts.effectiveDeckId, seqState: opts.seqState,
      isHistoricalReadOnly: opts.isHistoricalReadOnly, snapshotTag: opts.snapshotTag,
      onAuthenticated: () => { lastActivityMs = Date.now(); opts.onAuthRef.current(); },
    });
  window.addEventListener("message", onWindowMessage);

  const unsub = opts.seqState.transport?.subscribe((raw) => {
    const valid = validateHostIncomingBroadcast({
      raw, deckId: opts.effectiveDeckId, seqState: opts.seqState, transportMode: opts.transportMode,
    });
    if (valid) {
      lastActivityMs = Date.now();
      opts.onCmdRef.current(valid);
    }
  });

  const stopHeartbeat = startHostHeartbeatMonitor({
    effectiveDeckId: opts.effectiveDeckId, transportMode: opts.transportMode,
    seqState: opts.seqState, getLastActivityMs: () => lastActivityMs,
    onDisconnect: () => {
      if (opts.connectionStateRef.current === "AUTHENTICATED") opts.onDisconnectRef.current();
    },
  });

  const onBeforeUnload = () => { opts.seqState.transport?.close(); opts.seqState.transport = null; };
  window.addEventListener("beforeunload", onBeforeUnload);

  return () => {
    stopHeartbeat();
    window.removeEventListener("message", onWindowMessage);
    window.removeEventListener("beforeunload", onBeforeUnload);
    unsub?.();
  };
}

export function useHostSyncEffects(opts: {
  effectiveDeckId: string;
  connectionState: PresenterConnectionState;
  launchGeneration: number;
  transportMode: PresenterTransportMode;
  seqRef: React.MutableRefObject<HostSequencerState>;
  isHistoricalReadOnly: boolean;
  snapshotTag: string | null;
  onAuthenticated: () => void;
  onBroadcastCommand: (ev: PresenterBroadcastEvent) => void;
  onDisconnect: () => void;
}) {
  const { effectiveDeckId, connectionState, launchGeneration, transportMode, seqRef, isHistoricalReadOnly, snapshotTag } = opts;
  const connectionStateRef = useRef(connectionState);
  connectionStateRef.current = connectionState;
  const onAuthRef = useRef(opts.onAuthenticated);
  const onCmdRef = useRef(opts.onBroadcastCommand);
  const onDisconnectRef = useRef(opts.onDisconnect);
  onAuthRef.current = opts.onAuthenticated;
  onCmdRef.current = opts.onBroadcastCommand;
  onDisconnectRef.current = opts.onDisconnect;

  useEffect(() => {
    if (connectionState === "CLOSED" || connectionState === "POPUP_BLOCKED_INLINE_FALLBACK") return undefined;
    if (typeof window === "undefined") return undefined;
    return subscribeHostSyncListeners({
      effectiveDeckId, transportMode, seqState: seqRef.current,
      isHistoricalReadOnly, snapshotTag, connectionStateRef, onAuthRef, onCmdRef, onDisconnectRef,
    });
  // Intentional: keep listeners pinned across OPENING_POPUP -> AUTHENTICATED to avoid dropping REQUEST_SYNC_FULL_STATE
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [effectiveDeckId, isHistoricalReadOnly, launchGeneration, seqRef, snapshotTag, transportMode, connectionState === "CLOSED" || connectionState === "POPUP_BLOCKED_INLINE_FALLBACK"]);
}

function extractPresenterApiError(err: unknown, snapshotTag: string | null): PresenterApiErrorState | null {
  if (!err || typeof err !== "object") return null;
  const rec = err as { status?: unknown; code?: unknown; message?: unknown };
  const status = typeof rec.status === "number" ? rec.status : 0;
  if (status === 404 && !snapshotTag) {
    return {
      status,
      code: typeof rec.code === "string" ? rec.code : "E_DECK_NOT_FOUND",
      message: typeof rec.message === "string" ? rec.message : "Deck not found",
    };
  }
  if (status >= 400 || status === 0) {
    return {
      status,
      code: typeof rec.code === "string" ? rec.code : status === 0 ? "E_NETWORK" : `HTTP_${status}`,
      message: typeof rec.message === "string" ? rec.message : "Presenter handshake failed",
    };
  }
  return null;
}

export function useHostPresenterApiSync(opts: {
  projectId?: string;
  deckId?: string | null;
  activeSlideIndex: number;
  snapshotTag: string | null;
  setServerSlides: (s: PresenterSlideSnapshotDto[] | null) => void;
  setServerTitle: (t: string | null) => void;
  setServerReadOnly: (r: boolean | null) => void;
  setApiError: (e: PresenterApiErrorState | null) => void;
}) {
  const { projectId, deckId, activeSlideIndex, snapshotTag, setServerSlides, setServerTitle, setServerReadOnly, setApiError } = opts;
  const reqGenRef = useRef(0);
  return useCallback(
    async (mode: PresenterTransportMode) => {
      const gen = ++reqGenRef.current;
      setApiError(null);
      if (!deckId || deckId === "local-deck") return;
      try {
        const res = await apiPresentationPresenter.createPresenterSession(
          { projectId, deckId },
          { snapshotTag: snapshotTag ?? undefined, transportMode: mode, activeSlideIndex }
        );
        if (reqGenRef.current !== gen) return;
        setServerSlides(res.slides);
        setServerTitle(res.deckTitle);
        setServerReadOnly(res.isHistoricalReadOnly);
      } catch (err) {
        if (reqGenRef.current !== gen) return;
        const surfaced = extractPresenterApiError(err, snapshotTag);
        if (surfaced) setApiError(surfaced);
      }
    },
    [activeSlideIndex, deckId, projectId, setApiError, setServerReadOnly, setServerSlides, setServerTitle, snapshotTag]
  );
}

function handlePresenterWindowClosingCommand(
  ev: Extract<PresenterBroadcastEvent, { type: "PRESENTER_WINDOW_CLOSING" }>,
  seqState: HostSequencerState,
  setConnectionState: (s: PresenterConnectionState) => void,
  onDisconnect?: () => void
): void {
  if (ev.reason === "beforeunload") {
    if (typeof window !== "undefined") {
      window.setTimeout(() => { if (seqState.popupWindow?.closed) onDisconnect?.(); }, BEFOREUNLOAD_CLOSED_RECHECK_MS);
    }
    return;
  }
  rotateHostSessionIdentity(seqState);
  setConnectionState("CLOSED");
}

function handlePresenterSlideNavCommand(opts: {
  ev: Extract<PresenterBroadcastEvent, { type: "SLIDE_NAV" }>;
  seqState: HostSequencerState;
  effectiveDeckId: string;
  transportMode: PresenterTransportMode;
  activeSlideIndex: number;
  broadcastSyncFullState: () => void;
  navigateSlide: (idx: number) => void;
}): void {
  const { ev, seqState } = opts;
  if (typeof ev.baseRevision === "number" && ev.baseRevision < seqState.stateRevision) {
    emitPresenterTelemetry({ metric: "presenter.channel_rejected.count", value: 1, deckId: opts.effectiveDeckId, sessionId: seqState.sessionId, transportMode: opts.transportMode, rejectReason: "stale_revision" });
    opts.broadcastSyncFullState();
    return;
  }
  const offsetMap: Record<"prev" | "next", number> = { prev: opts.activeSlideIndex - 1, next: opts.activeSlideIndex + 1 };
  opts.navigateSlide(ev.direction === "goto" ? ev.targetIndex : offsetMap[ev.direction]);
}

export function processHostBroadcastCommand(opts: {
  ev: PresenterBroadcastEvent;
  seqState: HostSequencerState;
  effectiveDeckId: string;
  transportMode: PresenterTransportMode;
  activeSlideIndex: number;
  isHistoricalReadOnly?: boolean;
  setConnectionState: (s: PresenterConnectionState) => void;
  broadcastSyncFullState: () => void;
  navigateSlide: (idx: number) => void;
  setBlackout: (m: PresenterBlackoutMode) => void;
  dispatchTimer: (a: "start" | "pause" | "reset") => void;
  onToggleAudienceFullscreen?: (req: "toggle" | "enter" | "exit") => void;
  onDisconnect?: () => void;
}): void {
  const { ev, seqState, effectiveDeckId, transportMode } = opts;
  if (ev.type === "HEARTBEAT_PONG") return;
  if (ev.type === "REQUEST_SYNC_FULL_STATE") {
    if (ev.reason === "reload") {
      emitPresenterTelemetry({ metric: "presenter.rehydrate.count", value: 1, deckId: effectiveDeckId, sessionId: seqState.sessionId, transportMode });
    }
    opts.setConnectionState("AUTHENTICATED");
    opts.broadcastSyncFullState();
    return;
  }
  if (ev.type === "PRESENTER_WINDOW_CLOSING") {
    handlePresenterWindowClosingCommand(ev, seqState, opts.setConnectionState, opts.onDisconnect);
    return;
  }
  if (opts.isHistoricalReadOnly) {
    emitPresenterTelemetry({ metric: "presenter.channel_rejected.count", value: 1, deckId: effectiveDeckId, sessionId: seqState.sessionId, transportMode, rejectReason: "readonly_snap_lock" });
    opts.broadcastSyncFullState();
    return;
  }
  if (ev.type === "SLIDE_NAV") { handlePresenterSlideNavCommand({ ...opts, ev }); return; }
  if (ev.type === "BLACKOUT_TOGGLE") { opts.setBlackout(ev.mode); return; }
  if (ev.type === "TIMER_COMMAND") { opts.dispatchTimer(ev.action); return; }
  if (ev.type === "FULLSCREEN_TOGGLE") { opts.onToggleAudienceFullscreen?.(ev.requestedState); }
}
