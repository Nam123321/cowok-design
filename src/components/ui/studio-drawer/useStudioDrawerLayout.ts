// @story 05.6, 06.5
// @implements [5.26] Resizable Master-Detail RightDrawer Layout Hook (<= 85vw + Mobile 2-Tab)
import { useCallback, useEffect, useRef, useState } from "react";
import {
  STUDIO_DRAWER_DEFAULT_VW,
  STUDIO_DRAWER_MAX_VW,
  STUDIO_DRAWER_MIN_PX,
  STUDIO_DRAWER_MOBILE_MAX_VW,
  STUDIO_MASTER_DEFAULT_PX,
  STUDIO_MASTER_MAX_PX,
  STUDIO_MASTER_MIN_PX,
  STUDIO_MOBILE_BREAKPOINT_PX,
  STUDIO_SUBCHAT_DEFAULT_PX,
  STUDIO_SUBCHAT_MAX_PX,
  STUDIO_SUBCHAT_MIN_PX,
  type StudioMobileTab,
} from "./types";

const FALLBACK_VIEWPORT_WIDTH = 1280;

function getViewportWidth(): number {
  return typeof window !== "undefined" ? window.innerWidth : FALLBACK_VIEWPORT_WIDTH;
}

export function clampStudioDrawerWidth(widthPx: number, vw = getViewportWidth()): number {
  const minW = Math.min(STUDIO_DRAWER_MIN_PX, Math.round(vw * STUDIO_DRAWER_MOBILE_MAX_VW));
  const maxW = Math.round(vw * STUDIO_DRAWER_MAX_VW);
  return Math.max(minW, Math.min(maxW, Math.round(widthPx)));
}

export function clampStudioMasterWidth(widthPx: number): number {
  return Math.max(STUDIO_MASTER_MIN_PX, Math.min(STUDIO_MASTER_MAX_PX, Math.round(widthPx)));
}

export function clampStudioSubChatHeight(heightPx: number): number {
  return Math.max(STUDIO_SUBCHAT_MIN_PX, Math.min(STUDIO_SUBCHAT_MAX_PX, Math.round(heightPx)));
}

function attachPointerDrag(
  cleanupRef: React.MutableRefObject<(() => void) | null>,
  onMove: (ev: MouseEvent) => void
): void {
  cleanupRef.current?.();
  const finish = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", finish);
    window.removeEventListener("blur", finish);
    if (cleanupRef.current === finish) cleanupRef.current = null;
  };
  cleanupRef.current = finish;
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", finish);
  window.addEventListener("blur", finish);
}

function useResponsiveDrawerSync(
  setIsMobile: (mobile: boolean) => void,
  setDrawerWidthPx: React.Dispatch<React.SetStateAction<number>>
): void {
  useEffect(() => {
    const onResize = () => {
      const vw = getViewportWidth();
      setIsMobile(vw < STUDIO_MOBILE_BREAKPOINT_PX);
      setDrawerWidthPx((cur) => clampStudioDrawerWidth(cur, vw));
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [setDrawerWidthPx, setIsMobile]);
}

export interface StudioDrawerLayoutState {
  drawerWidthPx: number;
  masterWidthPx: number;
  masterCollapsed: boolean;
  subChatHeightPx: number;
  subChatCollapsed: boolean;
  isMobile: boolean;
  mobileTab: StudioMobileTab;
  setMobileTab: (tab: StudioMobileTab) => void;
  toggleMasterCollapsed: () => void;
  toggleSubChatCollapsed: () => void;
  expandSubChatPanel: () => void;
  startDrawerResize: (ev: React.MouseEvent) => void;
  startMasterResize: (ev: React.MouseEvent) => void;
  startSubChatResize: (ev: React.MouseEvent) => void;
}

function useDrawerDragHandlers(
  cleanupRef: React.MutableRefObject<(() => void) | null>,
  dims: { drawerWidthPx: number; masterWidthPx: number; subChatHeightPx: number },
  setters: {
    setDrawerWidthPx: React.Dispatch<React.SetStateAction<number>>;
    setMasterWidthPx: React.Dispatch<React.SetStateAction<number>>;
    setSubChatHeightPx: React.Dispatch<React.SetStateAction<number>>;
  }
) {
  const startDrawerResize = useCallback((ev: React.MouseEvent) => {
    ev.preventDefault();
    const startX = ev.clientX;
    attachPointerDrag(cleanupRef, (m) => setters.setDrawerWidthPx(clampStudioDrawerWidth(dims.drawerWidthPx + (startX - m.clientX))));
  }, [cleanupRef, dims.drawerWidthPx, setters]);

  const startMasterResize = useCallback((ev: React.MouseEvent) => {
    ev.preventDefault();
    const startX = ev.clientX;
    attachPointerDrag(cleanupRef, (m) => setters.setMasterWidthPx(clampStudioMasterWidth(dims.masterWidthPx + (m.clientX - startX))));
  }, [cleanupRef, dims.masterWidthPx, setters]);

  const startSubChatResize = useCallback((ev: React.MouseEvent) => {
    ev.preventDefault();
    const startY = ev.clientY;
    attachPointerDrag(cleanupRef, (m) => setters.setSubChatHeightPx(clampStudioSubChatHeight(dims.subChatHeightPx + (startY - m.clientY))));
  }, [cleanupRef, dims.subChatHeightPx, setters]);

  return { startDrawerResize, startMasterResize, startSubChatResize };
}

export function useStudioDrawerLayout(initialMasterPx = STUDIO_MASTER_DEFAULT_PX): StudioDrawerLayoutState {
  const [drawerWidthPx, setDrawerWidthPx] = useState<number>(() =>
    clampStudioDrawerWidth(Math.round(getViewportWidth() * STUDIO_DRAWER_DEFAULT_VW))
  );
  const [masterWidthPx, setMasterWidthPx] = useState<number>(() => clampStudioMasterWidth(initialMasterPx));
  const [masterCollapsed, setMasterCollapsed] = useState<boolean>(false);
  const [subChatHeightPx, setSubChatHeightPx] = useState<number>(STUDIO_SUBCHAT_DEFAULT_PX);
  const [subChatCollapsed, setSubChatCollapsed] = useState<boolean>(false);
  const [isMobile, setIsMobile] = useState<boolean>(() => getViewportWidth() < STUDIO_MOBILE_BREAKPOINT_PX);
  const [mobileTab, setMobileTab] = useState<StudioMobileTab>("control");
  const activeDragCleanupRef = useRef<(() => void) | null>(null);
  useResponsiveDrawerSync(setIsMobile, setDrawerWidthPx);
  useEffect(() => () => { activeDragCleanupRef.current?.(); }, []);

  const drags = useDrawerDragHandlers(
    activeDragCleanupRef,
    { drawerWidthPx, masterWidthPx, subChatHeightPx },
    { setDrawerWidthPx, setMasterWidthPx, setSubChatHeightPx }
  );
  const expandSubChatPanel = useCallback(() => {
    setMasterCollapsed(false);
    setSubChatCollapsed(false);
    setMobileTab("control");
  }, []);

  return {
    drawerWidthPx, masterWidthPx, masterCollapsed, subChatHeightPx, subChatCollapsed,
    isMobile, mobileTab, setMobileTab, expandSubChatPanel,
    toggleMasterCollapsed: useCallback(() => setMasterCollapsed((v) => !v), []),
    toggleSubChatCollapsed: useCallback(() => setSubChatCollapsed((v) => !v), []),
    ...drags,
  };
}
