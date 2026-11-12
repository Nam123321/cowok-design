// @story 04.1
// @implements AC-03, AC-07
import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { CanvasTabItem } from "./CompanionCanvasTab";
import type { ScreenPreviewData, SlidePhaseItem } from "./companion-canvas-data";
import { HtmlPreviewToolbar } from "./HtmlPreviewToolbar";
import {
  buildPreviewDocumentHtml,
  IFRAME_SANDBOX_PERMISSIONS,
  IframeToHostMessageSchema,
  wrapUserHtmlSrcDoc,
} from "../utils/html-sandbox-wrapper";

const VIEWPORT_WIDTHS: Record<"desktop" | "tablet" | "mobile", string> = {
  mobile: "375px",
  tablet: "620px",
  desktop: "100%",
};

const ASPECT_RATIO_VALUES: Record<"auto" | "16-9" | "16-10", string | undefined> = {
  auto: undefined,
  "16-9": "16 / 9",
  "16-10": "16 / 10",
};

const InlineCodeEditorPanel: React.FC<{
  activeTab: CanvasTabItem;
  inlineCodeText: string;
  onChangeInlineCode: (val: string) => void;
  onCloseEditor: () => void;
}> = ({ activeTab, inlineCodeText, onChangeInlineCode, onCloseEditor }) => (
  <div style={{ width: "100%", height: "100%", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-md)", display: "flex", flexDirection: "column", overflow: "hidden" }}>
    <div style={{ height: "30px", padding: "0 12px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", fontFamily: "var(--font-mono)", fontSize: "11px" }}>
      <span>&lt;/&gt; {activeTab.filePath || activeTab.title}</span>
      <button type="button" onClick={onCloseEditor} className="cowok-btn cowok-btn-primary" style={{ height: "22px", padding: "0 8px", fontSize: "10.5px", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)" }}>
        ✓ Áp dụng lên HTML
      </button>
    </div>
    <textarea
      value={inlineCodeText}
      onChange={(e) => onChangeInlineCode(e.target.value)}
      spellCheck={false}
      style={{ flex: 1, width: "100%", padding: "12px", border: "none", backgroundColor: "var(--bg-canvas)", color: "var(--text-primary)", fontFamily: "var(--font-mono)", fontSize: "12px", lineHeight: "1.55", resize: "none", outline: "none", boxSizing: "border-box" }}
    />
  </div>
);

export interface CompanionLiveHtmlRightPaneProps {
  activeTab: CanvasTabItem;
  structureLens: "userflow" | "slide-cms";
  selectedNodeId?: string | null;
  activeSlide: SlidePhaseItem;
  activeScreenData: ScreenPreviewData;
  pointerMode: "interact" | "click-dom" | "drag-box";
  onSetPointerMode: (mode: "interact" | "click-dom" | "drag-box") => void;
  isInlineCodeOpen: boolean;
  onToggleInlineCode: () => void;
  inlineCodeText: string;
  onChangeInlineCode: (val: string) => void;
  viewportPreset: "desktop" | "tablet" | "mobile";
  onSetViewportPreset: (vp: "desktop" | "tablet" | "mobile") => void;
  aspectRatio: "auto" | "16-9" | "16-10";
  onSetAspectRatio: (ar: "auto" | "16-9" | "16-10") => void;
  isPresenting: boolean;
  onTogglePresenting: () => void;
  isLeftStructureHidden: boolean;
  onToggleLeftStructureHidden: () => void;
  highlightedRegionId: string;
  onRegionClick: (regionId: string, label: string) => void;
  customRawHtml?: string;
}

function useSandboxedIframeBridge(
  props: CompanionLiveHtmlRightPaneProps,
  allowDesignCdn: boolean
) {
  const [epoch, setEpoch] = useState(1);
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const port1Ref = useRef<MessagePort | null>(null);

  useEffect(() => {
    port1Ref.current?.close();
    port1Ref.current = null;
    setEpoch((prev) => prev + 1);
  }, [props.activeTab.id, props.selectedNodeId, props.structureLens, props.activeSlide.id, allowDesignCdn]);

  useEffect(() => () => { port1Ref.current?.close(); port1Ref.current = null; }, []);

  const handleIframeLoad = () => {
    port1Ref.current?.close();
    port1Ref.current = null;
    if (typeof MessageChannel === "undefined" || !iframeRef.current?.contentWindow) return;
    const channel = new MessageChannel();
    const currentEpoch = epoch;
    port1Ref.current = channel.port1;
    channel.port1.onmessage = (ev: MessageEvent) => {
      const parsed = IframeToHostMessageSchema.safeParse(ev.data);
      if (!parsed.success || parsed.data.epoch !== currentEpoch) {
        channel.port1.close();
        return;
      }
      if (parsed.data.type === "ELEMENT_CLICKED") {
        props.onRegionClick(parsed.data.selector, `${props.selectedNodeId || "SCR-02"} • ${parsed.data.selector}`);
      }
    };
    iframeRef.current.contentWindow.postMessage({ type: "INIT_PORT", epoch: currentEpoch }, "*", [channel.port2]);
  };

  return { epoch, iframeRef, handleIframeLoad };
}

export const CompanionLiveHtmlRightPane: React.FC<CompanionLiveHtmlRightPaneProps> = (props) => {
  const { t } = useTranslation();
  const { formatCurrency } = useFormatter();
  const [allowDesignCdn, setAllowDesignCdn] = useState(false);
  const [marquee, setMarquee] = useState<{
    isDragging: boolean;
    startX: number;
    startY: number;
    currentX: number;
    currentY: number;
  }>({
    isDragging: false,
    startX: 0,
    startY: 0,
    currentX: 0,
    currentY: 0,
  });
  const { epoch, iframeRef, handleIframeLoad } = useSandboxedIframeBridge(props, allowDesignCdn);

  const rawHtml = useMemo(() => {
    if (props.customRawHtml) return props.customRawHtml;
    return buildPreviewDocumentHtml({
      structureLens: props.structureLens,
      activeSlide: props.activeSlide,
      activeScreenData: props.activeScreenData,
      formattedAmount: formatCurrency(props.activeScreenData.amount, "USD"),
    });
  }, [props.customRawHtml, props.structureLens, props.activeSlide, props.activeScreenData, formatCurrency]);

  const wrappedSrcDoc = useMemo(() => wrapUserHtmlSrcDoc(rawHtml, { allowDesignCdn, epoch }), [rawHtml, allowDesignCdn, epoch]);
  const routeBadge = `${props.activeTab.title}#${props.structureLens === "userflow" ? props.selectedNodeId || "SCR-02" : props.activeSlide.code.split(" ")[0]}`;

  return (
    <div className="canvas-live-html-right-pane" aria-label={t("canvas.liveHtmlPane", "Live HTML Preview Sandbox")} style={{ flex: 1, minWidth: "280px", height: "100%", display: "flex", flexDirection: "column", backgroundColor: "var(--bg-surface)", overflow: "hidden" }}>
      <HtmlPreviewToolbar {...props} allowDesignCdn={allowDesignCdn} onToggleDesignCdn={() => setAllowDesignCdn((prev) => !prev)} />
      <div style={{ flex: 1, overflow: "auto", padding: "14px", backgroundColor: "var(--bg-subtle)", display: "flex", flexDirection: "column", alignItems: "center" }}>
        {props.isInlineCodeOpen ? (
          <InlineCodeEditorPanel activeTab={props.activeTab} inlineCodeText={props.inlineCodeText} onChangeInlineCode={props.onChangeInlineCode} onCloseEditor={props.onToggleInlineCode} />
        ) : (
          <div className="live-html-frame" data-quotable="true" data-project-preview="true" style={{ position: "relative", width: VIEWPORT_WIDTHS[props.viewportPreset], maxWidth: "820px", minHeight: "360px", flex: 1, aspectRatio: ASPECT_RATIO_VALUES[props.aspectRatio], backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-md)", boxShadow: "var(--shadow-sm)", overflow: "hidden", display: "flex", flexDirection: "column", transition: "width 0.18s ease" }}>
            <div style={{ height: "28px", padding: "0 12px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", fontFamily: "var(--font-mono)", fontSize: "10px", color: "var(--text-muted)" }}>
              <span>{routeBadge}</span>
              <span>{`${props.viewportPreset.toUpperCase()} • ${allowDesignCdn ? "DESIGN_CDN" : "STRICT_OFFLINE"}`}</span>
            </div>
            <div style={{ position: "relative", flex: 1, width: "100%", height: "100%", minHeight: "320px" }}>
              <iframe ref={iframeRef} title={t("canvas.sandboxIframeTitle", "Sandboxed Live HTML Preview")} data-testid="sandboxed-html-iframe" sandbox={IFRAME_SANDBOX_PERMISSIONS} referrerPolicy="no-referrer" srcDoc={wrappedSrcDoc} onLoad={handleIframeLoad} style={{ width: "100%", height: "100%", minHeight: "320px", border: "none", backgroundColor: "#FFFFFF" }} />
              {props.pointerMode === "drag-box" && (
                <div
                  className="marquee-drag-overlay"
                  data-testid="marquee-drag-overlay"
                  style={{
                    position: "absolute",
                    inset: 0,
                    zIndex: 20,
                    cursor: "crosshair",
                    userSelect: "none",
                  }}
                  onMouseDown={(e) => {
                    const rect = e.currentTarget.getBoundingClientRect();
                    setMarquee({
                      isDragging: true,
                      startX: e.clientX - rect.left,
                      startY: e.clientY - rect.top,
                      currentX: e.clientX - rect.left,
                      currentY: e.clientY - rect.top,
                    });
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
                    const endX = e.clientX - rect.left;
                    const endY = e.clientY - rect.top;
                    const w = Math.round(Math.abs(endX - marquee.startX));
                    const h = Math.round(Math.abs(endY - marquee.startY));

                    setMarquee((prev) => ({ ...prev, isDragging: false }));

                    // Filter boundary: ignore drags < 16x16 px
                    if (w >= 16 && h >= 16) {
                      const left = Math.round(Math.min(marquee.startX, endX));
                      const top = Math.round(Math.min(marquee.startY, endY));
                      props.onRegionClick(
                        `box-${left}-${top}-${w}x${h}`,
                        `Vùng ${w}×${h}px`
                      );
                    }
                  }}
                >
                  {marquee.isDragging && (() => {
                    const left = Math.min(marquee.startX, marquee.currentX);
                    const top = Math.min(marquee.startY, marquee.currentY);
                    const w = Math.abs(marquee.currentX - marquee.startX);
                    const h = Math.abs(marquee.currentY - marquee.startY);
                    return (
                      <div
                        style={{
                          position: "absolute",
                          left: `${left}px`,
                          top: `${top}px`,
                          width: `${w}px`,
                          height: `${h}px`,
                          border: "1.5px dashed var(--action-primary)",
                          backgroundColor: "rgba(0, 87, 255, 0.12)",
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
                          {Math.round(w)} × {Math.round(h)} px
                        </span>
                      </div>
                    );
                  })()}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
