// @story 04.1, 06.2
// @implements AC-01, AC-03, AC-06
import React, { useEffect, useRef } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ConflictState } from "../store/flow-workbench-types";
import type { SlidePhaseItem } from "./companion-canvas-data";
import { DraftDeployBanner } from "./DraftDeployBanner";
import { CmsBlockEditor } from "./CmsBlockEditor";
import { useCmsSlideEditorController } from "../hooks/useCmsSlideEditorController";
import type { SlideCopilotLeaseState } from "../hooks/useSlideCopilotController";
import type { ChartSpecV12, ValidateChartResponse } from "../../../lib/validations/presentation-chart-specs";
import { useFormatter } from "../../../hooks/useFormatter";

const ICON_SIZE_XS = 10;
const ICON_SIZE_MD = 12;
const ICON_SIZE_LG = 13;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;

const buildLensButtonStyle = (isActive: boolean): React.CSSProperties => ({
  height: "26px",
  padding: "0 8px",
  borderRadius: "4px 4px 0 0",
  border: "none",
  borderBottom: isActive ? "2px solid var(--action-primary)" : "2px solid transparent",
  backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
  color: isActive ? "var(--text-primary)" : "var(--text-secondary)",
  fontWeight: isActive ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
  fontSize: "11px",
  whiteSpace: "nowrap",
  flexShrink: 0,
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  cursor: "pointer",
  transition: "all var(--duration-fast) var(--ease-standard)",
});

const SlideListItemCard: React.FC<{
  slide: SlidePhaseItem;
  index: number;
  isCurrent: boolean;
  lease?: SlideCopilotLeaseState;
  onSelect: () => void;
}> = ({ slide, index, isCurrent, lease, onSelect }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const isBusy = lease && (lease.leaseState === "active" || lease.leaseState === "queued");
  return (
    <button
      type="button"
      data-slide-card-index={index}
      data-testid={`slide-card-${slide.id}`}
      onClick={onSelect}
      className="ff-slide-list-card"
      data-current={isCurrent ? "true" : "false"}
    >
      <div className="ff-slide-list-card-head">
        <span className="ff-slide-list-card-code">{slide.code}</span>
        {isBusy && (
          <span className="ff-slide-copilot-badge" data-copilot-status={lease.leaseState}>
            <BrandIcon name={lease.leaseState === "active" ? "sparkles" : "clock"} size={ICON_SIZE_XS} color="var(--agent-accent)" />
            <span>
              {lease.leaseState === "active"
                ? `${formatNumber(lease.remainingSeconds)}s`
                : `${t("presentation.copilot.queueLabel", "Queue")} #${formatNumber(Math.max(1, lease.queuePosition))}`}
            </span>
          </span>
        )}
        <span className="ff-slide-list-card-action">
          <span>{t("canvas.editSlideContent", "Sửa nội dung")}</span>
          <BrandIcon name="edit" size={ICON_SIZE_XS} color="currentColor" />
        </span>
      </div>
      <div className="ff-slide-list-card-title">{slide.headline || slide.title}</div>
      <div className="ff-slide-list-card-subtitle">{slide.subtitle}</div>
    </button>
  );
};

const StructurePaneHeader: React.FC<{
  structureLens: "userflow" | "slide-cms";
  isDeckTab?: boolean;
  onSetLens: (lens: "userflow" | "slide-cms") => void;
  isRightPreviewHidden: boolean;
  onToggleRightPreviewHidden: () => void;
}> = ({ structureLens, isDeckTab = true, onSetLens, isRightPreviewHidden, onToggleRightPreviewHidden }) => {
  const { t } = useTranslation();
  const toggleTitle = isRightPreviewHidden
    ? t("canvas.restoreSplitView", "Thu gọn về chế độ Chia đôi (Hiện lại Live HTML Preview)")
    : t("canvas.expandStructurePane", "Mở rộng toàn khung Cấu trúc (Thu gọn Live HTML Preview)");
  const slideLensLabel = isDeckTab ? t("canvas.slideCmsLens", "Trình chiếu Slide") : t("canvas.appCmsLens", "Biên tập CMS");

  return (
    <div className="structure-pane-header" style={{ height: "35px", minHeight: "35px", padding: "0 8px", backgroundColor: "var(--bg-surface)", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
      <div className="btn-1click-view-toggle" role="tablist" style={{ display: "inline-flex", alignItems: "center", gap: "4px", flexWrap: "nowrap", minWidth: 0 }}>
        <button
          type="button"
          role="tab"
          aria-selected={structureLens === "userflow"}
          data-testid="lens-userflow-btn"
          title={t("canvas.userflowLensTooltip", "Sơ đồ luồng người dùng 2D (Userflow DAG): thiết kế cấu trúc màn hình, điều kiện rẽ nhánh và luồng chuyển trạng thái.")}
          onClick={() => onSetLens("userflow")}
          style={buildLensButtonStyle(structureLens === "userflow")}
        >
          <BrandIcon name="workflow" size={ICON_SIZE_MD} color="currentColor" />
          <span>{t("canvas.userflowLens", "Luồng người dùng")}</span>
        </button>
        <span data-testid="lens-slide-cms" style={{ display: "inline-flex" }}>
          <button
            type="button"
            role="tab"
            aria-selected={structureLens === "slide-cms"}
            data-testid="lens-slide-cms-btn"
            title={t("canvas.slideCmsLensTooltip", "Trình chiếu Slide & Zero-Code CMS: biên tập nội dung slide, tiêu đề và chỉ số KPI trực quan không cần viết mã.")}
            onClick={() => onSetLens("slide-cms")}
            style={buildLensButtonStyle(structureLens === "slide-cms")}
          >
            <BrandIcon name="layers" size={ICON_SIZE_MD} color="currentColor" />
            <span>{slideLensLabel}</span>
          </button>
        </span>
      </div>
      <button
        type="button"
        onClick={onToggleRightPreviewHidden}
        title={toggleTitle}
        aria-label={t("canvas.expandStructurePane", "Thu gọn / Mở rộng khung Cấu trúc")}
        className="cowok-btn cowok-btn-ghost"
        style={{ width: "28px", height: "28px", padding: 0, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: isRightPreviewHidden ? "var(--bg-selected)" : "var(--bg-workspace)", color: isRightPreviewHidden ? "var(--text-primary)" : "var(--text-secondary)" }}
      >
        <BrandIcon name={isRightPreviewHidden ? "minimize" : "maximize"} size={ICON_SIZE_LG} color="currentColor" />
      </button>
    </div>
  );
};

const MIN_STRUCTURE_PANE_WIDTH_PX = "320px";

function useSlideCmsTrackAccessibility(isCmsDrawerOpen: boolean, activeSlideIndex: number) {
  const slideListHalfRef = useRef<HTMLDivElement>(null);
  const cmsDrawerHalfRef = useRef<HTMLDivElement>(null);
  const backButtonRef = useRef<HTMLButtonElement>(null);
  const prevDrawerOpenRef = useRef<boolean>(isCmsDrawerOpen);

  // Enforce symmetric inert attribute on offscreen track half
  useEffect(() => {
    const listEl = slideListHalfRef.current as (HTMLDivElement & { inert?: boolean }) | null;
    const drawerEl = cmsDrawerHalfRef.current as (HTMLDivElement & { inert?: boolean }) | null;
    if (listEl) {
      listEl.inert = isCmsDrawerOpen;
      if (isCmsDrawerOpen) listEl.setAttribute("inert", "");
      else listEl.removeAttribute("inert");
    }
    if (drawerEl) {
      drawerEl.inert = !isCmsDrawerOpen;
      if (!isCmsDrawerOpen) drawerEl.setAttribute("inert", "");
      else drawerEl.removeAttribute("inert");
    }
  }, [isCmsDrawerOpen]);

  useEffect(() => {
    if (prevDrawerOpenRef.current === isCmsDrawerOpen) return;
    prevDrawerOpenRef.current = isCmsDrawerOpen;
    if (isCmsDrawerOpen) {
      backButtonRef.current?.focus({ preventScroll: true });
    } else {
      slideListHalfRef.current?.querySelector<HTMLButtonElement>(`[data-slide-card-index="${activeSlideIndex}"]`)?.focus({ preventScroll: true });
    }
  }, [activeSlideIndex, isCmsDrawerOpen]);

  return { slideListHalfRef, cmsDrawerHalfRef, backButtonRef };
}

export type CmsStudioControllerAdapter = Pick<
  ReturnType<typeof useCmsSlideEditorController>,
  | "slideUid"
  | "slots"
  | "speakerNotes"
  | "syncState"
  | "overflowAlert"
  | "customHtmlOverride"
  | "isHydrating"
  | "setComposing"
  | "applySlideFieldEdit"
  | "applySpeakerNotesEdit"
>;

interface SlideCmsStudioBodyProps {
  structureLens: "userflow" | "slide-cms";
  slides: SlidePhaseItem[];
  activeSlideIndex: number;
  activeSlide: SlidePhaseItem;
  isCmsDrawerOpen: boolean;
  isPresenting?: boolean;
  isReadOnly?: boolean;
  cmsController?: CmsStudioControllerAdapter;
  cmsFilterFfIds?: string[];
  onClearCmsFilter?: () => void;
  onSelectSlide: (idx: number) => void;
  onCloseCmsDrawer: () => void;
  onUpdateActiveSlide: (patch: Partial<SlidePhaseItem>) => void;
  copilotLeasesBySlide?: Record<string, SlideCopilotLeaseState>;
  chartErrorByFfId?: Record<string, { code: string; message: string } | null>;
  onValidateChartSpec?: (ffId: string, spec: ChartSpecV12) => Promise<ValidateChartResponse | null>;
  onRenderChartStage?: (ffId: string, spec: ChartSpecV12, ariaLabel: string) => void;
  onRetryChartWithCopilot?: (ffId: string, errors?: string[]) => void;
  onCancelCopilot?: () => void;
  onOpenCopilot?: (ffId?: string) => void;
}

function buildCmsBlockEditorProps(p: SlideCmsStudioBodyProps, backButtonRef: React.RefObject<HTMLButtonElement | null>) {
  const c = p.cmsController;
  const slideUid = c?.slideUid ?? p.activeSlide.id;
  return {
    structureLens: p.structureLens,
    activeSlide: p.activeSlide,
    slideUid,
    slots: c?.slots ?? [],
    speakerNotes: c?.speakerNotes ?? "",
    syncState: c?.syncState ?? ("SYNCED" as const),
    overflowAlert: c?.overflowAlert ?? null,
    customHtmlOverride: c?.customHtmlOverride ?? false,
    isHydrating: c?.isHydrating ?? false,
    isPresenting: p.isPresenting,
    isReadOnly: p.isReadOnly,
    backButtonRef,
    onCloseDrawer: p.onCloseCmsDrawer,
    onComposingChange: (val: boolean) => c?.setComposing(val),
    onEditField: (opts: Parameters<NonNullable<typeof c>["applySlideFieldEdit"]>[0]) => {
      if (c) c.applySlideFieldEdit(opts);
      else if (opts.fieldKey !== "slot") p.onUpdateActiveSlide({ [opts.fieldKey]: opts.nextValue } as Partial<SlidePhaseItem>);
    },
    onEditSpeakerNotes: (notes: string) => c?.applySpeakerNotesEdit(notes),
    filterFfIds: p.cmsFilterFfIds,
    onClearFilter: p.onClearCmsFilter,
    copilotLease: p.copilotLeasesBySlide?.[slideUid] ?? p.copilotLeasesBySlide?.[p.activeSlide.id],
    chartErrorByFfId: p.chartErrorByFfId,
    onValidateChartSpec: p.onValidateChartSpec,
    onRenderChartStage: p.onRenderChartStage,
    onRetryChartWithCopilot: p.onRetryChartWithCopilot,
    onCancelCopilot: p.onCancelCopilot,
    onOpenCopilot: p.onOpenCopilot,
  };
}

const SlideCmsStudioBody: React.FC<SlideCmsStudioBodyProps> = (p) => {
  const { slideListHalfRef, cmsDrawerHalfRef, backButtonRef } = useSlideCmsTrackAccessibility(p.isCmsDrawerOpen, p.activeSlideIndex);
  const editorProps = buildCmsBlockEditorProps(p, backButtonRef);
  return (
    <div className="slide-deck-cms-studio ff-cms-studio-root">
      <div id="slide-cms-track-viewport" data-testid="slide-cms-track-viewport" className="ff-slide-cms-track-viewport">
        <div className="ff-slide-cms-track" data-drawer-open={p.isCmsDrawerOpen ? "true" : "false"}>
          <div ref={slideListHalfRef} id="slide-list-half" data-testid="slide-list-half" aria-hidden={p.isCmsDrawerOpen ? "true" : undefined} className="ff-slide-cms-half">
            {p.slides.map((sld, idx) => (
              <SlideListItemCard key={sld.id} slide={sld} index={idx} isCurrent={idx === p.activeSlideIndex} lease={p.copilotLeasesBySlide?.[sld.id]} onSelect={() => p.onSelectSlide(idx)} />
            ))}
          </div>
          <div ref={cmsDrawerHalfRef} aria-hidden={!p.isCmsDrawerOpen ? "true" : undefined} className="ff-slide-cms-half">
            <CmsBlockEditor {...editorProps} />
          </div>
        </div>
      </div>
    </div>
  );
};

export const CompanionStructureLeftPane: React.FC<{
  widthPercent: number;
  isRightPreviewHidden: boolean;
  structureLens: "userflow" | "slide-cms";
  isDeckTab?: boolean;
  onSetLens: (lens: "userflow" | "slide-cms") => void;
  onToggleRightPreviewHidden: () => void;
  isDraftDirty: boolean;
  orphanCount?: number;
  conflictState?: ConflictState | null;
  onDeployDraft?: () => void;
  onDiscardDraft?: () => void;
  onResolveConflict?: (choice: "keep_mine" | "take_agent" | "inspect_collisions") => void;
  slides: SlidePhaseItem[];
  activeSlideIndex: number;
  isCmsDrawerOpen: boolean;
  isPresenting?: boolean;
  isReadOnly?: boolean;
  cmsController?: CmsStudioControllerAdapter;
  cmsFilterFfIds?: string[];
  onClearCmsFilter?: () => void;
  onSelectSlide: (idx: number) => void;
  onCloseCmsDrawer: () => void;
  onUpdateActiveSlide: (patch: Partial<SlidePhaseItem>) => void;
  isWalkthroughActive?: boolean;
  walkthroughStepLabel?: string;
  copilotLeasesBySlide?: Record<string, SlideCopilotLeaseState>;
  chartErrorByFfId?: Record<string, { code: string; message: string } | null>;
  onValidateChartSpec?: (ffId: string, spec: ChartSpecV12) => Promise<ValidateChartResponse | null>;
  onRenderChartStage?: (ffId: string, spec: ChartSpecV12, ariaLabel: string) => void;
  onRetryChartWithCopilot?: (ffId: string, errors?: string[]) => void;
  onCancelCopilot?: () => void;
  onOpenCopilot?: (ffId?: string) => void;
  children: React.ReactNode;
}> = (props) => {
  const { t } = useTranslation();
  const activeSlide = props.slides[props.activeSlideIndex] || props.slides[0];
  return (
    <div id="structureSubPane" className="canvas-structure-left-pane" aria-label={t("canvas.structurePane", "Khung Cấu trúc Userflow & Slide CMS")} style={{ width: props.isRightPreviewHidden ? "100%" : `${props.widthPercent}%`, minWidth: MIN_STRUCTURE_PANE_WIDTH_PX, height: "100%", display: "flex", flexDirection: "column", backgroundColor: "var(--bg-canvas)", overflow: "hidden", position: "relative" }}>
      <StructurePaneHeader structureLens={props.structureLens} isDeckTab={props.isDeckTab} onSetLens={props.onSetLens} isRightPreviewHidden={props.isRightPreviewHidden} onToggleRightPreviewHidden={props.onToggleRightPreviewHidden} />
      {props.isWalkthroughActive && (
        <div data-testid="walkthrough-dag-sync-banner" style={{ padding: "5px 10px", backgroundColor: "var(--bg-selected)", borderBottom: "1px solid var(--action-primary)", color: "var(--action-primary)", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
          <span style={{ display: "inline-flex", alignItems: "center", gap: "5px" }}>
            <BrandIcon name="workflow" size={ICON_SIZE_MD} color="currentColor" />
            <span>{t("canvas.walkthroughDagSyncLabel", "Walkthrough Dual Screen — Đồng bộ DAG")}</span>
          </span>
          {props.walkthroughStepLabel && <span style={{ fontFamily: "var(--font-mono)", fontSize: "10px" }}>{props.walkthroughStepLabel}</span>}
        </div>
      )}
      <DraftDeployBanner isDraftDirty={props.isDraftDirty} orphanCount={props.orphanCount} conflictState={props.conflictState} onDeployDraft={props.onDeployDraft} onDiscardDraft={props.onDiscardDraft} onResolveConflict={props.onResolveConflict} />
      <div style={{ flex: 1, position: "relative", overflow: "hidden" }}>
        {props.structureLens === "userflow" ? props.children : (
          <SlideCmsStudioBody
            structureLens={props.structureLens}
            slides={props.slides}
            activeSlideIndex={props.activeSlideIndex}
            activeSlide={activeSlide}
            isCmsDrawerOpen={props.isCmsDrawerOpen}
            isPresenting={props.isPresenting}
            isReadOnly={props.isReadOnly}
            cmsController={props.cmsController}
            cmsFilterFfIds={props.cmsFilterFfIds}
            onClearCmsFilter={props.onClearCmsFilter}
            onSelectSlide={props.onSelectSlide}
            onCloseCmsDrawer={props.onCloseCmsDrawer}
            onUpdateActiveSlide={props.onUpdateActiveSlide}
            copilotLeasesBySlide={props.copilotLeasesBySlide}
            chartErrorByFfId={props.chartErrorByFfId}
            onValidateChartSpec={props.onValidateChartSpec}
            onRenderChartStage={props.onRenderChartStage}
            onRetryChartWithCopilot={props.onRetryChartWithCopilot}
            onCancelCopilot={props.onCancelCopilot}
            onOpenCopilot={props.onOpenCopilot}
          />
        )}
      </div>
    </div>
  );
};
