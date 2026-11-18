// @story 05.2
import React from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import { ChangeProposalDrawer } from "./ChangeProposalDrawer";
import type { useNodeProposalController } from "../hooks/useNodeProposalController";

export const PROPOSAL_DRAWER_WIDTH_PX = 420;

export const CanvasProposalToolbar: React.FC<{
  selectedNodeId?: string | null;
  selectedNodeKey?: string | null;
  proposalsCount: number;
  isDesignerTalkOpen: boolean;
  isDrawerOpen: boolean;
  onToggleDesignerTalk: () => void;
  onToggleDrawer: () => void;
}> = ({ selectedNodeId, selectedNodeKey, proposalsCount, isDesignerTalkOpen, isDrawerOpen, onToggleDesignerTalk, onToggleDrawer }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const readableScope = selectedNodeKey || selectedNodeId;

  return (
    <div className="cp-canvas-toolbar" data-testid="proposal-canvas-toolbar">
      <div className="cp-canvas-toolbar-group">
        <span className="cp-scope-pill" data-testid="canvas-active-scope-pill">
          {readableScope ? `${t("proposals.scopeNode", "NODE")}: ${readableScope}` : t("proposals.scopeBatch", "BATCH")}
        </span>
      </div>
      <div className="cp-canvas-toolbar-group">
        <button
          type="button"
          className={`cp-toolbar-btn${isDesignerTalkOpen ? " cp-toolbar-btn--active" : ""}`}
          data-testid="toggle-designer-talk-btn"
          aria-pressed={isDesignerTalkOpen}
          onClick={onToggleDesignerTalk}
        >
          <span>{t("proposals.designerTalkBtn", "Designer Talk")}</span>
          <span className="cp-toolbar-kbd" aria-hidden="true">{"T"}</span>
        </button>
        <button
          type="button"
          className={`cp-toolbar-btn${isDrawerOpen ? " cp-toolbar-btn--active" : ""}`}
          data-testid="toggle-change-proposal-drawer-btn"
          aria-pressed={isDrawerOpen}
          onClick={onToggleDrawer}
        >
          <span>{t("proposals.drawerToggleBtn", "Proposals")} ({formatNumber(proposalsCount)})</span>
        </button>
      </div>
    </div>
  );
};

const NOOP_PROPOSAL_MUTATION = () => {};

export const CanvasProposalDrawerHost: React.FC<{
  controller: ReturnType<typeof useNodeProposalController>;
  nodeLabelsById?: Record<string, string>;
  onOpenInlineEditor?: () => void;
  isReadOnly?: boolean;
}> = ({ controller, nodeLabelsById, onOpenInlineEditor, isReadOnly = false }) => (
  <ChangeProposalDrawer
    proposal={controller.activeProposal}
    proposals={controller.proposals}
    nodeLabelsById={nodeLabelsById}
    beforeImageUrl={controller.activeDiffFrames?.beforeImageUrl}
    afterImageUrl={controller.activeDiffFrames?.afterImageUrl}
    onSelectProposal={controller.setSelectedProposalId}
    onCreateQuickProposal={isReadOnly ? NOOP_PROPOSAL_MUTATION : () => { void controller.createQuickDraftProposal(); }}
    onOpenInlineEditor={isReadOnly ? undefined : onOpenInlineEditor}
    isOpen={controller.isDrawerOpen}
    onClose={() => controller.setIsDrawerOpen(false)}
    onApply={isReadOnly ? NOOP_PROPOSAL_MUTATION : (id) => { void controller.applyProposal(id); }}
    onReject={isReadOnly ? NOOP_PROPOSAL_MUTATION : (id) => { void controller.rejectProposal(id); }}
    onRebaseAndValidate={isReadOnly ? NOOP_PROPOSAL_MUTATION : (id) => { void controller.rebaseAndValidate(id); }}
    onComputeVisualDiff={isReadOnly ? NOOP_PROPOSAL_MUTATION : (id) => { void controller.computeVisualDiff(id); }}
    errorBanner={controller.errorBanner}
    receiptToast={controller.receiptToast}
    onDismissToast={controller.dismissToast}
    isRevalidating={controller.isRevalidating}
    isComputingDiff={controller.isComputingDiff}
  />
);
