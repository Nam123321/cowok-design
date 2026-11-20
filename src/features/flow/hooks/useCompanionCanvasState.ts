// @story-04.1, 04.5
// @implements AC-01, AC-03
import React, { createContext, useState, useEffect, useCallback, useRef } from "react";
import { DEFAULT_TABS, INITIAL_SLIDES, SlidePhaseItem, createCanvasTabByKind } from "../components/companion-canvas-data";
import type { CanvasTabItem } from "../components/CompanionCanvasTab";
import type { FixtureVariant, ViewportPreset } from "../../../lib/validations/screen-fixture";

export const ReadOnlyCanvasContext = createContext<boolean>(false);

export const DEFAULT_LEFT_PANE_PCT = 38;
export const MIN_LEFT_PANE_PCT = 26;
export const MAX_LEFT_PANE_PCT = 72;
export const PERCENT_MULTIPLIER = 100;
export const DEFAULT_INLINE_CODE = `export const CheckoutScreenConfig = {\n  screenId: "SCR-02",\n  headline: "Xác thực Ủy thác Giao dịch Kho bạc Đa Chữ Ký",\n  subtitle: "Tối ưu hóa xác thực khóa cứng WebAuthn Passkey ngay trên một mặt phẳng duy nhất.",\n  ctaLabel: "Ký & Giải ngân Ngay"\n};`;

export function useInternalSplitResizer(
  isDraggingInternalSplit: boolean,
  setIsDraggingInternalSplit: React.Dispatch<React.SetStateAction<boolean>>,
  splitContainerRef: React.RefObject<HTMLDivElement | null>,
  setLeftPanePercent: React.Dispatch<React.SetStateAction<number>>
) {
  useEffect(() => {
    if (!isDraggingInternalSplit) return;
    const handleMove = (e: MouseEvent) => {
      if (!splitContainerRef.current) return;
      const rect = splitContainerRef.current.getBoundingClientRect();
      if (rect.width <= 0) return;
      const pct = ((e.clientX - rect.left) / rect.width) * PERCENT_MULTIPLIER;
      setLeftPanePercent(Math.max(MIN_LEFT_PANE_PCT, Math.min(MAX_LEFT_PANE_PCT, Math.round(pct))));
    };
    const handleUp = () => setIsDraggingInternalSplit(false);
    window.addEventListener("mousemove", handleMove);
    window.addEventListener("mouseup", handleUp);
    return () => { window.removeEventListener("mousemove", handleMove); window.removeEventListener("mouseup", handleUp); };
  }, [isDraggingInternalSplit, setIsDraggingInternalSplit, splitContainerRef, setLeftPanePercent]);
}

export function useTabNavigationState(propActiveTabId?: string, onTabChange?: (tabId: string, format: CanvasTabItem["format"]) => void) {
  const [tabs, setTabs] = useState<CanvasTabItem[]>(DEFAULT_TABS);
  const [internalActiveTabId, setInternalActiveTabId] = useState<string>("tab-userflow");
  const [isNewTabMenuOpen, setIsNewTabMenuOpen] = useState(false);
  const activeTab = tabs.find((item) => item.id === (propActiveTabId || internalActiveTabId)) || tabs[0];
  const handleTabSelect = (tabId: string, format: CanvasTabItem["format"]) => { setInternalActiveTabId(tabId); onTabChange?.(tabId, format); };
  const handleCreateTab = (kind: "app-flow" | "slide-deck" | "md" | "assets" | "code", customTitle?: string) => {
    const newTab = createCanvasTabByKind(kind, tabs.length, customTitle);
    setTabs((prev) => [...prev, newTab]);
    setIsNewTabMenuOpen(false);
    handleTabSelect(newTab.id, newTab.format);
  };
  const handleClose = (e: React.MouseEvent, tabId: string) => {
    e.stopPropagation();
    if (tabs.length <= 1) return;
    const nextTabs = tabs.filter((item) => item.id !== tabId);
    setTabs(nextTabs);
    if (activeTab.id === tabId) handleTabSelect(nextTabs[0].id, nextTabs[0].format);
  };
  return { tabs, activeTab, isNewTabMenuOpen, setIsNewTabMenuOpen, handleTabSelect, handleCreateTab, handleClose };
}

export function useInspectHotkeyShortcut(setPointerMode: React.Dispatch<React.SetStateAction<"interact" | "click-dom" | "drag-box">>) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName?.toLowerCase();
      if (tag === "input" || tag === "textarea" || tag === "select" || target?.isContentEditable) return;
      if (e.key === "c" || e.key === "C") {
        setPointerMode((prev) => (prev === "interact" ? "click-dom" : "interact"));
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [setPointerMode]);
}

function useEditorBufferState() {
  const [isInlineCodeOpen, setIsInlineCodeOpen] = useState(false);
  const [inlineCodeText, setInlineCodeText] = useState<string>(DEFAULT_INLINE_CODE);
  const [htmlCodeText, setHtmlCodeText] = useState<string>("");
  const [baselineHtmlText, setBaselineHtmlText] = useState<string>("");
  const [showDirtyGuardPrompt, setShowDirtyGuardPrompt] = useState<boolean>(false);
  const [customHtmlByTarget, setCustomHtmlByTarget] = useState<Record<string, string>>({});
  const dirtyBuffer = isInlineCodeOpen && htmlCodeText !== baselineHtmlText;
  const setCustomHtmlForTarget = useCallback((k: string, h: string) => setCustomHtmlByTarget((prev) => ({ ...prev, [k]: h })), []);
  return { isInlineCodeOpen, setIsInlineCodeOpen, inlineCodeText, setInlineCodeText, htmlCodeText, setHtmlCodeText, baselineHtmlText, setBaselineHtmlText, dirtyBuffer, showDirtyGuardPrompt, setShowDirtyGuardPrompt, customHtmlByTarget, setCustomHtmlForTarget };
}

import type { StageWorkstreamMode } from "../utils/stage-capability";

export function useDualPaneViewportState(activeTab: CanvasTabItem, sessionKey?: string, sessionEntryKind?: StageWorkstreamMode) {
  const [structureLens, setStructureLens] = useState<"userflow" | "slide-cms">(
    sessionEntryKind === "presentation" || activeTab.format === "slide-cms" ? "slide-cms" : "userflow"
  );
  const prevLensRef = useRef<"userflow" | "slide-cms">(structureLens);
  const [leftPanePercent, setLeftPanePercent] = useState<number>(DEFAULT_LEFT_PANE_PCT);
  const [isDraggingInternalSplit, setIsDraggingInternalSplit] = useState(false);
  const [isLeftStructureHidden, setIsLeftStructureHidden] = useState(false);
  const [isRightPreviewHidden, setIsRightPreviewHidden] = useState(false);
  const [pointerMode, setPointerMode] = useState<"interact" | "click-dom" | "drag-box">("interact");
  const [viewportPreset, setViewportPreset] = useState<ViewportPreset>("desktop-1440");
  const [aspectRatio, setAspectRatio] = useState<"auto" | "16-9" | "16-10">(
    structureLens === "slide-cms" ? "16-9" : "auto"
  );
  const [zoomMode, setZoomMode] = useState<"fit" | "pan-100">("fit");
  const [isPresenting, setIsPresenting] = useState(false);
  const [highlightedRegionId, setHighlightedRegionId] = useState<string>("sec-biometric");
  const [slides, setSlides] = useState<SlidePhaseItem[]>(INITIAL_SLIDES);
  const [activeSlideUid, setActiveSlideUid] = useState<string>(INITIAL_SLIDES[0]?.id ?? "");
  const activeSlideIndex = Math.max(0, slides.findIndex((s) => s.id === activeSlideUid));
  const setActiveSlideIndex = useCallback(
    (next: number | ((prev: number) => number)) => {
      setActiveSlideUid((currUid) => {
        const currIdx = Math.max(0, slides.findIndex((s) => s.id === currUid));
        const targetIdx = typeof next === "function" ? next(currIdx) : next;
        const clamped = Math.max(0, Math.min(slides.length - 1, targetIdx));
        return slides[clamped]?.id ?? currUid;
      });
    },
    [slides]
  );
  const [isCmsDrawerOpen, setIsCmsDrawerOpen] = useState<boolean>(false);
  const [activeSubState, setActiveSubState] = useState<FixtureVariant>("DEFAULT");
  const editor = useEditorBufferState();

  const isSlideStudioActive = structureLens === "slide-cms" && activeTab.format === "slide-cms";
  useEffect(() => {
    const prev = prevLensRef.current;
    if (prev !== structureLens) {
      prevLensRef.current = structureLens;
      setIsPresenting(false);
    }
  }, [structureLens]);

  const wasSlideStudioActiveRef = useRef(isSlideStudioActive);
  useEffect(() => {
    const wasActive = wasSlideStudioActiveRef.current;
    wasSlideStudioActiveRef.current = isSlideStudioActive;
    if (isSlideStudioActive && !wasActive) {
      setAspectRatio("16-9");
      if (typeof window !== "undefined") {
        window.dispatchEvent(new CustomEvent("cowok:slide-studio-entered"));
      }
    } else if (!isSlideStudioActive && wasActive) {
      setAspectRatio("auto");
    }
  }, [isSlideStudioActive]);

  useInspectHotkeyShortcut(setPointerMode);
  const lastSyncContextRef = useRef({ tabId: activeTab.id, sessionKey: sessionKey || "", entryKind: sessionEntryKind });
  useEffect(() => {
    const prev = lastSyncContextRef.current;
    const nextKey = sessionKey || "";
    const tabChanged = prev.tabId !== activeTab.id;
    const entryKindChanged = prev.entryKind !== sessionEntryKind;
    const switchedBetweenSessions = Boolean(prev.sessionKey) && prev.sessionKey !== nextKey;
    lastSyncContextRef.current = { tabId: activeTab.id, sessionKey: nextKey, entryKind: sessionEntryKind };
    if (!tabChanged && !entryKindChanged && !switchedBetweenSessions) return;
    if (tabChanged) {
      if (activeTab.format === "slide-cms" || activeTab.defaultLens === "slide-cms") { setStructureLens("slide-cms"); setAspectRatio("16-9"); setIsPresenting(false); return; }
      if (activeTab.format === "dag" || activeTab.defaultLens === "userflow") { setStructureLens("userflow"); setAspectRatio("auto"); setIsPresenting(false); return; }
    }
    if (entryKindChanged) {
      if (sessionEntryKind === "presentation") { setStructureLens("slide-cms"); setAspectRatio("16-9"); setIsPresenting(false); return; }
      if (sessionEntryKind === "fe-dag") { setStructureLens("userflow"); setAspectRatio("auto"); setIsPresenting(false); return; }
    }
    if (activeTab.format === "slide-cms" || activeTab.defaultLens === "slide-cms") { setStructureLens("slide-cms"); setAspectRatio("16-9"); }
    else if (activeTab.format === "dag" || activeTab.defaultLens === "userflow") { setStructureLens("userflow"); setAspectRatio("auto"); }
  }, [activeTab.id, activeTab.format, activeTab.defaultLens, sessionKey, sessionEntryKind]);

  return {
    structureLens, setStructureLens, leftPanePercent, setLeftPanePercent,
    isDraggingInternalSplit, setIsDraggingInternalSplit, isLeftStructureHidden, setIsLeftStructureHidden,
    isRightPreviewHidden, setIsRightPreviewHidden, pointerMode, setPointerMode,
    viewportPreset, setViewportPreset, aspectRatio, setAspectRatio, zoomMode, setZoomMode, isPresenting, setIsPresenting,
    highlightedRegionId, setHighlightedRegionId, slides, setSlides,
    activeSlideUid, setActiveSlideUid, activeSlideIndex, setActiveSlideIndex, isCmsDrawerOpen, setIsCmsDrawerOpen,
    activeSubState, setActiveSubState, ...editor,
  };
}
