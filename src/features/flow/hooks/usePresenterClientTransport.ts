// @story 06.4
// @implements AC-01, AC-02, AC-04
import { useEffect } from "react";
import {
  PresenterHandshakeSchema,
  type PresenterBlackoutMode,
  type PresenterBroadcastEvent,
  type PresenterSlideSnapshotDto,
  type PresenterTimerSnapshot,
  type PresenterTransportMode,
} from "../../../lib/validations/presentation-presenter";
import {
  buildPresenterChannelName,
  createPresenterTransportAdapter,
  emitPresenterTelemetry,
  generatePresenterUuid,
  validateIncomingPresenterEvent,
  type PresenterTransportAdapter,
} from "../utils/presenter-broadcast";
import { getMonotonicNowMs } from "../utils/presenter-timer";

export interface PresenterClientSyncedState {
  deckTitle: string;
  aspectRatio: "16:9" | "16:10" | "Auto";
  revision: number;
  activeSlideIndex: number;
  slidesCount: number;
  blackoutMode: PresenterBlackoutMode;
  isAudienceFullscreen: boolean;
  isHistoricalReadOnly: boolean;
  snapshotTag: string | null;
  timer: PresenterTimerSnapshot;
  slides: PresenterSlideSnapshotDto[];
  currentSlideHtml?: string;
  nextSlideHtml?: string | null;
}

export interface ClientTransportRefState {
  senderInstanceId: string;
  windowNonce: string;
  sessionId: string;
  sessionSecret: string | null;
  hostSenderInstanceId: string | null;
  processedCommandKeys: string[];
  closingSent: boolean;
  epoch: number;
  localStateRevision: number;
  localSeq: number;
  lastAcceptedSeqByInstance: Map<string, number>;
  transport: PresenterTransportAdapter | null;
}

function isPageReloadNavigation(): boolean {
  if (typeof performance === "undefined" || typeof performance.getEntriesByType !== "function") return false;
  const nav = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined;
  return nav?.type === "reload";
}

function applyIncomingSyncFullState(opts: {
  ev: Extract<PresenterBroadcastEvent, { type: "SYNC_FULL_STATE" }>;
  st: ClientTransportRefState;
  setSyncLatencyMs: (ms: number) => void;
  setTimerAnchorPerfMs: (ms: number) => void;
  setSyncedState: (s: PresenterClientSyncedState) => void;
}): void {
  const { ev, st } = opts;
  const isNewer =
    st.localStateRevision === -1 ||
    ev.epoch > st.epoch ||
    (ev.epoch === st.epoch && ev.stateRevision > st.localStateRevision);
  if (!isNewer) return;
  st.epoch = ev.epoch;
  st.localStateRevision = ev.stateRevision;
  opts.setSyncLatencyMs(Math.max(0, Date.now() - ev.sentAtMs));
  opts.setTimerAnchorPerfMs(getMonotonicNowMs());
  opts.setSyncedState({
    deckTitle: ev.deckTitle, aspectRatio: ev.aspectRatio, revision: ev.revision,
    activeSlideIndex: ev.activeSlideIndex, slidesCount: ev.slidesCount, blackoutMode: ev.blackoutMode,
    isAudienceFullscreen: ev.isAudienceFullscreen, isHistoricalReadOnly: ev.isHistoricalReadOnly,
    snapshotTag: ev.snapshotTag, timer: ev.timer, slides: ev.slides,
    currentSlideHtml: ev.currentSlideHtml, nextSlideHtml: ev.nextSlideHtml,
  });
}

function bindClientBroadcastChannel(opts: {
  deckId: string;
  channelName: string;
  rehydrated: boolean;
  origin: string;
  transportMode: PresenterTransportMode;
  st: ClientTransportRefState;
  sendPresenterEvent: (fn: (b: Omit<PresenterBroadcastEvent, "type">) => PresenterBroadcastEvent) => void;
  setSyncLatencyMs: (ms: number) => void;
  setTimerAnchorPerfMs: (ms: number) => void;
  setSyncedState: (s: PresenterClientSyncedState) => void;
}): void {
  const { st, deckId, origin, transportMode } = opts;
  st.transport?.close();
  const adapter = createPresenterTransportAdapter(deckId, opts.channelName);
  st.transport = adapter;
  adapter.subscribe((raw) => {
    if (!st.sessionSecret) return;
    const valid = validateIncomingPresenterEvent(raw, {
      expectedOrigin: origin, expectedDeckId: deckId, expectedSessionId: st.sessionId,
      expectedSecret: st.sessionSecret, expectedEpoch: st.epoch, expectedSenderRole: "host",
      activeSenderInstanceId: st.hostSenderInstanceId,
      lastAcceptedSeqByInstance: st.lastAcceptedSeqByInstance,
      processedCommandKeys: st.processedCommandKeys,
    });
    if (!valid.ok) {
      emitPresenterTelemetry({ metric: "presenter.channel_rejected.count", value: 1, deckId, sessionId: st.sessionId, transportMode, rejectReason: valid.reason });
      return;
    }
    if (valid.event.type === "SYNC_FULL_STATE") {
      if (!st.hostSenderInstanceId) st.hostSenderInstanceId = valid.event.senderInstanceId;
      applyIncomingSyncFullState({ ev: valid.event, st, setSyncLatencyMs: opts.setSyncLatencyMs, setTimerAnchorPerfMs: opts.setTimerAnchorPerfMs, setSyncedState: opts.setSyncedState });
      return;
    }
    if (valid.event.type === "HEARTBEAT_PING") {
      opts.sendPresenterEvent((base) => ({ ...base, type: "HEARTBEAT_PONG", pingId: valid.event.type === "HEARTBEAT_PING" ? valid.event.pingId : generatePresenterUuid() }));
    }
  });
  opts.sendPresenterEvent((base) => ({ ...base, type: "REQUEST_SYNC_FULL_STATE", reason: opts.rehydrated ? "reload" : "initial" }));
}

function createWindowMessageHandler(opts: {
  deckId: string;
  origin: string;
  transportMode: PresenterTransportMode;
  st: ClientTransportRefState;
  sendPresenterEvent: (fn: (b: Omit<PresenterBroadcastEvent, "type">) => PresenterBroadcastEvent) => void;
  setIsAuthenticated: (v: boolean) => void;
  setSyncLatencyMs: (ms: number) => void;
  setTimerAnchorPerfMs: (ms: number) => void;
  setSyncedState: (s: PresenterClientSyncedState) => void;
}) {
  const { deckId, origin, st } = opts;
  return (ev: MessageEvent) => {
    if (ev.origin !== origin) return;
    if (!window.opener || ev.source !== window.opener) return;
    const parsed = PresenterHandshakeSchema.safeParse(ev.data);
    if (!parsed.success || parsed.data.type !== "PRESENTER_SECRET_PROVISION") return;
    const msg = parsed.data;
    if (msg.deckId !== deckId || msg.sessionId !== st.sessionId || msg.windowNonce !== st.windowNonce) return;
    st.sessionSecret = msg.sessionSecret;
    st.epoch = msg.epoch;
    st.localSeq = Math.max(0, msg.nextSeqFloor - 1);
    st.hostSenderInstanceId = null;
    st.processedCommandKeys = [];
    st.lastAcceptedSeqByInstance.clear();
    opts.setIsAuthenticated(true);
    const rehydrated = isPageReloadNavigation();
    window.opener.postMessage({ type: "PRESENTER_HANDSHAKE_ACK", deckId, sessionId: st.sessionId, sessionSecret: msg.sessionSecret, senderInstanceId: st.senderInstanceId, epoch: msg.epoch, stateRevision: msg.stateRevision, nextSeqFloor: msg.nextSeqFloor, rehydrated }, origin);
    bindClientBroadcastChannel({ ...opts, channelName: msg.channelName || buildPresenterChannelName(deckId, st.sessionId), rehydrated });
  };
}

export function usePresenterClientTransport(opts: {
  deckId: string;
  transportMode: PresenterTransportMode;
  safeRef: React.MutableRefObject<ClientTransportRefState>;
  sendPresenterEvent: (fn: (b: Omit<PresenterBroadcastEvent, "type">) => PresenterBroadcastEvent) => void;
  setIsAuthenticated: (v: boolean) => void;
  setSyncLatencyMs: (ms: number) => void;
  setTimerAnchorPerfMs: (ms: number) => void;
  setSyncedState: (s: PresenterClientSyncedState) => void;
}): void {
  const { deckId, transportMode, safeRef, sendPresenterEvent } = opts;
  useEffect(() => {
    if (typeof window === "undefined") return undefined;
    const origin = window.location.origin;
    const st = safeRef.current;
    const onWindowMessage = createWindowMessageHandler({
      deckId, origin, transportMode, st, sendPresenterEvent,
      setIsAuthenticated: opts.setIsAuthenticated, setSyncLatencyMs: opts.setSyncLatencyMs,
      setTimerAnchorPerfMs: opts.setTimerAnchorPerfMs, setSyncedState: opts.setSyncedState,
    });
    window.addEventListener("message", onWindowMessage);
    window.opener?.postMessage({ type: "PRESENTER_WINDOW_READY", deckId, sessionId: st.sessionId, windowNonce: st.windowNonce, senderInstanceId: st.senderInstanceId }, origin);
    const onBeforeUnload = () => {
      if (!st.closingSent && st.transport && st.sessionSecret) {
        st.closingSent = true;
        sendPresenterEvent((base) => ({ ...base, type: "PRESENTER_WINDOW_CLOSING", reason: "beforeunload" }));
      }
      st.transport?.close();
      st.transport = null;
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => {
      window.removeEventListener("message", onWindowMessage);
      window.removeEventListener("beforeunload", onBeforeUnload);
      st.transport?.close();
      st.transport = null;
    };
  }, [deckId, opts.setIsAuthenticated, opts.setSyncLatencyMs, opts.setSyncedState, opts.setTimerAnchorPerfMs, safeRef, sendPresenterEvent, transportMode]);
}
