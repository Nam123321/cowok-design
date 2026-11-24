// @story 05.6, 06.5
// @implements [5.26] StudioDrawerShell, PAT-09, CMP-17 Reusable RightDrawer Master-Detail Workbench
import React, { useEffect } from "react";
import { BrandIcon } from "../BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { StudioSubChatPane, type StudioSubChatPaneProps } from "./StudioSubChatPane";
import {
  STUDIO_DRAWER_Z,
  STUDIO_FONT_MEDIUM,
  STUDIO_FONT_SEMIBOLD,
  STUDIO_ICON_LG,
  STUDIO_ICON_MD,
  STUDIO_ICON_SM,
  STUDIO_RESIZER_Z,
  STUDIO_STROKE_MONO,
  type StudioMobileTab,
} from "./types";
import type { StudioDrawerLayoutState } from "./useStudioDrawerLayout";

const DISABLED_OPACITY = 0.45;

const BACKDROP_STYLE: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  backgroundColor: "var(--bg-overlay)",
  zIndex: STUDIO_DRAWER_Z,
  display: "flex",
  justifyContent: "flex-end",
};

const ICON_BTN_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-sm)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-secondary)",
  padding: "4px 7px",
  fontSize: "11px",
  fontWeight: STUDIO_FONT_MEDIUM,
  cursor: "pointer",
  whiteSpace: "nowrap",
};

export interface StudioDrawerShellProps {
  isOpen: boolean;
  title: string;
  titleId: string;
  testId: string;
  titleTestId?: string;
  iconName?: string;
  metricLine?: React.ReactNode;
  layout: StudioDrawerLayoutState;
  canUndo?: boolean;
  canRedo?: boolean;
  onUndo?: () => void;
  onRedo?: () => void;
  headerActions?: React.ReactNode;
  closeButtonTestId?: string;
  closeButtonInnerTestId?: string;
  onClose: () => void;
  asForm?: boolean;
  formTestId?: string;
  onFormSubmit?: (e: React.FormEvent) => void;
  masterContent: React.ReactNode;
  subChat: Omit<StudioSubChatPaneProps, "heightPx" | "collapsed" | "onToggleCollapsed">;
  detailContent: React.ReactNode;
  mobileControlLabel?: string;
  mobileCanvasLabel?: string;
  toastSlot?: React.ReactNode;
}

const MobileSwitchBar: React.FC<{
  activeTab: StudioMobileTab;
  controlLabel: string;
  canvasLabel: string;
  onChange: (t: StudioMobileTab) => void;
}> = ({ activeTab, controlLabel, canvasLabel, onChange }) => {
  const pillStyle = (active: boolean): React.CSSProperties => ({
    height: "28px",
    padding: "0 10px",
    borderRadius: "var(--radius-sm)",
    border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
    backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
    color: active ? "var(--action-primary)" : "var(--text-secondary)",
    fontSize: "11.5px",
    fontWeight: STUDIO_FONT_SEMIBOLD,
    cursor: "pointer",
  });
  return (
    <div data-testid="studio-mobile-tabs" style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px", padding: "6px 12px", borderBottom: "1px solid var(--border-default)", backgroundColor: "var(--bg-subtle)" }}>
      <button type="button" data-testid="studio-mobile-tab-control" onClick={() => onChange("control")} style={pillStyle(activeTab === "control")}>{controlLabel}</button>
      <button type="button" data-testid="studio-mobile-tab-canvas" onClick={() => onChange("canvas")} style={pillStyle(activeTab === "canvas")}>{canvasLabel}</button>
    </div>
  );
};

const DrawerHeaderChrome: React.FC<{
  title: string;
  titleId: string;
  titleTestId: string;
  iconName: string;
  metricLine?: React.ReactNode;
  canUndo?: boolean;
  canRedo?: boolean;
  masterCollapsed: boolean;
  onUndo?: () => void;
  onRedo?: () => void;
  onToggleMaster: () => void;
  headerActions?: React.ReactNode;
  closeButtonTestId: string;
  closeButtonInnerTestId?: string;
  onClose: () => void;
}> = (p) => {
  const { t } = useTranslation();
  return (
    <header data-testid="studio-drawer-header" style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", padding: "10px 14px", borderBottom: "1px solid var(--border-default)", backgroundColor: "var(--bg-elevated)", flexShrink: 0, flexWrap: "wrap" }}>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "8px", minWidth: 0, flexWrap: "wrap" }}>
        <BrandIcon name={p.iconName} size={STUDIO_ICON_LG} strokeWidth={STUDIO_STROKE_MONO} color="var(--action-primary)" />
        <h2 id={p.titleId} data-testid={p.titleTestId} style={{ margin: 0, fontSize: "14px", fontWeight: STUDIO_FONT_SEMIBOLD, color: "var(--text-primary)", whiteSpace: "nowrap" }}>
          {p.title}
        </h2>
        {p.metricLine}
      </div>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
        {p.onUndo && (
          <button type="button" data-testid="studio-undo-btn" disabled={!p.canUndo} onClick={p.onUndo} title={t("studioDrawer.undoTooltip", "Hoàn tác (Cmd+Z)")} style={{ ...ICON_BTN_STYLE, opacity: p.canUndo ? 1 : DISABLED_OPACITY, cursor: p.canUndo ? "pointer" : "not-allowed" }}>
            <BrandIcon name="undo" size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
            <span>{t("studioDrawer.undo", "Hoàn tác")}</span>
          </button>
        )}
        {p.onRedo && (
          <button type="button" data-testid="studio-redo-btn" disabled={!p.canRedo} onClick={p.onRedo} title={t("studioDrawer.redoTooltip", "Làm lại (Cmd+Shift+Z)")} style={{ ...ICON_BTN_STYLE, opacity: p.canRedo ? 1 : DISABLED_OPACITY, cursor: p.canRedo ? "pointer" : "not-allowed" }}>
            <BrandIcon name="redo" size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
            <span>{t("studioDrawer.redo", "Làm lại")}</span>
          </button>
        )}
        <button type="button" data-testid="studio-toggle-master-btn" aria-pressed={p.masterCollapsed} onClick={p.onToggleMaster} style={ICON_BTN_STYLE}>
          <BrandIcon name={p.masterCollapsed ? "panel-right" : "panel-left-close"} size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
          <span>{p.masterCollapsed ? t("brandStudio.expandMasterBtn", "Mở bảng trái") : t("brandStudio.collapseMasterBtn", "Thu gọn trái")}</span>
        </button>
        {p.headerActions}
        <button type="button" aria-label={t("common.close", "Đóng")} data-testid={p.closeButtonTestId} onClick={p.onClose} style={{ ...ICON_BTN_STYLE, padding: "5px" }}>
          {p.closeButtonInnerTestId ? (
            <span data-testid={p.closeButtonInnerTestId} style={{ display: "inline-flex", alignItems: "center" }}>
              <BrandIcon name="x" size={STUDIO_ICON_MD} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
            </span>
          ) : (
            <BrandIcon name="x" size={STUDIO_ICON_MD} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
          )}
        </button>
      </div>
    </header>
  );
};

const DrawerBodySplit: React.FC<{ props: StudioDrawerShellProps }> = ({ props }) => {
  const { layout } = props;
  const showMaster = layout.isMobile ? layout.mobileTab === "control" : !layout.masterCollapsed;
  const showDetail = layout.isMobile ? layout.mobileTab === "canvas" : true;
  return (
    <div style={{ flex: 1, display: "flex", minHeight: 0, overflow: "hidden" }}>
      {showMaster && (
        <div data-testid="studio-master-pane" style={{ width: layout.isMobile ? "100%" : `${layout.masterWidthPx}px`, flexShrink: 0, display: "flex", flexDirection: "column", borderRight: layout.isMobile ? "none" : "1px solid var(--border-default)", backgroundColor: "var(--bg-canvas)", minHeight: 0 }}>
          <div style={{ flex: 1, minHeight: 0, display: "flex", flexDirection: "column", overflow: "hidden" }}>{props.masterContent}</div>
          {!layout.subChatCollapsed && (
            <div role="separator" aria-orientation="horizontal" data-testid="studio-control-chat-resizer" onMouseDown={layout.startSubChatResize} style={{ height: "5px", cursor: "row-resize", backgroundColor: "var(--border-subtle)", flexShrink: 0 }} />
          )}
          <StudioSubChatPane {...props.subChat} heightPx={layout.subChatHeightPx} collapsed={layout.subChatCollapsed} onToggleCollapsed={layout.toggleSubChatCollapsed} />
        </div>
      )}
      {!layout.isMobile && !layout.masterCollapsed && (
        <div role="separator" aria-orientation="vertical" data-testid="studio-master-detail-resizer" onMouseDown={layout.startMasterResize} style={{ width: "5px", cursor: "col-resize", backgroundColor: "var(--border-subtle)", flexShrink: 0 }} />
      )}
      {showDetail && (
        <section data-testid="studio-detail-pane" style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", overflow: "hidden", position: "relative" }}>
          {props.detailContent}
        </section>
      )}
    </div>
  );
};

const DrawerInnerFrame: React.FC<{ props: StudioDrawerShellProps }> = ({ props }) => {
  const { t } = useTranslation();
  const { layout } = props;
  return (
    <>
      {!layout.isMobile && (
        <div role="separator" aria-orientation="vertical" data-testid="studio-drawer-resize-handle" onMouseDown={layout.startDrawerResize} style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: "6px", cursor: "col-resize", zIndex: STUDIO_RESIZER_Z }} />
      )}
      <DrawerHeaderChrome
        title={props.title} titleId={props.titleId} titleTestId={props.titleTestId ?? "studio-drawer-title"}
        iconName={props.iconName ?? "sparkles"} metricLine={props.metricLine}
        canUndo={props.canUndo} canRedo={props.canRedo} masterCollapsed={layout.masterCollapsed}
        onUndo={props.onUndo} onRedo={props.onRedo} onToggleMaster={layout.toggleMasterCollapsed}
        headerActions={props.headerActions} closeButtonTestId={props.closeButtonTestId ?? "studio-drawer-close-btn"}
        closeButtonInnerTestId={props.closeButtonInnerTestId} onClose={props.onClose}
      />
      {layout.isMobile && (
        <MobileSwitchBar
          activeTab={layout.mobileTab}
          controlLabel={props.mobileControlLabel ?? t("deckGen.mobileControlTab", "Điều khiển & Chat")}
          canvasLabel={props.mobileCanvasLabel ?? t("brandStudio.mobileCanvasTab", "Canvas xem trước")}
          onChange={layout.setMobileTab}
        />
      )}
      <DrawerBodySplit props={props} />
    </>
  );
};

export const StudioDrawerShell: React.FC<StudioDrawerShellProps> = (props) => {
  const { layout } = props;
  useEffect(() => {
    if (!props.isOpen) return;
    const onKey = (ev: KeyboardEvent) => {
      if (ev.isComposing || ev.defaultPrevented) return;
      if (ev.key === "Escape") props.onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [props.isOpen, props.onClose]);

  if (!props.isOpen) return <>{props.toastSlot}</>;

  const panelStyle: React.CSSProperties = {
    position: "relative", height: "100%",
    width: layout.isMobile ? "100vw" : `${layout.drawerWidthPx}px`,
    maxWidth: layout.isMobile ? "100vw" : "85vw",
    backgroundColor: "var(--bg-surface)", color: "var(--text-primary)",
    borderLeft: "1px solid var(--border-default)", boxShadow: "var(--shadow-lg)",
    display: "flex", flexDirection: "column", overflow: "hidden",
  };

  return (
    <div role="dialog" aria-modal="true" aria-labelledby={props.titleId} data-testid={props.testId} onClick={(e) => { if (e.target === e.currentTarget) props.onClose(); }} style={BACKDROP_STYLE}>
      <div data-testid="studio-right-drawer" style={panelStyle}>
        {props.asForm ? (
          <form onSubmit={props.onFormSubmit} data-ff-id={props.formTestId} data-testid={props.formTestId} style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0 }}>
            <DrawerInnerFrame props={props} />
          </form>
        ) : (
          <DrawerInnerFrame props={props} />
        )}
        {props.toastSlot}
      </div>
    </div>
  );
};
