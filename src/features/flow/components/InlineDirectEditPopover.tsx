// @story 05.1, 06.2
// @implements AC-10, AC-07 (Single-Component Direct Inline Edit Popover synced <16ms)
import React, { useEffect, useRef, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { SpatialTargetSelection } from "../utils/sandbox-bridge-protocol";
import { computeFloatingPodLayout } from "../utils/floating-pod-layout";

const POPOVER_WIDTH_PX = 310;
const POPOVER_EST_HEIGHT_PX = 185;

export interface InlineDirectEditPopoverProps {
  target: SpatialTargetSelection;
  initialValue: string;
  slotLabel?: string;
  stageWidth?: number;
  stageHeight?: number;
  isReadOnly?: boolean;
  onLiveChange: (nextValue: string, previousValue: string) => void;
  onComposingChange?: (composing: boolean) => void;
  onOpenFullCmsDrawer?: () => void;
  onClose: () => void;
}

function useInlineDirectEditState(opts: {
  initialValue: string;
  elementFfId: string;
  onLiveChange: (nextValue: string, previousValue: string) => void;
}) {
  const [value, setValue] = useState(opts.initialValue);
  const [isComposing, setIsComposing] = useState(false);
  const prevValueRef = useRef(opts.initialValue);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    setValue(opts.initialValue);
    prevValueRef.current = opts.initialValue;
  }, [opts.initialValue, opts.elementFfId]);

  useEffect(() => {
    inputRef.current?.focus({ preventScroll: true });
    inputRef.current?.select();
  }, [opts.elementFfId]);

  const handleChange = (next: string) => {
    const prev = prevValueRef.current;
    setValue(next);
    prevValueRef.current = next;
    opts.onLiveChange(next, prev);
  };

  return { value, isComposing, setIsComposing, inputRef, handleChange };
}

const InlineDirectEditFooter: React.FC<{
  onOpenFullCmsDrawer?: () => void;
  onClose: () => void;
}> = ({ onOpenFullCmsDrawer, onClose }) => {
  const { t } = useTranslation();
  return (
    <div className="ff-stage-pod-footer">
      {onOpenFullCmsDrawer && (
        <button type="button" className="cowok-btn cowok-btn-secondary ff-cms-mini-btn" data-testid="inline-edit-open-drawer" onClick={onOpenFullCmsDrawer}>
          <span>{t("spatial.inlineEdit.openLeftCms", "Mở trong CMS")}</span>
        </button>
      )}
      <button type="button" className="cowok-btn cowok-btn-primary ff-cms-mini-btn" data-testid="inline-edit-done" onClick={onClose}>
        <span>{t("spatial.inlineEdit.done", "Xong")}</span>
      </button>
    </div>
  );
};

export const InlineDirectEditPopover: React.FC<InlineDirectEditPopoverProps> = ({
  target, initialValue, slotLabel, stageWidth, stageHeight, isReadOnly = false,
  onLiveChange, onComposingChange, onOpenFullCmsDrawer, onClose,
}) => {
  const { t } = useTranslation();
  const st = useInlineDirectEditState({ initialValue, elementFfId: target.elementFfId, onLiveChange });
  const { left, top, width: podWidth } = computeFloatingPodLayout({
    rect: target.rect, stageWidth, stageHeight, preferredWidth: POPOVER_WIDTH_PX, estimatedHeight: POPOVER_EST_HEIGHT_PX,
  });

  return (
    <div
      className="ff-stage-floating-pod ff-inline-edit-pod"
      data-testid="inline-direct-edit-popover"
      role="dialog"
      aria-label={t("spatial.inlineEdit.dialogLabel", "Chỉnh sửa trực tiếp thành phần")}
      style={{ left: `${left}px`, top: `${top}px`, width: `${podWidth}px` }}
      onMouseDown={(e) => e.stopPropagation()}
    >
      <div className="ff-stage-pod-header">
        <span className="ff-stage-pod-badge">
          <BrandIcon name="edit" size={12} color="var(--human-accent)" />
          <span>{slotLabel || `#${target.elementFfId}`}</span>
        </span>
        <button type="button" className="ff-spatial-pill-close" data-testid="inline-edit-close" aria-label={t("common.close", "Đóng")} onClick={onClose}>
          <BrandIcon name="close" size={11} color="currentColor" />
        </button>
      </div>
      <textarea
        ref={st.inputRef} rows={3} className="ff-cms-textarea" data-testid="inline-direct-edit-input" disabled={isReadOnly} value={st.value}
        onCompositionStart={() => { st.setIsComposing(true); onComposingChange?.(true); }}
        onCompositionEnd={() => { st.setIsComposing(false); onComposingChange?.(false); }}
        onChange={(e) => st.handleChange(e.target.value)}
        onKeyDown={(e) => {
          if (!st.isComposing && (e.key === "Escape" || ((e.metaKey || e.ctrlKey) && e.key === "Enter"))) {
            e.preventDefault(); onClose();
          }
        }}
      />
      <InlineDirectEditFooter onOpenFullCmsDrawer={onOpenFullCmsDrawer} onClose={onClose} />
    </div>
  );
};
