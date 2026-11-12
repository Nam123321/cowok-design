// @story-04.1
// @implements AC-3
import React, { useState, useEffect } from "react";
import {
  DEFAULT_TABS,
  INITIAL_SLIDES,
  SlidePhaseItem,
  createCanvasTabByKind,
} from "../components/companion-canvas-data";
import type { CanvasTabItem } from "../components/CompanionCanvasTab";

export const DEFAULT_LEFT_PANE_PCT = 48;
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
    return () => {
      window.removeEventListener("mousemove", handleMove);
      window.removeEventListener("mouseup", handleUp);
    };
  }, [isDraggingInternalSplit, setIsDraggingInternalSplit, splitContainerRef, setLeftPanePercent]);
}

export function useTabNavigationState(
  propActiveTabId?: string,
  onTabChange?: (tabId: string, format: CanvasTabItem["format"]) => void
) {
  const [tabs, setTabs] = useState<CanvasTabItem[]>(DEFAULT_TABS);
  const [internalActiveTabId, setInternalActiveTabId] = useState<string>("tab-userflow");
  const [isNewTabMenuOpen, setIsNewTabMenuOpen] = useState(false);
  const currentTabId = propActiveTabId || internalActiveTabId;
  const activeTab = tabs.find((item) => item.id === currentTabId) || tabs[0];

  const handleTabSelect = (tabId: string, format: CanvasTabItem["format"]) => {
    setInternalActiveTabId(tabId);
    onTabChange?.(tabId, format);
  };

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

export function useDualPaneViewportState(activeTab: CanvasTabItem) {
  const [structureLens, setStructureLens] = useState<"userflow" | "slide-cms">("userflow");
  const [leftPanePercent, setLeftPanePercent] = useState<number>(DEFAULT_LEFT_PANE_PCT);
  const [isDraggingInternalSplit, setIsDraggingInternalSplit] = useState(false);
  const [isLeftStructureHidden, setIsLeftStructureHidden] = useState(false);
  const [isRightPreviewHidden, setIsRightPreviewHidden] = useState(false);
  const [pointerMode, setPointerMode] = useState<"interact" | "click-dom" | "drag-box">("interact");
  const [isInlineCodeOpen, setIsInlineCodeOpen] = useState(false);
  const [viewportPreset, setViewportPreset] = useState<"desktop" | "tablet" | "mobile">("desktop");
  const [aspectRatio, setAspectRatio] = useState<"auto" | "16-9" | "16-10">("auto");
  const [isPresenting, setIsPresenting] = useState(false);
  const [highlightedRegionId, setHighlightedRegionId] = useState<string>("sec-biometric");
  const [slides, setSlides] = useState<SlidePhaseItem[]>(INITIAL_SLIDES);
  const [activeSlideIndex, setActiveSlideIndex] = useState<number>(0);
  const [isCmsDrawerOpen, setIsCmsDrawerOpen] = useState<boolean>(false);
  const [inlineCodeText, setInlineCodeText] = useState<string>(DEFAULT_INLINE_CODE);

  useEffect(() => {
    if (activeTab.format === "slide-cms" || activeTab.defaultLens === "slide-cms") {
      setStructureLens("slide-cms");
      setAspectRatio("16-9");
    } else if (activeTab.format === "dag" || activeTab.defaultLens === "userflow") {
      setStructureLens("userflow");
      setAspectRatio("auto");
    }
  }, [activeTab.id, activeTab.format, activeTab.defaultLens]);

  return {
    structureLens, setStructureLens, leftPanePercent, setLeftPanePercent,
    isDraggingInternalSplit, setIsDraggingInternalSplit, isLeftStructureHidden, setIsLeftStructureHidden,
    isRightPreviewHidden, setIsRightPreviewHidden, pointerMode, setPointerMode,
    isInlineCodeOpen, setIsInlineCodeOpen, viewportPreset, setViewportPreset,
    aspectRatio, setAspectRatio, isPresenting, setIsPresenting,
    highlightedRegionId, setHighlightedRegionId, slides, setSlides,
    activeSlideIndex, setActiveSlideIndex, isCmsDrawerOpen, setIsCmsDrawerOpen,
    inlineCodeText, setInlineCodeText,
  };
}
