// @story 04.1, 04.5, 05.1
// @implements AC-01, AC-02, AC-03, AC-04 (IframeKeepAlivePool LRU-5 Live Preview Pane + 3-Way Comments)
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { WalkthroughFloatingBar } from "./WalkthroughFloatingBar";
import type { ExportInitialSection } from "../../snapshots/components/ExportBundleModal";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { CanvasTabItem } from "./CompanionCanvasTab";
import type { ScreenPreviewData, SlidePhaseItem } from "./companion-canvas-data";
import { HtmlPreviewToolbar } from "./HtmlPreviewToolbar";
import type { HtmlToolbarCmsActions } from "./HtmlPreviewCmsActions";
import { CmsStageRecoveryPopup, type CmsStageRecoveryProps } from "./CmsStageRecoveryPopup";
import { InlineCodeEditorPanel } from "./InlineCodeEditorPanel";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { ElementCommentThread } from "../../../components/comments/ElementCommentThread";
import {
  PreviewDeviceFrame,
  SLIDE_STAGE_BASE_WIDTH_PX,
  SLIDE_STAGE_DEFAULT_WIDTH_PX,
  SLIDE_STAGE_GUTTER_PX,
  SLIDE_STAGE_MIN_FIT_WIDTH_PX,
  SLIDE_STAGE_MIN_SCALE,
} from "./CompanionPreviewDeviceFrame";
import type { ElementCommentResponse } from "../../../lib/validations/element-comment";
import type { GenerateDeckResponse } from "../../../lib/validations/deck-generation";
import type { IframeToHostDeckMessage, WordingSlotDto } from "../../../lib/validations/presentation";
import {
  validateUserHtmlPayload,
  wrapUserHtmlSrcDoc,
} from "../utils/html-sandbox-wrapper";
import { useSandboxedIframeBridge, type SpatialTargetSelection } from "../hooks/useSandboxedIframeBridge";
import { useStageSpatialController } from "../hooks/useStageSpatialController";
import { StageSpatialOverlaysLayer } from "./StageSpatialOverlaysLayer";
import type { PostPatchByFfIdFn } from "../hooks/useCmsSlideEditorController";
import { usePreviewPaneActions } from "../hooks/usePreviewPaneActions";
import { useRawPreviewHtml, useSampleFixtureSyncEffect } from "../hooks/usePreviewFixtureHtml";
import { registerScreenBaseHtmlProvider } from "../../../lib/sketch-polish-occ-store";
import {
  buildSlideWalkthroughLabel,
  resolveStageMode,
  STAGE_CAPABILITY_BY_MODE,
} from "../utils/stage-capability";
import {
  normalizeFixtureRef,
  type FixtureVariant,
  type ViewportPreset,
} from "../../../lib/validations/screen-fixture";

const INVALID_HTML_MSG_MAP: Record<"empty" | "too_large" | "invalid_html", { key: string; fallback: string }> = {
  empty: { key: "canvas.invalidHtmlEmpty", fallback: "HTML cannot be empty." },
  too_large: { key: "canvas.invalidHtmlTooLarge", fallback: "HTML payload exceeds 1 MB limit." },
  invalid_html: { key: "canvas.invalidHtmlFormat", fallback: "Please enter valid HTML markup before applying." },
};

export interface CompanionLiveHtmlRightPaneProps {
  projectId?: string;
  onDeckGenerated?: (response: GenerateDeckResponse) => void;
  onSyncToMainChat?: (summaryText: string) => void;
  onEmbedIntoActiveSlide?: (htmlSnippet: string, title: string) => void;
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
  zoomMode?: "fit" | "pan-100";
  onSetZoomMode?: (mode: "fit" | "pan-100") => void;
  isPresenting: boolean;
  onTogglePresenting: () => void;
  isLeftStructureHidden: boolean;
  onToggleLeftStructureHidden: () => void;
  highlightedRegionId: string;
  onRegionClick: (regionId: string, label: string) => void;
  onQuoteText?: (text: string) => void;
  customRawHtml?: string;
  onApplyHtml?: (html: string) => void | string | null | Promise<void | string | null>;
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
  deckAutoPlay?: boolean;
  onToggleDeckAutoPlay?: () => void;
  onDeckPrev?: () => void;
  onDeckNext?: () => void;
  onExitPresent?: () => void;
  slidesCount?: number;
  activeSlideIndex?: number;
  snapshotsCount?: number;
  onOpenVersionHistory?: () => void;
  onOpenExportSection?: (section: ExportInitialSection) => void;
  versionMenuNode?: React.ReactNode;
  isReadOnly?: boolean;
  onRegisterDeckKeyNav?: (fn: ((dir: "prev" | "next" | "goto", targetIndex?: number) => void) | null) => void;
  onChangeSlideIndex?: (index: number) => void;
  onRegisterPatchByFfId?: (fn: PostPatchByFfIdFn | null) => void;
  onDeckTelemetryEvent?: (msg: IframeToHostDeckMessage) => void;
  pinnedSlideIds?: string[];
  overflowAlertPx?: number;
  cmsActions?: HtmlToolbarCmsActions;
  cmsRecovery?: CmsStageRecoveryProps;
  cmsSlots?: WordingSlotDto[];
  cmsSyncState?: string;
  onCmsInlineFieldEdit?: (ffId: string, nextValue: string, previousValue: string) => void;
  onCmsComposingChange?: (composing: boolean) => void;
  onOpenRegionCmsEdit?: (coveredFfIds: string[]) => void;
  onLinkToDag?: () => void;
  isLeftDagFocused?: boolean;
  onRegisterRender2DChart?: (fn: ReturnType<typeof useSandboxedIframeBridge>["postRender2DChart"] | null) => void;
  floatingCopilotNode?: React.ReactNode;
  copilotToastNode?: React.ReactNode;
  isPresenterActive?: boolean;
  onOpenPresenter?: () => void;
  blackoutMode?: "none" | "black" | "white";
  onRegisterDeckBlackout?: (fn: ((mode: "none" | "black" | "white") => void) | null) => void;
  presenterToastNode?: React.ReactNode;
  stageSkin?: "sketch-paper" | "brand-color";
  sketchPolishBarNode?: React.ReactNode;
  onRegisterStageSkinSwap?: (fn: ((skin: "sketch-paper" | "brand-color", cssVars?: Record<string, string>) => void) | null) => void;
  onRegisterStageApplyHtml?: (fn: ((html: string) => void) | null) => void;
  onElementRectsChange?: (rects: Record<string, import("../hooks/useSandboxedIframeBridge").ElementRectBox>) => void;
}

function usePaneInlineApplyHandler(deps: {
  props: CompanionLiveHtmlRightPaneProps;
  postApplyHtml: (html: string) => void;
  setHtmlEditorError: (err: string | null) => void;
  setLocalCustomRawHtml: (h: string) => void;
  t: (k: string, f: string) => string;
}) {
  const { props, postApplyHtml, setHtmlEditorError, setLocalCustomRawHtml, t } = deps;
  return useCallback(() => {
    const check = validateUserHtmlPayload(props.inlineCodeText);
    if (!check.valid) {
      const e = INVALID_HTML_MSG_MAP[check.reason ?? "invalid_html"];
      setHtmlEditorError(t(e.key, e.fallback));
      return;
    }
    setHtmlEditorError(null);
    if (!props.onApplyHtml) {
      postApplyHtml(props.inlineCodeText);
      setLocalCustomRawHtml(props.inlineCodeText);
      props.onToggleInlineCode();
      return;
    }
    void Promise.resolve(props.onApplyHtml(props.inlineCodeText)).then((applied) => {
      if (applied === null) {
        setHtmlEditorError(t("presentation.error.mxssBlocked", "HTML blocked by server mXSS / CSS security sanitizer."));
        return;
      }
      postApplyHtml(typeof applied === "string" ? applied : props.inlineCodeText);
    });
  }, [postApplyHtml, props, setHtmlEditorError, setLocalCustomRawHtml, t]);
}

const COMMENTS_DRAWER_BASE: React.CSSProperties = {
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-md)",
  padding: "12px",
  overflowY: "auto",
  display: "flex",
  flexDirection: "column",
  gap: "8px",
};

const COMMENTS_DRAWER_OVERLAY_STYLE: React.CSSProperties = {
  ...COMMENTS_DRAWER_BASE,
  position: "absolute",
  top: "10px",
  right: "10px",
  bottom: "10px",
  width: "min(280px, calc(100% - 20px))",
  zIndex: 24,
  boxShadow: "var(--shadow-lg)",
};

const COMMENTS_DRAWER_SIDE_STYLE: React.CSSProperties = { ...COMMENTS_DRAWER_BASE, width: "280px", flexShrink: 0 };

const CommentsDrawerAside: React.FC<{
  nodeId: string;
  actions: ReturnType<typeof usePreviewPaneActions>;
  overlayMode?: boolean;
  onClose?: () => void;
}> = ({ nodeId, actions, overlayMode = false, onClose }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  return (
    <aside data-testid="comments-right-drawer" style={overlayMode ? COMMENTS_DRAWER_OVERLAY_STYLE : COMMENTS_DRAWER_SIDE_STYLE}>
      {onClose && (
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
          <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-primary)" }}>{t("spatial.commentsDrawerTitle", "Pinned Comments")} ({formatNumber(actions.comments.length)})</span>
          <button type="button" className="ff-spatial-pill-close" data-testid="comments-drawer-close-btn" aria-label={t("common.close", "Đóng")} onClick={onClose} style={{ width: "22px", height: "22px" }}><BrandIcon name="close" size={11} color="currentColor" /></button>
        </div>
      )}
      <ElementCommentThread nodeId={nodeId} comments={actions.comments} prefillFfId={actions.prefillFfId} collapsedCommentIds={actions.collapsedCommentIds} staleToastMessage={actions.staleToastMessage} toastTone={actions.toastTone} onDismissStaleToast={() => actions.setStaleToastMessage(null)} onAddComment={(c, ffId) => actions.handleAddComment(c, ffId)} onResolveComment={(id) => { void actions.handleResolveComment(id); }} onReanchorComment={(id) => { void actions.handleReanchorComment(id); }} />
    </aside>
  );
};

function useObservedContainerSize(
  ref: React.RefObject<HTMLDivElement | null>,
  fallbackWidth: number,
  fallbackHeight = 640
): { width: number; height: number } {
  const [size, setSize] = useState({ width: fallbackWidth, height: fallbackHeight });
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const sync = () => {
      const nextW = el.clientWidth > 0 ? el.clientWidth : fallbackWidth;
      const nextH = el.clientHeight > 0 ? el.clientHeight : fallbackHeight;
      setSize((prev) => (prev.width === nextW && prev.height === nextH ? prev : { width: nextW, height: nextH }));
    };
    sync();
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    return () => ro.disconnect();
  }, [fallbackHeight, fallbackWidth, ref]);
  return size;
}

function computeSlideStageScale(
  structureLens: "userflow" | "slide-cms",
  zoomMode: "fit" | "pan-100",
  stageWidth: number
): number {
  if (structureLens !== "slide-cms") return 1;
  if (zoomMode === "pan-100") return 1;
  const usable = Math.max(stageWidth - SLIDE_STAGE_GUTTER_PX, SLIDE_STAGE_MIN_FIT_WIDTH_PX);
  return Math.max(SLIDE_STAGE_MIN_SCALE, Math.min(1, usable / SLIDE_STAGE_BASE_WIDTH_PX));
}

function buildViewportSpatialOverlay(opts: {
  props: CompanionLiveHtmlRightPaneProps;
  spatial: ReturnType<typeof useStageSpatialController>;
  overlayStageWidth: number;
  overlayStageHeight: number;
}) {
  const { props, spatial, overlayStageWidth, overlayStageHeight } = opts;
  const openDrawer = spatial.activeTarget
    ? () => { props.onOpenRegionCmsEdit?.([spatial.activeTarget!.elementFfId]); spatial.closeSpatialOverlay(); }
    : undefined;
  const jumpInline = !props.isReadOnly
    ? () => { spatial.closeSpatialOverlay(); props.onToggleInlineCode(); }
    : undefined;
  return (
    <StageSpatialOverlaysLayer
      activeTarget={spatial.activeTarget} activeTool={spatial.activeTool} structureLens={props.structureLens}
      isLeftStructureHidden={props.isLeftStructureHidden} isLeftDagFocusedOnNode={spatial.isLeftDagFocusedOnNode}
      isReadOnly={props.isReadOnly} stageWidth={overlayStageWidth} stageHeight={overlayStageHeight}
      cmsSlots={props.cmsSlots} cmsSyncState={props.cmsSyncState} activeSlideHeadline={props.activeSlide.headline}
      activeSlideSubtitle={props.activeSlide.subtitle} isSubmittingComment={spatial.isSubmittingComment}
      onSelectAction={spatial.handleSelectAction} onSubmitComment={spatial.handleSubmitComment}
      onInlineLiveChange={spatial.handleInlineLiveChange} onInlineComposingChange={props.onCmsComposingChange}
      onOpenFullCmsDrawer={openDrawer} onJumpToInlineHtml={jumpInline} onClose={spatial.closeSpatialOverlay}
    />
  );
}

const PaneStageViewportBody: React.FC<{
  props: CompanionLiveHtmlRightPaneProps;
  activeSubState: FixtureVariant;
  htmlEditorError: string | null;
  setHtmlEditorError: (v: string | null) => void;
  handleApplyInlineHtml: () => void;
  routeBadge: string;
  bridge: ReturnType<typeof useSandboxedIframeBridge>;
  wrappedSrcDoc: string;
  visiblePins: ElementCommentResponse[];
  isCommentsDrawerOpen: boolean;
  setIsCommentsDrawerOpen: (v: boolean) => void;
  actions: ReturnType<typeof usePreviewPaneActions>;
  spatial: ReturnType<typeof useStageSpatialController>;
}> = ({ props, activeSubState, htmlEditorError, setHtmlEditorError, handleApplyInlineHtml, routeBadge, bridge, wrappedSrcDoc, visiblePins, isCommentsDrawerOpen, setIsCommentsDrawerOpen, actions, spatial }) => {
  const viewportRef = useRef<HTMLDivElement>(null);
  const previewCanvasRef = useRef<HTMLDivElement>(null);
  const { width: stageWidth, height: stageHeight } = useObservedContainerSize(viewportRef, SLIDE_STAGE_DEFAULT_WIDTH_PX);
  const { width: observedPreviewW, height: observedPreviewH } = useObservedContainerSize(previewCanvasRef, isCommentsDrawerOpen ? Math.max(240, stageWidth - 292) : stageWidth, stageHeight);
  const overlayStageWidth = isCommentsDrawerOpen ? Math.min(observedPreviewW || stageWidth, Math.max(220, stageWidth - 292)) : observedPreviewW || stageWidth;
  const overlayStageHeight = observedPreviewH || stageHeight;
  const zoomMode = props.zoomMode ?? "fit";
  const isSlideLens = props.structureLens === "slide-cms";
  const slideScale = computeSlideStageScale(props.structureLens, zoomMode, overlayStageWidth);
  const canvasWrapperStyle: React.CSSProperties = isSlideLens
    ? { margin: "auto", display: "flex", justifyContent: zoomMode === "pan-100" ? "flex-start" : "center", alignItems: "center", minWidth: 0, width: zoomMode === "pan-100" ? `${SLIDE_STAGE_BASE_WIDTH_PX}px` : "100%", flex: 1 }
    : { flex: 1, minWidth: 0, display: "flex", justifyContent: "center" };
  return (
    <div ref={viewportRef} className={isSlideLens ? "deck-zoom-viewport" : undefined} style={{ position: "relative", flex: 1, overflow: "auto", padding: "14px", backgroundColor: "var(--bg-subtle)", display: "flex", gap: "12px", justifyContent: isSlideLens ? "flex-start" : "center", alignItems: isSlideLens ? "flex-start" : "stretch" }}>
      {props.isInlineCodeOpen ? (
        <InlineCodeEditorPanel activeTab={props.activeTab} inlineCodeText={props.inlineCodeText} dirtyBuffer={Boolean(props.dirtyBuffer)} showDirtyGuardPrompt={Boolean(props.showDirtyGuardPrompt)} errorMsg={htmlEditorError} onChangeInlineCode={(val) => { if (htmlEditorError) setHtmlEditorError(null); props.onChangeInlineCode(val); }} onApplyHtml={handleApplyInlineHtml} onConfirmDiscardDirty={props.onConfirmDiscardDirty} onCancelCloseEditor={props.onCancelCloseEditor} />
      ) : (
        <div ref={previewCanvasRef} className={isSlideLens ? "deck-zoom-canvas" : undefined} style={canvasWrapperStyle}>
          <PreviewDeviceFrame routeBadge={routeBadge} viewportPreset={props.viewportPreset} structureLens={props.structureLens} zoomMode={zoomMode} slideScale={slideScale} activeSubState={activeSubState} aspectRatio={props.aspectRatio} isPortVerified={bridge.isPortVerified} protocolError={bridge.protocolError} wrappedSrcDoc={bridge.wrappedSrcDoc || wrappedSrcDoc} iframeRef={bridge.iframeRef} onLoad={bridge.handleIframeLoad} poolSlots={bridge.poolSlots} activeSlotId={bridge.activeSlotId} onSlotLoad={bridge.handleSlotIframeLoad} pointerMode={props.pointerMode} visiblePins={visiblePins} elementRects={bridge.elementRects} onOpenComments={() => setIsCommentsDrawerOpen(true)} onRegionClick={props.onRegionClick} onSpatialTargetSelect={spatial.handleSpatialTargetSelect} spatialOverlayNode={buildViewportSpatialOverlay({ props, spatial, overlayStageWidth, overlayStageHeight })} />
        </div>
      )}
      {isCommentsDrawerOpen && !props.isInlineCodeOpen && (
        <CommentsDrawerAside nodeId={props.selectedNodeId || props.activeSlide.id} actions={actions} overlayMode={stageWidth > 0 && stageWidth < 660} onClose={() => setIsCommentsDrawerOpen(false)} />
      )}
    </div>
  );
};

function useLocalizedSubStateLabels(t: (key: string, fallback: string) => string) {
  return useMemo(
    () => ({
      loadingTitle: t("canvas.substateLoadingTitle", "Synchronizing screen state..."),
      loadingBody: t("canvas.substateLoadingBody", "Loading live fixture data from Rust SQLite WAL."),
      emptyTitle: t("canvas.substateEmptyTitle", "No data available (0 Records)"),
      emptyBody: t("canvas.substateEmptyBody", "Create a record or inject sample fixtures to preview this screen."),
      errorTitle: t("canvas.substateErrorTitle", "Validation error (422 Unprocessable)"),
      errorBody: t("canvas.substateErrorBody", "ERR_INSUFFICIENT_QUORUM: Additional signature or constraint check required."),
    }),
    [t]
  );
}

function useDeckProtocolCallback(onChangeSlideIndex?: (index: number) => void) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const [deckWarning, setDeckWarning] = useState<string | null>(null);
  const deckSlideCountRef = useRef<number>(0);
  const lastReportedSlideIdxRef = useRef<number | null>(null);
  const handleDeckProtocolEvent = useCallback(
    (msg: IframeToHostDeckMessage) => {
      if (msg.type === "DECK_PROTOCOL_INIT") {
        deckSlideCountRef.current = msg.slideCount;
        lastReportedSlideIdxRef.current = null;
        setDeckWarning(null);
      } else if (msg.type === "SLIDE_CHANGE") {
        setDeckWarning(null);
        // Only let iframe SLIDE_CHANGE steer host slide selection when the iframe itself holds > 1 .slide elements and index changed.
        if (deckSlideCountRef.current > 1 && lastReportedSlideIdxRef.current !== msg.slideIndex) {
          lastReportedSlideIdxRef.current = msg.slideIndex;
          onChangeSlideIndex?.(msg.slideIndex);
        }
      } else if (msg.type === "SLIDE_OVERFLOW_WARN") {
        const overflowPx = typeof msg.overflowPx === "number" ? msg.overflowPx : Math.max(0, Math.round(msg.scrollHeight - msg.clientHeight));
        if (overflowPx <= 0 || msg.scrollHeight <= msg.clientHeight) {
          setDeckWarning(null);
        } else {
          setDeckWarning(
            `${t("presentation.slideOverflowWarning", "Slide tràn khung hiển thị")}: #${formatNumber(msg.slideIndex + 1)} (${formatNumber(msg.scrollHeight)}px > ${formatNumber(msg.clientHeight)}px)`
          );
        }
      }
    },
    [formatNumber, onChangeSlideIndex, t]
  );
  return { deckWarning, deckSlideCountRef, handleDeckProtocolEvent };
}

function usePaneCommentsAndBridgeSync(
  props: CompanionLiveHtmlRightPaneProps,
  bridge: ReturnType<typeof useSandboxedIframeBridge>,
  onSelectSubState: (v: FixtureVariant) => void,
  deckSlideCountRef: React.MutableRefObject<number>
) {
  useEffect(() => {
    const guardedPostKeyNav = (dir: "prev" | "next" | "goto", targetIndex?: number) => {
      if (deckSlideCountRef.current > 1) {
        bridge.postDeckKeyNav(dir, targetIndex);
      }
    };
    props.onRegisterDeckKeyNav?.(guardedPostKeyNav);
    return () => props.onRegisterDeckKeyNav?.(null);
  }, [bridge.postDeckKeyNav, deckSlideCountRef, props.onRegisterDeckKeyNav]);

  useEffect(() => {
    props.onRegisterPatchByFfId?.(bridge.postPatchByFfId);
    return () => props.onRegisterPatchByFfId?.(null);
  }, [bridge.postPatchByFfId, props.onRegisterPatchByFfId]);

  useEffect(() => {
    props.onRegisterRender2DChart?.(bridge.postRender2DChart);
    return () => props.onRegisterRender2DChart?.(null);
  }, [bridge.postRender2DChart, props.onRegisterRender2DChart]);

  useEffect(() => {
    props.onRegisterDeckBlackout?.(bridge.postDeckBlackout);
    return () => props.onRegisterDeckBlackout?.(null);
  }, [bridge.postDeckBlackout, props.onRegisterDeckBlackout]);

  useEffect(() => {
    props.onRegisterStageSkinSwap?.(bridge.postStageSkinSwap);
    return () => props.onRegisterStageSkinSwap?.(null);
  }, [bridge.postStageSkinSwap, props.onRegisterStageSkinSwap]);

  useEffect(() => {
    props.onRegisterStageApplyHtml?.(bridge.postApplyHtml);
    return () => props.onRegisterStageApplyHtml?.(null);
  }, [bridge.postApplyHtml, props.onRegisterStageApplyHtml]);

  useEffect(() => {
    props.onElementRectsChange?.(bridge.elementRects);
  }, [bridge.elementRects, props.onElementRectsChange]);

  const actions = usePreviewPaneActions({
    projectId: props.projectId, selectedNodeId: props.selectedNodeId, artifactSha256: bridge.artifactSha256,
    elementRects: bridge.elementRects, postFixtureHydrate: bridge.postFixtureHydrate,
    onSetViewportPreset: props.onSetViewportPreset, onSelectSubState, isReadOnly: props.isReadOnly,
  });
  const nodeBadgeKey = props.selectedNodeKey || props.selectedNodeId || props.activeSlide.code.split(" ")[0];
  const routeBadge = `${props.activeTab.title}#${props.structureLens === "userflow" ? nodeBadgeKey : props.activeSlide.code.split(" ")[0]}`;
  const visiblePins = actions.comments.filter((c) => Boolean(bridge.elementRects[c.elementFfId]?.width && bridge.elementRects[c.elementFfId]?.height));
  return { actions, nodeBadgeKey, routeBadge, visiblePins };
}

function usePaneSpatialAndApply(opts: {
  props: CompanionLiveHtmlRightPaneProps;
  bridge: ReturnType<typeof useSandboxedIframeBridge>;
  actions: ReturnType<typeof usePreviewPaneActions>;
  setHtmlEditorError: (err: string | null) => void;
  setLocalCustomRawHtml: (h: string) => void;
  rawHtml: string;
  subStateLabels: ReturnType<typeof useLocalizedSubStateLabels>;
  t: (k: string, f: string) => string;
}) {
  const { props, bridge, actions, t } = opts;
  const spatial = useStageSpatialController({
    projectId: props.projectId, selectedNodeId: props.selectedNodeId, activeSlideId: props.activeSlide.id,
    structureLens: props.structureLens, pointerMode: props.pointerMode, isLeftStructureHidden: props.isLeftStructureHidden,
    isLeftDagFocused: props.isLeftDagFocused, isReadOnly: props.isReadOnly, artifactSha256: bridge.artifactSha256,
    actions, postPatchByFfId: bridge.postPatchByFfId, onCmsInlineFieldEdit: props.onCmsInlineFieldEdit,
    onOpenRegionCmsEdit: props.onOpenRegionCmsEdit, onLinkToDag: props.onLinkToDag,
    onHighlightRegion: (selector) => props.onRegionClick(selector, selector),
  });
  const handleApplyInlineHtml = usePaneInlineApplyHandler({ props, postApplyHtml: bridge.postApplyHtml, setHtmlEditorError: opts.setHtmlEditorError, setLocalCustomRawHtml: opts.setLocalCustomRawHtml, t });
  const wrappedSrcDoc = useMemo(() => {
    if (bridge.poolSlots.length > 0) return "";
    return wrapUserHtmlSrcDoc(opts.rawHtml, { epoch: bridge.epoch, bootSecret: bridge.bootSecret, artifactSha256: bridge.artifactSha256, subStateLabels: opts.subStateLabels });
  }, [bridge.artifactSha256, bridge.bootSecret, bridge.epoch, bridge.poolSlots.length, opts.rawHtml, opts.subStateLabels]);
  return { spatial, handleApplyInlineHtml, wrappedSrcDoc };
}

function useLiveHtmlPaneController(props: CompanionLiveHtmlRightPaneProps) {
  const { t, locale } = useTranslation();
  const [localCustomRawHtml, setLocalCustomRawHtml] = useState<string | undefined>(undefined);
  const [localSubState, setLocalSubState] = useState<FixtureVariant>("DEFAULT");
  const [htmlEditorError, setHtmlEditorError] = useState<string | null>(null);
  const [isCommentsDrawerOpen, setIsCommentsDrawerOpen] = useState(false);

  const activeSubState = normalizeFixtureRef(props.activeSubState ?? localSubState);
  const effectiveCustomHtml = props.customRawHtml ?? localCustomRawHtml;
  const rawHtml = useRawPreviewHtml(props, activeSubState, effectiveCustomHtml);
  const rawHtmlRef = useRef(rawHtml);
  rawHtmlRef.current = rawHtml;
  useEffect(() => {
    const sid = props.selectedNodeId || "default";
    registerScreenBaseHtmlProvider(props.projectId, sid, () => rawHtmlRef.current);
    return () => registerScreenBaseHtmlProvider(props.projectId, sid, null);
  }, [props.projectId, props.selectedNodeId]);
  const subStateLabels = useLocalizedSubStateLabels(t);
  const actionsRef = useRef<ReturnType<typeof usePreviewPaneActions> | null>(null);
  const spatialSelectRef = useRef<((target: SpatialTargetSelection) => void) | null>(null);
  const { deckWarning, deckSlideCountRef, handleDeckProtocolEvent } = useDeckProtocolCallback(props.onChangeSlideIndex);
  const combinedDeckProtocolEvent = useCallback((msg: IframeToHostDeckMessage) => {
    handleDeckProtocolEvent(msg);
    props.onDeckTelemetryEvent?.(msg);
  }, [handleDeckProtocolEvent, props.onDeckTelemetryEvent]);
  const committedSlideCacheKey = props.structureLens === "slide-cms" ? props.activeSlide.id : props.customRawHtml ? undefined : `app-live::${props.activeTab.id}::${props.selectedNodeId || "default"}::${activeSubState}::${locale}`;

  const bridge = useSandboxedIframeBridge({
    projectId: props.projectId, activeTabId: props.activeTab.id, selectedNodeId: props.selectedNodeId, selectedNodeKey: props.selectedNodeKey,
    structureLens: props.structureLens, activeSlideId: props.activeSlide.id, activeSubState, effectiveCustomHtml, rawHtml, subStateLabels,
    onQuoteText: props.onQuoteText, committedSlideCacheKey, pinnedSlideIds: props.pinnedSlideIds, onDeckProtocolEvent: combinedDeckProtocolEvent,
    onRegionClick: (selector, label, ffId) => { if (ffId) actionsRef.current?.setPrefillFfId(ffId); props.onRegionClick(selector, label); },
    onSpatialTargetSelect: (target) => spatialSelectRef.current?.(target),
  });
  const handleSelectSubState = useCallback((v: FixtureVariant) => { setLocalSubState(v); bridge.postFixtureHydrate(v); props.onSelectSubState?.(v); }, [bridge, props]);
  const { actions, nodeBadgeKey, routeBadge, visiblePins } = usePaneCommentsAndBridgeSync(props, bridge, handleSelectSubState, deckSlideCountRef);
  actionsRef.current = actions;
  const { spatial, handleApplyInlineHtml, wrappedSrcDoc } = usePaneSpatialAndApply({ props, bridge, actions, setHtmlEditorError, setLocalCustomRawHtml, rawHtml, subStateLabels, t });
  spatialSelectRef.current = spatial.handleSpatialTargetSelect;
  useSampleFixtureSyncEffect({ projectId: props.projectId, selectedNodeId: props.selectedNodeId, postFixtureHydrate: bridge.postFixtureHydrate, setLocalSubState, onSelectSubState: props.onSelectSubState });

  return { t, locale, activeSubState, htmlEditorError, setHtmlEditorError, isCommentsDrawerOpen, setIsCommentsDrawerOpen, bridge: { ...bridge, protocolError: bridge.protocolError || deckWarning }, actions, spatial, handleApplyInlineHtml, wrappedSrcDoc, nodeBadgeKey, routeBadge, visiblePins };
}

function buildWalkthroughBarProps(
  props: CompanionLiveHtmlRightPaneProps,
  nodeBadgeKey: string,
  formatNumber: (v: number) => string
) {
  const stageMode = resolveStageMode(props.structureLens);
  const isPresentation = stageMode === "presentation" && Boolean(props.isPresenting);
  const showBar = Boolean(props.isWalkthroughActive || isPresentation);
  const slideBadgeId = props.activeSlide?.code?.split("•")[0]?.trim() || props.activeSlide?.id;
  const presentationLabel = buildSlideWalkthroughLabel({
    slideId: slideBadgeId,
    activeIndex: props.activeSlideIndex ?? 0,
    totalSlides: props.slidesCount ?? 1,
    formatNumber,
  });
  return {
    showBar,
    stepLabel: isPresentation ? presentationLabel : (props.walkthroughStepLabel || nodeBadgeKey),
    autoPlay: isPresentation ? props.deckAutoPlay : props.walkthroughAutoPlay,
    showViewModeToggle: STAGE_CAPABILITY_BY_MODE[stageMode].showDualScreenToggle,
    onPrev: isPresentation ? props.onDeckPrev : props.onWalkthroughPrev,
    onToggleAuto: isPresentation ? props.onToggleDeckAutoPlay : props.onToggleWalkthroughAutoPlay,
    onNext: isPresentation ? props.onDeckNext : props.onWalkthroughNext,
    onExit: isPresentation ? (props.onExitPresent ?? props.onTogglePresenting) : props.onExitWalkthrough,
  };
}

const StageOverflowAndWalkthroughOverlay: React.FC<{
  props: CompanionLiveHtmlRightPaneProps;
  nodeBadgeKey: string;
  locale: string;
  t: (k: string, d: string) => string;
}> = ({ props, nodeBadgeKey, locale, t }) => {
  const { formatNumber } = useFormatter({ locale });
  const bar = buildWalkthroughBarProps(props, nodeBadgeKey, formatNumber);
  return (
    <>
      {typeof props.overflowAlertPx === "number" && props.overflowAlertPx > 0 && (
        <div data-testid="stage-e3-overflow-guide" className="ff-stage-e3-overflow-guide">
          {`${t("presentation.cms.e3Overflow", "Tràn khung 16:9 (E3)")}: +${formatNumber(props.overflowAlertPx)}px`}
        </div>
      )}
      {bar.showBar && (
        <WalkthroughFloatingBar
          stepLabel={bar.stepLabel} autoPlay={bar.autoPlay} viewMode={props.walkthroughViewMode}
          showViewModeToggle={bar.showViewModeToggle} onPrev={bar.onPrev} onToggleAuto={bar.onToggleAuto}
          onNext={bar.onNext} onSetViewMode={props.onSetWalkthroughViewMode} onExit={bar.onExit}
        />
      )}
    </>
  );
};

export const CompanionLiveHtmlRightPane: React.FC<CompanionLiveHtmlRightPaneProps> = (props) => {
  const c = useLiveHtmlPaneController(props);
  return (
    <div id="htmlSubPane" className="canvas-live-html-right-pane" data-stage-skin={props.stageSkin ?? "brand-color"} aria-label={c.t("canvas.liveHtmlPane", "Live HTML Preview Sandbox")} style={{ flex: 1, minWidth: 0, height: "100%", display: "flex", flexDirection: "column", backgroundColor: "var(--bg-surface)", overflow: "hidden", position: "relative" }}>
      <HtmlPreviewToolbar
        projectId={props.projectId}
        onDeckGenerated={props.onDeckGenerated}
        onSyncToMainChat={props.onSyncToMainChat}
        onEmbedIntoActiveSlide={props.onEmbedIntoActiveSlide}
        structureLens={props.structureLens}
        pointerMode={props.pointerMode} onSetPointerMode={props.onSetPointerMode} activeSubState={c.activeSubState}
        onSelectSubState={(v) => { void c.actions.handleSelectFixtureVariant(v); }} isInlineCodeOpen={props.isInlineCodeOpen}
        dirtyBuffer={props.dirtyBuffer} onToggleInlineCode={props.onToggleInlineCode} viewportPreset={props.viewportPreset}
        onSetViewportPreset={(vp) => { void c.actions.handlePersistViewport(vp); }} aspectRatio={props.aspectRatio}
        onSetAspectRatio={props.onSetAspectRatio} zoomMode={props.zoomMode} onSetZoomMode={props.onSetZoomMode}
        isPresenting={props.isPresenting} onTogglePresenting={props.onTogglePresenting}
        isLeftStructureHidden={props.isLeftStructureHidden} onToggleLeftStructureHidden={props.onToggleLeftStructureHidden}
        isCommentsDrawerOpen={c.isCommentsDrawerOpen} commentCount={c.actions.comments.length}
        onToggleCommentsDrawer={() => c.setIsCommentsDrawerOpen((v) => !v)} isSampleDataOpen={props.isSampleDataOpen}
        onToggleSampleData={props.onToggleSampleData} isWalkthroughActive={props.isWalkthroughActive}
        onToggleWalkthrough={props.onToggleWalkthrough} snapshotsCount={props.snapshotsCount}
        onOpenVersionHistory={props.onOpenVersionHistory} onOpenExportSection={props.onOpenExportSection} isReadOnly={props.isReadOnly}
        cmsActions={props.cmsActions}
        isPresenterActive={props.isPresenterActive}
        onOpenPresenter={props.onOpenPresenter}
      />
      {props.sketchPolishBarNode}
      <div style={{ position: "relative", flex: 1, minHeight: 0, display: "flex", overflow: "hidden" }}>
        <PaneStageViewportBody
          props={props} activeSubState={c.activeSubState} htmlEditorError={c.htmlEditorError} setHtmlEditorError={c.setHtmlEditorError}
          handleApplyInlineHtml={c.handleApplyInlineHtml} routeBadge={c.routeBadge} bridge={c.bridge} wrappedSrcDoc={c.wrappedSrcDoc}
          visiblePins={c.visiblePins} isCommentsDrawerOpen={c.isCommentsDrawerOpen} setIsCommentsDrawerOpen={c.setIsCommentsDrawerOpen} actions={c.actions} spatial={c.spatial}
        />
        {props.blackoutMode && props.blackoutMode !== "none" && (
          <div
            role="status"
            aria-live="assertive"
            data-testid="audience-blackout-overlay"
            data-mode={props.blackoutMode}
            className={`ff-audience-blackout ff-audience-blackout--${props.blackoutMode}`}
          />
        )}
        {props.cmsRecovery && <CmsStageRecoveryPopup {...props.cmsRecovery} />}
        <StageOverflowAndWalkthroughOverlay props={props} nodeBadgeKey={c.nodeBadgeKey} locale={c.locale} t={c.t} />
        {props.copilotToastNode}
        {props.presenterToastNode}
        {props.floatingCopilotNode}
      </div>
    </div>
  );
};
