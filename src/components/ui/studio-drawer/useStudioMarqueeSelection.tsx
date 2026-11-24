// @story 05.6, 06.5
// @implements [5.26] Unified Click-to-Select + Drag-Marquee Region Selection for Detail Canvas
import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  STUDIO_FONT_BOLD,
  STUDIO_MARQUEE_MIN_PX,
  type StudioPinnedComment,
  type StudioPointerMode,
  type StudioRegionRect,
  type StudioSpatialTarget,
} from "./types";

const MARQUEE_Z = 32;
const PIN_MARKER_Z = 30;
const PIN_MARKER_SIZE_PX = 22;
const PIN_MARKER_OFFSET_PX = 11;

interface UseStudioMarqueeOptions {
  pointerMode: StudioPointerMode;
  onSelectTarget: (target: StudioSpatialTarget) => void;
}

function getRelativeCanvasPoint(container: HTMLElement, clientX: number, clientY: number): { x: number; y: number } {
  const rect = container.getBoundingClientRect();
  return {
    x: Math.max(0, Math.round(clientX - rect.left + container.scrollLeft)),
    y: Math.max(0, Math.round(clientY - rect.top + container.scrollTop)),
  };
}

function resolveClosestFfId(targetEl: EventTarget | null): { ffId: string; label?: string } {
  const el = targetEl instanceof HTMLElement ? targetEl.closest("[data-ff-id]") : null;
  if (!el) return { ffId: "canvas-surface", label: "Canvas" };
  const ffId = el.getAttribute("data-ff-id") || "canvas-surface";
  return { ffId, label: el.getAttribute("data-ff-label") || ffId };
}

function buildFinalTarget(
  rect: StudioRegionRect,
  start: { x: number; y: number },
  hit: { ffId: string; label?: string },
  regionPrefix: string
): StudioSpatialTarget {
  const isRegion = rect.w >= STUDIO_MARQUEE_MIN_PX || rect.h >= STUDIO_MARQUEE_MIN_PX;
  if (isRegion) {
    return {
      ffId: hit.ffId,
      label: `${regionPrefix} ${rect.w}×${rect.h}px`,
      x: rect.x + Math.round(rect.w / 2),
      y: rect.y,
      kind: "region",
      regionRect: rect,
    };
  }
  return { ffId: hit.ffId, label: hit.label, x: start.x, y: start.y, kind: "element" };
}

function attachMarqueeListeners(opts: {
  container: HTMLElement;
  start: { x: number; y: number };
  hit: { ffId: string; label?: string };
  regionPrefix: string;
  setMarqueeRect: (r: StudioRegionRect | null) => void;
  onSelectTarget: (t: StudioSpatialTarget) => void;
  onDone: () => void;
}): () => void {
  let current: StudioRegionRect = { x: opts.start.x, y: opts.start.y, w: 0, h: 0 };
  const onMove = (ev: MouseEvent) => {
    const cur = getRelativeCanvasPoint(opts.container, ev.clientX, ev.clientY);
    current = {
      x: Math.min(opts.start.x, cur.x), y: Math.min(opts.start.y, cur.y),
      w: Math.abs(cur.x - opts.start.x), h: Math.abs(cur.y - opts.start.y),
    };
    if (current.w >= STUDIO_MARQUEE_MIN_PX || current.h >= STUDIO_MARQUEE_MIN_PX) opts.setMarqueeRect(current);
  };
  const cleanup = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    window.removeEventListener("blur", onCancel);
    opts.onDone();
  };
  const onCancel = () => { cleanup(); opts.setMarqueeRect(null); };
  const onUp = () => { cleanup(); opts.setMarqueeRect(null); opts.onSelectTarget(buildFinalTarget(current, opts.start, opts.hit, opts.regionPrefix)); };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
  window.addEventListener("blur", onCancel);
  return cleanup;
}

export function useStudioMarqueeSelection({ pointerMode, onSelectTarget }: UseStudioMarqueeOptions) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const cleanupDragRef = useRef<(() => void) | null>(null);
  const [marqueeRect, setMarqueeRect] = useState<StudioRegionRect | null>(null);
  const regionPrefix = t("studioDrawer.regionLabel", "Vùng");

  useEffect(() => () => { cleanupDragRef.current?.(); }, []);

  const handleCanvasMouseDown = useCallback((ev: React.MouseEvent<HTMLDivElement>) => {
    if (pointerMode !== "select-region" || ev.button !== 0) return;
    const rawTarget = ev.target as HTMLElement | null;
    if (rawTarget?.closest('[data-testid="studio-spatial-action-bar"], [data-studio-pin-marker="true"]')) return;
    const container = containerRef.current;
    if (!container) return;
    ev.preventDefault();
    cleanupDragRef.current?.();
    cleanupDragRef.current = attachMarqueeListeners({
      container,
      start: getRelativeCanvasPoint(container, ev.clientX, ev.clientY),
      hit: resolveClosestFfId(ev.target),
      regionPrefix,
      setMarqueeRect,
      onSelectTarget,
      onDone: () => { cleanupDragRef.current = null; },
    });
  }, [onSelectTarget, pointerMode, regionPrefix]);

  const handleCanvasContextMenu = useCallback((ev: React.MouseEvent<HTMLDivElement>) => {
    const container = containerRef.current;
    if (!container) return;
    ev.preventDefault();
    const pt = getRelativeCanvasPoint(container, ev.clientX, ev.clientY);
    const hit = resolveClosestFfId(ev.target);
    onSelectTarget({ ffId: hit.ffId, label: hit.label, x: pt.x, y: pt.y, kind: "element" });
  }, [onSelectTarget]);

  return { containerRef, marqueeRect, handleCanvasMouseDown, handleCanvasContextMenu };
}

export const StudioMarqueeOverlay: React.FC<{ rect: StudioRegionRect | null }> = ({ rect }) => {
  if (!rect) return null;
  return (
    <div
      data-testid="studio-marquee-box"
      style={{
        position: "absolute", left: `${rect.x}px`, top: `${rect.y}px`, width: `${rect.w}px`, height: `${rect.h}px`,
        zIndex: MARQUEE_Z, border: "1.5px dashed var(--action-primary)",
        backgroundColor: "color-mix(in srgb, var(--action-primary) 12%, transparent)", borderRadius: "var(--radius-xs)", pointerEvents: "none",
      }}
    />
  );
};

export const StudioCanvasPinsLayer: React.FC<{
  pins: readonly StudioPinnedComment[];
  onSelectPin?: (pin: StudioPinnedComment) => void;
}> = ({ pins, onSelectPin }) => {
  const { t } = useTranslation();
  const pinPrefix = t("studioDrawer.pinAria", "Bình luận");
  return (
    <>
      {pins.map((pin) => (
        <button
          key={pin.id} type="button" data-studio-pin-marker="true" data-testid={`studio-canvas-pin-${pin.num}`}
          title={`#${pin.num} ${pin.label}: ${pin.text}`} aria-label={`${pinPrefix} #${pin.num}: ${pin.text}`} onClick={() => onSelectPin?.(pin)}
          style={{
            position: "absolute", left: `${Math.max(PIN_MARKER_OFFSET_PX, pin.x - PIN_MARKER_OFFSET_PX)}px`, top: `${Math.max(PIN_MARKER_OFFSET_PX, pin.y - PIN_MARKER_OFFSET_PX)}px`,
            width: `${PIN_MARKER_SIZE_PX}px`, height: `${PIN_MARKER_SIZE_PX}px`, borderRadius: "50%", border: "1.5px solid var(--bg-surface)",
            backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "10px", fontWeight: STUDIO_FONT_BOLD,
            display: "inline-flex", alignItems: "center", justifyContent: "center", boxShadow: "var(--shadow-sm)", cursor: "pointer", zIndex: PIN_MARKER_Z, padding: 0,
          }}
        >
          {pin.num}
        </button>
      ))}
    </>
  );
};
