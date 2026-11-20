// @story 04.5, 05.2
// @implements AC-05 (Comment-First Inline CommentPodPopover & Structured Comment Envelope — EC-E04-10)
import React, { useEffect, useRef, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { SpatialTargetSelection } from "../../flow/utils/sandbox-bridge-protocol";
import { computeFloatingPodLayout } from "../../flow/utils/floating-pod-layout";

const POPOVER_WIDTH_PX = 310;
const POPOVER_EST_HEIGHT_PX = 185;

export interface CommentPodPopoverProps {
  target: SpatialTargetSelection;
  stageWidth?: number;
  stageHeight?: number;
  isSubmitting?: boolean;
  onSubmitComment: (opts: { body: string; sendToAgent: boolean }) => Promise<void> | void;
  onClose: () => void;
}

const CommentPodHeader: React.FC<{
  target: SpatialTargetSelection;
  onClose: () => void;
}> = ({ target, onClose }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const isRegion = target.targetType === "spatial_region";
  return (
    <div className="ff-stage-pod-header">
      <span className="ff-stage-pod-badge">
        <BrandIcon name="message-square" size={12} color="var(--action-primary)" />
        <span>
          {isRegion
            ? `${t("spatial.commentPod.regionPrefix", "Vùng")} ${formatNumber(target.rect.width)}×${formatNumber(target.rect.height)} ${t("spatial.unitPx", "px")} · ${formatNumber(target.coveredFfIds.length)} ${t("spatial.commentPod.blocks", "khối")}`
            : `#${target.elementFfId}`}
        </span>
      </span>
      <button
        type="button"
        className="ff-spatial-pill-close"
        data-testid="comment-pod-close"
        aria-label={t("common.close", "Đóng")}
        onClick={onClose}
      >
        <BrandIcon name="close" size={11} color="currentColor" />
      </button>
    </div>
  );
};

const CommentPodFooter: React.FC<{
  canSubmit: boolean;
  onDispatch: (sendToAgent: boolean) => void;
}> = ({ canSubmit, onDispatch }) => {
  const { t } = useTranslation();
  return (
    <div className="ff-stage-pod-footer">
      <button
        type="button"
        className="cowok-btn cowok-btn-secondary ff-cms-mini-btn"
        data-testid="comment-pod-pin-only"
        disabled={!canSubmit}
        onClick={() => onDispatch(false)}
      >
        <span>{t("spatial.commentPod.pinOnly", "Chỉ ghim")}</span>
      </button>
      <button
        type="button"
        className="cowok-btn cowok-btn-primary ff-cms-mini-btn"
        data-testid="comment-pod-send-agent"
        disabled={!canSubmit}
        onClick={() => onDispatch(true)}
      >
        <BrandIcon name="sparkles" size={12} color="currentColor" />
        <span>{t("spatial.commentPod.sendToAgent", "Đính kèm vào Chat")}</span>
      </button>
    </div>
  );
};

const CommentPodTextarea: React.FC<{
  body: string;
  isSubmitting: boolean;
  canSubmit: boolean;
  onChangeBody: (v: string) => void;
  onDispatch: (sendToAgent: boolean) => void;
  onClose: () => void;
}> = ({ body, isSubmitting, canSubmit, onChangeBody, onDispatch, onClose }) => {
  const { t } = useTranslation();
  const [isComposing, setIsComposing] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    textareaRef.current?.focus({ preventScroll: true });
  }, []);
  return (
    <textarea
      ref={textareaRef}
      rows={3}
      className="ff-cms-textarea ff-comment-pod-textarea"
      data-testid="comment-pod-input"
      value={body}
      disabled={isSubmitting}
      placeholder={t("spatial.commentPod.placeholder", "Nhập yêu cầu chỉnh sửa hoặc ghi chú cho vị trí này...")}
      onCompositionStart={() => setIsComposing(true)}
      onCompositionEnd={() => setIsComposing(false)}
      onChange={(e) => onChangeBody(e.target.value)}
      onKeyDown={(e) => {
        if (isComposing) return;
        if (e.key === "Escape") { e.preventDefault(); onClose(); }
        else if ((e.metaKey || e.ctrlKey) && e.key === "Enter" && canSubmit) { e.preventDefault(); onDispatch(true); }
      }}
    />
  );
};

export const CommentPodPopover: React.FC<CommentPodPopoverProps> = ({
  target,
  stageWidth,
  stageHeight,
  isSubmitting = false,
  onSubmitComment,
  onClose,
}) => {
  const { t } = useTranslation();
  const [body, setBody] = useState("");
  const { left, top, width: podWidth } = computeFloatingPodLayout({
    rect: target.rect,
    stageWidth,
    stageHeight,
    preferredWidth: POPOVER_WIDTH_PX,
    estimatedHeight: POPOVER_EST_HEIGHT_PX,
  });

  const canSubmit = body.trim().length > 0 && !isSubmitting;
  const handleDispatch = (sendToAgent: boolean) => {
    const trimmed = body.trim();
    if (trimmed && !isSubmitting) void onSubmitComment({ body: trimmed, sendToAgent });
  };

  return (
    <div
      className="ff-stage-floating-pod ff-comment-pod"
      data-testid="comment-pod-popover"
      data-anchor-kind={target.targetType}
      role="dialog"
      aria-label={t("spatial.commentPod.dialogLabel", "Viết nhận xét gắn tọa độ")}
      style={{ left: `${left}px`, top: `${top}px`, width: `${podWidth}px` }}
      onMouseDown={(e) => e.stopPropagation()}
    >
      <CommentPodHeader target={target} onClose={onClose} />
      <CommentPodTextarea
        body={body}
        isSubmitting={isSubmitting}
        canSubmit={canSubmit}
        onChangeBody={setBody}
        onDispatch={handleDispatch}
        onClose={onClose}
      />
      <CommentPodFooter canSubmit={canSubmit} onDispatch={handleDispatch} />
    </div>
  );
};
