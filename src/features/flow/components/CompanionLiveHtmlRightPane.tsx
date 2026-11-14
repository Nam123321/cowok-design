// @story 04.1
// @implements AC-03, AC-07
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { CanvasTabItem } from "./CompanionCanvasTab";
import type { ScreenPreviewData, SlidePhaseItem } from "./companion-canvas-data";
import { HtmlPreviewToolbar } from "./HtmlPreviewToolbar";
import {
  buildPreviewDocumentHtml,
  generateEpochNonce,
  IFRAME_SANDBOX_PERMISSIONS,
  IframeToHostMessageSchema,
  validateUserHtmlPayload,
  wrapUserHtmlSrcDoc,
} from "../utils/html-sandbox-wrapper";
import { normalizeViewportPreset, type ViewportPreset } from "../../../lib/validations/screen-fixture";

const VIEWPORT_WIDTHS: Record<ViewportPreset, string> = {
  "mobile-390": "375px",
  "tablet-768": "620px",
  "desktop-1440": "100%",
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
  onApplyHtml: () => void;
  errorMsg?: string | null;
}> = ({ activeTab, inlineCodeText, onChangeInlineCode, onApplyHtml, errorMsg }) => {
  const { t } = useTranslation();
  return (
    <div style={{ width: "100%", height: "100%", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-md)", display: "flex", flexDirection: "column", overflow: "hidden" }}>
      <div style={{ height: "30px", padding: "0 12px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", fontFamily: "var(--font-mono)", fontSize: "11px" }}>
        <span>&lt;/&gt; {activeTab.filePath || activeTab.title}</span>
        <button type="button" data-testid="apply-inline-html-btn" onClick={onApplyHtml} className="cowok-btn cowok-btn-primary" style={{ height: "22px", padding: "0 8px", fontSize: "10.5px", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)" }}>
          {t("canvas.applyToHtml", "✓ Áp dụng lên HTML")}
        </button>
      </div>
      {errorMsg && (
        <div
          role="alert"
          data-testid="inline-html-error"
          style={{
            padding: "6px 12px",
            backgroundColor: "var(--bg-subtle)",
            borderBottom: "1px solid var(--state-error)",
            color: "var(--state-error)",
            fontSize: "11px",
            fontFamily: "var(--font-mono)",
          }}
        >
          {errorMsg}
        </div>
      )}
      <textarea
        data-testid="inline-html-textarea"
        value={inlineCodeText}
        onChange={(e) => onChangeInlineCode(e.target.value)}
        spellCheck={false}
        style={{ flex: 1, width: "100%", padding: "12px", border: "none", backgroundColor: "var(--bg-canvas)", color: "var(--text-primary)", fontFamily: "var(--font-mono)", fontSize: "12px", lineHeight: "1.55", resize: "none", outline: "none", boxSizing: "border-box" }}
      />
    </div>
  );
};

export interface CompanionLiveHtmlRightPaneProps {
  activeTab: CanvasTabItem;
  structureLens: "userflow" | "slide-cms";
  selectedNodeId?: string | null;
  selectedNodeKey?: string | null;
  activeSlide: SlidePhaseItem;
  activeScreenData: ScreenPreviewData;
  pointerMode: "interact" | "click-dom" | "drag-box";
  onSetPointerMode: (mode: "interact" | "click-dom" | "drag-box") => void;
  isInlineCodeOpen: boolean;
  onToggleInlineCode: () => void;
  inlineCodeText: string;
  onChangeInlineCode: (val: string) => void;
  viewportPreset: ViewportPreset | "desktop" | "tablet" | "mobile";
  onSetViewportPreset: (vp: ViewportPreset) => void;
  aspectRatio: "auto" | "16-9" | "16-10";
  onSetAspectRatio: (ar: "auto" | "16-9" | "16-10") => void;
  isPresenting: boolean;
  onTogglePresenting: () => void;
  isLeftStructureHidden: boolean;
  onToggleLeftStructureHidden: () => void;
  highlightedRegionId: string;
  onRegionClick: (regionId: string, label: string) => void;
  customRawHtml?: string;
  onApplyHtml?: (html: string) => void;
}

const PREVIEW_HOST_TOKEN_KEYS = [
  "--bg-canvas",
  "--bg-surface",
  "--text-primary",
  "--text-secondary",
  "--text-muted",
  "--border-default",
  "--action-primary",
  "--text-on-action",
  "--human-accent",
  "--state-success",
  "--font-sans",
] as const;

function readHostThemeCssVars(): Record<string, string> | undefined {
  if (typeof window === "undefined" || typeof document === "undefined" || !document.documentElement) {
    return undefined;
  }
  const styles = window.getComputedStyle(document.documentElement);
  const resolved: Record<string, string> = {};
  for (const key of PREVIEW_HOST_TOKEN_KEYS) {
    const val = styles.getPropertyValue(key).trim();
    if (val) resolved[key] = val;
  }
  return Object.keys(resolved).length > 0 ? resolved : undefined;
}

function isSandboxFrameAllowedToBind(iframeEl: HTMLIFrameElement): boolean {
  const sandboxAttr = iframeEl.getAttribute("sandbox") || "";
  if (!sandboxAttr.includes("allow-scripts") || sandboxAttr.includes("allow-same-origin")) {
    return false;
  }
  try {
    const href = iframeEl.contentWindow?.location?.href;
    if (href && href !== "about:srcdoc" && href !== "about:blank") {
      return false;
    }
  } catch {
    // Opaque origin SecurityError is expected for allow-scripts srcDoc iframes without allow-same-origin.
  }
  return true;
}

function bindSandboxedPort(
  iframeEl: HTMLIFrameElement,
  epoch: number,
  nodeLabel: string,
  onRegionClick: (regionId: string, label: string) => void
): MessagePort | null {
  if (typeof MessageChannel === "undefined" || !iframeEl.contentWindow) return null;
  const channel = new MessageChannel();
  channel.port1.onmessage = (ev: MessageEvent) => {
    const parsed = IframeToHostMessageSchema.safeParse(ev.data);
    if (!parsed.success || parsed.data.epoch !== epoch) {
      channel.port1.close();
      return;
    }
    if (parsed.data.type === "ELEMENT_CLICKED") {
      onRegionClick(parsed.data.selector, `${nodeLabel} • ${parsed.data.selector}`);
    }
  };
  iframeEl.contentWindow.postMessage({ type: "INIT_PORT", epoch }, "*", [channel.port2]);
  return channel.port1;
}

function useSandboxedIframeBridge(
  props: CompanionLiveHtmlRightPaneProps,
  allowDesignCdn: boolean,
  effectiveCustomHtml?: string
) {
  const [epoch, setEpoch] = useState<number>(() => generateEpochNonce());
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const port1Ref = useRef<MessagePort | null>(null);
  const boundEpochRef = useRef<number | null>(null);
  const isFirstMountRef = useRef(true);

  useEffect(() => {
    if (isFirstMountRef.current) {
      isFirstMountRef.current = false;
      return;
    }
    port1Ref.current?.close();
    port1Ref.current = null;
    boundEpochRef.current = null;
    setEpoch(generateEpochNonce());
  }, [props.activeTab.id, props.selectedNodeId, props.structureLens, props.activeSlide.id, allowDesignCdn, effectiveCustomHtml]);

  useEffect(() => () => { port1Ref.current?.close(); port1Ref.current = null; }, []);

  const handleIframeLoad = () => {
    const iframeEl = iframeRef.current;
    if (!iframeEl || !isSandboxFrameAllowedToBind(iframeEl) || boundEpochRef.current === epoch) return;
    port1Ref.current?.close();
    const nodeLabel = props.selectedNodeKey || props.selectedNodeId || "SCR-02";
    const port1 = bindSandboxedPort(iframeEl, epoch, nodeLabel, props.onRegionClick);
    port1Ref.current = port1;
    if (port1) boundEpochRef.current = epoch;
  };

  const postApplyHtml = useCallback((html: string) => {
    port1Ref.current?.postMessage({ type: "APPLY_HTML", epoch, html });
  }, [epoch]);

  return { epoch, iframeRef, handleIframeLoad, postApplyHtml };
}

const INVALID_HTML_MSG_MAP: Record<"empty" | "too_large" | "invalid_html", { key: string; fallback: string }> = {
  empty: { key: "canvas.invalidHtmlEmpty", fallback: "HTML cannot be empty." },
  too_large: { key: "canvas.invalidHtmlTooLarge", fallback: "HTML payload exceeds 1 MB limit." },
  invalid_html: { key: "canvas.invalidHtmlFormat", fallback: "Please enter valid HTML markup before applying." },
};

const MIN_MARQUEE_PX = 16;

const MarqueeSelectionOverlay: React.FC<{
  onRegionClick: (regionId: string, label: string) => void;
}> = ({ onRegionClick }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const [marquee, setMarquee] = useState({ isDragging: false, startX: 0, startY: 0, currentX: 0, currentY: 0 });
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
        setMarquee((prev) => ({ ...prev, currentX: e.clientX - rect.left, currentY: e.clientY - rect.top }));
      }}
      onMouseUp={(e) => {
        if (!marquee.isDragging) return;
        const rect = e.currentTarget.getBoundingClientRect();
        const rw = Math.round(Math.abs(e.clientX - rect.left - marquee.startX));
        const rh = Math.round(Math.abs(e.clientY - rect.top - marquee.startY));
        const rLeft = Math.round(Math.min(marquee.startX, e.clientX - rect.left));
        const rTop = Math.round(Math.min(marquee.startY, e.clientY - rect.top));
        setMarquee((prev) => ({ ...prev, isDragging: false }));
        if (rw >= MIN_MARQUEE_PX && rh >= MIN_MARQUEE_PX) {
          onRegionClick(`box-${rLeft}-${rTop}-${rw}x${rh}`, `${t("canvas.marqueeRegionPrefix", "Vùng")} ${formatNumber(rw)}×${formatNumber(rh)}px`);
        }
      }}
    >
      {marquee.isDragging && (
        <div style={{ position: "absolute", left: `${left}px`, top: `${top}px`, width: `${w}px`, height: `${h}px`, border: "1.5px dashed var(--action-primary)", backgroundColor: "var(--marquee-fill)", pointerEvents: "none", boxSizing: "border-box" }}>
          <span style={{ position: "absolute", bottom: "-20px", right: 0, fontSize: "10px", fontFamily: "var(--font-mono)", backgroundColor: "var(--bg-elevated)", border: "1px solid var(--border-default)", padding: "1px 5px", borderRadius: "var(--radius-xs)", color: "var(--text-primary)", whiteSpace: "nowrap" }}>
            {formatNumber(Math.round(w))} × {formatNumber(Math.round(h))} px
          </span>
        </div>
      )}
    </div>
  );
};

export const CompanionLiveHtmlRightPane: React.FC<CompanionLiveHtmlRightPaneProps> = (props) => {
  const { t, locale } = useTranslation();
  const { formatCurrency } = useFormatter({ locale });
  const [allowDesignCdn, setAllowDesignCdn] = useState(false);
  const [localCustomRawHtml, setLocalCustomRawHtml] = useState<string | undefined>(undefined);
  const [htmlEditorError, setHtmlEditorError] = useState<string | null>(null);
  const effectiveCustomHtml = props.customRawHtml ?? localCustomRawHtml;
  const { epoch, iframeRef, handleIframeLoad, postApplyHtml } = useSandboxedIframeBridge(props, allowDesignCdn, effectiveCustomHtml);

  const rawHtml = useMemo(() => {
    if (effectiveCustomHtml) return effectiveCustomHtml;
    const rawAmount = props.structureLens === "slide-cms" ? props.activeSlide.kpi1 : props.activeScreenData.amount;
    const formattedAmount = typeof rawAmount === "number" ? formatCurrency(rawAmount, "USD") : rawAmount;
    return buildPreviewDocumentHtml({
      structureLens: props.structureLens,
      activeSlide: props.activeSlide,
      activeScreenData: props.activeScreenData,
      formattedAmount,
      labels: {
        lang: locale,
        kpi1Label: t("canvas.previewKpiLimitLabel", "Transaction Limit"),
        kpi2Label: t("canvas.previewKpiSecurityLabel", "Security Status"),
        kpi3Label: t("canvas.previewKpiLatencyLabel", "Response Time"),
        actionHint: t("canvas.previewFallbackHint", "Auto-synced live HTML preview from current graph state."),
      },
      cssVars: readHostThemeCssVars(),
    });
  }, [effectiveCustomHtml, props.structureLens, props.activeSlide, props.activeScreenData, formatCurrency, locale, t]);

  const handleApplyInlineHtml = useCallback(() => {
    const check = validateUserHtmlPayload(props.inlineCodeText);
    if (!check.valid) {
      const entry = INVALID_HTML_MSG_MAP[check.reason ?? "invalid_html"];
      setHtmlEditorError(t(entry.key, entry.fallback));
      return;
    }
    setHtmlEditorError(null);
    postApplyHtml(props.inlineCodeText);
    if (props.onApplyHtml) props.onApplyHtml(props.inlineCodeText);
    else setLocalCustomRawHtml(props.inlineCodeText);
    props.onToggleInlineCode();
  }, [props, postApplyHtml, t]);

  const handleChangeInlineCode = useCallback(
    (val: string) => {
      if (htmlEditorError) setHtmlEditorError(null);
      props.onChangeInlineCode(val);
    },
    [htmlEditorError, props]
  );

  const wrappedSrcDoc = useMemo(() => wrapUserHtmlSrcDoc(rawHtml, { allowDesignCdn, epoch }), [rawHtml, allowDesignCdn, epoch]);
  const nodeBadgeKey = props.selectedNodeKey || props.selectedNodeId || "SCR-02";
  const routeBadge = `${props.activeTab.title}#${props.structureLens === "userflow" ? nodeBadgeKey : props.activeSlide.code.split(" ")[0]}`;

  return (
    <div className="canvas-live-html-right-pane" aria-label={t("canvas.liveHtmlPane", "Live HTML Preview Sandbox")} style={{ flex: 1, minWidth: "280px", height: "100%", display: "flex", flexDirection: "column", backgroundColor: "var(--bg-surface)", overflow: "hidden" }}>
      <HtmlPreviewToolbar {...props} allowDesignCdn={allowDesignCdn} onToggleDesignCdn={() => setAllowDesignCdn((prev) => !prev)} />
      <div style={{ flex: 1, overflow: "auto", padding: "14px", backgroundColor: "var(--bg-subtle)", display: "flex", flexDirection: "column", alignItems: "center" }}>
        {props.isInlineCodeOpen ? (
          <InlineCodeEditorPanel activeTab={props.activeTab} inlineCodeText={props.inlineCodeText} onChangeInlineCode={handleChangeInlineCode} onApplyHtml={handleApplyInlineHtml} errorMsg={htmlEditorError} />
        ) : (
          <div className="live-html-frame" data-quotable="true" data-project-preview="true" style={{ position: "relative", width: VIEWPORT_WIDTHS[normalizeViewportPreset(props.viewportPreset)], maxWidth: "820px", minHeight: "360px", flex: 1, aspectRatio: ASPECT_RATIO_VALUES[props.aspectRatio], backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-md)", boxShadow: "var(--shadow-sm)", overflow: "hidden", display: "flex", flexDirection: "column", transition: "width 0.18s ease" }}>
            <div style={{ height: "28px", padding: "0 12px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", fontFamily: "var(--font-mono)", fontSize: "10px", color: "var(--text-muted)" }}>
              <span>{routeBadge}</span>
              <span>{`${normalizeViewportPreset(props.viewportPreset).toUpperCase()} • ${allowDesignCdn ? "DESIGN_CDN" : "STRICT_OFFLINE"}`}</span>
            </div>
            <div style={{ position: "relative", flex: 1, width: "100%", height: "100%", minHeight: "320px" }}>
              <iframe ref={iframeRef} title={t("canvas.sandboxIframeTitle", "Sandboxed Live HTML Preview")} data-testid="sandboxed-html-iframe" sandbox={IFRAME_SANDBOX_PERMISSIONS} referrerPolicy="no-referrer" srcDoc={wrappedSrcDoc} onLoad={handleIframeLoad} style={{ width: "100%", height: "100%", minHeight: "320px", border: "none", backgroundColor: "var(--bg-surface)" }} />
              {props.pointerMode === "drag-box" && <MarqueeSelectionOverlay onRegionClick={props.onRegionClick} />}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
