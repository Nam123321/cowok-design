// @story 04.1, 04.5, 05.1, 05.3, 06.1
// @implements AC-01, AC-02, AC-03, AC-06 (2-Row Responsive Toolbar: Primary Actions + Capability-Gated Utility Strip)
import React, { useEffect, useRef, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { normalizeViewportPreset, type FixtureVariant, type ViewportPreset } from "../../../lib/validations/screen-fixture";
import type { GenerateDeckResponse } from "../../../lib/validations/deck-generation";
import type { ExportInitialSection } from "../../snapshots/components/ExportBundleModal";
import { resolveStageMode, STAGE_CAPABILITY_BY_MODE } from "../utils/stage-capability";
import { SlideAspectAndZoomControls, ViewportSwitcherControl } from "./HtmlPreviewSlideControls";
import {
  HtmlPreviewCmsActions,
  resolveCmsToolbarDensity,
  type CmsToolbarDensity,
  type HtmlToolbarCmsActions,
} from "./HtmlPreviewCmsActions";
import {
  ExportActionDropdown,
  NARROW_TOOLBAR_BREAKPOINT_PX,
  NARROW_UTILITY_FLOW_BREAKPOINT_PX,
  NARROW_UTILITY_SLIDE_BREAKPOINT_PX,
  OverflowActionsDropdown,
  SubStateSwitcher,
  TOOLBAR_BTN_BASE,
  TOOLBAR_ICON_BTN_BASE,
} from "./HtmlPreviewToolbarControls";

const ICON_SIZE_SM = 12;
const ICON_SIZE_MD = 13;
const ICON_STROKE = 1.85;
const DISABLED_OPACITY = 0.45;
const WEIGHT_MEDIUM = 500;
const WEIGHT_SEMIBOLD = 600;
const WEIGHT_BOLD = 700;
const BADGE_OFFSET_PX = "-4px";
const BADGE_FONT_PX = "8.5px";
type ToolbarViewportPreset = ViewportPreset | "desktop" | "tablet" | "mobile";

const NEXT_ASPECT_RATIO: Record<"auto" | "16-9" | "16-10", "auto" | "16-9" | "16-10"> = {
  auto: "16-9",
  "16-9": "16-10",
  "16-10": "auto",
};

const ROW_STYLE: React.CSSProperties = {
  minHeight: "30px",
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "6px",
  flexWrap: "nowrap",
};

const BADGE_DOT_BASE: React.CSSProperties = {
  position: "absolute",
  top: BADGE_OFFSET_PX,
  right: BADGE_OFFSET_PX,
  minWidth: "13px",
  height: "13px",
  padding: "0 3px",
  borderRadius: "var(--radius-pill)",
  fontSize: BADGE_FONT_PX,
  fontWeight: WEIGHT_BOLD,
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
};

export interface HtmlPreviewToolbarProps {
  projectId?: string;
  onDeckGenerated?: (response: GenerateDeckResponse) => void;
  onSyncToMainChat?: (summaryText: string) => void;
  onEmbedIntoActiveSlide?: (htmlSnippet: string, title: string) => void;
  structureLens?: "userflow" | "slide-cms";
  pointerMode: "interact" | "click-dom" | "drag-box";
  onSetPointerMode: (mode: "interact" | "click-dom" | "drag-box") => void;
  activeSubState?: FixtureVariant;
  onSelectSubState?: (variant: FixtureVariant) => void;
  isInlineCodeOpen: boolean;
  dirtyBuffer?: boolean;
  onToggleInlineCode: () => void;
  viewportPreset: ToolbarViewportPreset;
  onSetViewportPreset: (vp: ViewportPreset) => void;
  aspectRatio: "auto" | "16-9" | "16-10";
  onSetAspectRatio: (ar: "auto" | "16-9" | "16-10") => void;
  zoomMode?: "fit" | "pan-100";
  onSetZoomMode?: (mode: "fit" | "pan-100") => void;
  isPresenting: boolean;
  onTogglePresenting: () => void;
  isLeftStructureHidden: boolean;
  onToggleLeftStructureHidden: () => void;
  isCommentsDrawerOpen?: boolean;
  commentCount?: number;
  onToggleCommentsDrawer?: () => void;
  isSampleDataOpen?: boolean;
  onToggleSampleData?: () => void;
  isWalkthroughActive?: boolean;
  onToggleWalkthrough?: () => void;
  snapshotsCount?: number;
  onOpenVersionHistory?: () => void;
  onOpenExportSection?: (section: ExportInitialSection) => void;
  isReadOnly?: boolean;
  cmsActions?: HtmlToolbarCmsActions;
  isPresenterActive?: boolean;
  onOpenPresenter?: () => void;
}

const CMS_WIDE_TOOLBAR_BREAKPOINT_PX = 740;
const ULTRA_NARROW_TOOLBAR_BREAKPOINT_PX = 380;

function getUnifiedPlaybackConfig(p: HtmlPreviewToolbarProps, t: (k: string, d?: string) => string) {
  const isSlideLens = p.structureLens === "slide-cms";
  const isPlaybackActive = isSlideLens ? p.isPresenting : Boolean(p.isWalkthroughActive);
  const label = isSlideLens && p.isPresenting ? t("presentation.exitPresent", "Thoát") : t("canvas.walkthroughLabel", "Walkthrough");
  let title: string;
  if (isSlideLens) {
    title = t("presentation.presentDeckTooltip", "Trình chiếu toàn màn hình (Nhấn Esc để thoát)");
  } else if (p.isWalkthroughActive) {
    title = t("canvas.stopWalkthrough", "Thoát Walkthrough");
  } else {
    title = t("canvas.startWalkthroughTooltip", "Walkthrough: Chạy kịch bản kết nối DAG & thiết kế (Dual Screen / Toàn bộ thiết kế)");
  }
  return { isSlideLens, isPlaybackActive, label, title, onClick: isSlideLens ? p.onTogglePresenting : p.onToggleWalkthrough };
}

const OpenPresenterButton: React.FC<{
  isSlideLens: boolean;
  isPresenterActive?: boolean;
  isCompactLabels?: boolean;
  onOpenPresenter?: () => void;
}> = ({ isSlideLens, isPresenterActive, isCompactLabels, onOpenPresenter }) => {
  const { t } = useTranslation();
  if (!isSlideLens || !onOpenPresenter) return null;
  const btnPadding = isCompactLabels ? "0 5px" : TOOLBAR_BTN_BASE.padding;
  return (
    <button
      type="button"
      data-testid="btn-open-presenter"
      aria-pressed={Boolean(isPresenterActive)}
      aria-label={t("presentation.presenter.openButton", "Presenter")}
      onClick={onOpenPresenter}
      title={t("presentation.presenter.openButtonTooltip", "Mở chế độ Trình chiếu 2 màn hình — chỉ dùng cho Presentation Deck (BroadcastChannel / Chia đôi nội tuyến)")}
      style={{
        ...TOOLBAR_BTN_BASE,
        padding: btnPadding,
        backgroundColor: isPresenterActive ? "var(--bg-selected)" : "var(--bg-workspace)",
        border: isPresenterActive ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
        color: isPresenterActive ? "var(--action-primary)" : "var(--text-primary)",
        fontWeight: WEIGHT_SEMIBOLD,
      }}
    >
      <BrandIcon name="monitor" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
      {!isCompactLabels && <span>{t("presentation.presenter.openButton", "Presenter")}</span>}
    </button>
  );
};

const WalkthroughPlaybackButton: React.FC<{
  p: HtmlPreviewToolbarProps;
  isCompactLabels?: boolean;
}> = ({ p, isCompactLabels }) => {
  const { t } = useTranslation();
  const playback = getUnifiedPlaybackConfig(p, t);
  if (!p.onToggleWalkthrough && !playback.isSlideLens) return null;
  const playbackBg = playback.isPlaybackActive
    ? (playback.isSlideLens ? "var(--agent-accent)" : "var(--action-primary)")
    : "var(--bg-workspace)";
  const btnPadding = isCompactLabels ? "0 5px" : TOOLBAR_BTN_BASE.padding;
  return (
    <button
      type="button"
      data-testid="walkthrough-playback-btn"
      data-deck-present-trigger={playback.isSlideLens ? "true" : undefined}
      aria-pressed={playback.isPlaybackActive}
      aria-label={playback.label}
      onClick={playback.onClick}
      title={playback.title}
      style={{
        ...TOOLBAR_BTN_BASE,
        padding: btnPadding,
        backgroundColor: playbackBg,
        border: "1px solid var(--border-default)",
        color: playback.isPlaybackActive ? "var(--action-primary-fg)" : "var(--text-primary)",
        fontWeight: WEIGHT_SEMIBOLD,
      }}
    >
      <BrandIcon name={playback.isPlaybackActive ? (playback.isSlideLens ? "minimize" : "pause") : "play"} size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
      {!isCompactLabels && <span>{playback.label}</span>}
    </button>
  );
};

const PrimaryPlaybackControls: React.FC<HtmlPreviewToolbarProps & { isCompactLabels?: boolean }> = (p) => {
  const { t } = useTranslation();
  const isSelecting = p.pointerMode !== "interact";
  const isSlideLens = p.structureLens === "slide-cms";
  const btnPadding = p.isCompactLabels ? "0 5px" : TOOLBAR_BTN_BASE.padding;
  return (
    <>
      <button
        type="button"
        data-testid="pointer-mode-toggle"
        aria-pressed={isSelecting}
        aria-label={isSelecting ? t("canvas.modeSelectRegion", "Chọn vùng") : t("canvas.modeInteractShort", "Tương tác")}
        onClick={() => p.onSetPointerMode(isSelecting ? "interact" : "click-dom")}
        title={t("canvas.modeToggleTooltip", "Tương tác ↔ Chọn vùng: nhấn để click phần tử hoặc kéo vùng (>=16x16px) vào bình luận/chat (Phím tắt C)")}
        style={{
          ...TOOLBAR_BTN_BASE,
          padding: btnPadding,
          backgroundColor: isSelecting ? "var(--action-primary)" : "var(--bg-workspace)",
          border: "1px solid var(--border-default)",
          color: isSelecting ? "var(--action-primary-fg)" : "var(--text-primary)",
          fontWeight: WEIGHT_SEMIBOLD,
        }}
      >
        <BrandIcon name={isSelecting ? "crosshair" : "cursor-click"} size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
        {!p.isCompactLabels && <span>{isSelecting ? t("canvas.modeSelectRegion", "Chọn vùng") : t("canvas.modeInteractShort", "Tương tác")}</span>}
      </button>
      <WalkthroughPlaybackButton p={p} isCompactLabels={p.isCompactLabels} />
      <OpenPresenterButton isSlideLens={isSlideLens} isPresenterActive={p.isPresenterActive} isCompactLabels={p.isCompactLabels} onOpenPresenter={p.onOpenPresenter} />
    </>
  );
};

const WideModeEditorActions: React.FC<HtmlPreviewToolbarProps> = (p) => {
  const { t } = useTranslation();
  const secondaryStyle = (active?: boolean): React.CSSProperties => ({
    ...TOOLBAR_BTN_BASE,
    position: "relative",
    backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
    border: "1px solid var(--border-default)",
    color: active ? "var(--text-primary)" : "var(--text-secondary)",
    fontWeight: active ? WEIGHT_SEMIBOLD : WEIGHT_MEDIUM,
    opacity: p.isReadOnly ? DISABLED_OPACITY : 1,
    cursor: p.isReadOnly ? "not-allowed" : "pointer",
  });
  return (
    <>
      <button type="button" disabled={p.isReadOnly} data-testid="canvas-mode-inline-editor" aria-pressed={p.isInlineCodeOpen} onClick={p.onToggleInlineCode} title={t("canvas.editHtmlTooltip", "Sửa trực tiếp mã nguồn HTML/CSS (E)")} style={secondaryStyle(p.isInlineCodeOpen)}>
        <BrandIcon name="code" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
        <span>{t("canvas.editHtmlLabel", "Sửa HTML")}</span>
        {p.dirtyBuffer && <span data-testid="toolbar-dirty-indicator" style={{ width: "6px", height: "6px", borderRadius: "50%", backgroundColor: "var(--agent-accent)" }} />}
      </button>
      {p.onToggleSampleData && (
        <button type="button" disabled={p.isReadOnly} data-testid="canvas-mode-sample-data" aria-pressed={Boolean(p.isSampleDataOpen)} onClick={p.onToggleSampleData} title={t("canvas.sampleDataTooltip", "Dữ liệu mẫu JSON Fixtures (HAPPY / EMPTY / LONG_TEXT / ERROR)")} style={secondaryStyle(p.isSampleDataOpen)}>
          <BrandIcon name="database" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
          <span>{t("canvas.sampleDataLabel", "Dữ liệu mẫu")}</span>
        </button>
      )}
      <ExportActionDropdown onOpenExportSection={p.onOpenExportSection} />
    </>
  );
};

const RowOnePrimaryActions: React.FC<
  HtmlPreviewToolbarProps & { isNarrow: boolean; isUltraNarrow: boolean; cmsDensity: CmsToolbarDensity }
> = (p) => {
  const { t } = useTranslation();
  const expandTitle = p.isLeftStructureHidden
    ? t("canvas.restoreSplitView", "Thu gọn về chế độ Chia đôi")
    : t("canvas.expandHtmlPane", "Mở rộng khung thiết kế");
  return (
    <div style={ROW_STYLE}>
      <div data-testid="canvas-mode-group" style={{ display: "flex", alignItems: "center", gap: "4px", flexWrap: "nowrap", minWidth: 0, overflow: "hidden" }}>
        <PrimaryPlaybackControls {...p} isCompactLabels={p.isUltraNarrow} />
        {!p.isNarrow ? (
          <WideModeEditorActions {...p} />
        ) : (
          <OverflowActionsDropdown
            isInlineCodeOpen={p.isInlineCodeOpen}
            dirtyBuffer={p.dirtyBuffer}
            isSampleDataOpen={p.isSampleDataOpen}
            isReadOnly={p.isReadOnly}
            onToggleInlineCode={p.onToggleInlineCode}
            onToggleSampleData={p.onToggleSampleData}
            onOpenExportSection={p.onOpenExportSection}
          />
        )}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "6px", flexWrap: "nowrap", flexShrink: 0 }}>
        {p.cmsActions && <HtmlPreviewCmsActions actions={p.cmsActions} density={p.cmsDensity} />}
        <button
          type="button"
          data-testid="toggle-expand-html-btn"
          onClick={p.onToggleLeftStructureHidden}
          title={expandTitle}
          aria-label={expandTitle}
          style={{
            ...TOOLBAR_ICON_BTN_BASE,
            width: "26px",
            border: "1px solid var(--border-default)",
            backgroundColor: p.isLeftStructureHidden ? "var(--bg-selected)" : "var(--bg-workspace)",
            color: p.isLeftStructureHidden ? "var(--text-primary)" : "var(--text-secondary)",
          }}
        >
          <BrandIcon name={p.isLeftStructureHidden ? "minimize" : "maximize"} size={ICON_SIZE_MD} color="currentColor" />
        </button>
      </div>
    </div>
  );
};

const CommentsAndHistoryIcons: React.FC<HtmlPreviewToolbarProps> = (p) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const commentsCount = p.commentCount ?? 0;
  const snapshotsCount = p.snapshotsCount ?? 0;
  const commentsTitle = `${t("comments.drawerTitle", "Bình luận ghim trên phần tử")} (${formatNumber(commentsCount)})`;
  const historyTitle = `${t("snapshots.historyTitle", "Lịch sử Phiên bản")} (${formatNumber(snapshotsCount)})`;
  return (
    <>
      {p.onToggleCommentsDrawer && (
        <button
          type="button"
          data-testid="toggle-comments-drawer-btn"
          aria-pressed={Boolean(p.isCommentsDrawerOpen)}
          onClick={p.onToggleCommentsDrawer}
          title={commentsTitle}
          aria-label={commentsTitle}
          style={{
            ...TOOLBAR_ICON_BTN_BASE,
            border: p.isCommentsDrawerOpen ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
            backgroundColor: p.isCommentsDrawerOpen ? "var(--bg-selected)" : "var(--bg-workspace)",
            color: p.isCommentsDrawerOpen ? "var(--action-primary)" : "var(--text-secondary)",
          }}
        >
          <BrandIcon name="message-square" size={ICON_SIZE_SM} color="currentColor" />
          {commentsCount > 0 && <span style={{ ...BADGE_DOT_BASE, backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)" }}>{formatNumber(commentsCount)}</span>}
        </button>
      )}
      {p.onOpenVersionHistory && (
        <button
          type="button"
          data-testid="toggle-version-history-btn"
          onClick={p.onOpenVersionHistory}
          title={historyTitle}
          aria-label={historyTitle}
          style={{ ...TOOLBAR_ICON_BTN_BASE, border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-secondary)" }}
        >
          <BrandIcon name="history" size={ICON_SIZE_MD} color="currentColor" />
          {snapshotsCount > 0 && <span style={{ ...BADGE_DOT_BASE, backgroundColor: "var(--bg-elevated)", border: "1px solid var(--border-default)", color: "var(--text-primary)" }}>{formatNumber(snapshotsCount)}</span>}
        </button>
      )}
    </>
  );
};

const RowTwoIconUtilityStrip: React.FC<HtmlPreviewToolbarProps & { isCompactRowTwo: boolean }> = (p) => {
  const { t } = useTranslation();
  const capability = STAGE_CAPABILITY_BY_MODE[resolveStageMode(p.structureLens)];
  const activePreset = normalizeViewportPreset(p.viewportPreset);
  const aspectLabelMap: Record<"auto" | "16-9" | "16-10", string> = {
    auto: t("canvas.aspectAutoFit", "Tự động vừa khung"),
    "16-9": t("canvas.aspectDeck169", "16:9 Deck"),
    "16-10": t("canvas.aspectStudio1610", "16:10 Studio"),
  };
  const nextAspect = NEXT_ASPECT_RATIO[p.aspectRatio];
  const aspectTitle = `${t("canvas.aspectRatio", "Tỷ lệ khung hình")}: ${aspectLabelMap[p.aspectRatio]} (→ ${aspectLabelMap[nextAspect]})`;
  return (
    <div style={{ ...ROW_STYLE, borderTop: "1px solid var(--border-subtle)", paddingTop: "3px" }}>
      <SubStateSwitcher activeSubState={p.activeSubState} isReadOnly={p.isReadOnly} isCompact={p.isCompactRowTwo} onSelectSubState={p.onSelectSubState} />
      <div style={{ display: "flex", alignItems: "center", gap: "4px", flexWrap: "nowrap", flexShrink: 0 }}>
        {capability.showViewportPresets && <ViewportSwitcherControl activePreset={activePreset} isReadOnly={p.isReadOnly} isCompact={p.isCompactRowTwo} onSetViewportPreset={p.onSetViewportPreset} />}
        {capability.showSlideAspectAndZoom ? (
          <SlideAspectAndZoomControls aspectRatio={p.aspectRatio} zoomMode={p.zoomMode} isCompact={p.isCompactRowTwo} projectId={p.projectId} onSetAspectRatio={p.onSetAspectRatio} onSetZoomMode={p.onSetZoomMode} onDeckGenerated={p.onDeckGenerated} onSyncToMainChat={p.onSyncToMainChat} onEmbedIntoActiveSlide={p.onEmbedIntoActiveSlide} />
        ) : (
          <button
            type="button"
            data-testid="aspect-ratio-toggle-btn"
            onClick={() => p.onSetAspectRatio(nextAspect)}
            title={aspectTitle}
            aria-label={aspectTitle}
            style={{
              ...TOOLBAR_ICON_BTN_BASE,
              border: "1px solid var(--border-default)",
              backgroundColor: p.aspectRatio !== "auto" ? "var(--bg-selected)" : "var(--bg-workspace)",
              color: p.aspectRatio !== "auto" ? "var(--action-primary)" : "var(--text-secondary)",
            }}
          >
            <BrandIcon name="frame" size={ICON_SIZE_SM} color="currentColor" />
          </button>
        )}
        <CommentsAndHistoryIcons {...p} />
      </div>
    </div>
  );
};

export const HtmlPreviewToolbar: React.FC<HtmlPreviewToolbarProps> = (props) => {
  const toolbarRef = useRef<HTMLDivElement>(null);
  const [toolbarWidth, setToolbarWidth] = useState(0);
  useEffect(() => {
    const el = toolbarRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const sync = () => setToolbarWidth(el.clientWidth);
    sync();
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const narrowRowOneThreshold = props.cmsActions ? CMS_WIDE_TOOLBAR_BREAKPOINT_PX : NARROW_TOOLBAR_BREAKPOINT_PX;
  const isNarrowRowOne = toolbarWidth > 0 && toolbarWidth < narrowRowOneThreshold;
  const isUltraNarrow = toolbarWidth > 0 && toolbarWidth < ULTRA_NARROW_TOOLBAR_BREAKPOINT_PX;
  const cmsDensity = resolveCmsToolbarDensity(toolbarWidth);
  const utilityThreshold = props.structureLens === "slide-cms" ? NARROW_UTILITY_SLIDE_BREAKPOINT_PX : NARROW_UTILITY_FLOW_BREAKPOINT_PX;
  const isCompactRowTwo = toolbarWidth > 0 && toolbarWidth < utilityThreshold;
  return (
    <div ref={toolbarRef} className="html-pane-toolbar" style={{ padding: "4px 8px", backgroundColor: "var(--bg-surface)", borderBottom: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "3px" }}>
      <RowOnePrimaryActions {...props} isNarrow={isNarrowRowOne} isUltraNarrow={isUltraNarrow} cmsDensity={cmsDensity} />
      <RowTwoIconUtilityStrip {...props} isCompactRowTwo={isCompactRowTwo} />
    </div>
  );
};
