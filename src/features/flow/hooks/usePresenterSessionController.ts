// @story 06.4
// @implements AC-01, AC-02, AC-03, AC-04
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  transitionPresenterConnectionState,
  type PresenterBlackoutMode,
  type PresenterBroadcastEvent,
  type PresenterConnectionState,
  type PresenterSlideSnapshotDto,
  type PresenterTransportMode,
} from "../../../lib/validations/presentation-presenter";
import type { SlidePhaseItem } from "../components/companion-canvas-data";
import {
  buildPresenterChannelName,
  createPresenterTransportAdapter,
  emitPresenterTelemetry,
  openPresenterPopupSync,
  resolvePresenterTransportMode,
} from "../utils/presenter-broadcast";
import {
  buildFallbackPresenterSlides,
  buildPresenterSlideHtml,
  buildSyncFullStateEnvelope,
  createInitialHostSequencerState,
  normalizeDeckAspectRatio,
  rotateHostSessionIdentity,
  type HostSequencerState,
} from "../utils/presenter-host-helpers";
import {
  applyTimerCommand,
  computeTimerElapsedMs,
  createInitialTimerState,
  getMonotonicNowMs,
  type PresenterTimerState,
} from "../utils/presenter-timer";
import {
  processHostBroadcastCommand,
  useHostPresenterApiSync,
  useHostSyncEffects,
  type PresenterApiErrorState,
} from "./usePresenterHostTransport";

const STOPWATCH_TICK_MS = 250;

export interface UsePresenterSessionControllerParams {
  projectId?: string;
  deckId?: string | null;
  deckTitle?: string;
  deckRevision?: number;
  aspectRatio: "auto" | "16-9" | "16-10";
  slides: SlidePhaseItem[];
  activeSlideIndex: number;
  speakerNotesBySlideId?: Record<string, string>;
  customHtmlBySlideId?: Record<string, string>;
  isAudienceFullscreen: boolean;
  isHistoricalReadOnly: boolean;
  snapshotTag?: string | null;
  onChangeSlide: (nextIndex: number) => void;
  onPostDeckKeyNav?: (direction: "prev" | "next" | "goto", targetIndex?: number) => void;
  onPostDeckBlackout?: (mode: PresenterBlackoutMode) => void;
  onToggleAudienceFullscreen?: (requested: "toggle" | "enter" | "exit") => void;
}

function useHostStopwatchTick(timerState: PresenterTimerState): number {
  const [displayMs, setDisplayMs] = useState<number>(() => computeTimerElapsedMs(timerState));
  useEffect(() => {
    setDisplayMs(computeTimerElapsedMs(timerState));
    if (!timerState.isRunning || typeof window === "undefined") return undefined;
    const id = window.setInterval(() => setDisplayMs(computeTimerElapsedMs(timerState)), STOPWATCH_TICK_MS);
    return () => window.clearInterval(id);
  }, [timerState]);
  return displayMs;
}

function useMergedPresenterSlides(slides: SlidePhaseItem[], notesMap: Record<string, string> | undefined, serverSlides: PresenterSlideSnapshotDto[] | null) {
  return useMemo(() => {
    const base = buildFallbackPresenterSlides(slides, notesMap);
    if (!serverSlides || serverSlides.length !== base.length) return base;
    return base.map((item, idx) => ({ ...item, speakerNotes: notesMap?.[slides[idx]?.id ?? ""] ?? serverSlides[idx]?.speakerNotes ?? item.speakerNotes }));
  }, [serverSlides, slides, notesMap]);
}

function useHostPopupLauncher(opts: {
  effectiveDeckId: string;
  safeSeqRef: React.MutableRefObject<HostSequencerState>;
  setConnectionState: (s: PresenterConnectionState) => void;
  setTransportMode: (m: PresenterTransportMode) => void;
  setIsPopupBlockedToastVisible: (v: boolean) => void;
  bumpLaunchGeneration: () => void;
  fetchServerSnapshot: (m: PresenterTransportMode) => Promise<void>;
}) {
  return useCallback(() => {
    const seqState = opts.safeSeqRef.current;
    rotateHostSessionIdentity(seqState);
    seqState.openedAtPerfMs = getMonotonicNowMs();
    const popup = openPresenterPopupSync(opts.effectiveDeckId, seqState.sessionId);
    if (!popup) {
      opts.setConnectionState("POPUP_BLOCKED_INLINE_FALLBACK");
      opts.setTransportMode("inline_fallback");
      opts.setIsPopupBlockedToastVisible(true);
      emitPresenterTelemetry({ metric: "presenter.popup_blocked.count", value: 1, deckId: opts.effectiveDeckId, sessionId: seqState.sessionId, transportMode: "inline_fallback" });
      void opts.fetchServerSnapshot("inline_fallback");
      return;
    }
    const mode = resolvePresenterTransportMode();
    seqState.transport = createPresenterTransportAdapter(opts.effectiveDeckId, buildPresenterChannelName(opts.effectiveDeckId, seqState.sessionId));
    seqState.popupWindow = popup;
    opts.setTransportMode(mode);
    opts.setIsPopupBlockedToastVisible(false);
    opts.setConnectionState("OPENING_POPUP");
    opts.bumpLaunchGeneration();
    void opts.fetchServerSnapshot(mode);
  }, [opts]);
}

function useHostSlideAndTimerActions(opts: {
  params: UsePresenterSessionControllerParams;
  timerState: PresenterTimerState;
  setBlackoutMode: (m: PresenterBlackoutMode) => void;
  setTimerState: (t: PresenterTimerState) => void;
  broadcastSyncFullState: (o?: { activeSlideIndex?: number; blackoutMode?: PresenterBlackoutMode; timerState?: PresenterTimerState; bumpRevision?: boolean }) => void;
}) {
  const { params, timerState, setBlackoutMode, setTimerState, broadcastSyncFullState } = opts;
  const setAndForwardBlackout = useCallback((nextMode: PresenterBlackoutMode) => {
    setBlackoutMode(nextMode);
    params.onPostDeckBlackout?.(nextMode);
    broadcastSyncFullState({ blackoutMode: nextMode, bumpRevision: true });
  }, [broadcastSyncFullState, params, setBlackoutMode]);
  const dispatchTimer = useCallback((action: "start" | "pause" | "reset") => {
    const nextTimer = applyTimerCommand(timerState, action, getMonotonicNowMs());
    setTimerState(nextTimer);
    broadcastSyncFullState({ timerState: nextTimer, bumpRevision: true });
  }, [broadcastSyncFullState, setTimerState, timerState]);
  const navigateSlide = useCallback((nextIdx: number) => {
    if (params.slides.length <= 0) return;
    const clamped = Math.max(0, Math.min(params.slides.length - 1, nextIdx));
    params.onChangeSlide(clamped);
    params.onPostDeckKeyNav?.("goto", clamped);
    broadcastSyncFullState({ activeSlideIndex: clamped, bumpRevision: true });
  }, [broadcastSyncFullState, params]);
  return { setAndForwardBlackout, dispatchTimer, navigateSlide };
}

function useHostStateStore() {
  const [connectionState, setRawConnectionState] = useState<PresenterConnectionState>("CLOSED");
  const setConnectionState = useCallback((next: PresenterConnectionState) => {
    setRawConnectionState((prev) => transitionPresenterConnectionState(prev, next));
  }, []);
  const [launchGeneration, setLaunchGeneration] = useState(0);
  const [transportMode, setTransportMode] = useState<PresenterTransportMode>("broadcast_channel");
  const [blackoutMode, setBlackoutMode] = useState<PresenterBlackoutMode>("none");
  const [timerState, setTimerState] = useState<PresenterTimerState>(createInitialTimerState);
  const [syncLatencyMs, setSyncLatencyMs] = useState<number | null>(null);
  const [isPopupBlockedToastVisible, setIsPopupBlockedToastVisible] = useState(false);
  const [serverSlides, setServerSlides] = useState<PresenterSlideSnapshotDto[] | null>(null);
  const [serverTitle, setServerTitle] = useState<string | null>(null);
  const [serverReadOnly, setServerReadOnly] = useState<boolean | null>(null);
  const [apiError, setApiError] = useState<PresenterApiErrorState | null>(null);
  return {
    connectionState, setConnectionState, launchGeneration, setLaunchGeneration, transportMode, setTransportMode,
    blackoutMode, setBlackoutMode, timerState, setTimerState, syncLatencyMs, setSyncLatencyMs,
    isPopupBlockedToastVisible, setIsPopupBlockedToastVisible, serverSlides, setServerSlides,
    serverTitle, setServerTitle, serverReadOnly, setServerReadOnly, apiError, setApiError,
  };
}

function useBroadcastSyncSender(opts: {
  effectiveDeckId: string;
  params: UsePresenterSessionControllerParams;
  safeSeqRef: React.MutableRefObject<HostSequencerState>;
  presenterSlides: PresenterSlideSnapshotDto[];
  serverTitle: string | null;
  blackoutMode: PresenterBlackoutMode;
  effectiveReadOnly: boolean;
  snapshotTag: string | null;
  timerState: PresenterTimerState;
}) {
  const latestRef = useRef(opts);
  latestRef.current = opts;
  return useCallback(
    (override?: { activeSlideIndex?: number; blackoutMode?: PresenterBlackoutMode; timerState?: PresenterTimerState; bumpRevision?: boolean }) => {
      const cur = latestRef.current;
      const seqState = cur.safeSeqRef.current;
      if (!seqState.transport) return;
      if (override?.bumpRevision) seqState.stateRevision += 1;
      const idx = override?.activeSlideIndex ?? cur.params.activeSlideIndex;
      const curSlide = cur.params.slides[idx] || cur.params.slides[0];
      const nxtSlide = cur.params.slides[idx + 1];
      seqState.transport.send(buildSyncFullStateEnvelope({
        seqState, deckId: cur.effectiveDeckId, deckTitle: cur.serverTitle || cur.params.deckTitle || "Presentation Deck",
        aspectRatio: normalizeDeckAspectRatio(cur.params.aspectRatio), revision: cur.params.deckRevision ?? 1,
        activeSlideIndex: idx, slides: cur.presenterSlides, blackoutMode: override?.blackoutMode ?? cur.blackoutMode,
        isAudienceFullscreen: cur.params.isAudienceFullscreen, isHistoricalReadOnly: cur.effectiveReadOnly, snapshotTag: cur.snapshotTag,
        timerState: override?.timerState ?? cur.timerState,
        currentSlideHtml: buildPresenterSlideHtml(curSlide, curSlide ? cur.params.customHtmlBySlideId?.[curSlide.id] : undefined),
        nextSlideHtml: nxtSlide ? buildPresenterSlideHtml(nxtSlide, cur.params.customHtmlBySlideId?.[nxtSlide.id]) : null,
      }));
    },
    []
  );
}

function useHostLifecycleControls(opts: {
  effectiveDeckId: string;
  safeSeqRef: React.MutableRefObject<HostSequencerState>;
  transportMode: PresenterTransportMode;
  effectiveReadOnly: boolean;
  connectionState: PresenterConnectionState;
  params: UsePresenterSessionControllerParams;
  setConnectionState: (s: PresenterConnectionState) => void;
  setIsPopupBlockedToastVisible: (v: boolean) => void;
  broadcastSyncFullState: (o?: { bumpRevision?: boolean }) => void;
}) {
  const { connectionState, params, broadcastSyncFullState, safeSeqRef, setConnectionState, setIsPopupBlockedToastVisible, effectiveDeckId, effectiveReadOnly, transportMode } = opts;
  const wasAuthRef = useRef(false);
  const notesSignature = useMemo(() => JSON.stringify(params.speakerNotesBySlideId ?? null), [params.speakerNotesBySlideId]);
  const prevStateSigRef = useRef<string>("");
  useEffect(() => {
    const isAuth = connectionState === "AUTHENTICATED";
    const sig = `${params.activeSlideIndex}|${params.isAudienceFullscreen}|${notesSignature}`;
    if (isAuth && wasAuthRef.current && prevStateSigRef.current !== sig) broadcastSyncFullState({ bumpRevision: true });
    wasAuthRef.current = isAuth;
    prevStateSigRef.current = sig;
  }, [connectionState, params.activeSlideIndex, params.isAudienceFullscreen, notesSignature, broadcastSyncFullState]);
  useEffect(() => () => { safeSeqRef.current.transport?.close(); safeSeqRef.current.transport = null; }, [safeSeqRef]);
  const closePresenterSession = useCallback(() => {
    rotateHostSessionIdentity(safeSeqRef.current);
    setConnectionState("CLOSED");
    setIsPopupBlockedToastVisible(false);
  }, [safeSeqRef, setConnectionState, setIsPopupBlockedToastVisible]);
  const guardPresenterMutation = useCallback(
    <TArgs extends unknown[]>(fn: (...args: TArgs) => void) => (...args: TArgs) => {
      if (effectiveReadOnly) {
        emitPresenterTelemetry({ metric: "presenter.channel_rejected.count", value: 1, deckId: effectiveDeckId, sessionId: safeSeqRef.current.sessionId, transportMode, rejectReason: "readonly_snap_lock" });
        return;
      }
      fn(...args);
    },
    [effectiveDeckId, effectiveReadOnly, safeSeqRef, transportMode]
  );
  return { closePresenterSession, guardPresenterMutation };
}

export function usePresenterSessionController(params: UsePresenterSessionControllerParams) {
  const effectiveDeckId = params.deckId?.trim() || "local-deck";
  const snapshotTag = params.snapshotTag ?? null;
  const st = useHostStateStore();
  const seqRef = useRef<HostSequencerState | null>(null);
  if (!seqRef.current) seqRef.current = createInitialHostSequencerState();
  const safeSeqRef = seqRef as React.MutableRefObject<HostSequencerState>;
  const effectiveReadOnly = Boolean(params.isHistoricalReadOnly || st.serverReadOnly);
  const presenterSlides = useMergedPresenterSlides(params.slides, params.speakerNotesBySlideId, st.serverSlides);
  const broadcastSyncFullState = useBroadcastSyncSender({ effectiveDeckId, params, safeSeqRef, presenterSlides, serverTitle: st.serverTitle, blackoutMode: st.blackoutMode, effectiveReadOnly, snapshotTag, timerState: st.timerState });
  const fetchServerSnapshot = useHostPresenterApiSync({ projectId: params.projectId, deckId: params.deckId, activeSlideIndex: params.activeSlideIndex, snapshotTag, setServerSlides: st.setServerSlides, setServerTitle: st.setServerTitle, setServerReadOnly: st.setServerReadOnly, setApiError: st.setApiError });
  const launchPresenterSync = useHostPopupLauncher({ effectiveDeckId, safeSeqRef, setConnectionState: st.setConnectionState, setTransportMode: st.setTransportMode, setIsPopupBlockedToastVisible: st.setIsPopupBlockedToastVisible, bumpLaunchGeneration: () => st.setLaunchGeneration((n) => n + 1), fetchServerSnapshot });
  const actions = useHostSlideAndTimerActions({ params, timerState: st.timerState, setBlackoutMode: st.setBlackoutMode, setTimerState: st.setTimerState, broadcastSyncFullState });
  const lifecycle = useHostLifecycleControls({ effectiveDeckId, safeSeqRef, transportMode: st.transportMode, effectiveReadOnly, connectionState: st.connectionState, params, setConnectionState: st.setConnectionState, setIsPopupBlockedToastVisible: st.setIsPopupBlockedToastVisible, broadcastSyncFullState });
  const handleBroadcastCommand = useCallback((ev: PresenterBroadcastEvent) => processHostBroadcastCommand({ ev, seqState: safeSeqRef.current, effectiveDeckId, transportMode: st.transportMode, activeSlideIndex: params.activeSlideIndex, isHistoricalReadOnly: effectiveReadOnly, setConnectionState: st.setConnectionState, broadcastSyncFullState, navigateSlide: actions.navigateSlide, setBlackout: actions.setAndForwardBlackout, dispatchTimer: actions.dispatchTimer, onToggleAudienceFullscreen: params.onToggleAudienceFullscreen, onDisconnect: lifecycle.closePresenterSession }), [actions, broadcastSyncFullState, effectiveDeckId, effectiveReadOnly, lifecycle.closePresenterSession, params.activeSlideIndex, params.onToggleAudienceFullscreen, safeSeqRef, st]);
  const handleAuthenticated = useCallback(() => {
    const latency = Math.max(0, Math.round(getMonotonicNowMs() - safeSeqRef.current.openedAtPerfMs));
    st.setSyncLatencyMs(latency);
    st.setConnectionState("AUTHENTICATED");
    emitPresenterTelemetry({ metric: "presenter.sync.latency_ms", value: latency, deckId: effectiveDeckId, sessionId: safeSeqRef.current.sessionId, transportMode: st.transportMode });
    broadcastSyncFullState();
  }, [broadcastSyncFullState, effectiveDeckId, safeSeqRef, st]);
  useHostSyncEffects({ effectiveDeckId, connectionState: st.connectionState, launchGeneration: st.launchGeneration, transportMode: st.transportMode, seqRef: safeSeqRef, isHistoricalReadOnly: effectiveReadOnly, snapshotTag, onAuthenticated: handleAuthenticated, onBroadcastCommand: handleBroadcastCommand, onDisconnect: lifecycle.closePresenterSession });
  const activeSlide = params.slides[params.activeSlideIndex] || params.slides[0];
  const nextSlide = params.slides[params.activeSlideIndex + 1];
  return {
    connectionState: st.connectionState, transportMode: st.transportMode, blackoutMode: st.blackoutMode, timerState: st.timerState,
    elapsedMs: useHostStopwatchTick(st.timerState), syncLatencyMs: st.syncLatencyMs, apiError: st.apiError, clearApiError: () => st.setApiError(null),
    isInlineFallbackOpen: st.connectionState === "POPUP_BLOCKED_INLINE_FALLBACK",
    isPresenterActive: st.connectionState === "AUTHENTICATED" || st.connectionState === "POPUP_BLOCKED_INLINE_FALLBACK",
    isPopupBlockedToastVisible: st.isPopupBlockedToastVisible, dismissPopupBlockedToast: () => st.setIsPopupBlockedToastVisible(false),
    isHistoricalReadOnly: effectiveReadOnly, snapshotTag, presenterSlides,
    currentSlideSnapshot: presenterSlides[params.activeSlideIndex] || presenterSlides[0] || null,
    nextSlideSnapshot: presenterSlides[params.activeSlideIndex + 1] || null,
    currentSlideHtml: buildPresenterSlideHtml(activeSlide, activeSlide ? params.customHtmlBySlideId?.[activeSlide.id] : undefined),
    nextSlideHtml: nextSlide ? buildPresenterSlideHtml(nextSlide, params.customHtmlBySlideId?.[nextSlide.id]) : null,
    openPresenterView: launchPresenterSync, reopenPresenterWindow: launchPresenterSync,
    closePresenterSession: lifecycle.closePresenterSession, setBlackoutMode: actions.setAndForwardBlackout,
    toggleBlackoutKey: (mode: "black" | "white") => actions.setAndForwardBlackout(st.blackoutMode === mode ? "none" : mode),
    dispatchTimer: actions.dispatchTimer, navigateSlide: actions.navigateSlide, guardPresenterMutation: lifecycle.guardPresenterMutation,
  };
}
