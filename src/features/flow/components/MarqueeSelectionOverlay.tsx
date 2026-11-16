// @story 04.1, 04.5
// @implements AC-03 (Spatial Marquee >= 16x16px Selection Overlay)
import React, { useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import { SpatialMarqueeSelectionSchema } from "../../../lib/validations/flow-workbench";

export const MIN_MARQUEE_PX = 16;

export const MarqueeSelectionOverlay: React.FC<{
  onRegionClick: (regionId: string, label: string) => void;
}> = ({ onRegionClick }) => {
  const { locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const [marquee, setMarquee] = useState({
    isDragging: false,
    startX: 0,
    startY: 0,
    currentX: 0,
    currentY: 0,
  });

  const left = Math.min(marquee.startX, marquee.currentX);
  const top = Math.min(marquee.startY, marquee.currentY);
  const w = Math.abs(marquee.currentX - marquee.startX);
  const h = Math.abs(marquee.currentY - marquee.startY);

  return (
    <div
      className="marquee-drag-overlay"
      data-testid="marquee-drag-overlay"
      style={{ position: "absolute", inset: 0, zIndex: 20, cursor: "crosshair", userSelect: "none" }}
      onMouseDown={(e) => {
        const rect = e.currentTarget.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        setMarquee({ isDragging: true, startX: x, startY: y, currentX: x, currentY: y });
      }}
      onMouseMove={(e) => {
        if (!marquee.isDragging) return;
        const rect = e.currentTarget.getBoundingClientRect();
        setMarquee((prev) => ({
          ...prev,
          currentX: e.clientX - rect.left,
          currentY: e.clientY - rect.top,
        }));
      }}
      onMouseUp={(e) => {
        if (!marquee.isDragging) return;
        const rect = e.currentTarget.getBoundingClientRect();
        const rw = Math.round(Math.abs(e.clientX - rect.left - marquee.startX));
        const rh = Math.round(Math.abs(e.clientY - rect.top - marquee.startY));
        const rLeft = Math.round(Math.min(marquee.startX, e.clientX - rect.left));
        const rTop = Math.round(Math.min(marquee.startY, e.clientY - rect.top));
        setMarquee((prev) => ({ ...prev, isDragging: false }));
        const valid = SpatialMarqueeSelectionSchema.safeParse({
          x: rLeft,
          y: rTop,
          width: rw,
          height: rh,
          elementFfIds: [`box-${rLeft}-${rTop}`],
        });
        if (valid.success) {
          onRegionClick(
            `box-${rLeft}-${rTop}-${rw}x${rh}`,
            `@[Region ${formatNumber(rw)}×${formatNumber(rh)}px @ (${rLeft},${rTop})]`
          );
        }
      }}
    >
      {marquee.isDragging && (
        <div
          style={{
            position: "absolute",
            left: `${left}px`,
            top: `${top}px`,
            width: `${w}px`,
            height: `${h}px`,
            border: "1.5px dashed var(--action-primary)",
            backgroundColor: "var(--marquee-fill)",
            pointerEvents: "none",
            boxSizing: "border-box",
          }}
        >
          <span
            style={{
              position: "absolute",
              bottom: "-20px",
              right: 0,
              fontSize: "10px",
              fontFamily: "var(--font-mono)",
              backgroundColor: "var(--bg-elevated)",
              border: "1px solid var(--border-default)",
              padding: "1px 5px",
              borderRadius: "var(--radius-xs)",
              color: "var(--text-primary)",
              whiteSpace: "nowrap",
            }}
          >
            {formatNumber(Math.round(w))} × {formatNumber(Math.round(h))} px
          </span>
        </div>
      )}
    </div>
  );
};
