// @story 05.1, 04.5, 06.2
// @implements AC-10 (Compact Icon-Only Context-Aware Floating Pill with Hover Tooltips & Pure Semantic CSS)
import React, { useEffect } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { SpatialTargetSelection } from "../utils/sandbox-bridge-protocol";

export type SpatialActionKind = "comment" | "edit-draft" | "inspect" | "link-dag";

const ICON_PX = 14;
const PILL_OFFSET_Y = 8;
const PILL_MIN_X = 12;
const PILL_MIN_Y = 12;
const PILL_EST_WIDTH_PX = 160;
const PILL_FALLBACK_WIDTH_PX = 340;
const BOTTOM_TOOLTIP_CLEARANCE_PX = 88;
const RIGHT_ALIGN_THRESHOLD_PX = 200;

export interface StageSpatialActionPillProps {
  target: SpatialTargetSelection;
  structureLens: "userflow" | "slide-cms";
  isLeftStructureHidden: boolean;
  isLeftDagFocusedOnNode: boolean;
  isReadOnly?: boolean;
  stageWidth?: number;
  stageHeight?: number;
  onSelectAction: (action: SpatialActionKind) => void;
  onClose: () => void;
}

export function shouldShowLinkToDagAction(opts: {
  structureLens: "userflow" | "slide-cms";
  isLeftStructureHidden: boolean;
  isLeftDagFocusedOnNode: boolean;
}): boolean {
  if (opts.structureLens === "slide-cms") return false;
  if (!opts.isLeftStructureHidden && opts.isLeftDagFocusedOnNode) return false;
  return true;
}

function isEditableKeyboardEvent(ev: KeyboardEvent): boolean {
  if (ev.isComposing) return true;
  const el = ev.target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName?.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || el.isContentEditable;
}

const PILL_TOP_OFFSET_PX = 40;
const PILL_START_ALIGN_PX = 120;

function useSpatialPillHotkeys(opts: {
  isReadOnly: boolean;
  isRegion: boolean;
  showLinkDag: boolean;
  onSelectAction: (action: SpatialActionKind) => void;
  onClose: () => void;
}) {
  const { isReadOnly, isRegion, showLinkDag, onSelectAction, onClose } = opts;
  useEffect(() => {
    const onKeyDown = (ev: KeyboardEvent) => {
      if (isEditableKeyboardEvent(ev)) return;
      const key = ev.key.toLowerCase();
      if (key === "escape") { ev.preventDefault(); onClose(); return; }
      if (key === "c" && !isReadOnly) { ev.preventDefault(); onSelectAction("comment"); }
      else if (key === "e" && !isReadOnly) { ev.preventDefault(); onSelectAction("edit-draft"); }
      else if (key === "i" && !isRegion) { ev.preventDefault(); onSelectAction("inspect"); }
      else if (key === "l" && showLinkDag) { ev.preventDefault(); onSelectAction("link-dag"); }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isReadOnly, isRegion, onClose, onSelectAction, showLinkDag]);
}

function computePillLayout(rect: SpatialTargetSelection["rect"], stageWidth?: number, stageHeight?: number) {
  const effectiveStageWidth = stageWidth ?? PILL_FALLBACK_WIDTH_PX;
  const maxLeft = Math.max(PILL_MIN_X, effectiveStageWidth - PILL_EST_WIDTH_PX);
  const clampedLeft = Math.min(Math.max(PILL_MIN_X, rect.left), maxLeft);
  const preferredTop = rect.top - PILL_TOP_OFFSET_PX;
  const clampedTop = preferredTop >= PILL_MIN_Y ? preferredTop : rect.top + rect.height + PILL_OFFSET_Y;
  const resolvedTop = Math.max(PILL_MIN_Y, clampedTop);
  const tooltipPlacement = typeof stageHeight === "number" && stageHeight > 0 && resolvedTop + BOTTOM_TOOLTIP_CLEARANCE_PX > stageHeight ? "top" : "bottom";
  const tooltipAlign = clampedLeft < PILL_START_ALIGN_PX ? "start" : clampedLeft + RIGHT_ALIGN_THRESHOLD_PX > effectiveStageWidth ? "end" : "center";
  return { clampedLeft, resolvedTop, tooltipPlacement, tooltipAlign };
}

export const StageSpatialActionPill: React.FC<StageSpatialActionPillProps> = ({
  target, structureLens, isLeftStructureHidden, isLeftDagFocusedOnNode, isReadOnly = false,
  stageWidth, stageHeight, onSelectAction, onClose,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const isRegion = target.targetType === "spatial_region";
  const showLinkDag = shouldShowLinkToDagAction({ structureLens, isLeftStructureHidden, isLeftDagFocusedOnNode });
  useSpatialPillHotkeys({ isReadOnly, isRegion, showLinkDag, onSelectAction, onClose });
  const { clampedLeft, resolvedTop, tooltipPlacement, tooltipAlign } = computePillLayout(target.rect, stageWidth, stageHeight);

  const commentTip = t("spatial.action.commentTooltip", "Ghim nhận xét & gửi cho Agent");
  const editTip = isRegion
    ? (target.coveredFfIds.length > 0 ? t("spatial.action.editRegionCountTooltip", "Mở khung CMS bên trái lọc theo {{count}} khối trong vùng chọn").replace("{{count}}", formatNumber(target.coveredFfIds.length)) : t("spatial.action.editRegionTooltip", "Mở khung CMS bên trái lọc theo các khối trong vùng chọn"))
    : t("spatial.action.editSingleTooltip", "Sửa trực tiếp ngay trên thành phần này");
  const inspectTip = isRegion ? t("spatial.action.inspectRegionDisabled", "Chỉ khả dụng khi chọn {{count}} thành phần đơn lẻ").replace("{{count}}", formatNumber(1)) : t("spatial.action.inspectTooltip", "Xem thông số & token CSS");
  const linkDagTip = t("spatial.action.linkDagTooltip", "Nhảy tới Node tương ứng trên sơ đồ Userflow DAG");
  const closeTip = t("common.close", "Đóng");

  return (
    <div className="ff-spatial-action-pill" data-testid="stage-spatial-action-pill" data-target-type={target.targetType} data-trigger-source={target.triggerSource} data-tooltip-placement={tooltipPlacement} data-tooltip-align={tooltipAlign} role="toolbar" aria-label={t("spatial.action.toolbarLabel", "Thanh thao tác nhanh theo ngữ cảnh")} style={{ left: `${clampedLeft}px`, top: `${resolvedTop}px` }} onMouseDown={(e) => e.stopPropagation()}>
      <button type="button" className="ff-spatial-pill-icon-btn" data-testid="spatial-action-comment" disabled={isReadOnly} aria-label={commentTip} data-tooltip={commentTip} onClick={() => onSelectAction("comment")}><BrandIcon name="message-square" size={ICON_PX} color="currentColor" /></button>
      <button type="button" className="ff-spatial-pill-icon-btn" data-testid="spatial-action-edit-draft" disabled={isReadOnly} aria-label={editTip} data-tooltip={editTip} onClick={() => onSelectAction("edit-draft")}><BrandIcon name="edit" size={ICON_PX} color="currentColor" /></button>
      <button type="button" className="ff-spatial-pill-icon-btn" data-testid="spatial-action-inspect" disabled={isRegion} aria-disabled={isRegion ? "true" : undefined} aria-label={inspectTip} data-tooltip={inspectTip} onClick={() => { if (!isRegion) onSelectAction("inspect"); }}><BrandIcon name="info" size={ICON_PX} color="currentColor" /></button>
      {showLinkDag && <button type="button" className="ff-spatial-pill-icon-btn" data-testid="spatial-action-link-dag" aria-label={linkDagTip} data-tooltip={linkDagTip} onClick={() => onSelectAction("link-dag")}><BrandIcon name="workflow" size={ICON_PX} color="currentColor" /></button>}
      <span className="ff-spatial-pill-divider" aria-hidden="true" />
      <button type="button" className="ff-spatial-pill-icon-btn ff-spatial-pill-close" data-testid="spatial-action-close" aria-label={closeTip} data-tooltip={closeTip} onClick={onClose}><BrandIcon name="close" size={11} color="currentColor" /></button>
    </div>
  );
};
