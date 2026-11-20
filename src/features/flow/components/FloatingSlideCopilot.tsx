// @story 06.3
// @implements AC-02, AC-04 (Floating Slide Copilot Drawer — 0 useEffect, --agent-accent 3px rail)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { CowokSelect } from "../../../components/ui/CowokSelect";
import { AgentChatComposer, ChatStreamItem } from "../../../components/workbench/ChatPrimitivesBridge";
import { ChartKind2DSchema, type ChartKind2D } from "../../../lib/validations/presentation-chart-specs";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { startResizeDrag, useFloatingWindowGeometry } from "../hooks/useFloatingChatSync";
import type {
  SlideCopilotLeaseState,
  SlideCopilotMessage,
  SlideCopilotReceiptToast,
} from "../hooks/useSlideCopilotController";
import type { StructuredCommentEnvelope } from "../../../lib/validations/element-comment";

const COPILOT_MIN_WIDTH = 360;
const COPILOT_MAX_WIDTH = 900;
const COPILOT_MIN_HEIGHT = 480;
const COPILOT_MAX_HEIGHT = 850;

export interface FloatingSlideCopilotProps {
  isOpen: boolean;
  isCollapsed: boolean;
  isReadOnly: boolean;
  slideCode: string;
  slideUid: string;
  targetFfId: string | null;
  draftPrompt: string;
  attachedComment: StructuredCommentEnvelope | null;
  activeLease: SlideCopilotLeaseState;
  messages: SlideCopilotMessage[];
  activeGlobalSlots: number;
  maxGlobalSlots: number;
  onDraftPromptChange: (value: string) => void;
  onSendPrompt: (chartKind?: ChartKind2D) => void;
  onCancelLease: () => void;
  onCollapse: () => void;
  onExpand: () => void;
  onClose: () => void;
  onClearAttachedComment: () => void;
}

export const SlideCopilotToastBanner: React.FC<{
  toast: SlideCopilotReceiptToast | null;
  onDismiss: () => void;
}> = ({ toast, onDismiss }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  if (!toast) return null;
  return (
    <div className="ff-copilot-toast" data-testid="copilot-commit-toast" role="status">
      <BrandIcon name="check-circle" size={14} color="var(--state-success)" />
      <span>
        {t("presentation.copilot.commitToast", "Slide Copilot committed")} {t("presentation.copilot.revisionPrefix", "rev")} {formatNumber(toast.newRevision)} ({formatNumber(toast.patchedCount)})
      </span>
      <button type="button" className="ff-cms-mini-btn" aria-label={t("common.close", "Close")} onClick={onDismiss}>
        <BrandIcon name="close" size={11} color="currentColor" />
      </button>
    </div>
  );
};

function CopilotMetricLine(props: {
  targetFfId: string | null;
  activeLease: SlideCopilotLeaseState;
  activeGlobalSlots: number;
  maxGlobalSlots: number;
  onCancelLease: () => void;
}) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const { activeLease } = props;
  const isBusy = activeLease.leaseState === "active" || activeLease.leaseState === "queued";

  return (
    <div className="ff-copilot-metric-line" data-pattern="MetricLine-5.10" data-testid="copilot-metric-line">
      <span className="ff-copilot-target-chip">#{props.targetFfId || activeLease.targetFfId || "slide"}</span>
      <span className="ff-copilot-slots-chip">
        {formatNumber(props.activeGlobalSlots)}/{formatNumber(props.maxGlobalSlots)} {t("presentation.copilot.activeSlides", "active")}
      </span>
      {activeLease.leaseState === "active" && (
        <span className="ff-copilot-lease-timer" data-testid="copilot-countdown-badge">
          <BrandIcon name="clock" size={11} color="var(--agent-accent)" />
          <span>{formatNumber(activeLease.remainingSeconds)}s</span>
        </span>
      )}
      {activeLease.leaseState === "queued" && (
        <span className="ff-copilot-queue-chip" data-testid="copilot-queue-badge">
          {t("presentation.copilot.queueLabel", "Queue")} #{formatNumber(Math.max(1, activeLease.queuePosition))}
        </span>
      )}
      {activeLease.leaseState === "preflushing" && (
        <span className="ff-copilot-queue-chip">{t("presentation.copilot.syncing", "Syncing...")}</span>
      )}
      {isBusy && (
        <button type="button" className="cowok-btn ff-cms-mini-btn ff-copilot-cancel-btn" data-testid="slide-copilot-cancel-btn" onClick={props.onCancelLease}>
          {t("presentation.copilot.cancel", "Cancel")}
        </button>
      )}
    </div>
  );
}

function CopilotCollapsedPill(props: {
  slideCode: string;
  activeLease: SlideCopilotLeaseState;
  pillX: number;
  pillBottom: number;
  onHeaderMouseDown: (e: React.MouseEvent) => void;
  onExpand: () => void;
  onClose: () => void;
}) {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  return (
    <div
      className="ff-copilot-pill"
      data-testid="floating-slide-copilot-pill"
      style={{ left: `${props.pillX}px`, bottom: `${props.pillBottom}px` }}
      onMouseDown={props.onHeaderMouseDown}
    >
      <BrandIcon name="sparkles" size={13} color="var(--agent-accent)" />
      <span className="ff-copilot-pill-title">{props.slideCode}</span>
      {props.activeLease.leaseState === "active" && (
        <span className="ff-copilot-lease-timer">{formatNumber(props.activeLease.remainingSeconds)}s</span>
      )}
      {props.activeLease.leaseState === "queued" && (
        <span className="ff-copilot-queue-chip">#{formatNumber(Math.max(1, props.activeLease.queuePosition))}</span>
      )}
      <button type="button" className="cowok-btn ff-cms-mini-btn" onClick={props.onExpand}>
        {t("presentation.copilot.expand", "Open")}
      </button>
      <button type="button" className="ff-cms-mini-btn" aria-label={t("common.close", "Close")} onClick={props.onClose}>
        <BrandIcon name="close" size={11} color="currentColor" />
      </button>
    </div>
  );
}

function clampCopilotBounds(win: { width: number; height: number; x: number; bottom: number }) {
  return {
    width: Math.min(COPILOT_MAX_WIDTH, Math.max(COPILOT_MIN_WIDTH, win.width)),
    height: Math.min(COPILOT_MAX_HEIGHT, Math.max(COPILOT_MIN_HEIGHT, win.height)),
    x: win.x,
    bottom: win.bottom,
  };
}

function CopilotWindowHeader(props: {
  slideCode: string;
  onHeaderMouseDown: (e: React.MouseEvent) => void;
  onCollapse: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="ff-copilot-header" onMouseDown={props.onHeaderMouseDown}>
      <div className="ff-copilot-header-left">
        <BrandIcon name="sparkles" size={14} color="var(--agent-accent)" />
        <span>{t("presentation.copilot.title", "Slide Copilot")} · {props.slideCode}</span>
      </div>
      <div className="ff-copilot-header-actions">
        <button type="button" className="ff-cms-mini-btn" aria-label={t("presentation.copilot.collapse", "Minimize")} onClick={props.onCollapse}>
          <BrandIcon name="chevron-down" size={12} color="currentColor" />
        </button>
        <button type="button" className="ff-cms-mini-btn" data-testid="slide-copilot-close-btn" aria-label={t("common.close", "Close")} onClick={props.onClose}>
          <BrandIcon name="close" size={12} color="currentColor" />
        </button>
      </div>
    </div>
  );
}

const CopilotOutputKindBar: React.FC<{
  selectedChartKind: ChartKind2D | "";
  onChangeKind: (val: ChartKind2D | "") => void;
}> = ({ selectedChartKind, onChangeKind }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "4px 10px 0 10px", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", fontSize: "11px", color: "var(--text-secondary)" }}>
      <label htmlFor="slide-copilot-chart-kind" style={{ whiteSpace: "nowrap" }}>{t("presentation.copilot.outputKindLabel", "Output")}</label>
      <div style={{ minWidth: "180px", flex: 1 }}>
        <CowokSelect
          id="slide-copilot-chart-kind"
          testId="slide-copilot-chart-kind"
          ariaLabel={t("presentation.copilot.outputKindLabel", "Output")}
          value={selectedChartKind}
          onChange={(val) => onChangeKind(val as ChartKind2D | "")}
          options={[
            { value: "", label: t("presentation.copilot.outputKindAuto", "Auto (Rewrite / Edit Selected)") },
            ...ChartKind2DSchema.options.map((k) => ({ value: k, label: t(`presentation.charts.kinds.${k}`, k) })),
          ]}
        />
      </div>
    </div>
  );
};

export const FloatingSlideCopilot: React.FC<FloatingSlideCopilotProps> = (props) => {
  const { t, locale } = useTranslation();
  const { formatTime } = useFormatter({ locale });
  const [selectedChartKind, setSelectedChartKind] = useState<ChartKind2D | "">("");
  const geo = useFloatingWindowGeometry(props.isCollapsed, "cowok_slide_copilot_geometry", "right");
  if (!props.isOpen) return null;
  const defaultRightX = typeof window !== "undefined" ? Math.max(24, window.innerWidth - geo.width - 24) : 24;
  const curX = geo.anchor?.x ?? defaultRightX;
  const curBottom = geo.anchor?.bottom ?? 24;
  if (props.isCollapsed) {
    return <CopilotCollapsedPill slideCode={props.slideCode} activeLease={props.activeLease} pillX={curX} pillBottom={curBottom} onHeaderMouseDown={geo.handleHeaderMouseDown} onExpand={props.onExpand} onClose={props.onClose} />;
  }
  const bounded = clampCopilotBounds({ width: geo.width, height: geo.customHeight ?? COPILOT_MIN_HEIGHT, x: curX, bottom: curBottom });
  const applyResize = (next: { width?: number; height?: number; x?: number; bottom?: number }) => {
    if (typeof next.width === "number") geo.setWidth(next.width);
    if (typeof next.height === "number") geo.setCustomHeight(next.height);
    if (typeof next.x === "number" || typeof next.bottom === "number") geo.setAnchor({ x: next.x ?? curX, bottom: next.bottom ?? curBottom });
  };
  const chips = props.attachedComment ? [`#${props.attachedComment.elementFfId}: ${props.attachedComment.commentBody.slice(0, 48)}`] : [];

  return (
    <div className="ff-copilot-window" data-testid="floating-slide-copilot" role="dialog" aria-label={t("presentation.copilot.dialogLabel", "Slide Copilot")} style={{ left: `${bounded.x}px`, bottom: `${bounded.bottom}px`, width: `${bounded.width}px`, height: `${bounded.height}px` }}>
      <div className="ff-copilot-resize-n" onMouseDown={(e) => startResizeDrag(e, "n", bounded, applyResize)} />
      <div className="ff-copilot-resize-w" onMouseDown={(e) => startResizeDrag(e, "w", bounded, applyResize)} />
      <CopilotWindowHeader slideCode={props.slideCode} onHeaderMouseDown={geo.handleHeaderMouseDown} onCollapse={props.onCollapse} onClose={props.onClose} />
      <CopilotMetricLine targetFfId={props.targetFfId} activeLease={props.activeLease} activeGlobalSlots={props.activeGlobalSlots} maxGlobalSlots={props.maxGlobalSlots} onCancelLease={props.onCancelLease} />
      {props.activeLease.errorBanner && (
        <div className="ff-copilot-error-banner" role="alert" data-testid="copilot-error-banner"><BrandIcon name="shield-alert" size={12} color="var(--state-danger)" /><span>{props.activeLease.errorBanner.code}: {props.activeLease.errorBanner.message}</span></div>
      )}
      {props.isReadOnly && (
        <div className="ff-copilot-readonly-guard" role="note" data-testid="slide-copilot-readonly-guard"><BrandIcon name="info" size={12} color="var(--text-secondary)" /><span>{t("presentation.copilot.snapshotReadOnly", "Historical snapshot is read-only. Copilot dispatch disabled.")}</span></div>
      )}
      <div className="ff-copilot-stream" data-testid="slide-copilot-stream">
        {props.messages.map((m) => <ChatStreamItem key={m.id} msg={{ id: m.id, sender: m.role === "user" ? "human" : "agent", text: m.content, timestamp: formatTime(m.timestampMs) }} />)}
      </div>
      {!props.isReadOnly && (
        <>
          <CopilotOutputKindBar selectedChartKind={selectedChartKind} onChangeKind={setSelectedChartKind} />
          <AgentChatComposer testIdPrefix="slide-copilot" compact={true} inputValue={props.draftPrompt} onInputChange={props.onDraftPromptChange} onSend={() => props.onSendPrompt(selectedChartKind || undefined)} attachmentChips={chips} onRemoveAttachmentChip={props.onClearAttachedComment} placeholder={t("presentation.copilot.composerPlaceholder", "Ask Slide Copilot to rewrite text or update 2D chart...")} />
        </>
      )}
    </div>
  );
};
