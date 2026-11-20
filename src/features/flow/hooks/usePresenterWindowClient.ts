// @story 06.4
// @implements AC-01, AC-02, AC-04
import { useCallback, useEffect, useRef, useState } from "react";
import type {
  PresenterBlackoutMode,
  PresenterBroadcastEvent,
  PresenterTimerSnapshot,
} from "../../../lib/validations/presentation-presenter";
import { generatePresenterUuid, resolvePresenterTransportMode } from "../utils/presenter-broadcast";
import { computeClientDisplayMs, getMonotonicNowMs } from "../utils/presenter-timer";
import { shouldIgnorePresentationKeyEvent } from "./useDeckPresentationController";
import {
  usePresenterClientTransport,
  type ClientTransportRefState,
  type PresenterClientSyncedState,
} from "./usePresenterClientTransport";

export type { PresenterClientSyncedState };

const CLIENT_TIMER_TICK_MS = 250;
const MIN_NOTES_FONT_PX = 15;
const MAX_NOTES_FONT_PX = 26;
const DEFAULT_NOTES_FONT_PX = 18;
const MIN_SESSION_ID_LEN = 16;
const NOTES_FONT_STORAGE_KEY = "cowok:presenter:notes-font-px";

function readStoredNotesFontPx(): number {
  if (typeof window === "undefined") return DEFAULT_NOTES_FONT_PX;
  try {
    const raw = Number(window.sessionStorage?.getItem(NOTES_FONT_STORAGE_KEY) || DEFAULT_NOTES_FONT_PX);
    if (Number.isFinite(raw) && raw >= MIN_NOTES_FONT_PX && raw <= MAX_NOTES_FONT_PX) return raw;
  } catch { /* ignore */ }
  return DEFAULT_NOTES_FONT_PX;
}

function readNonSecretSessionId(): string {
  if (typeof window === "undefined") return "local-presenter-session";
  const raw = new URLSearchParams(window.location.search).get("sessionId")?.trim() || "";
  return raw.length >= MIN_SESSION_ID_LEN ? raw : "local-presenter-session";
}

function usePresenterClientTimer(timer: PresenterTimerSnapshot | null, receivedPerfMs: number): number {
  const [displayMs, setDisplayMs] = useState(0);
  useEffect(() => {
    if (!timer) { setDisplayMs(0); return undefined; }
    setDisplayMs(computeClientDisplayMs(timer, receivedPerfMs));
    if (!timer.isRunning || typeof window === "undefined") return undefined;
    const id = window.setInterval(() => setDisplayMs(computeClientDisplayMs(timer, receivedPerfMs)), CLIENT_TIMER_TICK_MS);
    return () => window.clearInterval(id);
  }, [receivedPerfMs, timer]);
  return displayMs;
}

function dispatchPresenterWindowKey(opts: {
  ev: KeyboardEvent;
  isShortcutsOpen: boolean;
  closeShortcuts: () => void;
  closeWindow: (reason: "escape_key") => void;
  sendNav: (dir: "prev" | "next") => void;
  toggleBlackout: (mode: "black" | "white", trigger: "B" | "W") => void;
  toggleFullscreen: () => void;
  toggleTimer: () => void;
}) {
  if (shouldIgnorePresentationKeyEvent(opts.ev)) return;
  if (opts.ev.key === "Escape") {
    opts.ev.preventDefault();
    if (opts.isShortcutsOpen) opts.closeShortcuts();
    else opts.closeWindow("escape_key");
    return;
  }
  if (opts.ev.key === "ArrowRight" || opts.ev.key === "PageDown" || opts.ev.key === " ") { opts.ev.preventDefault(); opts.sendNav("next"); return; }
  if (opts.ev.key === "ArrowLeft" || opts.ev.key === "PageUp") { opts.ev.preventDefault(); opts.sendNav("prev"); return; }
  if (opts.ev.key === "b" || opts.ev.key === "B") { opts.ev.preventDefault(); opts.toggleBlackout("black", "B"); return; }
  if (opts.ev.key === "w" || opts.ev.key === "W") { opts.ev.preventDefault(); opts.toggleBlackout("white", "W"); return; }
  if (opts.ev.key === "f" || opts.ev.key === "F") { opts.ev.preventDefault(); opts.toggleFullscreen(); return; }
  if (opts.ev.key === "p" || opts.ev.key === "P") { opts.ev.preventDefault(); opts.toggleTimer(); }
}

function usePresenterSender(deckId: string, safeRef: React.MutableRefObject<ClientTransportRefState>) {
  return useCallback(
    (buildPayload: (base: Omit<PresenterBroadcastEvent, "type">) => PresenterBroadcastEvent) => {
      const st = safeRef.current;
      if (!st.transport || !st.sessionSecret || typeof window === "undefined") return;
      st.localSeq += 1;
      st.transport.send(buildPayload({
        deckId, sessionId: st.sessionId, sessionSecret: st.sessionSecret,
        senderOrigin: window.location.origin, senderRole: "presenter", senderInstanceId: st.senderInstanceId,
        epoch: st.epoch, stateRevision: Math.max(0, st.localStateRevision), seq: st.localSeq,
        commandId: generatePresenterUuid(), baseRevision: Math.max(0, st.localStateRevision), sentAtMs: Date.now(),
      }));
    },
    [deckId, safeRef]
  );
}

function resolveNavTargetIndex(direction: "prev" | "next" | "goto", curIdx: number, targetIndex?: number): number {
  if (typeof targetIndex === "number") return targetIndex;
  if (direction === "next") return curIdx + 1;
  if (direction === "prev") return Math.max(0, curIdx - 1);
  return curIdx;
}

function usePresenterClientCommands(deckId: string, safeRef: React.MutableRefObject<ClientTransportRefState>, syncedState: PresenterClientSyncedState | null) {
  const sendPresenterEvent = usePresenterSender(deckId, safeRef);
  const handleCloseWindow = useCallback((reason: "escape_key" | "close_button" | "beforeunload") => {
    safeRef.current.closingSent = true;
    sendPresenterEvent((base) => ({ ...base, type: "PRESENTER_WINDOW_CLOSING", reason }));
    safeRef.current.transport?.close();
    safeRef.current.transport = null;
    if (reason !== "beforeunload" && typeof window !== "undefined") {
      try { window.close(); } catch { /* ignore headless */ }
    }
  }, [safeRef, sendPresenterEvent]);
  const sendNav = useCallback((direction: "prev" | "next" | "goto", targetIndex?: number) => {
    const idx = resolveNavTargetIndex(direction, syncedState?.activeSlideIndex ?? 0, targetIndex);
    sendPresenterEvent((base) => ({ ...base, type: "SLIDE_NAV", direction, targetIndex: idx }));
  }, [sendPresenterEvent, syncedState?.activeSlideIndex]);
  const toggleBlackout = useCallback((mode: "black" | "white", trigger: "B" | "W" | "button") => {
    const nextMode: PresenterBlackoutMode = syncedState?.blackoutMode === mode ? "none" : mode;
    sendPresenterEvent((base) => ({ ...base, type: "BLACKOUT_TOGGLE", mode: nextMode, keyTrigger: trigger }));
  }, [sendPresenterEvent, syncedState?.blackoutMode]);
  const sendTimer = useCallback((action: "start" | "pause" | "reset") => {
    sendPresenterEvent((base) => ({ ...base, type: "TIMER_COMMAND", action }));
  }, [sendPresenterEvent]);
  return { sendPresenterEvent, handleCloseWindow, sendNav, toggleBlackout, sendTimer };
}

function usePresenterWindowKeySync(opts: {
  isShortcutsOpen: boolean;
  setIsShortcutsOpen: (v: boolean) => void;
  cmds: ReturnType<typeof usePresenterClientCommands>;
  isTimerRunning: boolean;
  toggleFullscreen: () => void;
}) {
  const { isShortcutsOpen, setIsShortcutsOpen, cmds, isTimerRunning, toggleFullscreen } = opts;
  useEffect(() => {
    if (typeof window === "undefined") return undefined;
    const onKeyDown = (ev: KeyboardEvent) => dispatchPresenterWindowKey({
      ev, isShortcutsOpen, closeShortcuts: () => setIsShortcutsOpen(false),
      closeWindow: cmds.handleCloseWindow, sendNav: cmds.sendNav, toggleBlackout: cmds.toggleBlackout,
      toggleFullscreen, toggleTimer: () => cmds.sendTimer(isTimerRunning ? "pause" : "start"),
    });
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [cmds, isShortcutsOpen, isTimerRunning, setIsShortcutsOpen, toggleFullscreen]);
}

export function usePresenterWindowClient(deckId: string) {
  const [syncedState, setSyncedState] = useState<PresenterClientSyncedState | null>(null);
  const [timerAnchorPerfMs, setTimerAnchorPerfMs] = useState<number>(getMonotonicNowMs);
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [notesFontPx, setNotesFontPx] = useState<number>(readStoredNotesFontPx);
  const [isShortcutsOpen, setIsShortcutsOpen] = useState(false);
  const [syncLatencyMs, setSyncLatencyMs] = useState<number | null>(null);
  const clientRef = useRef<ClientTransportRefState | null>(null);
  if (!clientRef.current) {
    clientRef.current = {
      senderInstanceId: generatePresenterUuid(), windowNonce: generatePresenterUuid(),
      sessionId: readNonSecretSessionId(), sessionSecret: null,
      hostSenderInstanceId: null, processedCommandKeys: [], closingSent: false,
      epoch: 1, localStateRevision: -1, localSeq: 0, lastAcceptedSeqByInstance: new Map(), transport: null,
    };
  }
  const safeRef = clientRef as React.MutableRefObject<ClientTransportRefState>;
  const transportMode = resolvePresenterTransportMode();
  const cmds = usePresenterClientCommands(deckId, safeRef, syncedState);
  usePresenterClientTransport({ deckId, transportMode, safeRef, sendPresenterEvent: cmds.sendPresenterEvent, setIsAuthenticated, setSyncLatencyMs, setTimerAnchorPerfMs, setSyncedState });
  const toggleFullscreen = useCallback(() => cmds.sendPresenterEvent((base) => ({ ...base, type: "FULLSCREEN_TOGGLE", requestedState: "toggle" })), [cmds]);
  usePresenterWindowKeySync({ isShortcutsOpen, setIsShortcutsOpen, cmds, isTimerRunning: Boolean(syncedState?.timer.isRunning), toggleFullscreen });
  const stepNotesFontPx = useCallback((delta: number) => {
    setNotesFontPx((prev) => {
      const next = Math.max(MIN_NOTES_FONT_PX, Math.min(MAX_NOTES_FONT_PX, prev + delta));
      try { window.sessionStorage?.setItem(NOTES_FONT_STORAGE_KEY, String(next)); } catch { /* ignore */ }
      return next;
    });
  }, []);
  return {
    isAuthenticated, syncedState, elapsedMs: usePresenterClientTimer(syncedState?.timer ?? null, timerAnchorPerfMs),
    syncLatencyMs, transportMode, notesFontPx, stepNotesFontPx, isShortcutsOpen, setIsShortcutsOpen,
    sendNav: cmds.sendNav, toggleBlackout: cmds.toggleBlackout,
    clearBlackout: () => cmds.sendPresenterEvent((base) => ({ ...base, type: "BLACKOUT_TOGGLE", mode: "none", keyTrigger: "reset" })),
    sendTimer: cmds.sendTimer, toggleFullscreen, closeWindow: () => cmds.handleCloseWindow("close_button"),
  };
}
