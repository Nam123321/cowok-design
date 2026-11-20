// @story 04.5, 05.1, 06.2
// @implements AC-05, AC-10, AC-07 (Spatial Action Pill, CommentPod Structured Envelope & Dual-Mode Edit Draft Controller)
import { useCallback, useEffect, useState } from "react";
import { StructuredCommentEnvelopeSchema } from "../../../lib/validations/element-comment";
import { shouldShowLinkToDagAction, type SpatialActionKind } from "../components/StageSpatialActionPill";
import type { ActiveSpatialTool } from "../components/StageSpatialOverlaysLayer";
import type { SpatialTargetSelection } from "../utils/sandbox-bridge-protocol";
import type { usePreviewPaneActions } from "./usePreviewPaneActions";

export interface UseStageSpatialControllerParams {
  projectId?: string;
  selectedNodeId?: string | null;
  activeSlideId: string;
  structureLens: "userflow" | "slide-cms";
  pointerMode: "interact" | "click-dom" | "drag-box";
  isLeftStructureHidden: boolean;
  isLeftDagFocused?: boolean;
  isReadOnly?: boolean;
  artifactSha256: string;
  actions: ReturnType<typeof usePreviewPaneActions>;
  postPatchByFfId: (ffId: string, patch: { textContent?: string; slotKind?: string }) => void;
  onCmsInlineFieldEdit?: (ffId: string, nextValue: string, previousValue: string) => void;
  onOpenRegionCmsEdit?: (coveredFfIds: string[]) => void;
  onLinkToDag?: () => void;
  onHighlightRegion?: (selector: string) => void;
}

export function useStageSpatialController(p: UseStageSpatialControllerParams) {
  const [activeTarget, setActiveTarget] = useState<SpatialTargetSelection | null>(null);
  const [activeTool, setActiveTool] = useState<ActiveSpatialTool | null>(null);
  const [isSubmittingComment, setIsSubmittingComment] = useState(false);

  const isLeftDagFocusedOnNode = p.isLeftDagFocused ?? (p.structureLens === "userflow");

  const closeSpatialOverlay = useCallback(() => {
    setActiveTool(null);
    setActiveTarget(null);
  }, []);

  useEffect(() => {
    closeSpatialOverlay();
  }, [closeSpatialOverlay, p.activeSlideId, p.selectedNodeId, p.structureLens]);

  const executeActionForTarget = useCallback(
    (target: SpatialTargetSelection, action: SpatialActionKind) => {
      if (action === "comment") {
        if (p.isReadOnly) return;
        setActiveTarget(target);
        setActiveTool("comment");
        return;
      }
      if (action === "inspect") {
        if (target.targetType === "spatial_region") return;
        setActiveTarget(target);
        setActiveTool("inspect");
        return;
      }
      if (action === "edit-draft") {
        if (p.isReadOnly) return;
        if (target.targetType === "spatial_region") {
          p.onOpenRegionCmsEdit?.(target.coveredFfIds);
          closeSpatialOverlay();
          return;
        }
        setActiveTarget(target);
        setActiveTool("edit-draft");
        return;
      }
      if (action === "link-dag") {
        p.onLinkToDag?.();
        closeSpatialOverlay();
      }
    },
    [closeSpatialOverlay, p]
  );

  const handleSpatialTargetSelect = useCallback(
    (target: SpatialTargetSelection) => {
      // In Interact mode, ignore normal left-clicks so interactive HTML works freely; Right-Click ALWAYS opens the Pill!
      if (target.triggerSource === "select-click" && p.pointerMode === "interact") return;
      p.actions.setPrefillFfId(target.elementFfId);
      p.onHighlightRegion?.(target.selector);

      const mod = target.modifiers;
      if (mod?.altKey && mod?.shiftKey && target.targetType === "html_element") {
        executeActionForTarget(target, "inspect");
        return;
      }
      if (mod?.altKey && !p.isReadOnly) {
        executeActionForTarget(target, "comment");
        return;
      }
      if (mod?.shiftKey && !p.isReadOnly) {
        executeActionForTarget(target, "edit-draft");
        return;
      }
      if (
        (mod?.metaKey || mod?.ctrlKey) &&
        shouldShowLinkToDagAction({
          structureLens: p.structureLens,
          isLeftStructureHidden: p.isLeftStructureHidden,
          isLeftDagFocusedOnNode,
        })
      ) {
        executeActionForTarget(target, "link-dag");
        return;
      }
      setActiveTarget(target);
      setActiveTool("pill");
    },
    [executeActionForTarget, isLeftDagFocusedOnNode, p]
  );

  const handleSelectAction = useCallback(
    (action: SpatialActionKind) => {
      if (!activeTarget) return;
      executeActionForTarget(activeTarget, action);
    },
    [activeTarget, executeActionForTarget]
  );

  const handleSubmitComment = useCallback(
    async ({ body, sendToAgent }: { body: string; sendToAgent: boolean }) => {
      if (!activeTarget || p.isReadOnly) return;
      setIsSubmittingComment(true);
      try {
        const created = await p.actions.handleCreateSpatialComment(body, activeTarget.elementFfId, {
          anchorKind: activeTarget.targetType,
          domHash: activeTarget.domHash,
          regionRect: activeTarget.targetType === "spatial_region" ? activeTarget.rect : undefined,
          coveredFfIds: activeTarget.coveredFfIds,
        });
        if (created && sendToAgent && typeof window !== "undefined") {
          const envelope = StructuredCommentEnvelopeSchema.parse({
            commentId: created.id,
            projectId: p.projectId,
            nodeId: p.selectedNodeId || undefined,
            slideUid: p.structureLens === "slide-cms" ? p.activeSlideId : undefined,
            targetType: activeTarget.targetType,
            elementFfId: activeTarget.elementFfId,
            coveredFfIds: activeTarget.coveredFfIds,
            regionRect: activeTarget.targetType === "spatial_region" ? activeTarget.rect : undefined,
            domHash: created.domHash || activeTarget.domHash || "",
            artifactSha256: created.artifactSha256 || p.artifactSha256,
            commentBody: body,
            targetLabel: activeTarget.label,
          });
          window.dispatchEvent(new CustomEvent("cowok:structured-comment", { detail: envelope }));
        }
        closeSpatialOverlay();
      } finally {
        setIsSubmittingComment(false);
      }
    },
    [activeTarget, closeSpatialOverlay, p]
  );

  const handleInlineLiveChange = useCallback(
    (nextValue: string, previousValue: string) => {
      if (!activeTarget || p.isReadOnly) return;
      if (p.onCmsInlineFieldEdit) {
        p.onCmsInlineFieldEdit(activeTarget.elementFfId, nextValue, previousValue);
      } else {
        p.postPatchByFfId(activeTarget.elementFfId, { textContent: nextValue, slotKind: "text" });
      }
    },
    [activeTarget, p]
  );

  return {
    activeTarget,
    activeTool,
    isLeftDagFocusedOnNode,
    isSubmittingComment,
    handleSpatialTargetSelect,
    handleSelectAction,
    handleSubmitComment,
    handleInlineLiveChange,
    closeSpatialOverlay,
  };
}
