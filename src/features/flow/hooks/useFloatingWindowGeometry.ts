// @story 04.4
// @implements AC-01, AC-02 (Floating & Draggable Position + Resize Persistence Engine)
import { useState, useRef, useEffect, useCallback } from "react";

export const FLOATING_CHAT_LAYOUT_STORAGE_KEY = "cowok:floating-chat:layout:v1";

export const MIN_WIDTH = 320;
export const MAX_WIDTH = 640;
export const DEFAULT_WIDTH = 440;
export const MIN_HEIGHT = 280;
export const MAX_HEIGHT = 680;
export const DEFAULT_HEIGHT = 480;

const VIEWPORT_PAD_X = 16;
const VIEWPORT_PAD_Y = 32;
const VIEWPORT_BOTTOM_OFFSET = 48;
const COLLAPSED_HEIGHT = 48;
const DEFAULT_VIEWPORT_WIDTH = 1280;
const DEFAULT_VIEWPORT_HEIGHT = 800;
const FALLBACK_DEF_X = 100;
const FALLBACK_DEF_Y = 200;

export function getEffectiveMaxHeight(): number {
  if (typeof window === "undefined") return MAX_HEIGHT;
  return Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, window.innerHeight - VIEWPORT_BOTTOM_OFFSET));
}

export interface FloatingChatLayout {
  x: number;
  y: number;
  width: number;
  height: number;
  hasCustomPosition: boolean;
}

function clampNumber(val: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, val));
}

export function loadSavedLayout(): FloatingChatLayout | null {
  if (typeof window === "undefined" || !window.localStorage) return null;
  try {
    const raw = window.localStorage.getItem(FLOATING_CHAT_LAYOUT_STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return null;

    const width = clampNumber(Number(parsed.width) || DEFAULT_WIDTH, MIN_WIDTH, MAX_WIDTH);
    const height = clampNumber(Number(parsed.height) || DEFAULT_HEIGHT, MIN_HEIGHT, getEffectiveMaxHeight());
    const viewportW = typeof window !== "undefined" ? window.innerWidth : DEFAULT_VIEWPORT_WIDTH;
    const viewportH = typeof window !== "undefined" ? window.innerHeight : DEFAULT_VIEWPORT_HEIGHT;

    const maxX = Math.max(0, viewportW - width);
    const maxY = Math.max(0, viewportH - height);
    const x = clampNumber(Number(parsed.x) || 0, 0, maxX);
    const y = clampNumber(Number(parsed.y) || 0, 0, maxY);
    const hasCustomPosition = Boolean(parsed.hasCustomPosition);

    return { x, y, width, height, hasCustomPosition };
  } catch {
    return null;
  }
}

export function saveLayout(layout: FloatingChatLayout): void {
  if (typeof window === "undefined" || !window.localStorage) return;
  try {
    window.localStorage.setItem(FLOATING_CHAT_LAYOUT_STORAGE_KEY, JSON.stringify(layout));
  } catch {
    // ignore storage exceptions
  }
}

function getInitialPosition(saved: FloatingChatLayout | null, anchorX?: number, anchorY?: number) {
  if (saved && saved.hasCustomPosition) return { x: saved.x, y: saved.y };
  if (anchorX !== undefined && anchorY !== undefined) return { x: anchorX, y: anchorY };
  if (saved) return { x: saved.x, y: saved.y };
  const defX = typeof window !== "undefined" ? Math.max(VIEWPORT_PAD_X, Math.round((window.innerWidth - DEFAULT_WIDTH) / 2)) : FALLBACK_DEF_X;
  const defY = typeof window !== "undefined" ? Math.max(VIEWPORT_PAD_X, window.innerHeight - DEFAULT_HEIGHT - VIEWPORT_PAD_Y) : FALLBACK_DEF_Y;
  return { x: defX, y: defY };
}

interface DragHandlerOptions {
  position: { x: number; y: number };
  setPosition: React.Dispatch<React.SetStateAction<{ x: number; y: number }>>;
  width: number;
  height: number;
  isCollapsed: boolean;
  onPersist: (pos: { x: number; y: number }) => void;
}

function useDragHandler(opts: DragHandlerOptions) {
  const [isDragging, setIsDragging] = useState(false);
  const dragStartRef = useRef({ mouseX: 0, mouseY: 0, startX: 0, startY: 0 });

  const handleHeaderMouseDown = useCallback((e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button") || (e.target as HTMLElement).closest("input")) return;
    e.preventDefault();
    setIsDragging(true);
    dragStartRef.current = { mouseX: e.clientX, mouseY: e.clientY, startX: opts.position.x, startY: opts.position.y };
  }, [opts.position.x, opts.position.y]);

  useEffect(() => {
    if (!isDragging) return;
    const onMove = (e: MouseEvent) => {
      const vw = typeof window !== "undefined" ? window.innerWidth : DEFAULT_VIEWPORT_WIDTH;
      const vh = typeof window !== "undefined" ? window.innerHeight : DEFAULT_VIEWPORT_HEIGHT;
      const maxX = Math.max(0, vw - opts.width);
      const maxY = Math.max(0, vh - (opts.isCollapsed ? COLLAPSED_HEIGHT : opts.height));
      const nx = clampNumber(dragStartRef.current.startX + (e.clientX - dragStartRef.current.mouseX), 0, maxX);
      const ny = clampNumber(dragStartRef.current.startY + (e.clientY - dragStartRef.current.mouseY), 0, maxY);
      opts.setPosition({ x: nx, y: ny });
    };
    const onUp = (e: MouseEvent) => {
      setIsDragging(false);
      const vw = typeof window !== "undefined" ? window.innerWidth : DEFAULT_VIEWPORT_WIDTH;
      const vh = typeof window !== "undefined" ? window.innerHeight : DEFAULT_VIEWPORT_HEIGHT;
      const maxX = Math.max(0, vw - opts.width);
      const maxY = Math.max(0, vh - (opts.isCollapsed ? COLLAPSED_HEIGHT : opts.height));
      const fx = clampNumber(dragStartRef.current.startX + (e.clientX - dragStartRef.current.mouseX), 0, maxX);
      const fy = clampNumber(dragStartRef.current.startY + (e.clientY - dragStartRef.current.mouseY), 0, maxY);
      opts.setPosition({ x: fx, y: fy });
      opts.onPersist({ x: fx, y: fy });
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [isDragging, opts]);

  return { isDragging, handleHeaderMouseDown };
}

interface ResizeHandlerOptions {
  width: number;
  setWidth: React.Dispatch<React.SetStateAction<number>>;
  customHeight: number | null;
  setCustomHeight: React.Dispatch<React.SetStateAction<number | null>>;
  onPersist: (w: number, h: number) => void;
}

function useResizeHandler(opts: ResizeHandlerOptions) {
  return useCallback((e: React.MouseEvent, dir: "e" | "s" | "se") => {
    e.preventDefault();
    e.stopPropagation();
    const sx = e.clientX;
    const sy = e.clientY;
    const sw = opts.width;
    const sh = opts.customHeight ?? DEFAULT_HEIGHT;
    const effMaxH = getEffectiveMaxHeight();

    const onMove = (ev: MouseEvent) => {
      if (dir === "e" || dir === "se") {
        opts.setWidth(clampNumber(sw + (ev.clientX - sx), MIN_WIDTH, MAX_WIDTH));
      }
      if (dir === "s" || dir === "se") {
        opts.setCustomHeight(clampNumber(sh + (ev.clientY - sy), MIN_HEIGHT, effMaxH));
      }
    };
    const onUp = (ev: MouseEvent) => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      const fw = dir === "e" || dir === "se" ? clampNumber(sw + (ev.clientX - sx), MIN_WIDTH, MAX_WIDTH) : sw;
      const fh = dir === "s" || dir === "se" ? clampNumber(sh + (ev.clientY - sy), MIN_HEIGHT, effMaxH) : sh;
      opts.setWidth(fw);
      opts.setCustomHeight(fh);
      opts.onPersist(fw, fh);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, [opts]);
}

function useInitialLayoutState(anchorX?: number, anchorY?: number) {
  const [saved] = useState<FloatingChatLayout | null>(() => loadSavedLayout());
  const [hasCustomPosition, setHasCustomPosition] = useState<boolean>(() => saved?.hasCustomPosition ?? false);
  const [position, setPosition] = useState<{ x: number; y: number }>(() => getInitialPosition(saved, anchorX, anchorY));
  const [width, setWidth] = useState<number>(() => saved?.width ?? DEFAULT_WIDTH);
  const [customHeight, setCustomHeight] = useState<number | null>(() => saved?.height ?? null);

  useEffect(() => {
    if (anchorX !== undefined && anchorY !== undefined && !hasCustomPosition) {
      setPosition({ x: anchorX, y: anchorY });
    }
  }, [anchorX, anchorY, hasCustomPosition]);

  return { saved, hasCustomPosition, setHasCustomPosition, position, setPosition, width, setWidth, customHeight, setCustomHeight };
}

function useLayoutPersistCallbacks(state: ReturnType<typeof useInitialLayoutState>) {
  const persistDrag = useCallback((pos: { x: number; y: number }) => {
    state.setHasCustomPosition(true);
    saveLayout({ x: pos.x, y: pos.y, width: state.width, height: state.customHeight ?? DEFAULT_HEIGHT, hasCustomPosition: true });
  }, [state.width, state.customHeight, state.setHasCustomPosition]);

  const persistResize = useCallback((w: number, h: number) => {
    saveLayout({ x: state.position.x, y: state.position.y, width: w, height: h, hasCustomPosition: state.hasCustomPosition });
  }, [state.position.x, state.position.y, state.hasCustomPosition]);

  return { persistDrag, persistResize };
}

function useRedockHandler(state: ReturnType<typeof useInitialLayoutState>, anchorX?: number, anchorY?: number) {
  return useCallback(() => {
    const defX = typeof window !== "undefined" ? Math.max(VIEWPORT_PAD_X, Math.round((window.innerWidth - DEFAULT_WIDTH) / 2)) : FALLBACK_DEF_X;
    const defY = typeof window !== "undefined" ? Math.max(VIEWPORT_PAD_X, window.innerHeight - DEFAULT_HEIGHT - VIEWPORT_PAD_Y) : FALLBACK_DEF_Y;
    const nx = anchorX !== undefined ? anchorX : defX;
    const ny = anchorY !== undefined ? anchorY : defY;
    state.setHasCustomPosition(false);
    state.setPosition({ x: nx, y: ny });
    state.setWidth(DEFAULT_WIDTH);
    state.setCustomHeight(null);
    saveLayout({ x: nx, y: ny, width: DEFAULT_WIDTH, height: DEFAULT_HEIGHT, hasCustomPosition: false });
  }, [anchorX, anchorY, state]);
}

export function useFloatingWindowGeometry(
  isCollapsed: boolean,
  anchorX?: number,
  anchorY?: number
) {
  const state = useInitialLayoutState(anchorX, anchorY);
  const { persistDrag, persistResize } = useLayoutPersistCallbacks(state);
  const dragOpts: DragHandlerOptions = {
    position: state.position,
    setPosition: state.setPosition,
    width: state.width,
    height: state.customHeight ?? DEFAULT_HEIGHT,
    isCollapsed,
    onPersist: persistDrag,
  };
  const { isDragging, handleHeaderMouseDown } = useDragHandler(dragOpts);
  const handleStartResize = useResizeHandler({
    width: state.width,
    setWidth: state.setWidth,
    customHeight: state.customHeight,
    setCustomHeight: state.setCustomHeight,
    onPersist: persistResize,
  });
  const handleRedockToBottom = useRedockHandler(state, anchorX, anchorY);

  return {
    position: state.position,
    setPosition: state.setPosition,
    hasCustomPosition: state.hasCustomPosition,
    setHasCustomPosition: state.setHasCustomPosition,
    width: state.width,
    setWidth: state.setWidth,
    customHeight: state.customHeight,
    setCustomHeight: state.setCustomHeight,
    isDragging,
    handleHeaderMouseDown,
    handleStartResize,
    handleRedockToBottom,
  };
}
