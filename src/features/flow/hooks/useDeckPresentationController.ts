// @story 06.1
// @implements AC-03, AC-05
import type React from "react";
import { useCallback, useEffect, useRef, useState } from "react";

const IME_COMPOSING_KEY_CODE = 229;
const DECK_AUTOPLAY_INTERVAL_MS = 2500;

export interface UseDeckPresentationControllerParams {
  slidesCount: number;
  activeSlideIndex: number;
  onChangeSlide: (nextIndex: number) => void;
  onExitWalkthrough?: () => void;
  isPresenting: boolean;
  setIsPresenting: React.Dispatch<React.SetStateAction<boolean>>;
  containerRef?: React.RefObject<HTMLElement | null>;
  onPostDeckKeyNav?: (direction: "prev" | "next" | "goto", targetIndex?: number) => void;
  isPresenterActive?: boolean;
  onToggleBlackoutKey?: (mode: "black" | "white", trigger: "B" | "W") => void;
}

export function shouldIgnorePresentationKeyEvent(ev: {
  isComposing?: boolean;
  keyCode?: number;
  metaKey?: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
  target?: EventTarget | null;
}): boolean {
  if (ev.isComposing || ev.keyCode === IME_COMPOSING_KEY_CODE) return true;
  if (ev.metaKey || ev.ctrlKey || ev.altKey) return true;
  const el = ev.target as HTMLElement | null;
  const tag = el?.tagName?.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || Boolean(el?.isContentEditable);
}

function useFullscreenEscapeSync(
  isPresentingRef: React.MutableRefObject<boolean>,
  setIsPresenting: React.Dispatch<React.SetStateAction<boolean>>
) {
  useEffect(() => {
    if (typeof document === "undefined") return;
    const onFullscreenChange = () => {
      if (!document.fullscreenElement && isPresentingRef.current) setIsPresenting(false);
    };
    document.addEventListener("fullscreenchange", onFullscreenChange);
    return () => document.removeEventListener("fullscreenchange", onFullscreenChange);
  }, [isPresentingRef, setIsPresenting]);
}

function dispatchPresentationKey(
  ev: KeyboardEvent,
  handlers: {
    slidesTotal: number;
    isPresenting: boolean;
    exitPresent: () => void;
    togglePresent: () => void;
    nextSlide: () => void;
    prevSlide: () => void;
    goToSlide: (idx: number) => void;
    onToggleBlackoutKey?: (mode: "black" | "white", trigger: "B" | "W") => void;
  }
) {
  if (shouldIgnorePresentationKeyEvent(ev)) return;
  if (ev.key === "Escape") {
    if (handlers.isPresenting) { ev.preventDefault(); handlers.exitPresent(); }
    return;
  }
  if (ev.key === "b" || ev.key === "B") { ev.preventDefault(); handlers.onToggleBlackoutKey?.("black", "B"); return; }
  if (ev.key === "w" || ev.key === "W") { ev.preventDefault(); handlers.onToggleBlackoutKey?.("white", "W"); return; }
  if (ev.key === "f" || ev.key === "F") { ev.preventDefault(); handlers.togglePresent(); return; }
  if (ev.key === "ArrowRight" || ev.key === "PageDown" || ev.key === " ") { ev.preventDefault(); handlers.nextSlide(); return; }
  if (ev.key === "ArrowLeft" || ev.key === "PageUp") { ev.preventDefault(); handlers.prevSlide(); return; }
  if (ev.key === "Home") { ev.preventDefault(); handlers.goToSlide(0); return; }
  if (ev.key === "End" && handlers.slidesTotal > 0) { ev.preventDefault(); handlers.goToSlide(handlers.slidesTotal - 1); }
}

function useDeckKeyboardNavigation(opts: {
  isPresenting: boolean;
  isPresenterActive: boolean;
  slidesCountRef: React.MutableRefObject<number>;
  exitPresent: () => void;
  togglePresent: () => void;
  nextSlide: () => void;
  prevSlide: () => void;
  goToSlide: (idx: number) => void;
  onToggleBlackoutKey?: (mode: "black" | "white", trigger: "B" | "W") => void;
}) {
  const { isPresenting, isPresenterActive, slidesCountRef, exitPresent, togglePresent, nextSlide, prevSlide, goToSlide, onToggleBlackoutKey } = opts;
  useEffect(() => {
    if ((!isPresenting && !isPresenterActive) || typeof window === "undefined") return;
    const handleKey = (ev: KeyboardEvent) =>
      dispatchPresentationKey(ev, {
        slidesTotal: slidesCountRef.current,
        isPresenting,
        exitPresent,
        togglePresent,
        nextSlide,
        prevSlide,
        goToSlide,
        onToggleBlackoutKey,
      });
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [exitPresent, goToSlide, isPresenterActive, isPresenting, nextSlide, onToggleBlackoutKey, prevSlide, slidesCountRef, togglePresent]);
}

function usePresentationLifecycleActions(opts: {
  containerRef?: React.RefObject<HTMLElement | null>;
  onExitWalkthrough?: () => void;
  setIsPresenting: React.Dispatch<React.SetStateAction<boolean>>;
  isPresentingRef: React.MutableRefObject<boolean>;
}) {
  const { containerRef, onExitWalkthrough, setIsPresenting, isPresentingRef } = opts;
  const exitPresent = useCallback(() => {
    setIsPresenting(false);
    if (typeof document === "undefined" || !document.fullscreenElement) return;
    try { void document.exitFullscreen?.(); } catch { /* ignore sandbox exit rejection */ }
  }, [setIsPresenting]);

  const enterPresent = useCallback(() => {
    onExitWalkthrough?.();
    setIsPresenting(true);
    const targetEl = containerRef?.current;
    if (!targetEl || typeof targetEl.requestFullscreen !== "function") return;
    try { void Promise.resolve(targetEl.requestFullscreen()).catch(() => {}); } catch { /* ignore headless */ }
  }, [containerRef, onExitWalkthrough, setIsPresenting]);

  const togglePresent = useCallback(() => {
    if (isPresentingRef.current) exitPresent();
    else enterPresent();
  }, [enterPresent, exitPresent, isPresentingRef]);

  return { enterPresent, exitPresent, togglePresent };
}

function useDeckAutoPlayTimer(opts: {
  isPresenting: boolean;
  autoPlay: boolean;
  slidesCountRef: React.MutableRefObject<number>;
  activeIdxRef: React.MutableRefObject<number>;
  goToSlide: (idx: number) => void;
}) {
  const { isPresenting, autoPlay, slidesCountRef, activeIdxRef, goToSlide } = opts;
  useEffect(() => {
    if (!isPresenting || !autoPlay) return;
    const timer = setInterval(() => {
      const total = slidesCountRef.current;
      if (total <= 1) return;
      goToSlide((activeIdxRef.current + 1) % total);
    }, DECK_AUTOPLAY_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [activeIdxRef, autoPlay, goToSlide, isPresenting, slidesCountRef]);
}

export function useDeckPresentationController(params: UseDeckPresentationControllerParams) {
  const { slidesCount, activeSlideIndex, onChangeSlide, onExitWalkthrough, isPresenting, setIsPresenting, containerRef, onPostDeckKeyNav, isPresenterActive = false, onToggleBlackoutKey } = params;
  const [protocolError, setProtocolError] = useState<string | null>(null);
  const [autoPlay, setAutoPlay] = useState(false);
  const activeIdxRef = useRef(activeSlideIndex);
  const slidesCountRef = useRef(slidesCount);
  const isPresentingRef = useRef(isPresenting);
  activeIdxRef.current = activeSlideIndex;
  slidesCountRef.current = slidesCount;
  isPresentingRef.current = isPresenting;

  const { enterPresent, exitPresent, togglePresent } = usePresentationLifecycleActions({ containerRef, onExitWalkthrough, setIsPresenting, isPresentingRef });
  const goToSlide = useCallback((rawIndex: number) => {
    const total = slidesCountRef.current;
    if (total <= 0) return;
    const clamped = Math.max(0, Math.min(total - 1, rawIndex));
    activeIdxRef.current = clamped;
    onChangeSlide(clamped);
    onPostDeckKeyNav?.("goto", clamped);
  }, [onChangeSlide, onPostDeckKeyNav]);

  const stopAutoAndGo = useCallback((delta: number) => { setAutoPlay(false); goToSlide(activeIdxRef.current + delta); }, [goToSlide]);
  const nextSlide = useCallback(() => stopAutoAndGo(1), [stopAutoAndGo]);
  const prevSlide = useCallback(() => stopAutoAndGo(-1), [stopAutoAndGo]);
  const toggleAutoPlay = useCallback(() => setAutoPlay((v) => !v), []);
  const handleExitPresent = useCallback(() => { setAutoPlay(false); exitPresent(); }, [exitPresent]);
  useEffect(() => { if (!isPresenting) setAutoPlay(false); }, [isPresenting]);

  useDeckAutoPlayTimer({ isPresenting, autoPlay, slidesCountRef, activeIdxRef, goToSlide });
  useFullscreenEscapeSync(isPresentingRef, setIsPresenting);
  useDeckKeyboardNavigation({ isPresenting, isPresenterActive, slidesCountRef, exitPresent: handleExitPresent, togglePresent, nextSlide, prevSlide, goToSlide, onToggleBlackoutKey });

  return { isDeckPresenting: isPresenting, autoPlay, toggleAutoPlay, enterPresent, exitPresent: handleExitPresent, togglePresent, nextSlide, prevSlide, goToSlide, protocolError, setProtocolError };
}
