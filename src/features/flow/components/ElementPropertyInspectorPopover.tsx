// @story 05.1
// @implements AC-10 (Lightweight Read-Only Element Property Inspector Popover)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { SpatialTargetSelection } from "../utils/sandbox-bridge-protocol";
import { computeFloatingPodLayout } from "../utils/floating-pod-layout";

const POPOVER_WIDTH_PX = 300;
const POPOVER_EST_HEIGHT_PX = 220;

export interface ElementPropertyInspectorPopoverProps {
  target: SpatialTargetSelection;
  slotKind?: string;
  slotSyncState?: string;
  stageWidth?: number;
  stageHeight?: number;
  onJumpToInlineHtml?: () => void;
  onClose: () => void;
}

export const ElementPropertyInspectorPopover: React.FC<ElementPropertyInspectorPopoverProps> = ({
  target,
  slotKind,
  slotSyncState,
  stageWidth,
  stageHeight,
  onJumpToInlineHtml,
  onClose,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const [copied, setCopied] = useState(false);

  const { left, top, width: podWidth } = computeFloatingPodLayout({
    rect: target.rect,
    stageWidth,
    stageHeight,
    preferredWidth: POPOVER_WIDTH_PX,
    estimatedHeight: POPOVER_EST_HEIGHT_PX,
  });

  const cssEntries = Object.entries(target.metadata?.cssVariables ?? {}).slice(0, 6);
  const overflowPx = target.metadata?.overflowPx ?? 0;

  const handleCopyFfId = () => {
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(target.elementFfId);
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 1400);
  };

  return (
    <div
      className="ff-stage-floating-pod ff-inspector-pod"
      data-testid="element-property-inspector"
      role="dialog"
      aria-label={t("spatial.inspect.dialogLabel", "Thông số thành phần (Chỉ đọc)")}
      style={{ left: `${left}px`, top: `${top}px`, width: `${podWidth}px` }}
      onMouseDown={(e) => e.stopPropagation()}
    >
      <div className="ff-stage-pod-header">
        <span className="ff-stage-pod-badge">
          <BrandIcon name="info" size={12} color="var(--action-primary)" />
          <span>{`<${target.metadata?.tagName || "div"}> #${target.elementFfId}`}</span>
        </span>
        <button
          type="button"
          className="ff-spatial-pill-close"
          data-testid="inspector-close"
          aria-label={t("common.close", "Đóng")}
          onClick={onClose}
        >
          <BrandIcon name="close" size={11} color="currentColor" />
        </button>
      </div>

      <dl className="ff-inspector-kv-list">
        <div className="ff-inspector-kv-row">
          <dt>{t("spatial.inspect.dimensions", "Kích thước")}</dt>
          <dd>{`${formatNumber(target.rect.width)} × ${formatNumber(target.rect.height)} ${t("spatial.unitPx", "px")}`}</dd>
        </div>
        {slotKind && (
          <div className="ff-inspector-kv-row">
            <dt>{t("spatial.inspect.slotKind", "Loại khối CMS")}</dt>
            <dd>{slotKind}{slotSyncState ? ` · ${slotSyncState}` : ""}</dd>
          </div>
        )}
        <div className="ff-inspector-kv-row">
          <dt>{t("spatial.inspect.overflowStatus", "Tràn khung E3")}</dt>
          <dd data-overflow={overflowPx > 0 ? "true" : "false"}>
            {overflowPx > 0
              ? `+${formatNumber(overflowPx)} ${t("spatial.unitPx", "px")}`
              : t("spatial.inspect.overflowOk", "An toàn (0 px)")}
          </dd>
        </div>
        {cssEntries.map(([k, v]) => (
          <div key={k} className="ff-inspector-kv-row">
            <dt>{k}</dt>
            <dd title={v}>{v}</dd>
          </div>
        ))}
      </dl>

      <div className="ff-stage-pod-footer">
        <button
          type="button"
          className="cowok-btn cowok-btn-secondary ff-cms-mini-btn"
          data-testid="inspector-copy-ffid"
          onClick={handleCopyFfId}
        >
          <BrandIcon name="copy-text" size={11} color="currentColor" />
          <span>{copied ? t("common.copied", "Đã sao chép") : t("spatial.inspect.copyId", "Copy data-ff-id")}</span>
        </button>
        {onJumpToInlineHtml && (
          <button
            type="button"
            className="cowok-btn cowok-btn-secondary ff-cms-mini-btn"
            data-testid="inspector-jump-html"
            onClick={onJumpToInlineHtml}
          >
            <BrandIcon name="file-text" size={11} color="currentColor" />
            <span>{t("spatial.inspect.jumpHtml", "Mở mã HTML")}</span>
          </button>
        )}
      </div>
    </div>
  );
};
