// @story 06.1
// @implements AC-03, AC-04, AC-05
import React, { useEffect, useRef, useState } from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { MarqueeSelectionOverlay } from "./MarqueeSelectionOverlay";
import { CommentPinMarker } from "../../../components/comments/CommentPinMarker";
import type { ElementCommentResponse } from "../../../lib/validations/element-comment";
import { IFRAME_SANDBOX_PERMISSIONS } from "../utils/html-sandbox-wrapper";
import type { ElementRectBox, IframeKeepAliveSlot, SpatialTargetSelection } from "../hooks/useSandboxedIframeBridge";
import {
  normalizeViewportPreset,
  type FixtureVariant,
  type ViewportPreset,
} from "../../../lib/validations/screen-fixture";

export const SLIDE_STAGE_BASE_WIDTH_PX = 1280;
export const SLIDE_STAGE_MIN_FIT_WIDTH_PX = 576;
export const SLIDE_STAGE_MIN_SCALE = 0.45;
export const SLIDE_STAGE_DEFAULT_WIDTH_PX = 960;
export const SLIDE_STAGE_GUTTER_PX = 28;
const PIN_EDGE_PADDING_PX = 4;
const PIN_OFFSET_PX = 14;
const PIN_Z_INDEX = 15;
const SLIDE_STAGE_MIN_HEIGHT_PX = 240;
const DEFAULT_FRAME_MIN_HEIGHT_PX = 320;
const DEFAULT_DEVICE_FRAME_MIN_HEIGHT_PX = 360;
const SCALE_PCT_MULTIPLIER = 100;
const LEGACY_DEVICE_FRAME_MAX_WIDTH_PX = 820;
const FONT_WEIGHT_SEMIBOLD = 600;

const VIEWPORT_WIDTHS: Record<ViewportPreset, string> = {
  "fold-outer-344": "344px",
  "mobile-390": "390px",
  "fold-inner-712": "712px",
  "tablet-768": "768px",
  "desktop-1440": "100%",
};
const ASPECT_RATIO_VALUES: Record<"auto" | "16-9" | "16-10", string | undefined> = {
  auto: undefined,
  "16-9": "16 / 9",
  "16-10": "16 / 10",
};
const SLIDE_ASPECT_DIMENSIONS: Record<"auto" | "16-9" | "16-10", { w: string; h: string }> = {
  "16-9": { w: "1920px", h: "1080px" },
  "16-10": { w: "1920px", h: "1200px" },
  auto: { w: "1440px", h: "900px" },
};

const PreviewIframeLayer: React.FC<{
  title: string;
  isSlideStage?: boolean;
  poolSlots?: IframeKeepAliveSlot[];
  activeSlotId?: string;
  wrappedSrcDoc: string;
  iframeRef: React.RefObject<HTMLIFrameElement | null>;
  onLoad: () => void;
  onSlotLoad?: (slotId: string, el: HTMLIFrameElement | null) => void;
}> = ({ title, isSlideStage, poolSlots, activeSlotId, wrappedSrcDoc, iframeRef, onLoad, onSlotLoad }) => {
  const frameBg = isSlideStage ? "var(--deck-stage-mat-bg)" : "var(--bg-surface)";
  const frameMinHeight = isSlideStage ? `${SLIDE_STAGE_MIN_HEIGHT_PX}px` : `${DEFAULT_FRAME_MIN_HEIGHT_PX}px`;
  if (!poolSlots || poolSlots.length === 0) {
    return (
      <iframe
        ref={iframeRef} title={title} data-testid="sandboxed-html-iframe"
        sandbox={IFRAME_SANDBOX_PERMISSIONS} referrerPolicy="no-referrer"
        srcDoc={wrappedSrcDoc} onLoad={onLoad}
        style={{ width: "100%", height: "100%", minHeight: frameMinHeight, border: "none", backgroundColor: frameBg }}
      />
    );
  }
  return (
    <>
      {poolSlots.map((slot) => {
        const isActive = slot.slotId === activeSlotId;
        return (
          <iframe
            key={slot.slotId} ref={slot.iframeRef} title={title}
            data-testid={isActive ? "sandboxed-html-iframe" : `sandboxed-html-iframe-cached-${slot.nodeId}`}
            sandbox={IFRAME_SANDBOX_PERMISSIONS} referrerPolicy="no-referrer"
            srcDoc={slot.wrappedSrcDoc} onLoad={(e) => onSlotLoad?.(slot.slotId, e.currentTarget)}
            style={{
              width: "100%", height: "100%", minHeight: frameMinHeight, border: "none", backgroundColor: frameBg,
              visibility: isActive ? "visible" : "hidden", pointerEvents: isActive ? "auto" : "none",
              position: isActive ? "relative" : "absolute", left: 0, top: 0,
            }}
          />
        );
      })}
    </>
  );
};

const PreviewCommentPinsLayer: React.FC<{
  visiblePins: ElementCommentResponse[];
  elementRects: Record<string, ElementRectBox>;
  onOpenComments: () => void;
}> = ({ visiblePins, elementRects, onOpenComments }) => (
  <>
    {visiblePins.map((c, idx) => {
      const rect = elementRects[c.elementFfId];
      if (!rect) return null;
      return (
        <div key={c.id} style={{ position: "absolute", left: `${Math.max(PIN_EDGE_PADDING_PX, rect.left + rect.width - PIN_OFFSET_PX)}px`, top: `${Math.max(PIN_EDGE_PADDING_PX, rect.top)}px`, zIndex: PIN_Z_INDEX }}>
          <CommentPinMarker index={idx + 1} anchorStatus={c.anchorStatus} elementFfId={c.elementFfId} onClick={onOpenComments} />
        </div>
      );
    })}
  </>
);

function buildPreviewFrameStyle(opts: {
  isSlideStage: boolean;
  isPan100: boolean;
  viewportPreset: ViewportPreset | "desktop" | "tablet" | "mobile";
  aspectRatio: "auto" | "16-9" | "16-10";
  slideScale: number;
}): React.CSSProperties & { "--slide-scale": string; "--slide-w": string; "--slide-h": string } {
  const { isSlideStage, isPan100, viewportPreset, aspectRatio, slideScale } = opts;
  const width = isSlideStage
    ? (isPan100 ? `${SLIDE_STAGE_BASE_WIDTH_PX}px` : "100%")
    : VIEWPORT_WIDTHS[normalizeViewportPreset(viewportPreset)];
  const minWidth = isSlideStage
    ? (isPan100 ? `${SLIDE_STAGE_BASE_WIDTH_PX}px` : "0px")
    : undefined;
  const maxWidth = isSlideStage
    ? (isPan100 ? "none" : `${SLIDE_STAGE_BASE_WIDTH_PX}px`)
    : `${LEGACY_DEVICE_FRAME_MAX_WIDTH_PX}px`;
  const dims = SLIDE_ASPECT_DIMENSIONS[aspectRatio];

  return {
    position: "relative",
    width,
    minWidth,
    maxWidth,
    minHeight: isSlideStage ? "0px" : `${DEFAULT_DEVICE_FRAME_MIN_HEIGHT_PX}px`,
    flex: isSlideStage ? "0 0 auto" : 1,
    aspectRatio: ASPECT_RATIO_VALUES[aspectRatio],
    backgroundColor: isSlideStage ? "var(--deck-stage-mat-bg)" : "var(--bg-surface)",
    border: isSlideStage ? "1px solid var(--deck-stage-border)" : "1px solid var(--border-default)",
    borderRadius: "var(--radius-md)",
    boxShadow: "var(--shadow-sm)",
    overflow: "hidden",
    display: "flex",
    flexDirection: "column",
    transition: "width var(--duration-fast) var(--ease-standard)",
    "--slide-scale": String(slideScale),
    "--slide-w": dims.w,
    "--slide-h": dims.h,
  };
}

function useStageAreaMeasurement(ref: React.RefObject<HTMLDivElement | null>) {
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const sync = () => {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) setSize({ width: Math.round(r.width), height: Math.round(r.height) });
    };
    sync();
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return size;
}

export const PreviewDeviceFrame: React.FC<{
  routeBadge: string; viewportPreset: ViewportPreset | "desktop" | "tablet" | "mobile"; structureLens: "userflow" | "slide-cms";
  zoomMode: "fit" | "pan-100"; slideScale: number;
  activeSubState: FixtureVariant; aspectRatio: "auto" | "16-9" | "16-10";
  isPortVerified: boolean; protocolError?: string | null; wrappedSrcDoc: string; iframeRef: React.RefObject<HTMLIFrameElement | null>;
  onLoad: () => void; pointerMode: "interact" | "click-dom" | "drag-box";
  visiblePins: ElementCommentResponse[]; elementRects: Record<string, ElementRectBox>;
  poolSlots?: IframeKeepAliveSlot[]; activeSlotId?: string;
  onSlotLoad?: (slotId: string, el: HTMLIFrameElement | null) => void;
  onOpenComments: () => void; onRegionClick: (regionId: string, label: string) => void;
  onSpatialTargetSelect?: (target: SpatialTargetSelection) => void;
  spatialOverlayNode?: React.ReactNode;
}> = (p) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const isSlideStage = p.structureLens === "slide-cms";
  const stageAreaRef = useRef<HTMLDivElement>(null);
  const stageAreaSize = useStageAreaMeasurement(stageAreaRef);
  const statusTag = p.isPortVerified ? "CSPRNG_VERIFIED" : "STRICT_OFFLINE";
  const scalePct = Math.round(p.slideScale * SCALE_PCT_MULTIPLIER);
  const pillLabel = isSlideStage ? `${p.aspectRatio.toUpperCase()} • ${formatNumber(scalePct)}% • ${statusTag}` : `${normalizeViewportPreset(p.viewportPreset).toUpperCase()} • ${p.activeSubState} • ${statusTag}`;
  const frameStyle = buildPreviewFrameStyle({ isSlideStage, isPan100: isSlideStage && p.zoomMode === "pan-100", viewportPreset: p.viewportPreset, aspectRatio: p.aspectRatio, slideScale: p.slideScale });

  return (
    <div className="live-html-frame" data-quotable="true" data-project-preview="true" data-slide-stage={isSlideStage ? "true" : undefined} data-aspect={isSlideStage ? p.aspectRatio : undefined} data-zoom-mode={isSlideStage ? p.zoomMode : undefined} style={frameStyle}>
      <div style={{ height: "28px", padding: "0 12px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", fontFamily: "var(--font-mono)", fontSize: "10px", color: "var(--text-muted)", fontVariantNumeric: "tabular-nums" }}>
        <span>{p.routeBadge}</span>
        <span data-testid="bridge-status-pill">{pillLabel}</span>
      </div>
      {p.protocolError && <div role="alert" data-testid="deck-protocol-error-banner" style={{ padding: "6px 12px", backgroundColor: "var(--state-danger)", color: "var(--text-on-action)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{p.protocolError}</div>}
      <div ref={stageAreaRef} style={{ position: "relative", flex: 1, width: "100%", height: "100%", minHeight: isSlideStage ? `${SLIDE_STAGE_MIN_HEIGHT_PX}px` : `${DEFAULT_FRAME_MIN_HEIGHT_PX}px` }}>
        <PreviewIframeLayer title={t("canvas.sandboxIframeTitle", "Sandboxed Live HTML Preview")} isSlideStage={isSlideStage} poolSlots={p.poolSlots} activeSlotId={p.activeSlotId} wrappedSrcDoc={p.wrappedSrcDoc} iframeRef={p.iframeRef} onLoad={p.onLoad} onSlotLoad={p.onSlotLoad} />
        <PreviewCommentPinsLayer visiblePins={p.visiblePins} elementRects={p.elementRects} onOpenComments={p.onOpenComments} />
        {(p.pointerMode === "click-dom" || p.pointerMode === "drag-box") && <MarqueeSelectionOverlay nodeLabel={p.routeBadge} elementRects={p.elementRects} onRegionClick={p.onRegionClick} onSpatialTargetSelect={p.onSpatialTargetSelect} />}
        {stageAreaSize && React.isValidElement<{ stageWidth?: number; stageHeight?: number }>(p.spatialOverlayNode) ? React.cloneElement(p.spatialOverlayNode, { stageWidth: stageAreaSize.width, stageHeight: stageAreaSize.height }) : p.spatialOverlayNode}
      </div>
    </div>
  );
};
