// @story 04.5, 05.1, 06.2
// @implements AC-05, AC-10, AC-07 (Stage Spatial Selection Outline + Action Pill + CommentPod + InlineEdit + Inspector)
import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type { WordingSlotDto } from "../../../lib/validations/presentation";
import type { SpatialTargetSelection } from "../utils/sandbox-bridge-protocol";
import { computeFloatingPodLayout } from "../utils/floating-pod-layout";
import { StageSpatialActionPill, type SpatialActionKind } from "./StageSpatialActionPill";
import { CommentPodPopover } from "../../comments/components/CommentPodPopover";
import { InlineDirectEditPopover } from "./InlineDirectEditPopover";
import { ElementPropertyInspectorPopover } from "./ElementPropertyInspectorPopover";

export { computeFloatingPodLayout };
export type ActiveSpatialTool = "pill" | "comment" | "edit-draft" | "inspect";

export interface StageSpatialOverlaysLayerProps {
  activeTarget: SpatialTargetSelection | null;
  activeTool: ActiveSpatialTool | null;
  structureLens: "userflow" | "slide-cms";
  isLeftStructureHidden: boolean;
  isLeftDagFocusedOnNode: boolean;
  isReadOnly?: boolean;
  stageWidth?: number;
  stageHeight?: number;
  cmsSlots?: WordingSlotDto[];
  cmsSyncState?: string;
  activeSlideHeadline?: string;
  activeSlideSubtitle?: string;
  isSubmittingComment?: boolean;
  onSelectAction: (action: SpatialActionKind) => void;
  onSubmitComment: (opts: { body: string; sendToAgent: boolean }) => Promise<void> | void;
  onInlineLiveChange: (nextValue: string, previousValue: string) => void;
  onInlineComposingChange?: (composing: boolean) => void;
  onOpenFullCmsDrawer?: () => void;
  onJumpToInlineHtml?: () => void;
  onClose: () => void;
}

function resolveInitialInlineValue(opts: {
  target: SpatialTargetSelection;
  cmsSlots?: WordingSlotDto[];
  activeSlideHeadline?: string;
  activeSlideSubtitle?: string;
}): { value: string; label?: string; slotKind?: string } {
  const { target, cmsSlots, activeSlideHeadline, activeSlideSubtitle } = opts;
  const matchedSlot = cmsSlots?.find((s) => s.ffId === target.elementFfId);
  if (matchedSlot) {
    return {
      value: matchedSlot.slotKind === "chart_json" ? (matchedSlot.chartsJson || matchedSlot.textValue) : matchedSlot.textValue,
      label: matchedSlot.slotLabel || matchedSlot.ffId,
      slotKind: matchedSlot.slotKind,
    };
  }
  if (target.elementFfId === "ff-slide-headline" || target.elementFfId.endsWith("-headline")) {
    return { value: activeSlideHeadline ?? target.metadata?.textContent ?? "", slotKind: "heading" };
  }
  if (target.elementFfId === "ff-slide-subtitle" || target.elementFfId.endsWith("-body")) {
    return { value: activeSlideSubtitle ?? target.metadata?.textContent ?? "", slotKind: "text" };
  }
  return { value: target.metadata?.textContent ?? "", slotKind: target.metadata?.slotKind };
}

function renderActiveSpatialTool(p: StageSpatialOverlaysLayerProps, activeTarget: SpatialTargetSelection, activeTool: ActiveSpatialTool, resolved: { value: string; label?: string; slotKind?: string }) {
  if (activeTool === "pill") {
    return <StageSpatialActionPill target={activeTarget} structureLens={p.structureLens} isLeftStructureHidden={p.isLeftStructureHidden} isLeftDagFocusedOnNode={p.isLeftDagFocusedOnNode} isReadOnly={p.isReadOnly} stageWidth={p.stageWidth} stageHeight={p.stageHeight} onSelectAction={p.onSelectAction} onClose={p.onClose} />;
  }
  if (activeTool === "comment") {
    return <CommentPodPopover target={activeTarget} stageWidth={p.stageWidth} stageHeight={p.stageHeight} isSubmitting={p.isSubmittingComment} onSubmitComment={p.onSubmitComment} onClose={p.onClose} />;
  }
  if (activeTool === "edit-draft" && activeTarget.targetType === "html_element") {
    return <InlineDirectEditPopover target={activeTarget} initialValue={resolved.value} slotLabel={resolved.label} stageWidth={p.stageWidth} stageHeight={p.stageHeight} isReadOnly={p.isReadOnly} onLiveChange={p.onInlineLiveChange} onComposingChange={p.onInlineComposingChange} onOpenFullCmsDrawer={p.onOpenFullCmsDrawer} onClose={p.onClose} />;
  }
  if (activeTool === "inspect" && activeTarget.targetType === "html_element") {
    return <ElementPropertyInspectorPopover target={activeTarget} slotKind={resolved.slotKind} slotSyncState={p.cmsSyncState} stageWidth={p.stageWidth} stageHeight={p.stageHeight} onJumpToInlineHtml={p.onJumpToInlineHtml} onClose={p.onClose} />;
  }
  return null;
}

export const StageSpatialOverlaysLayer: React.FC<StageSpatialOverlaysLayerProps> = (p) => {
  const { t } = useTranslation();
  if (!p.activeTarget || !p.activeTool) return null;
  const { activeTarget, activeTool } = p;
  const resolved = resolveInitialInlineValue({ target: activeTarget, cmsSlots: p.cmsSlots, activeSlideHeadline: p.activeSlideHeadline, activeSlideSubtitle: p.activeSlideSubtitle });
  const outlineW = Math.max(16, Math.min(activeTarget.rect.width, Math.max(16, (p.stageWidth ?? activeTarget.rect.width) - Math.max(0, activeTarget.rect.left) - 4)));
  return (
    <div className="ff-spatial-overlays-layer" data-testid="stage-spatial-overlays-layer" aria-label={t("spatial.overlaysRegionLabel", "Spatial overlays")}>
      <div className="ff-spatial-selection-outline" data-testid="spatial-selection-outline" data-target-type={activeTarget.targetType} style={{ left: `${Math.max(0, activeTarget.rect.left)}px`, top: `${Math.max(0, activeTarget.rect.top)}px`, width: `${outlineW}px`, height: `${Math.max(16, activeTarget.rect.height)}px` }} />
      {renderActiveSpatialTool(p, activeTarget, activeTool, resolved)}
    </div>
  );
};
