// @story 06.5
// @implements AC-02, AC-03, AC-04, AC-05, [5.26] StudioDrawerShell, PAT-09, CMP-17
import React, { useId, useMemo } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { CowokToast } from "../../../components/ui/CowokToast";
import { StudioDrawerShell } from "../../../components/ui/studio-drawer";
import { useTranslation } from "../../../hooks/useTranslation";
import { StudioComponentInspector } from "./dashboard-studio/StudioComponentInspector";
import {
  StudioBackendSyncPreview,
  StudioComponentListSection,
} from "./dashboard-studio/StudioControlSettings";
import { StudioDataTable } from "./dashboard-studio/StudioDataTable";
import { StudioDetailCanvas } from "./dashboard-studio/StudioDetailCanvas";
import {
  ICON_MD,
  ICON_SM,
  STUDIO_PRIMARY_BTN,
  STUDIO_SECONDARY_BTN,
} from "./dashboard-studio/studioStyles";
import { runHonestDataQaAudit } from "./dashboard-studio/studioStorytellingUtils";
import type {
  DeckCsvGeneratorModalProps,
  StudioCommentPin,
  StudioDataRow,
  StudioWidgetId,
} from "./dashboard-studio/types";
import { useDeckStudioController } from "./dashboard-studio/useDeckStudioController";

export type { DeckCsvGeneratorModalProps };

const NEW_ROW_DEFAULT_CUR = 500;
const NEW_ROW_DEFAULT_PREV = 420;
const SUBMITTING_OPACITY = 0.7;
const Z_INDEX_TOAST = 30;

const MasterControlScrollArea: React.FC<{ ctrl: ReturnType<typeof useDeckStudioController> }> = ({ ctrl }) => {
  const snap = ctrl.history.snapshot;
  const commitPatch = ctrl.handleManualPatch;
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

const DashboardHeaderActions: React.FC<{
  submitting: boolean;
  onEmbedIntoSlide: () => void;
}> = ({ submitting, onEmbedIntoSlide }) => {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" data-testid="studio-embed-slide-btn" onClick={onEmbedIntoSlide} style={STUDIO_SECONDARY_BTN}>
        <BrandIcon name="layers" size={ICON_SM} color="currentColor" />
        <span>Gắn vào Slide hiện tại</span>
      </button>
      <button type="submit" disabled={submitting} data-testid="deck-generate-submit-btn" style={{ ...STUDIO_PRIMARY_BTN, opacity: submitting ? SUBMITTING_OPACITY : 1, cursor: submitting ? "wait" : "pointer" }}>
        <BrandIcon name="sparkles" size={ICON_MD} color="currentColor" />
        <span data-testid="deck-gen-primary-cta">
          {submitting ? t("deckGen.generatingCta", "Đang tạo bộ Slide...") : "Lưu & Tạo Bộ Slide"}
        </span>
      </button>
    </>
  );
};

const DashboardDetailSlot: React.FC<{ ctrl: ReturnType<typeof useDeckStudioController> }> = ({ ctrl }) => {
  const snap = ctrl.history.snapshot;
  const addPin = (pin: StudioCommentPin) => ctrl.history.commit((p) => ({ ...p, pins: [...p.pins, pin] }));
  const updatePinText = (id: string, text: string) =>
    ctrl.history.preview((p) => ({ ...p, pins: p.pins.map((item) => (item.id === id ? { ...item, text } : item)) }));
  const deletePin = (id: string) => ctrl.history.commit((p) => ({ ...p, pins: p.pins.filter((item) => item.id !== id) }));
  const applyPinToChat = (pin: StudioCommentPin) => {
    ctrl.handleSelectElement(pin.targetFfId);
    ctrl.setSubChatInput(pin.text.trim() ? `Xử lý ghim #${pin.num}: ${pin.text}` : `Xử lý ghim #${pin.num}`);
  };
  return (
    <StudioDetailCanvas
      snapshot={snap} interactionMode={ctrl.interactionMode} selectedFfId={ctrl.selectedFfId}
      highlightedRowId={ctrl.highlightedRowId} onModeChange={ctrl.setInteractionMode}
      onSelectElement={ctrl.handleSelectElement}
      onToggleRowHighlight={(id) => ctrl.setHighlightedRowId((cur) => (cur === id ? null : id))}
      onClearRowHighlight={() => ctrl.setHighlightedRowId(null)}
      onCommitPatch={ctrl.handleManualPatch}
      onPreviewTarget={(targetValue) => ctrl.history.preview((p) => ({ ...p, targetValue }))}
      onCommitTarget={(baseTarget, nextTarget) => ctrl.history.commitFromBase({ ...snap, targetValue: baseTarget }, { ...snap, targetValue: nextTarget })}
      onAddPin={addPin} onUpdatePinText={updatePinText} onDeletePin={deletePin} onApplyPinToChat={applyPinToChat}
      onApplyAiLens={ctrl.handleApplyAiLens} onAutoFixQa={ctrl.handleAutoFixQa}
      onToggleCallout={ctrl.handleToggleCallout} onToggleSpotlight={ctrl.handleToggleSpotlight}
      onAskAiExplainRow={ctrl.handleAskAiExplainRow} onApplyWhatIf={ctrl.handleApplyWhatIf}
      onRevertToSource={ctrl.handleRevertToSource}
      onFocusSubChat={(prefill) => {
        ctrl.layout.expandSubChatPanel();
        ctrl.setSubChatInput(prefill);
      }}
      onCommentSubmitted={ctrl.handleRecordCommentPin}
    />
  );
};

export const DeckCsvGeneratorModal: React.FC<DeckCsvGeneratorModalProps> = (props) => {
  const { isOpen, projectId, aspectRatio = "16:9", onClose, onDeckGenerated, onSyncToMainChat, onEmbedIntoActiveSlide } = props;
  const titleId = useId();
  const { t } = useTranslation();
  const ctrl = useDeckStudioController({ isOpen, projectId, aspectRatio, onClose, onDeckGenerated, onSyncToMainChat, onEmbedIntoActiveSlide });
  const snap = ctrl.history.snapshot;
  const qaCount = useMemo(() => runHonestDataQaAudit(snap).length, [snap]);
  const metricText = qaCount === 0 ? `${snap.chartType} · Honest QA PASS` : `${snap.chartType} · ${qaCount} QA cảnh báo`;

  return (
    <StudioDrawerShell
      isOpen={isOpen} title={t("deckGen.studioTitle", "Studio Biểu đồ & Dashboard")} titleId={titleId}
      testId="deck-csv-generator-modal" titleTestId="studio-drawer-title" layout={ctrl.layout}
      canUndo={ctrl.history.canUndo} canRedo={ctrl.history.canRedo} onUndo={ctrl.history.undo} onRedo={ctrl.history.redo}
      asForm formTestId="deck-launchpad-form" onFormSubmit={ctrl.handleSubmit}
      closeButtonTestId="deck-close-btn" closeButtonInnerTestId="deck-gen-close-btn" onClose={onClose}
      mobileControlLabel={t("deckGen.mobileControlTab", "Điều khiển & Chat")}
      mobileCanvasLabel={t("deckGen.mobileCanvasTab", "Xem Dashboard")}
      metricLine={<span data-testid="dashboard-studio-metric-line" style={{ fontSize: "11px", color: "var(--text-secondary)", fontVariantNumeric: "tabular-nums" }}>{metricText}</span>}
      headerActions={<DashboardHeaderActions submitting={ctrl.submitting} onEmbedIntoSlide={ctrl.handleEmbedIntoSlide} />}
      masterContent={<MasterControlScrollArea ctrl={ctrl} />}
      subChat={{
        selectedFfId: ctrl.selectedFfId, pinCount: snap.pins.length, inputValue: ctrl.subChatInput, messages: ctrl.messages,
        attachmentChips: ctrl.subChatAttachmentChips, onAttachFiles: ctrl.handleAttachSubChatFiles, onRemoveAttachmentChip: ctrl.handleRemoveSubChatChip,
        placeholder: t("deckGen.subChatPlaceholder", "Nhập lệnh (VD: Chuyển biểu đồ đường, Mục tiêu 1200)..."),
        syncToMainChatEnabled: ctrl.syncToMainChatEnabled, onToggleSyncToMainChat: ctrl.toggleSyncToMainChat,
        onInputChange: ctrl.setSubChatInput, onSend: ctrl.handleSubChatSend,
      }}
      detailContent={<DashboardDetailSlot ctrl={ctrl} />}
      toastSlot={ctrl.toast && (
        <div style={{ position: "absolute", bottom: "14px", right: "14px", zIndex: Z_INDEX_TOAST, maxWidth: "420px" }}>
          <CowokToast tone={ctrl.toast.tone} message={ctrl.toast.message} testId="deck-gen-toast" durationMs={0} onDismiss={() => ctrl.setToast(null)} />
        </div>
      )}
    />
  );
};
