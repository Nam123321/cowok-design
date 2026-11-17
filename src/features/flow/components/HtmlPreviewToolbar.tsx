// @story 04.1, 04.5, 05.1, 05.3
// @implements AC-01, AC-02, AC-03 (2-Row Responsive Toolbar: Primary Actions + Icon-Only Viewport & History Strip)
import React, { useEffect, useRef, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { normalizeViewportPreset, type FixtureVariant, type ViewportPreset } from "../../../lib/validations/screen-fixture";
import type { ExportInitialSection } from "../../snapshots/components/ExportBundleModal";
import {
  ExportActionDropdown,
  NARROW_TOOLBAR_BREAKPOINT_PX,
  OverflowActionsDropdown,
  SubStateSwitcher,
  TOOLBAR_BTN_BASE,
  TOOLBAR_ICON_BTN_BASE,
} from "./HtmlPreviewToolbarControls";

const ICON_SIZE_SM = 12;
const ICON_SIZE_MD = 13;
type ToolbarViewportPreset = ViewportPreset | "desktop" | "tablet" | "mobile";

const VIEWPORT_ITEMS: ReadonlyArray<{ preset: ViewportPreset; icon: "monitor" | "tablet" | "smartphone"; pxWidth: number; i18nKey: string; defaultLabel: string }> = [
  { preset: "desktop-1440", icon: "monitor", pxWidth: 1440, i18nKey: "canvas.viewportDesktop", defaultLabel: "Desktop" },
  { preset: "tablet-768", icon: "tablet", pxWidth: 768, i18nKey: "canvas.viewportTablet", defaultLabel: "Tablet" },
  { preset: "mobile-390", icon: "smartphone", pxWidth: 390, i18nKey: "canvas.viewportMobile", defaultLabel: "Mobile" },
];

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

const VP_GROUP_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-xs)",
  padding: "2px",
  gap: "2px",
  flexWrap: "nowrap",
};

export interface HtmlPreviewToolbarProps {
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
}

const RowOnePrimaryActions: React.FC<HtmlPreviewToolbarProps & { isNarrow: boolean }> = (p) => {
  const { t } = useTranslation();
  const isSelecting = p.pointerMode !== "interact";
  const toggleLabel = isSelecting ? t("canvas.modeSelectRegion", "Chọn vùng") : t("canvas.modeInteractShort", "Tương tác");
  const toggleTitle = t(
    "canvas.modeToggleTooltip",
    "Tương tác ↔ Chọn vùng: nhấn để click phần tử hoặc kéo vùng (>=16x16px) vào bình luận/chat (Phím tắt C)"
  );
  const walkthroughTitle = p.isWalkthroughActive
    ? t("canvas.stopWalkthrough", "Thoát Walkthrough")
    : t("canvas.startWalkthroughTooltip", "Walkthrough: Chạy kịch bản kết nối DAG & thiết kế (Dual Screen / Toàn bộ thiết kế)");
  const expandTitle = p.isLeftStructureHidden
    ? t("canvas.restoreSplitView", "Thu gọn về chế độ Chia đôi")
    : t("canvas.expandHtmlPane", "Mở rộng khung thiết kế");

  return (
    <div style={ROW_STYLE}>
      <div data-testid="canvas-mode-group" style={{ display: "flex", alignItems: "center", gap: "4px", flexWrap: "nowrap", minWidth: 0 }}>
        <button
          type="button"
          data-testid="pointer-mode-toggle"
          aria-pressed={isSelecting}
          onClick={() => p.onSetPointerMode(isSelecting ? "interact" : "click-dom")}
          title={toggleTitle}
          style={{
            ...TOOLBAR_BTN_BASE,
            backgroundColor: isSelecting ? "var(--action-primary)" : "var(--bg-workspace)",
            border: "1px solid var(--border-default)",
            color: isSelecting ? "var(--action-primary-fg)" : "var(--text-primary)",
            fontWeight: 600,
          }}
        >
          <BrandIcon name={isSelecting ? "crosshair" : "cursor-click"} size={ICON_SIZE_SM} strokeWidth={1.85} color="currentColor" />
          <span>{toggleLabel}</span>
        </button>

        {p.onToggleWalkthrough && (
          <button
            type="button"
            data-testid="walkthrough-playback-btn"
            aria-pressed={Boolean(p.isWalkthroughActive)}
            onClick={p.onToggleWalkthrough}
            title={walkthroughTitle}
            style={{
              ...TOOLBAR_BTN_BASE,
              backgroundColor: p.isWalkthroughActive ? "var(--action-primary)" : "var(--bg-workspace)",
              border: "1px solid var(--border-default)",
              color: p.isWalkthroughActive ? "var(--action-primary-fg)" : "var(--text-primary)",
              fontWeight: 600,
            }}
          >
            <BrandIcon name={p.isWalkthroughActive ? "pause" : "play"} size={ICON_SIZE_SM} strokeWidth={1.85} color="currentColor" />
            <span>{t("canvas.walkthroughLabel", "Walkthrough")}</span>
          </button>
        )}

        {!p.isNarrow ? (
          <>
            <button
              type="button"
              disabled={p.isReadOnly}
              data-testid="canvas-mode-inline-editor"
              aria-pressed={p.isInlineCodeOpen}
              onClick={p.onToggleInlineCode}
              title={t("canvas.editHtmlTooltip", "Sửa trực tiếp mã nguồn HTML/CSS (E)")}
              style={{
                ...TOOLBAR_BTN_BASE,
                position: "relative",
                backgroundColor: p.isInlineCodeOpen ? "var(--bg-selected)" : "var(--bg-workspace)",
                border: "1px solid var(--border-default)",
                color: p.isInlineCodeOpen ? "var(--text-primary)" : "var(--text-secondary)",
                fontWeight: p.isInlineCodeOpen ? 600 : 500,
                opacity: p.isReadOnly ? 0.45 : 1,
                cursor: p.isReadOnly ? "not-allowed" : "pointer",
              }}
            >
              <BrandIcon name="code" size={ICON_SIZE_SM} strokeWidth={1.85} color="currentColor" />
              <span>{t("canvas.editHtmlLabel", "Sửa HTML")}</span>
              {p.dirtyBuffer && <span data-testid="toolbar-dirty-indicator" style={{ width: "6px", height: "6px", borderRadius: "50%", backgroundColor: "var(--agent-accent)" }} />}
            </button>

            {p.onToggleSampleData && (
              <button
                type="button"
                disabled={p.isReadOnly}
                data-testid="canvas-mode-sample-data"
                aria-pressed={Boolean(p.isSampleDataOpen)}
                onClick={p.onToggleSampleData}
                title={t("canvas.sampleDataTooltip", "Dữ liệu mẫu JSON Fixtures (HAPPY / EMPTY / LONG_TEXT / ERROR)")}
                style={{
                  ...TOOLBAR_BTN_BASE,
                  backgroundColor: p.isSampleDataOpen ? "var(--bg-selected)" : "var(--bg-workspace)",
                  border: "1px solid var(--border-default)",
                  color: p.isSampleDataOpen ? "var(--text-primary)" : "var(--text-secondary)",
                  fontWeight: p.isSampleDataOpen ? 600 : 500,
                  opacity: p.isReadOnly ? 0.45 : 1,
                  cursor: p.isReadOnly ? "not-allowed" : "pointer",
                }}
              >
                <BrandIcon name="database" size={ICON_SIZE_SM} strokeWidth={1.85} color="currentColor" />
                <span>{t("canvas.sampleDataLabel", "Dữ liệu mẫu")}</span>
              </button>
            )}

            <ExportActionDropdown onOpenExportSection={p.onOpenExportSection} />
          </>
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
  );
};

const RowTwoIconUtilityStrip: React.FC<HtmlPreviewToolbarProps> = (p) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const activePreset = normalizeViewportPreset(p.viewportPreset);
  const aspectLabelMap: Record<"auto" | "16-9" | "16-10", string> = {
    auto: t("canvas.aspectAutoFit", "Tự động vừa khung"),
    "16-9": t("canvas.aspectDeck169", "16:9 Deck"),
    "16-10": t("canvas.aspectStudio1610", "16:10 Studio"),
  };
  const nextAspect = NEXT_ASPECT_RATIO[p.aspectRatio];
  const aspectTitle = `${t("canvas.aspectRatio", "Tỷ lệ khung hình")}: ${aspectLabelMap[p.aspectRatio]} (→ ${aspectLabelMap[nextAspect]})`;
  const commentsCount = p.commentCount ?? 0;
  const snapshotsCount = p.snapshotsCount ?? 0;
  const commentsTitle = `${t("comments.drawerTitle", "Bình luận ghim trên phần tử")} (${formatNumber(commentsCount)})`;
  const historyTitle = `${t("snapshots.historyTitle", "Lịch sử Phiên bản")} (${formatNumber(snapshotsCount)})`;

  return (
    <div style={{ ...ROW_STYLE, borderTop: "1px solid var(--border-subtle)", paddingTop: "3px" }}>
      <SubStateSwitcher activeSubState={p.activeSubState} isReadOnly={p.isReadOnly} onSelectSubState={p.onSelectSubState} />
      <div style={{ display: "flex", alignItems: "center", gap: "4px", flexWrap: "nowrap", flexShrink: 0 }}>
        <div style={VP_GROUP_STYLE} role="group" aria-label={t("canvas.viewportSwitcherAria", "Kích thước màn hình")}>
          {VIEWPORT_ITEMS.map(({ preset, icon, pxWidth, i18nKey, defaultLabel }) => {
            const isActive = activePreset === preset;
            const vpTitle = `${t(i18nKey, defaultLabel)} (${formatNumber(pxWidth)}px)`;
            return (
              <button
                key={preset}
                type="button"
                disabled={p.isReadOnly}
                data-testid={`viewport-btn-${preset}`}
                aria-pressed={isActive}
                onClick={() => p.onSetViewportPreset(preset)}
                title={vpTitle}
                aria-label={vpTitle}
                style={{
                  ...TOOLBAR_ICON_BTN_BASE,
                  height: "20px",
                  backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
                  color: isActive ? "var(--action-primary)" : "var(--text-muted)",
                  opacity: p.isReadOnly ? 0.45 : 1,
                  cursor: p.isReadOnly ? "not-allowed" : "pointer",
                }}
              >
                <BrandIcon name={icon} size={ICON_SIZE_SM} color="currentColor" />
              </button>
            );
          })}
        </div>

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
            {commentsCount > 0 && (
              <span style={{ position: "absolute", top: "-4px", right: "-4px", minWidth: "13px", height: "13px", padding: "0 3px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "8.5px", fontWeight: 700, display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
                {formatNumber(commentsCount)}
              </span>
            )}
          </button>
        )}

        {p.onOpenVersionHistory && (
          <button
            type="button"
            data-testid="toggle-version-history-btn"
            onClick={p.onOpenVersionHistory}
            title={historyTitle}
            aria-label={historyTitle}
            style={{
              ...TOOLBAR_ICON_BTN_BASE,
              border: "1px solid var(--border-default)",
              backgroundColor: "var(--bg-workspace)",
              color: "var(--text-secondary)",
            }}
          >
            <BrandIcon name="history" size={ICON_SIZE_MD} color="currentColor" />
            {snapshotsCount > 0 && (
              <span style={{ position: "absolute", top: "-4px", right: "-4px", minWidth: "13px", height: "13px", padding: "0 3px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-elevated)", border: "1px solid var(--border-default)", color: "var(--text-primary)", fontSize: "8.5px", fontWeight: 700, display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
                {formatNumber(snapshotsCount)}
              </span>
            )}
          </button>
        )}
      </div>
    </div>
  );
};

export const HtmlPreviewToolbar: React.FC<HtmlPreviewToolbarProps> = (props) => {
  const toolbarRef = useRef<HTMLDivElement>(null);
  const [isNarrow, setIsNarrow] = useState(false);

  useEffect(() => {
    const el = toolbarRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const sync = () => setIsNarrow(el.clientWidth > 0 && el.clientWidth < NARROW_TOOLBAR_BREAKPOINT_PX);
    sync();
    const ro = new ResizeObserver(sync);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  return (
    <div
      ref={toolbarRef}
      className="html-pane-toolbar"
      style={{ padding: "4px 8px", backgroundColor: "var(--bg-surface)", borderBottom: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "3px" }}
    >
      <RowOnePrimaryActions {...props} isNarrow={isNarrow} />
      <RowTwoIconUtilityStrip {...props} />
    </div>
  );
};
