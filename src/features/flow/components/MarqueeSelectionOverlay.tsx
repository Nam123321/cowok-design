// @story 04.1, 04.5, 05.1
// @implements AC-03, AC-10 (Single-Component Hit + 60/120fps rAF Marquee >= 16x16px Multi-Element Intersection)
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import { SpatialMarqueeSelectionSchema } from "../../../lib/validations/flow-workbench";
import type { SpatialTargetSelection } from "../utils/sandbox-bridge-protocol";

export const MIN_MARQUEE_PX = 16;
const CLICK_MAX_DELTA_PX = 6;
const FALLBACK_CLICK_BOX_PX = 36;

interface ElementRectEntry {
  left: number;
  top: number;
  width: number;
  height: number;
}

export function findDeepestElementAtPoint(
  x: number,
  y: number,
  elementRects?: Record<string, ElementRectEntry>
): string | null {
  if (!elementRects) return null;
  let bestId: string | null = null;
  let bestArea = Number.POSITIVE_INFINITY;
  for (const [id, r] of Object.entries(elementRects)) {
    if (r.width <= 0 || r.height <= 0) continue;
    if (x >= r.left && x <= r.left + r.width && y >= r.top && y <= r.top + r.height) {
      const area = r.width * r.height;
      if (area < bestArea) {
        bestArea = area;
        bestId = id;
      }
    }
  }
  return bestId;
}

export function findIntersectingElementIds(
  left: number,
  top: number,
  width: number,
  height: number,
  elementRects?: Record<string, ElementRectEntry>
): string[] {
  if (!elementRects) return [];
  const right = left + width;
  const bottom = top + height;
  const matches: Array<{ id: string; top: number; left: number }> = [];
  for (const [id, r] of Object.entries(elementRects)) {
    if (r.width <= 0 || r.height <= 0) continue;
    const intersects = left < r.left + r.width && right > r.left && top < r.top + r.height && bottom > r.top;
    if (intersects) matches.push({ id, top: r.top, left: r.left });
  }
  matches.sort((a, b) => (a.top - b.top) || (a.left - b.left));
  return matches.map((m) => m.id);
}

export const MarqueeSelectionOverlay: React.FC<{
  nodeLabel?: string;
  elementRects?: Record<string, ElementRectEntry>;
  onRegionClick: (regionId: string, label: string, ffId?: string) => void;
  onSpatialTargetSelect?: (target: SpatialTargetSelection) => void;
}> = ({ nodeLabel = "SCR", elementRects, onRegionClick, onSpatialTargetSelect }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const [isDragging, setIsDragging] = useState(false);
  const dragRef = useRef({
    active: false,
    startX: 0,
    startY: 0,
    currentX: 0,
    currentY: 0,
    rafId: 0,
  });
  const boxRef = useRef<HTMLDivElement>(null);
  const sizeBadgeRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    return () => {
      if (dragRef.current.rafId) cancelAnimationFrame(dragRef.current.rafId);
    };
  }, []);

  const paintDragBoxDom = () => {
    const d = dragRef.current;
    const box = boxRef.current;
    if (!box) return;
    const left = Math.min(d.startX, d.currentX);
    const top = Math.min(d.startY, d.currentY);
    const w = Math.abs(d.currentX - d.startX);
    const h = Math.abs(d.currentY - d.startY);
    box.style.transform = `translate3d(${left}px, ${top}px, 0)`;
    box.style.width = `${w}px`;
    box.style.height = `${h}px`;
    if (sizeBadgeRef.current) {
      sizeBadgeRef.current.textContent = `${formatNumber(Math.round(w))} × ${formatNumber(Math.round(h))} ${t("spatial.unitPx", "px")}`;
    }
  };

  return (
    <div
      className="marquee-drag-overlay"
      data-testid="marquee-drag-overlay"
      style={{ position: "absolute", inset: 0, zIndex: 20, cursor: "crosshair", userSelect: "none" }}
      onContextMenu={(e) => {
        e.preventDefault();
        const bounds = e.currentTarget.getBoundingClientRect();
        const x = Math.round(e.clientX - bounds.left);
        const y = Math.round(e.clientY - bounds.top);
        const hitId = findDeepestElementAtPoint(x, y, elementRects) || Object.keys(elementRects || {})[0] || "sec-header";
        const hitRect = elementRects?.[hitId] ?? { left: x, top: y, width: FALLBACK_CLICK_BOX_PX, height: FALLBACK_CLICK_BOX_PX };
        const label = `@[Element ${nodeLabel} • #${hitId}]`;
        onSpatialTargetSelect?.({
          targetType: "html_element",
          triggerSource: "interact-contextmenu",
          selector: `[data-ff-id="${hitId}"]`,
          label,
          elementFfId: hitId,
          coveredFfIds: [hitId],
          rect: hitRect,
        });
      }}
      onMouseDown={(e) => {
        if (e.button !== 0) return;
        const rect = e.currentTarget.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        dragRef.current.active = true;
        dragRef.current.startX = x;
        dragRef.current.startY = y;
        dragRef.current.currentX = x;
        dragRef.current.currentY = y;
        setIsDragging(true);
        requestAnimationFrame(paintDragBoxDom);
      }}
      onMouseMove={(e) => {
        if (!dragRef.current.active) return;
        const rect = e.currentTarget.getBoundingClientRect();
        dragRef.current.currentX = e.clientX - rect.left;
        dragRef.current.currentY = e.clientY - rect.top;
        if (!dragRef.current.rafId) {
          dragRef.current.rafId = requestAnimationFrame(() => {
            dragRef.current.rafId = 0;
            paintDragBoxDom();
          });
        }
      }}
      onMouseUp={(e) => {
        if (!dragRef.current.active) return;
        if (dragRef.current.rafId) {
          cancelAnimationFrame(dragRef.current.rafId);
          dragRef.current.rafId = 0;
        }
        const rect = e.currentTarget.getBoundingClientRect();
        const endX = e.clientX - rect.left;
        const endY = e.clientY - rect.top;
        const rw = Math.round(Math.abs(endX - dragRef.current.startX));
        const rh = Math.round(Math.abs(endY - dragRef.current.startY));
        const rLeft = Math.round(Math.min(dragRef.current.startX, endX));
        const rTop = Math.round(Math.min(dragRef.current.startY, endY));
        dragRef.current.active = false;
        setIsDragging(false);

        const modifiers = { altKey: e.altKey, shiftKey: e.shiftKey, metaKey: e.metaKey, ctrlKey: e.ctrlKey };
        if (rw < CLICK_MAX_DELTA_PX && rh < CLICK_MAX_DELTA_PX) {
          const hitId = findDeepestElementAtPoint(rLeft, rTop, elementRects) || Object.keys(elementRects || {})[0] || "sec-header";
          const hitRect = elementRects?.[hitId] ?? { left: rLeft, top: rTop, width: FALLBACK_CLICK_BOX_PX, height: FALLBACK_CLICK_BOX_PX };
          const label = `@[Element ${nodeLabel} • #${hitId}]`;
          if (onSpatialTargetSelect) {
            onSpatialTargetSelect({
              targetType: "html_element",
              triggerSource: "select-click",
              selector: `[data-ff-id="${hitId}"]`,
              label,
              elementFfId: hitId,
              coveredFfIds: [hitId],
              rect: hitRect,
              modifiers,
            });
          } else {
            onRegionClick(`[data-ff-id="${hitId}"]`, label, hitId);
          }
          return;
        }

        const coveredFfIds = findIntersectingElementIds(rLeft, rTop, rw, rh, elementRects);
        const elementFfIds = coveredFfIds.length > 0 ? coveredFfIds : [`box-${rLeft}-${rTop}`];
        const valid = SpatialMarqueeSelectionSchema.safeParse({
          x: rLeft,
          y: rTop,
          width: rw,
          height: rh,
          elementFfIds,
        });
        if (valid.success) {
          const regionSelector = `box-${rLeft}-${rTop}-${rw}x${rh}`;
          const label = `@[Region ${formatNumber(rw)}×${formatNumber(rh)}px @ (${formatNumber(rLeft)},${formatNumber(rTop)})]`;
          if (onSpatialTargetSelect) {
            onSpatialTargetSelect({
              targetType: "spatial_region",
              triggerSource: "select-marquee",
              selector: regionSelector,
              label,
              elementFfId: coveredFfIds[0] || regionSelector,
              coveredFfIds,
              rect: { left: rLeft, top: rTop, width: rw, height: rh },
              modifiers,
            });
          } else {
            onRegionClick(regionSelector, label);
          }
        }
      }}
    >
      {isDragging && (
        <div ref={boxRef} className="ff-marquee-drag-box">
          <span ref={sizeBadgeRef} className="ff-marquee-drag-badge" />
        </div>
      )}
    </div>
  );
};
