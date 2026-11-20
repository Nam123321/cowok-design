// @story 06.2
// @implements AC-01, AC-03, AC-06
import React, { useEffect, useRef, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { SlidePhaseItem } from "./companion-canvas-data";
import type { CmsOverflowAlert, CmsSyncState } from "../hooks/useCmsSlideEditorController";
import { IME_COMPOSING_KEY_CODE } from "../utils/cms-draft-storage";
import type { PatchSlotItem, WordingSlotDto } from "../../../lib/validations/presentation";
import type { ChartSpecV12, ValidateChartResponse } from "../../../lib/validations/presentation-chart-specs";
import type { SlideCopilotLeaseState } from "../hooks/useSlideCopilotController";
import { OfflineChart2DRenderer } from "./OfflineChart2DRenderer";

const ICON_SM = 12;
const TEXTAREA_ROWS = 3;
const NOTES_ROWS = 3;

export interface CmsBlockEditorProps {
  structureLens: "userflow" | "slide-cms";
  activeSlide: SlidePhaseItem;
  slideUid: string;
  slots: WordingSlotDto[];
  speakerNotes: string;
  syncState: CmsSyncState;
  overflowAlert: CmsOverflowAlert | null;
  customHtmlOverride: boolean;
  isHydrating?: boolean;
  isPresenting?: boolean;
  isReadOnly?: boolean;
  backButtonRef?: React.RefObject<HTMLButtonElement | null>;
  onCloseDrawer: () => void;
  onComposingChange: (composing: boolean) => void;
  onEditField: (opts: {
    ffId: string;
    fieldKey: keyof SlidePhaseItem | "slot";
    slotKind: PatchSlotItem["slotKind"];
    nextValue: string;
    previousValue: string;
    bulletItems?: string[];
    chartsJson?: string;
  }) => void;
  onEditSpeakerNotes: (nextNotes: string) => void;
  onRequestAiOverflowAssist?: (payload: { slideUid: string; ffId?: string; overflowPx: number }) => void;
  filterFfIds?: string[];
  onClearFilter?: () => void;
  copilotLease?: SlideCopilotLeaseState;
  chartErrorByFfId?: Record<string, { code: string; message: string } | null>;
  onValidateChartSpec?: (ffId: string, spec: ChartSpecV12) => Promise<ValidateChartResponse | null>;
  onRenderChartStage?: (ffId: string, spec: ChartSpecV12, ariaLabel: string) => void;
  onRetryChartWithCopilot?: (ffId: string, errors?: string[]) => void;
  onCancelCopilot?: () => void;
  onOpenCopilot?: (ffId?: string) => void;
}

const SLOT_KIND_COPY: Record<PatchSlotItem["slotKind"], [string, string]> = {
  heading: ["presentation.cms.slotKind.heading", "Tiêu đề"],
  text: ["presentation.cms.slotKind.text", "Văn bản"],
  metric: ["presentation.cms.slotKind.metric", "Chỉ số"],
  bullet: ["presentation.cms.slotKind.bullet", "Danh sách"],
  image: ["presentation.cms.slotKind.image", "Hình ảnh"],
  color: ["presentation.cms.slotKind.color", "Màu sắc"],
  chart_json: ["presentation.cms.slotKind.chart_json", "Dữ liệu biểu đồ"],
};

const CmsDrawerHeader: React.FC<{
  structureLens: "userflow" | "slide-cms";
  backButtonRef?: React.RefObject<HTMLButtonElement | null>;
  slideCode: string;
  onCloseDrawer: () => void;
}> = (p) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const backLabel = p.structureLens === "slide-cms" 
    ? t("presentation.cms.backToSlides", "Danh sách Slide")
    : t("presentation.cms.backToUserflow", "Quay lại Luồng");
  return (
    <header className="ff-cms-drawer-header">
      <button ref={p.backButtonRef} type="button" data-testid="btn-back-slide-list" onClick={p.onCloseDrawer} className="cowok-btn cowok-btn-secondary ff-cms-back-btn">
        <BrandIcon name="arrow-left" size={ICON_SM} color="currentColor" />
        <span>{backLabel}</span>
      </button>
      <span className="ff-cms-slide-code">{p.slideCode}</span>
    </header>
  );
};

const CmsOverflowAlertBar: React.FC<{
  overflowAlert: CmsOverflowAlert | null;
  overflowPxLabel: string;
  onRequestAiAssist: () => void;
}> = ({ overflowAlert, overflowPxLabel, onRequestAiAssist }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  if (!overflowAlert || overflowAlert.overflowPx <= 0) return null;
  return (
    <div data-testid="e3-overflow-badge" data-severity={overflowAlert.severity} role="alert" className="ff-cms-e3-badge">
      <div className="ff-cms-e3-row">
        <BrandIcon name="shield-alert" size={ICON_SM} color="currentColor" />
        <span>{`${t("presentation.cms.e3Overflow", "Tràn khung 16:9 (E3)")}: +${overflowPxLabel}px`}</span>
      </div>
      {overflowAlert.severity === "danger" && (
        <button type="button" data-testid="cms-ai-assist-overflow" onClick={onRequestAiAssist} className="cowok-btn cowok-btn-secondary ff-cms-mini-btn">
          <BrandIcon name="sparkles" size={ICON_SM} color="currentColor" />
          <span>{t("presentation.cms.askCopilotTrim", "Nhờ AI rút gọn")}</span>
        </button>
      )}
    </div>
  );
};

function resolveSlotFieldKey(ffId: string): keyof SlidePhaseItem | "slot" {
  if (ffId === "ff-slide-headline" || ffId.endsWith("-headline")) return "headline";
  if (ffId === "ff-slide-subtitle" || ffId.endsWith("-body")) return "subtitle";
  return "slot";
}

function resolveSlotTestId(ffId: string, fieldKey: keyof SlidePhaseItem | "slot"): string {
  if (fieldKey === "headline") return "cms-input-headline";
  if (fieldKey === "subtitle") return "cms-input-subtitle";
  return `cms-slot-input-${ffId}`;
}

const CmsSlotInputField: React.FC<{
  slot: WordingSlotDto;
  inputId: string;
  testId: string;
  displayValue: string;
  isLocked: boolean;
  onComposingChange: (composing: boolean) => void;
  onChangeValue: (val: string) => void;
}> = ({ slot, inputId, testId, displayValue, isLocked, onComposingChange, onChangeValue }) => {
  const isMultiline = slot.slotKind === "text" || slot.slotKind === "bullet" || slot.slotKind === "chart_json";
  if (isMultiline) {
    return (
      <textarea
        id={inputId} data-testid={testId} rows={TEXTAREA_ROWS} disabled={isLocked} value={displayValue}
        onCompositionStart={() => onComposingChange(true)} onCompositionEnd={() => onComposingChange(false)}
        onChange={(e) => onChangeValue(e.target.value)} className="ff-cms-textarea"
      />
    );
  }
  return (
    <input
      id={inputId} data-testid={testId} type="text" disabled={isLocked} value={displayValue}
      onCompositionStart={() => onComposingChange(true)} onCompositionEnd={() => onComposingChange(false)}
      onChange={(e) => onChangeValue(e.target.value)} className="ff-cms-input"
    />
  );
};

const CmsDynamicSlotControl: React.FC<{
  slot: WordingSlotDto;
  isLocked: boolean;
  isRegionMatched?: boolean;
  chartError?: { code: string; message: string } | null;
  onComposingChange: (composing: boolean) => void;
  onEditField: CmsBlockEditorProps["onEditField"];
  onValidateChartSpec?: CmsBlockEditorProps["onValidateChartSpec"];
  onRenderChartStage?: CmsBlockEditorProps["onRenderChartStage"];
  onRetryChartWithCopilot?: CmsBlockEditorProps["onRetryChartWithCopilot"];
}> = ({ slot, isLocked, isRegionMatched, chartError, onComposingChange, onEditField, onValidateChartSpec, onRenderChartStage, onRetryChartWithCopilot }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const inputId = `cms-slot-${slot.ffId}`;
  const fieldKey = resolveSlotFieldKey(slot.ffId);
  const testId = resolveSlotTestId(slot.ffId, fieldKey);
  const [kindKey, kindFallback] = SLOT_KIND_COPY[slot.slotKind];
  const displayValue = slot.slotKind === "chart_json" ? (slot.chartsJson || slot.textValue) : slot.textValue;
  const handleValChange = (nextValue: string) =>
    onEditField({ ffId: slot.ffId, fieldKey, slotKind: slot.slotKind, nextValue, previousValue: displayValue, bulletItems: slot.slotKind === "bullet" ? nextValue.split("\n").filter(Boolean) : undefined, chartsJson: slot.slotKind === "chart_json" ? nextValue : undefined });

  return (
    <div className="ff-cms-field-card" data-slot-kind={slot.slotKind} data-slot-ff-id={slot.ffId} data-region-matched={isRegionMatched ? "true" : undefined}>
      <label htmlFor={inputId} className="ff-cms-field-label">{`${slot.slotLabel || slot.ffId} (${t(kindKey, kindFallback)})`}</label>
      {slot.slotKind === "chart_json" ? (
        <>
          <OfflineChart2DRenderer ffId={slot.ffId} rawJson={displayValue} disabled={isLocked} externalError={chartError} onChangeChartsJson={handleValChange} onValidateChartSpec={onValidateChartSpec} onRenderChartStage={onRenderChartStage} onRetryWithCopilot={onRetryChartWithCopilot} />
          <details className="ff-chart2d-raw-toggle">
            <summary>{t("presentation.charts.rawJsonToggle", "JSON Spec")}</summary>
            <CmsSlotInputField slot={slot} inputId={inputId} testId={testId} displayValue={displayValue} isLocked={isLocked} onComposingChange={onComposingChange} onChangeValue={handleValChange} />
          </details>
        </>
      ) : (
        <CmsSlotInputField slot={slot} inputId={inputId} testId={testId} displayValue={displayValue} isLocked={isLocked} onComposingChange={onComposingChange} onChangeValue={handleValChange} />
      )}
    </div>
  );
};

const CmsFallbackCoreFields: React.FC<{
  activeSlide: SlidePhaseItem;
  isLocked: boolean;
  onComposingChange: (composing: boolean) => void;
  onEditField: CmsBlockEditorProps["onEditField"];
}> = ({ activeSlide, isLocked, onComposingChange, onEditField }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  return (
    <>
      <div className="ff-cms-field-card">
        <label htmlFor="cms-input-headline" className="ff-cms-field-label">{t("canvas.slideHeadlineLabel", "Tiêu đề chính (Headline)")}</label>
        <input id="cms-input-headline" data-testid="cms-input-headline" type="text" disabled={isLocked} value={activeSlide.headline} onCompositionStart={() => onComposingChange(true)} onCompositionEnd={() => onComposingChange(false)} onChange={(e) => onEditField({ ffId: "ff-slide-headline", fieldKey: "headline", slotKind: "heading", nextValue: e.target.value, previousValue: activeSlide.headline })} className="ff-cms-input" />
      </div>
      <div className="ff-cms-field-card">
        <label htmlFor="cms-input-subtitle" className="ff-cms-field-label">{t("canvas.slideSubtitleLabel", "Nội dung trọng tâm (Subtitle)")}</label>
        <textarea id="cms-input-subtitle" data-testid="cms-input-subtitle" rows={TEXTAREA_ROWS} disabled={isLocked} value={activeSlide.subtitle} onCompositionStart={() => onComposingChange(true)} onCompositionEnd={() => onComposingChange(false)} onChange={(e) => onEditField({ ffId: "ff-slide-subtitle", fieldKey: "subtitle", slotKind: "text", nextValue: e.target.value, previousValue: activeSlide.subtitle })} className="ff-cms-textarea" />
      </div>
    </>
  );
};

const CmsCoreFields: React.FC<{
  activeSlide: SlidePhaseItem;
  slots: WordingSlotDto[];
  slotsCountLabel: string;
  isLocked: boolean;
  filterFfIds?: string[];
  onClearFilter?: () => void;
  chartErrorByFfId?: CmsBlockEditorProps["chartErrorByFfId"];
  onComposingChange: (composing: boolean) => void;
  onEditField: CmsBlockEditorProps["onEditField"];
  onValidateChartSpec?: CmsBlockEditorProps["onValidateChartSpec"];
  onRenderChartStage?: CmsBlockEditorProps["onRenderChartStage"];
  onRetryChartWithCopilot?: CmsBlockEditorProps["onRetryChartWithCopilot"];
}> = ({ activeSlide, slots, slotsCountLabel, isLocked, filterFfIds, onClearFilter, chartErrorByFfId, onComposingChange, onEditField, onValidateChartSpec, onRenderChartStage, onRetryChartWithCopilot }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const containerRef = useRef<HTMLDivElement>(null);
  const filterSet = filterFfIds && filterFfIds.length > 0 ? new Set(filterFfIds) : null;
  const matchedSlots = filterSet ? slots.filter((sl) => filterSet.has(sl.ffId)) : [];
  const visibleSlots = matchedSlots.length > 0 ? matchedSlots : slots;

  useEffect(() => {
    if (!filterFfIds || filterFfIds.length === 0) return;
    const firstEl = containerRef.current?.querySelector<HTMLElement>('[data-region-matched="true"]');
    firstEl?.scrollIntoView?.({ behavior: "smooth", block: "nearest" });
  }, [filterFfIds]);

  if (slots.length === 0) {
    return <CmsFallbackCoreFields activeSlide={activeSlide} isLocked={isLocked} onComposingChange={onComposingChange} onEditField={onEditField} />;
  }
  return (
    <div ref={containerRef} data-testid="cms-structured-slots-inventory">
      {matchedSlots.length > 0 && (
        <div className="ff-cms-region-filter-bar" data-testid="cms-region-filter-bar">
          <span>{t("spatial.regionFilter.active", "Đang lọc theo vùng chọn")} ({formatNumber(matchedSlots.length)}/{formatNumber(slots.length)})</span>
          {onClearFilter && <button type="button" className="cowok-btn cowok-btn-ghost ff-cms-mini-btn" data-testid="cms-clear-region-filter" onClick={onClearFilter}>{t("spatial.regionFilter.showAll", "Hiện tất cả")}</button>}
        </div>
      )}
      <div className="ff-cms-field-label">{`${t("presentation.cms.structuredSlots", "Trường cấu trúc")} (${slotsCountLabel})`}</div>
      {visibleSlots.map((slot) => (
        <CmsDynamicSlotControl key={slot.ffId} slot={slot} isLocked={isLocked} isRegionMatched={Boolean(filterSet?.has(slot.ffId))} chartError={chartErrorByFfId?.[slot.ffId]} onComposingChange={onComposingChange} onEditField={onEditField} onValidateChartSpec={onValidateChartSpec} onRenderChartStage={onRenderChartStage} onRetryChartWithCopilot={onRetryChartWithCopilot} />
      ))}
    </div>
  );
};

const CmsSpeakerNotesTray: React.FC<{
  speakerNotes: string;
  isReadOnly?: boolean;
  onComposingChange: (composing: boolean) => void;
  onEditSpeakerNotes: (nextNotes: string) => void;
}> = ({ speakerNotes, isReadOnly, onComposingChange, onEditSpeakerNotes }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const [showNotesHelp, setShowNotesHelp] = useState(false);
  return (
    <div className="ff-cms-notes-tray">
      <div className="ff-cms-notes-head">
        <label htmlFor="cms-input-speaker-notes" className="ff-cms-field-label">{t("presentation.cms.speakerNotesLabel", "Ghi chú diễn giả (Speaker Notes)")}</label>
        <button type="button" aria-expanded={showNotesHelp} aria-label={t("presentation.cms.speakerNotesHelpToggle", "Giải thích ghi chú diễn giả")} onClick={() => setShowNotesHelp((v) => !v)} className="ff-cms-help-toggle">
          <BrandIcon name="info" size={ICON_SM} color="currentColor" />
        </button>
      </div>
      {showNotesHelp && (
        <p className="ff-cms-notes-helper" role="note">
          {t("presentation.cms.speakerNotesHelper", "Chỉ hiển thị cho người thuyết trình, không thay đổi bố cục khung hình 16:9.")}
        </p>
      )}
      <textarea
        id="cms-input-speaker-notes" data-testid="cms-input-speaker-notes" rows={NOTES_ROWS} disabled={Boolean(isReadOnly)}
        placeholder={t("presentation.cms.speakerNotesPlaceholder", "Nhập lời dẫn hoặc điểm nhấn cần nói khi trình chiếu...")}
        value={speakerNotes} onCompositionStart={() => onComposingChange(true)} onCompositionEnd={() => onComposingChange(false)}
        onChange={(e) => onEditSpeakerNotes(e.target.value)} className="ff-cms-textarea"
      />
    </div>
  );
};

function formatLeaseBannerLabel(
  lease: SlideCopilotLeaseState,
  t: (k: string, f: string) => string,
  formatNumber: (v: number) => string
): string {
  if (lease.leaseState === "queued") {
    return `${t("presentation.copilot.queueLabel", "Queue")} #${formatNumber(Math.max(1, lease.queuePosition))}`;
  }
  if (lease.leaseState === "preflushing") {
    return t("presentation.copilot.syncing", "Syncing...");
  }
  return `${t("presentation.copilot.leaseLockBanner", "Copilot active")} · ${formatNumber(lease.remainingSeconds)}s`;
}

const CmsCopilotLeaseBanner: React.FC<{
  lease?: SlideCopilotLeaseState;
  isReadOnly?: boolean;
  onCancel?: () => void;
  onOpen?: () => void;
}> = ({ lease, isReadOnly, onCancel, onOpen }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const isBusy = lease && lease.leaseState !== "unlocked";
  if (!isBusy && (isReadOnly || !onOpen)) return null;
  if (!isBusy) {
    return (
      <div className="ff-cms-copilot-trigger-row">
        <button type="button" className="cowok-btn ff-cms-mini-btn" data-testid="cms-open-copilot-btn" onClick={onOpen}>
          <BrandIcon name="sparkles" size={ICON_SM} color="var(--agent-accent)" />
          <span>{t("presentation.copilot.openTrigger", "Slide Copilot")}</span>
        </button>
      </div>
    );
  }
  return (
    <div className="ff-cms-lease-banner" data-testid="slide-copilot-lease-badge" data-lease-state={lease.leaseState}>
      <span className="ff-cms-lease-label">
        <BrandIcon name="clock" size={ICON_SM} color="var(--agent-accent)" />
        {formatLeaseBannerLabel(lease, t, formatNumber)}
      </span>
      {onCancel && (lease.leaseState === "active" || lease.leaseState === "queued") && (
        <button type="button" className="cowok-btn ff-cms-mini-btn" onClick={onCancel}>
          {t("presentation.copilot.cancel", "Cancel")}
        </button>
      )}
    </div>
  );
};

export const CmsBlockEditor: React.FC<CmsBlockEditorProps> = (p) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const isLeaseLocked = Boolean(p.copilotLease && (p.copilotLease.leaseState === "active" || p.copilotLease.leaseState === "preflushing"));
  const isLocked = Boolean(p.isReadOnly || p.customHtmlOverride || isLeaseLocked);

  const handleDrawerKeyDown = (ev: React.KeyboardEvent<HTMLElement>) => {
    if (ev.key !== "Escape") return;
    if (p.isPresenting || ev.nativeEvent.isComposing || ev.keyCode === IME_COMPOSING_KEY_CODE) return;
    ev.stopPropagation();
    p.onCloseDrawer();
  };

  const handleRequestAiAssist = () => {
    if (!p.overflowAlert) return;
    const payload = { slideUid: p.slideUid, ffId: p.overflowAlert.ffId, overflowPx: p.overflowAlert.overflowPx };
    p.onRequestAiOverflowAssist?.(payload);
    if (typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent("cowok:ai-assist", { detail: { topic: "e3-overflow", ...payload } }));
    }
  };

  return (
    <section id="cms-slide-over-drawer" data-testid="cms-slide-over-drawer" role="region" aria-busy={p.isHydrating ? "true" : undefined} aria-label={t("presentation.cms.drawerLabel", "Trình biên tập nội dung Slide CMS")} onKeyDown={handleDrawerKeyDown} className="ff-cms-drawer">
      <CmsDrawerHeader structureLens={p.structureLens} backButtonRef={p.backButtonRef} slideCode={p.activeSlide.code} onCloseDrawer={p.onCloseDrawer} />
      <div className="ff-cms-scroll-body">
        <CmsCopilotLeaseBanner lease={p.copilotLease} isReadOnly={p.isReadOnly} onCancel={p.onCancelCopilot} onOpen={p.onOpenCopilot ? () => p.onOpenCopilot?.() : undefined} />
        <CmsOverflowAlertBar overflowAlert={p.overflowAlert} overflowPxLabel={formatNumber(p.overflowAlert?.overflowPx ?? 0)} onRequestAiAssist={handleRequestAiAssist} />
        <CmsCoreFields activeSlide={p.activeSlide} slots={p.slots} slotsCountLabel={formatNumber(p.slots.length)} isLocked={isLocked} filterFfIds={p.filterFfIds} onClearFilter={p.onClearFilter} chartErrorByFfId={p.chartErrorByFfId} onComposingChange={p.onComposingChange} onEditField={p.onEditField} onValidateChartSpec={p.onValidateChartSpec} onRenderChartStage={p.onRenderChartStage} onRetryChartWithCopilot={p.onRetryChartWithCopilot} />
        <CmsSpeakerNotesTray speakerNotes={p.speakerNotes} isReadOnly={Boolean(p.isReadOnly || isLeaseLocked)} onComposingChange={p.onComposingChange} onEditSpeakerNotes={p.onEditSpeakerNotes} />
      </div>
    </section>
  );
};
