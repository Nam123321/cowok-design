// @story 06.5
// @implements AC-02, AC-03, AC-04, AC-05 (Option D v2 Resizable Master-Detail RightDrawer Chart & Dashboard Studio)
import React, { useId } from "react";
import { CowokToast } from "../../../components/ui/CowokToast";
import { useTranslation } from "../../../hooks/useTranslation";
import { StudioComponentInspector } from "./dashboard-studio/StudioComponentInspector";
import {
  StudioBackendSyncPreview,
  StudioComponentListSection,
} from "./dashboard-studio/StudioControlSettings";
import { StudioDataTable } from "./dashboard-studio/StudioDataTable";
import { StudioDetailCanvas } from "./dashboard-studio/StudioDetailCanvas";
import { StudioHeader } from "./dashboard-studio/StudioHeader";
import { StudioSubChat } from "./dashboard-studio/StudioSubChat";
import {
  DRAWER_BACKDROP_STYLE,
  DRAWER_PANEL_BASE,
  segmentPillStyle,
} from "./dashboard-studio/studioStyles";
import type {
  DeckCsvGeneratorModalProps,
  StudioCommentPin,
  StudioDataRow,
  StudioMobileTab,
  StudioSnapshot,
  StudioWidgetId,
} from "./dashboard-studio/types";
import { useDeckStudioController } from "./dashboard-studio/useDeckStudioController";

export type { DeckCsvGeneratorModalProps };

const NEW_ROW_DEFAULT_CUR = 500;
const NEW_ROW_DEFAULT_PREV = 420;
const Z_INDEX_RESIZER = 20;
const Z_INDEX_TOAST = 30;

const MobileTabBar: React.FC<{
  activeTab: StudioMobileTab;
  onChange: (tab: StudioMobileTab) => void;
}> = ({ activeTab, onChange }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="studio-mobile-tabs" style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px", padding: "6px 12px", borderBottom: "1px solid var(--border-default)", backgroundColor: "var(--bg-subtle)" }}>
      <button type="button" data-testid="studio-mobile-tab-control" onClick={() => onChange("control")} style={segmentPillStyle(activeTab === "control")}>
        {t("deckGen.mobileControlTab", "Điều khiển & Chat")}
      </button>
      <button type="button" data-testid="studio-mobile-tab-canvas" onClick={() => onChange("canvas")} style={segmentPillStyle(activeTab === "canvas")}>
        {t("deckGen.mobileCanvasTab", "Xem Dashboard")}
      </button>
    </div>
  );
};

const MasterControlScrollArea: React.FC<{ ctrl: ReturnType<typeof useDeckStudioController> }> = ({ ctrl }) => {
  const snap = ctrl.history.snapshot;
  const commitPatch = (patch: Partial<StudioSnapshot>) => ctrl.history.commit((p) => ({ ...p, ...patch }));
  const toggleWidget = (id: StudioWidgetId) => ctrl.history.commit((p) => ({ ...p, widgets: { ...p.widgets, [id]: !p.widgets[id] } }));
  const updateRow = (rowId: string, patch: Partial<StudioDataRow>) =>
    ctrl.history.commit((p) => ({ ...p, rows: p.rows.map((r) => (r.id === rowId ? { ...r, ...patch } : r)) }));
  const addRow = () =>
    ctrl.history.commit((p) => ({ ...p, rows: [...p.rows, { id: `row-${Date.now()}`, label: `Kênh ${p.rows.length + 1}`, current: NEW_ROW_DEFAULT_CUR, previous: NEW_ROW_DEFAULT_PREV }] }));
  const deleteRow = (rowId: string) => ctrl.history.commit((p) => ({ ...p, rows: p.rows.filter((r) => r.id !== rowId) }));

  if (ctrl.controlView !== "general") {
    return (
      <div style={{ flex: 1, overflowY: "auto", minHeight: 0 }}>
        <StudioComponentInspector view={ctrl.controlView} snapshot={snap} onBack={() => ctrl.setControlView("general")} onPatch={commitPatch} onToggleWidget={toggleWidget} />
      </div>
    );
  }

  return (
    <div data-testid="studio-control-general" style={{ flex: 1, overflowY: "auto", padding: "12px", display: "flex", flexDirection: "column", gap: "14px", minHeight: 0 }}>
      <StudioDataTable
        fileInputRef={ctrl.fileInputRef} uploadedFile={ctrl.uploadedFile} pastedData={ctrl.pastedData}
        rows={snap.rows} unit={snap.unit} divergence={ctrl.divergence}
        highlightedRowId={ctrl.highlightedRowId} onHighlightRow={ctrl.setHighlightedRowId}
        onFileChange={ctrl.handleFileChange} onClearFile={ctrl.handleClearFile} onRawCsvChange={ctrl.handleRawCsvChange}
        onUpdateRow={updateRow} onAddRow={addRow} onDeleteRow={deleteRow} onRevertToSource={ctrl.handleRevertToSource}
      />
      <StudioBackendSyncPreview
        hasSource={Boolean(ctrl.uploadedFile || ctrl.pastedData.trim() || snap.rows.length > 0)}
        submitting={ctrl.submitting} normalizedChart={ctrl.normalizedChart} generatedSummary={ctrl.generatedSummary}
        onPreviewChart={() => { void ctrl.handlePreviewChart(); }}
      />
      <StudioComponentListSection snapshot={snap} onToggleWidget={toggleWidget} onDrillInto={ctrl.setControlView} />
    </div>
  );
};

const MasterColumnPane: React.FC<{ ctrl: ReturnType<typeof useDeckStudioController>; isMobile: boolean }> = ({ ctrl, isMobile }) => (
  <div
    data-testid="studio-master-pane"
    style={{
      width: isMobile ? "100%" : `${ctrl.layout.masterWidthPx}px`,
      flexShrink: 0, display: "flex", flexDirection: "column",
      borderRight: isMobile ? "none" : "1px solid var(--border-default)",
      backgroundColor: "var(--bg-surface)", minHeight: 0,
    }}
  >
    <MasterControlScrollArea ctrl={ctrl} />
    {!ctrl.layout.subChatCollapsed && (
      <div role="separator" aria-orientation="horizontal" data-testid="studio-control-chat-resizer" onMouseDown={ctrl.layout.startSubChatResize} style={{ height: "5px", cursor: "row-resize", backgroundColor: "var(--border-subtle)", flexShrink: 0 }} />
    )}
    <StudioSubChat
      heightPx={ctrl.layout.subChatHeightPx} collapsed={ctrl.layout.subChatCollapsed}
      selectedFfId={ctrl.selectedFfId} pinCount={ctrl.history.snapshot.pins.length}
      inputValue={ctrl.subChatInput} messages={ctrl.messages}
      onToggleCollapsed={ctrl.layout.toggleSubChatCollapsed} onInputChange={ctrl.setSubChatInput} onSend={ctrl.handleSubChatSend}
    />
  </div>
);

const StudioBodyContent: React.FC<{ ctrl: ReturnType<typeof useDeckStudioController> }> = ({ ctrl }) => {
  const snap = ctrl.history.snapshot;
  const { isMobile, mobileTab, masterCollapsed } = ctrl.layout;
  const showMaster = isMobile ? mobileTab === "control" : !masterCollapsed;
  const showDetail = isMobile ? mobileTab === "canvas" : true;
  const commitPatch = (patch: Partial<StudioSnapshot>) => ctrl.history.commit((p) => ({ ...p, ...patch }));
  const addPin = (pin: StudioCommentPin) => ctrl.history.commit((p) => ({ ...p, pins: [...p.pins, pin] }));
  const updatePinText = (id: string, text: string) =>
    ctrl.history.preview((p) => ({ ...p, pins: p.pins.map((item) => (item.id === id ? { ...item, text } : item)) }));
  const deletePin = (id: string) => ctrl.history.commit((p) => ({ ...p, pins: p.pins.filter((item) => item.id !== id) }));
  const applyPinToChat = (pin: StudioCommentPin) => {
    ctrl.handleSelectElement(pin.targetFfId);
    ctrl.setSubChatInput(pin.text.trim() ? `Xử lý ghim #${pin.num}: ${pin.text}` : `Xử lý ghim #${pin.num}`);
  };

  return (
    <div style={{ flex: 1, display: "flex", minHeight: 0, overflow: "hidden" }}>
      {showMaster && <MasterColumnPane ctrl={ctrl} isMobile={isMobile} />}
      {!isMobile && !masterCollapsed && (
        <div role="separator" aria-orientation="vertical" data-testid="studio-master-detail-resizer" onMouseDown={ctrl.layout.startMasterResize} style={{ width: "5px", cursor: "col-resize", backgroundColor: "var(--border-subtle)", flexShrink: 0 }} />
      )}
      {showDetail && (
        <StudioDetailCanvas
          snapshot={snap} interactionMode={ctrl.interactionMode} selectedFfId={ctrl.selectedFfId}
          highlightedRowId={ctrl.highlightedRowId} onModeChange={ctrl.setInteractionMode}
          onSelectElement={ctrl.handleSelectElement}
          onToggleRowHighlight={(id) => ctrl.setHighlightedRowId((cur) => (cur === id ? null : id))}
          onClearRowHighlight={() => ctrl.setHighlightedRowId(null)}
          onCommitPatch={commitPatch} onPreviewTarget={(targetValue) => ctrl.history.preview((p) => ({ ...p, targetValue }))}
          onCommitTarget={(baseTarget, nextTarget) => ctrl.history.commitFromBase({ ...snap, targetValue: baseTarget }, { ...snap, targetValue: nextTarget })}
          onAddPin={addPin} onUpdatePinText={updatePinText} onDeletePin={deletePin} onApplyPinToChat={applyPinToChat}
          onApplyAiLens={ctrl.handleApplyAiLens} onAutoFixQa={ctrl.handleAutoFixQa}
          onToggleCallout={ctrl.handleToggleCallout} onToggleSpotlight={ctrl.handleToggleSpotlight}
          onAskAiExplainRow={ctrl.handleAskAiExplainRow} onApplyWhatIf={ctrl.handleApplyWhatIf}
          onRevertToSource={ctrl.handleRevertToSource}
        />
      )}
    </div>
  );
};

export const DeckCsvGeneratorModal: React.FC<DeckCsvGeneratorModalProps> = ({
  isOpen, projectId, aspectRatio = "16:9", onClose, onDeckGenerated, onSyncToMainChat, onEmbedIntoActiveSlide,
}) => {
  const titleId = useId();
  const { t } = useTranslation();
  const ctrl = useDeckStudioController({ isOpen, projectId, aspectRatio, onClose, onDeckGenerated, onSyncToMainChat, onEmbedIntoActiveSlide });
  if (!isOpen) return null;

  const drawerStyle: React.CSSProperties = {
    ...DRAWER_PANEL_BASE,
    width: ctrl.layout.isMobile ? "100vw" : `${ctrl.layout.drawerWidthPx}px`,
    maxWidth: ctrl.layout.isMobile ? "100vw" : "85vw",
  };

  return (
    <div style={DRAWER_BACKDROP_STYLE} role="dialog" aria-modal="true" aria-labelledby={titleId} data-testid="deck-csv-generator-modal" onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div style={drawerStyle} data-testid="studio-right-drawer">
        {!ctrl.layout.isMobile && (
          <div role="separator" aria-orientation="vertical" aria-label={t("deckGen.resizeDrawerAria", "Kéo để đổi chiều rộng ngăn Studio")} data-testid="studio-drawer-resize-handle" onMouseDown={ctrl.layout.startDrawerResize} style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: "6px", cursor: "col-resize", zIndex: Z_INDEX_RESIZER }} />
        )}
        <form onSubmit={ctrl.handleSubmit} data-ff-id="deck-launchpad-form" data-testid="deck-launchpad-form" style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0 }}>
          <StudioHeader titleId={titleId} canUndo={ctrl.history.canUndo} canRedo={ctrl.history.canRedo} masterCollapsed={ctrl.layout.masterCollapsed} submitting={ctrl.submitting} onUndo={ctrl.history.undo} onRedo={ctrl.history.redo} onToggleMaster={ctrl.layout.toggleMasterCollapsed} onEmbedIntoSlide={ctrl.handleEmbedIntoSlide} onClose={onClose} />
          {ctrl.layout.isMobile && <MobileTabBar activeTab={ctrl.layout.mobileTab} onChange={ctrl.layout.setMobileTab} />}
          <StudioBodyContent ctrl={ctrl} />
        </form>
        {ctrl.toast && (
          <div style={{ position: "absolute", bottom: "14px", right: "14px", zIndex: Z_INDEX_TOAST, maxWidth: "420px" }}>
            <CowokToast tone={ctrl.toast.tone} message={ctrl.toast.message} testId="deck-gen-toast" durationMs={0} onDismiss={() => ctrl.setToast(null)} />
          </div>
        )}
      </div>
    </div>
  );
};
