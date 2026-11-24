// @story 06.5
// @implements AC-02, AC-04 (Resizable RightDrawer <= 85vw, Master-Detail & Control-SubChat Split Layout Hook)
import { useCallback, useEffect, useState } from "react";
import {
  DRAWER_DEFAULT_VW,
  DRAWER_MAX_VW,
  DRAWER_MIN_PX,
  DRAWER_MOBILE_MAX_VW,
  MASTER_DEFAULT_PX,
  MASTER_MAX_PX,
  MASTER_MIN_PX,
  MOBILE_BREAKPOINT_PX,
  SUBCHAT_DEFAULT_PX,
  SUBCHAT_MAX_PX,
  SUBCHAT_MIN_PX,
  type StudioMobileTab,
} from "./types";

const FALLBACK_VIEWPORT_WIDTH = 1280;

function getViewportWidth(): number {
  return typeof window !== "undefined" ? window.innerWidth : FALLBACK_VIEWPORT_WIDTH;
}

export function clampDrawerWidth(widthPx: number, vw = getViewportWidth()): number {
  const minW = Math.min(DRAWER_MIN_PX, Math.round(vw * DRAWER_MOBILE_MAX_VW));
  const maxW = Math.round(vw * DRAWER_MAX_VW);
  return Math.max(minW, Math.min(maxW, Math.round(widthPx)));
}

export function clampMasterWidth(widthPx: number): number {
  return Math.max(MASTER_MIN_PX, Math.min(MASTER_MAX_PX, Math.round(widthPx)));
}

export function clampSubChatHeight(heightPx: number): number {
  return Math.max(SUBCHAT_MIN_PX, Math.min(SUBCHAT_MAX_PX, Math.round(heightPx)));
}

function startPointerDrag(onMove: (ev: MouseEvent) => void): void {
  const handleUp = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", handleUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", handleUp);
}

function useViewportResizeSync(
  setIsMobile: (v: boolean) => void,
  setDrawerWidthPx: React.Dispatch<React.SetStateAction<number>>
): void {
  useEffect(() => {
    const onResize = () => {
      const vw = getViewportWidth();
      setIsMobile(vw < MOBILE_BREAKPOINT_PX);
      setDrawerWidthPx((cur) => clampDrawerWidth(cur, vw));
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [setDrawerWidthPx, setIsMobile]);
}

export function useStudioLayout() {
  const [drawerWidthPx, setDrawerWidthPx] = useState<number>(() =>
    clampDrawerWidth(Math.round(getViewportWidth() * DRAWER_DEFAULT_VW))
  );
  const [masterWidthPx, setMasterWidthPx] = useState<number>(MASTER_DEFAULT_PX);
  const [masterCollapsed, setMasterCollapsed] = useState<boolean>(false);
  const [subChatHeightPx, setSubChatHeightPx] = useState<number>(SUBCHAT_DEFAULT_PX);
  const [subChatCollapsed, setSubChatCollapsed] = useState<boolean>(false);
  const [isMobile, setIsMobile] = useState<boolean>(() => getViewportWidth() < MOBILE_BREAKPOINT_PX);
  const [mobileTab, setMobileTab] = useState<StudioMobileTab>("control");
  useViewportResizeSync(setIsMobile, setDrawerWidthPx);

  const startDrawerResize = useCallback((ev: React.MouseEvent) => {
    ev.preventDefault();
    const startX = ev.clientX;
    startPointerDrag((m) => setDrawerWidthPx(clampDrawerWidth(drawerWidthPx + (startX - m.clientX))));
  }, [drawerWidthPx]);

  const startMasterResize = useCallback((ev: React.MouseEvent) => {
    ev.preventDefault();
    const startX = ev.clientX;
    startPointerDrag((m) => setMasterWidthPx(clampMasterWidth(masterWidthPx + (m.clientX - startX))));
  }, [masterWidthPx]);

  const startSubChatResize = useCallback((ev: React.MouseEvent) => {
    ev.preventDefault();
    const startY = ev.clientY;
    startPointerDrag((m) => setSubChatHeightPx(clampSubChatHeight(subChatHeightPx + (startY - m.clientY))));
  }, [subChatHeightPx]);

  return {
    drawerWidthPx,
    masterWidthPx,
    masterCollapsed,
    subChatHeightPx,
    subChatCollapsed,
    isMobile,
    mobileTab,
    setMobileTab,
    toggleMasterCollapsed: useCallback(() => setMasterCollapsed((v) => !v), []),
    toggleSubChatCollapsed: useCallback(() => setSubChatCollapsed((v) => !v), []),
    startDrawerResize,
    startMasterResize,
    startSubChatResize,
  };
}
