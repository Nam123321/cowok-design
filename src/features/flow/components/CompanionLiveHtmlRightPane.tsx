// @story 04.1, 04.5, 05.1
// @implements AC-01, AC-02, AC-03, AC-04 (IframeKeepAlivePool LRU-5 Live Preview Pane + 3-Way Comments)
import React, { useCallback, useEffect, useMemo, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import type { ExportInitialSection } from "../../snapshots/components/ExportBundleModal";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { CanvasTabItem } from "./CompanionCanvasTab";
import type { ScreenPreviewData, SlidePhaseItem } from "./companion-canvas-data";
import { HtmlPreviewToolbar } from "./HtmlPreviewToolbar";
import { InlineCodeEditorPanel } from "./InlineCodeEditorPanel";
import { MarqueeSelectionOverlay } from "./MarqueeSelectionOverlay";
import { CommentPinMarker } from "../../../components/comments/CommentPinMarker";
import { ElementCommentThread } from "../../../components/comments/ElementCommentThread";
import type { ElementCommentResponse } from "../../../lib/validations/element-comment";
import {
  buildPreviewDocumentHtml,
  IFRAME_SANDBOX_PERMISSIONS,
  validateUserHtmlPayload,
  wrapUserHtmlSrcDoc,
} from "../utils/html-sandbox-wrapper";
import {
  useSandboxedIframeBridge,
  type ElementRectBox,
  type IframeKeepAliveSlot,
} from "../hooks/useSandboxedIframeBridge";
import { usePreviewPaneActions } from "../hooks/usePreviewPaneActions";
import {
  normalizeFixtureRef,
  normalizeViewportPreset,
  type FixtureVariant,
  type ViewportPreset,
} from "../../../lib/validations/screen-fixture";

const PIN_EDGE_PADDING_PX = 4;
const PIN_OFFSET_PX = 14;
const PIN_Z_INDEX = 15;
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
const INVALID_HTML_MSG_MAP: Record<"empty" | "too_large" | "invalid_html", { key: string; fallback: string }> = {
  empty: { key: "canvas.invalidHtmlEmpty", fallback: "HTML cannot be empty." },
  too_large: { key: "canvas.invalidHtmlTooLarge", fallback: "HTML payload exceeds 1 MB limit." },
  invalid_html: { key: "canvas.invalidHtmlFormat", fallback: "Please enter valid HTML markup before applying." },
};

function readHostThemeCssVars(): Record<string, string> | undefined {
  if (typeof window === "undefined" || typeof document === "undefined" || !document.documentElement) return undefined;
  const styles = window.getComputedStyle(document.documentElement);
  const resolved: Record<string, string> = {};
  for (const key of PREVIEW_HOST_TOKEN_KEYS) {
    const val = styles.getPropertyValue(key).trim();
    if (val) resolved[key] = val;
  }
  return Object.keys(resolved).length > 0 ? resolved : undefined;
}

export interface CompanionLiveHtmlRightPaneProps {
  projectId?: string;
  activeTab: CanvasTabItem;
  structureLens: "userflow" | "slide-cms";
  selectedNodeId?: string | null;
  selectedNodeKey?: string | null;
  activeSlide: SlidePhaseItem;
  activeScreenData: ScreenPreviewData;
  activeSubState?: FixtureVariant;
  onSelectSubState?: (variant: FixtureVariant) => void;
  pointerMode: "interact" | "click-dom" | "drag-box";
  onSetPointerMode: (mode: "interact" | "click-dom" | "drag-box") => void;
  isInlineCodeOpen: boolean;
  dirtyBuffer?: boolean;
  showDirtyGuardPrompt?: boolean;
  onToggleInlineCode: () => void;
  onConfirmDiscardDirty?: () => void;
  onCancelCloseEditor?: () => void;
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
  onQuoteText?: (text: string) => void;
  customRawHtml?: string;
  onApplyHtml?: (html: string) => void;
  onHoverCta?: (ffId: string, hovered: boolean) => void;
  isSampleDataOpen?: boolean;
  onToggleSampleData?: () => void;
  isWalkthroughActive?: boolean;
  onToggleWalkthrough?: () => void;
  walkthroughViewMode?: "dual" | "full";
  onSetWalkthroughViewMode?: (mode: "dual" | "full") => void;
  walkthroughStepLabel?: string;
  walkthroughAutoPlay?: boolean;
  onToggleWalkthroughAutoPlay?: () => void;
  onWalkthroughPrev?: () => void;
  onWalkthroughNext?: () => void;
  onExitWalkthrough?: () => void;
  snapshotsCount?: number;
  onOpenVersionHistory?: () => void;
  onOpenExportSection?: (section: ExportInitialSection) => void;
  versionMenuNode?: React.ReactNode;
  isReadOnly?: boolean;
}

const PreviewDeviceFrame: React.FC<{
  routeBadge: string;
  viewportPreset: ViewportPreset | "desktop" | "tablet" | "mobile";
  activeSubState: FixtureVariant;
  aspectRatio: "auto" | "16-9" | "16-10";
  isPortVerified: boolean;
  wrappedSrcDoc: string;
  iframeRef: React.RefObject<HTMLIFrameElement | null>;
  onLoad: () => void;
  pointerMode: "interact" | "click-dom" | "drag-box";
  visiblePins: ElementCommentResponse[];
  elementRects: Record<string, ElementRectBox>;
  poolSlots?: IframeKeepAliveSlot[];
  activeSlotId?: string;
  onSlotLoad?: (slotId: string, el: HTMLIFrameElement | null) => void;
  onOpenComments: () => void;
  onRegionClick: (regionId: string, label: string) => void;
}> = (p) => {
  const { t } = useTranslation();
  const vpLabel = normalizeViewportPreset(p.viewportPreset).toUpperCase();
  const statusTag = p.isPortVerified ? "CSPRNG_VERIFIED" : "STRICT_OFFLINE";

  return (
    <div
      className="live-html-frame"
      data-quotable="true"
      data-project-preview="true"
      style={{
        position: "relative",
        width: VIEWPORT_WIDTHS[normalizeViewportPreset(p.viewportPreset)],
        maxWidth: "820px",
        minHeight: "360px",
        flex: 1,
        aspectRatio: ASPECT_RATIO_VALUES[p.aspectRatio],
        backgroundColor: "var(--bg-surface)",
        border: "1px solid var(--border-default)",
        borderRadius: "var(--radius-md)",
        boxShadow: "var(--shadow-sm)",
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
        transition: "width 0.18s ease",
      }}
    >
      <div
        style={{
          height: "28px",
          padding: "0 12px",
          backgroundColor: "var(--bg-workspace)",
          borderBottom: "1px solid var(--border-default)",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          fontFamily: "var(--font-mono)",
          fontSize: "10px",
          color: "var(--text-muted)",
        }}
      >
        <span>{p.routeBadge}</span>
        <span data-testid="bridge-status-pill">{`${vpLabel} • ${p.activeSubState} • ${statusTag}`}</span>
      </div>
      <div style={{ position: "relative", flex: 1, width: "100%", height: "100%", minHeight: "320px" }}>
        {p.poolSlots && p.poolSlots.length > 0 ? (
          p.poolSlots.map((slot) => {
            const isActive = slot.slotId === p.activeSlotId;
            return (
              <iframe
                key={slot.slotId}
                ref={slot.iframeRef}
                title={t("canvas.sandboxIframeTitle", "Sandboxed Live HTML Preview")}
                data-testid={isActive ? "sandboxed-html-iframe" : `sandboxed-html-iframe-cached-${slot.nodeId}`}
                sandbox={IFRAME_SANDBOX_PERMISSIONS}
                referrerPolicy="no-referrer"
                srcDoc={slot.wrappedSrcDoc}
                onLoad={(e) => p.onSlotLoad?.(slot.slotId, e.currentTarget)}
                style={{
                  width: "100%",
                  height: "100%",
                  minHeight: "320px",
                  border: "none",
                  backgroundColor: "var(--bg-surface)",
                  visibility: isActive ? "visible" : "hidden",
                  pointerEvents: isActive ? "auto" : "none",
                  position: isActive ? "relative" : "absolute",
                  left: 0,
                  top: 0,
                }}
              />
            );
          })
        ) : (
          <iframe
            ref={p.iframeRef}
            title={t("canvas.sandboxIframeTitle", "Sandboxed Live HTML Preview")}
            data-testid="sandboxed-html-iframe"
            sandbox={IFRAME_SANDBOX_PERMISSIONS}
            referrerPolicy="no-referrer"
            srcDoc={p.wrappedSrcDoc}
            onLoad={p.onLoad}
            style={{
              width: "100%",
              height: "100%",
              minHeight: "320px",
              border: "none",
              backgroundColor: "var(--bg-surface)",
            }}
          />
        )}
        {p.visiblePins.map((c, idx) => {
          const rect = p.elementRects[c.elementFfId];
          if (!rect) return null;
          return (
            <div
              key={c.id}
              style={{
                position: "absolute",
                left: `${Math.max(PIN_EDGE_PADDING_PX, rect.left + rect.width - PIN_OFFSET_PX)}px`,
                top: `${Math.max(PIN_EDGE_PADDING_PX, rect.top)}px`,
                zIndex: PIN_Z_INDEX,
              }}
            >
              <CommentPinMarker
                index={idx + 1}
                anchorStatus={c.anchorStatus}
                elementFfId={c.elementFfId}
                onClick={p.onOpenComments}
              />
            </div>
          );
        })}
        {(p.pointerMode === "click-dom" || p.pointerMode === "drag-box") && (
          <MarqueeSelectionOverlay
            nodeLabel={p.routeBadge}
            elementRects={p.elementRects}
            onRegionClick={p.onRegionClick}
          />
        )}
      </div>
    </div>
  );
};

function useRawPreviewHtml(
  props: CompanionLiveHtmlRightPaneProps,
  activeSubState: FixtureVariant,
  effectiveCustomHtml?: string
): string {
  const { t, locale } = useTranslation();
  const { formatCurrency } = useFormatter({ locale });
  return useMemo(() => {
    if (effectiveCustomHtml) return effectiveCustomHtml;
    const rawAmount =
      props.structureLens === "slide-cms"
        ? (props.activeSlide?.kpi1 ?? "$0")
        : (props.activeScreenData?.amount ?? "$0");
    const formattedAmount = typeof rawAmount === "number" ? formatCurrency(rawAmount, "USD") : rawAmount;
    const fallbackScreenData: ScreenPreviewData = props.activeScreenData || {
      eyebrow: "SCREEN",
      headline: "Screen State",
      subtitle: "Preview",
      statusText: "Ready",
      latency: "12ms",
      actionTitle: "Action",
      ctaLabel: "Open",
    };
    return buildPreviewDocumentHtml({
      structureLens: props.structureLens,
      activeSlide: props.activeSlide,
      activeScreenData: fallbackScreenData,
      formattedAmount,
      subState: activeSubState,
      cssVars: readHostThemeCssVars(),
      labels: {
        lang: locale,
        kpi1Label: t("canvas.previewKpiLimitLabel", "Transaction Limit"),
        kpi2Label: t("canvas.previewKpiSecurityLabel", "Security Status"),
        kpi3Label: t("canvas.previewKpiLatencyLabel", "Response Time"),
        actionHint: t("canvas.previewFallbackHint", "Auto-synced live HTML preview from current graph state."),
      },
    });
  }, [
    activeSubState,
    effectiveCustomHtml,
    formatCurrency,
    locale,
    props.activeScreenData,
    props.activeSlide,
    props.structureLens,
    t,
  ]);
}

export const CompanionLiveHtmlRightPane: React.FC<CompanionLiveHtmlRightPaneProps> = (props) => {
  const { t } = useTranslation();
  const [localCustomRawHtml, setLocalCustomRawHtml] = useState<string | undefined>(undefined);
  const [localSubState, setLocalSubState] = useState<FixtureVariant>("DEFAULT");
  const [htmlEditorError, setHtmlEditorError] = useState<string | null>(null);
  const [isCommentsDrawerOpen, setIsCommentsDrawerOpen] = useState(false);

  const activeSubState = normalizeFixtureRef(props.activeSubState ?? localSubState);
  const effectiveCustomHtml = props.customRawHtml ?? localCustomRawHtml;
  const rawHtml = useRawPreviewHtml(props, activeSubState, effectiveCustomHtml);

  const bridge = useSandboxedIframeBridge({
    projectId: props.projectId,
    activeTabId: props.activeTab.id,
    selectedNodeId: props.selectedNodeId,
    selectedNodeKey: props.selectedNodeKey,
    structureLens: props.structureLens,
    activeSlideId: props.activeSlide.id,
    activeSubState,
    effectiveCustomHtml,
    rawHtml,
    onQuoteText: props.onQuoteText,
    onRegionClick: (selector, label, ffId) => {
      if (ffId) actions.setPrefillFfId(ffId);
      props.onRegionClick(selector, label);
    },
  });

  const onSelectSubStateProp = props.onSelectSubState;
  const handleSelectSubState = useCallback(
    (v: FixtureVariant) => {
      setLocalSubState(v);
      bridge.postFixtureHydrate(v);
      onSelectSubStateProp?.(v);
    },
    [bridge, onSelectSubStateProp]
  );

  const actions = usePreviewPaneActions({
    projectId: props.projectId,
    selectedNodeId: props.selectedNodeId,
    artifactSha256: bridge.artifactSha256,
    elementRects: bridge.elementRects,
    postFixtureHydrate: bridge.postFixtureHydrate,
    onSetViewportPreset: props.onSetViewportPreset,
    onSelectSubState: handleSelectSubState,
    isReadOnly: props.isReadOnly,
  });

  const handleApplyInlineHtml = useCallback(() => {
    const check = validateUserHtmlPayload(props.inlineCodeText);
    if (!check.valid) {
      const e = INVALID_HTML_MSG_MAP[check.reason ?? "invalid_html"];
      setHtmlEditorError(t(e.key, e.fallback));
      return;
    }
    setHtmlEditorError(null);
    bridge.postApplyHtml(props.inlineCodeText);
    if (props.onApplyHtml) {
      props.onApplyHtml(props.inlineCodeText);
    } else {
      setLocalCustomRawHtml(props.inlineCodeText);
      props.onToggleInlineCode();
    }
  }, [bridge, props, t]);

  useEffect(() => {
    const onSampleFixture = (ev: Event) => {
      const detail = (ev as CustomEvent<{ projectId?: string; nodeId?: string; variant?: FixtureVariant; fixtureJson?: string }>).detail;
      if (!detail?.fixtureJson) return;
      if (detail.projectId && props.projectId && detail.projectId !== props.projectId) return;
      const nextVar = detail.variant || "DEFAULT";
      setLocalSubState(nextVar);
      bridge.postFixtureHydrate(nextVar, detail.fixtureJson);
    };
    window.addEventListener("cowok:sample-fixture-applied", onSampleFixture);
    return () => window.removeEventListener("cowok:sample-fixture-applied", onSampleFixture);
  }, [bridge, props.projectId]);

  const wrappedSrcDoc = useMemo(
    () =>
      wrapUserHtmlSrcDoc(rawHtml, {
        epoch: bridge.epoch,
        bootSecret: bridge.bootSecret,
        artifactSha256: bridge.artifactSha256,
      }),
    [bridge.artifactSha256, bridge.bootSecret, bridge.epoch, rawHtml]
  );

  const nodeBadgeKey = props.selectedNodeKey || props.selectedNodeId || "SCR-02";
  const routeBadge = `${props.activeTab.title}#${
    props.structureLens === "userflow" ? nodeBadgeKey : props.activeSlide.code.split(" ")[0]
  }`;
  const visiblePins = actions.comments.filter((c) => {
    const r = bridge.elementRects[c.elementFfId];
    return Boolean(r && r.width > 0 && r.height > 0);
  });

  return (
    <div
      className="canvas-live-html-right-pane"
      aria-label={t("canvas.liveHtmlPane", "Live HTML Preview Sandbox")}
      style={{
        flex: 1,
        minWidth: "480px",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        backgroundColor: "var(--bg-surface)",
        overflow: "hidden",
        position: "relative",
      }}
    >
      <HtmlPreviewToolbar
        pointerMode={props.pointerMode}
        onSetPointerMode={props.onSetPointerMode}
        activeSubState={activeSubState}
        onSelectSubState={(v) => {
          void actions.handleSelectFixtureVariant(v);
        }}
        isInlineCodeOpen={props.isInlineCodeOpen}
        dirtyBuffer={props.dirtyBuffer}
        onToggleInlineCode={props.onToggleInlineCode}
        viewportPreset={props.viewportPreset}
        onSetViewportPreset={(vp) => {
          void actions.handlePersistViewport(vp);
        }}
        aspectRatio={props.aspectRatio}
        onSetAspectRatio={props.onSetAspectRatio}
        isPresenting={props.isPresenting}
        onTogglePresenting={props.onTogglePresenting}
        isLeftStructureHidden={props.isLeftStructureHidden}
        onToggleLeftStructureHidden={props.onToggleLeftStructureHidden}
        isCommentsDrawerOpen={isCommentsDrawerOpen}
        commentCount={actions.comments.length}
        onToggleCommentsDrawer={() => setIsCommentsDrawerOpen((v) => !v)}
        isSampleDataOpen={props.isSampleDataOpen}
        onToggleSampleData={props.onToggleSampleData}
        isWalkthroughActive={props.isWalkthroughActive}
        onToggleWalkthrough={props.onToggleWalkthrough}
        snapshotsCount={props.snapshotsCount}
        onOpenVersionHistory={props.onOpenVersionHistory}
        onOpenExportSection={props.onOpenExportSection}
        isReadOnly={props.isReadOnly}
      />
      <div
        style={{
          flex: 1,
          overflow: "auto",
          padding: "14px",
          backgroundColor: "var(--bg-subtle)",
          display: "flex",
          gap: "12px",
          justifyContent: "center",
        }}
      >
        {props.isInlineCodeOpen ? (
          <InlineCodeEditorPanel
            activeTab={props.activeTab}
            inlineCodeText={props.inlineCodeText}
            dirtyBuffer={Boolean(props.dirtyBuffer)}
            showDirtyGuardPrompt={Boolean(props.showDirtyGuardPrompt)}
            errorMsg={htmlEditorError}
            onChangeInlineCode={(val) => {
              if (htmlEditorError) setHtmlEditorError(null);
              props.onChangeInlineCode(val);
            }}
            onApplyHtml={handleApplyInlineHtml}
            onConfirmDiscardDirty={props.onConfirmDiscardDirty}
            onCancelCloseEditor={props.onCancelCloseEditor}
          />
        ) : (
          <PreviewDeviceFrame
            routeBadge={routeBadge}
            viewportPreset={props.viewportPreset}
            activeSubState={activeSubState}
            aspectRatio={props.aspectRatio}
            isPortVerified={bridge.isPortVerified}
            wrappedSrcDoc={bridge.wrappedSrcDoc || wrappedSrcDoc}
            iframeRef={bridge.iframeRef}
            onLoad={bridge.handleIframeLoad}
            poolSlots={bridge.poolSlots}
            activeSlotId={bridge.activeSlotId}
            onSlotLoad={bridge.handleSlotIframeLoad}
            pointerMode={props.pointerMode}
            visiblePins={visiblePins}
            elementRects={bridge.elementRects}
            onOpenComments={() => setIsCommentsDrawerOpen(true)}
            onRegionClick={props.onRegionClick}
          />
        )}
        {isCommentsDrawerOpen && !props.isInlineCodeOpen && (
          <aside
            data-testid="comments-right-drawer"
            style={{
              width: "280px",
              flexShrink: 0,
              backgroundColor: "var(--bg-surface)",
              border: "1px solid var(--border-default)",
              borderRadius: "var(--radius-md)",
              padding: "12px",
              overflowY: "auto",
            }}
          >
            <ElementCommentThread
              nodeId={props.selectedNodeId || "SCR-02"}
              comments={actions.comments}
              prefillFfId={actions.prefillFfId}
              collapsedCommentIds={actions.collapsedCommentIds}
              staleToastMessage={actions.staleToastMessage}
              toastTone={actions.toastTone}
              onDismissStaleToast={() => actions.setStaleToastMessage(null)}
              onAddComment={(c, ffId) => actions.handleAddComment(c, ffId)}
              onResolveComment={(id) => {
                void actions.handleResolveComment(id);
              }}
              onReanchorComment={(id) => {
                void actions.handleReanchorComment(id);
              }}
            />
          </aside>
        )}
      </div>
      {props.isWalkthroughActive && (
        <div
          data-testid="walkthrough-floating-bar"
          style={{
            position: "absolute",
            bottom: "14px",
            left: "50%",
            transform: "translateX(-50%)",
            zIndex: 25,
            display: "inline-flex",
            alignItems: "center",
            gap: "6px",
            padding: "5px 10px",
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-default)",
            borderRadius: "var(--radius-pill)",
            boxShadow: "var(--shadow-lg)",
            whiteSpace: "nowrap",
          }}
        >
          <span style={{ fontFamily: "var(--font-mono)", fontSize: "10.5px", fontWeight: 600, color: "var(--action-primary)" }}>
            {props.walkthroughStepLabel || nodeBadgeKey}
          </span>
          <button type="button" data-testid="walkthrough-prev-btn" onClick={props.onWalkthroughPrev} title={t("canvas.walkthroughPrev", "Màn hình trước")} style={{ width: "24px", height: "24px", borderRadius: "50%", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
            <BrandIcon name="arrow-left" size={11} color="currentColor" />
          </button>
          <button type="button" data-testid="walkthrough-auto-toggle" onClick={props.onToggleWalkthroughAutoPlay} title={props.walkthroughAutoPlay ? t("canvas.pauseWalkthrough", "Tạm dừng tự động") : t("canvas.resumeWalkthrough", "Chạy tự động")} style={{ width: "26px", height: "26px", borderRadius: "50%", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
            <BrandIcon name={props.walkthroughAutoPlay ? "pause" : "play"} size={11} color="currentColor" />
          </button>
          <button type="button" data-testid="walkthrough-next-btn" onClick={props.onWalkthroughNext} title={t("canvas.walkthroughNext", "Màn hình tiếp theo")} style={{ width: "24px", height: "24px", borderRadius: "50%", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
            <BrandIcon name="chevron-right" size={11} color="currentColor" />
          </button>
          <div style={{ display: "inline-flex", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-pill)", padding: "2px", gap: "2px" }}>
            <button type="button" data-testid="walkthrough-mode-dual" onClick={() => props.onSetWalkthroughViewMode?.("dual")} style={{ height: "22px", padding: "0 8px", borderRadius: "var(--radius-pill)", border: "none", fontSize: "10.5px", fontWeight: 600, cursor: "pointer", backgroundColor: (props.walkthroughViewMode ?? "dual") === "dual" ? "var(--bg-selected)" : "transparent", color: (props.walkthroughViewMode ?? "dual") === "dual" ? "var(--action-primary)" : "var(--text-secondary)" }}>
              {t("canvas.walkthroughDualMode", "Dual Screen")}
            </button>
            <button type="button" data-testid="walkthrough-mode-full" onClick={() => props.onSetWalkthroughViewMode?.("full")} style={{ height: "22px", padding: "0 8px", borderRadius: "var(--radius-pill)", border: "none", fontSize: "10.5px", fontWeight: 600, cursor: "pointer", backgroundColor: props.walkthroughViewMode === "full" ? "var(--bg-selected)" : "transparent", color: props.walkthroughViewMode === "full" ? "var(--action-primary)" : "var(--text-secondary)" }}>
              {t("canvas.walkthroughFullMode", "Toàn bộ thiết kế")}
            </button>
          </div>
          <button type="button" data-testid="walkthrough-exit-btn" onClick={props.onExitWalkthrough} title={t("canvas.exitWalkthrough", "Thoát Walkthrough (Esc)")} style={{ width: "24px", height: "24px", borderRadius: "50%", border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--text-secondary)", cursor: "pointer", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
            <BrandIcon name="close" size={11} color="currentColor" />
          </button>
        </div>
      )}
    </div>
  );
};
