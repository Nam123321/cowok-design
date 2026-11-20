// @story 05.2
import { useCallback, useEffect, useState } from "react";
import { apiChangeProposal } from "../../../api/change-proposal";
import { apiElementComment } from "../../../api/element-comment";
import { useTranslation } from "../../../hooks/useTranslation";
import { HarnessDaemonApiError } from "../../../api/harness-daemon-transport";
import type { ChangeProposalResponse, DecideTradeoffInput } from "../../../lib/validations/change-proposal";
import type { ProposalGateErrorBanner } from "../components/ValidationGateBadge";
import { shouldSuppressProposalHotkey } from "../utils/diff-engine";
import {
  buildUiChangeProposalInput,
  notifyProposalsChanged,
  subscribeProposalsChanged,
} from "../utils/proposal-events";
import { rasterizeProposalVisualFrames } from "../utils/visual-rasterizer";

export interface NodeProposalControllerOptions {
  projectId?: string;
  selectedNodeId?: string | null;
  selectedNodeKey?: string | null;
  isModalOpen: boolean;
  isInlineCodeOpen: boolean;
  disabled?: boolean;
}

function replaceProposalItem(list: ChangeProposalResponse[], updated: ChangeProposalResponse): ChangeProposalResponse[] {
  return list.map((item) => (item.id === updated.id ? updated : item));
}

function useProposalFetchEffect(projectId: string | undefined, selectedNodeId: string | null | undefined) {
  const [proposals, setProposals] = useState<ChangeProposalResponse[]>([]);

  const refreshProposals = useCallback(async () => {
    if (!projectId) {
      setProposals([]);
      return;
    }
    try {
      const items = await apiChangeProposal.getProposals(projectId, selectedNodeId ?? undefined);
      setProposals(items);
    } catch {
      /* keep offline workbench responsive when daemon is absent */
    }
  }, [projectId, selectedNodeId]);

  useEffect(() => {
    void refreshProposals();
  }, [refreshProposals]);

  useEffect(() => {
    if (!projectId) return;
    return subscribeProposalsChanged((changedPid) => {
      if (changedPid === projectId) void refreshProposals();
    });
  }, [projectId, refreshProposals]);

  return { proposals, setProposals, refreshProposals };
}

function useDesignerTalkToggleHotkey(
  isModalOpen: boolean,
  isInlineCodeOpen: boolean,
  setIsDesignerTalkOpen: React.Dispatch<React.SetStateAction<boolean>>
) {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const domModal = typeof document !== "undefined" && Boolean(document.querySelector('[aria-modal="true"]'));
      const domInlineEditor = typeof document !== "undefined" && Boolean(document.querySelector('[data-inline-code-editor="true"], [data-testid="inline-code-editor-panel"]'));
      if (shouldSuppressProposalHotkey(e, { isModalOpen: isModalOpen || domModal, isInlineEditorOpen: isInlineCodeOpen || domInlineEditor })) return;
      if (e.key.toLowerCase() === "t") {
        e.preventDefault();
        setIsDesignerTalkOpen((prev) => !prev);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isInlineCodeOpen, isModalOpen, setIsDesignerTalkOpen]);
}

export interface ProposalReceiptToastState {
  tone: "success" | "danger";
  message: string;
  actionLabel?: string;
  onAction?: () => void;
}

interface ProposalGateActionsDeps {
  projectId: string | undefined;
  setProposals: React.Dispatch<React.SetStateAction<ChangeProposalResponse[]>>;
  setErrorBanner: React.Dispatch<React.SetStateAction<ProposalGateErrorBanner | null>>;
  setReceiptToast: React.Dispatch<React.SetStateAction<ProposalReceiptToastState | null>>;
  refreshProposals: () => Promise<void>;
}

function handleProposalError(
  err: unknown,
  setErrorBanner: ProposalGateActionsDeps["setErrorBanner"] | null,
  setReceiptToast: ProposalGateActionsDeps["setReceiptToast"]
) {
  if (err instanceof HarnessDaemonApiError) {
    setErrorBanner?.({ status: err.status, code: err.code, message: err.message });
    setReceiptToast({ tone: "danger", message: err.message });
    return;
  }
  if (err instanceof Error && err.message) {
    setReceiptToast({ tone: "danger", message: err.message });
  }
}

function useProposalGateActions(deps: ProposalGateActionsDeps) {
  const { projectId, setProposals, setErrorBanner, setReceiptToast, refreshProposals } = deps;
  const { t } = useTranslation();
  const [isRevalidating, setIsRevalidating] = useState(false);

  const applyProposal = useCallback(async (proposalId: string) => {
    if (!projectId) return;
    try {
      const updated = await apiChangeProposal.applyProposal(projectId, proposalId);
      setErrorBanner(null);
      setProposals((prev) => replaceProposalItem(prev, updated));
      void refreshProposals();
      const applyMsg = t("proposals.appliedToast", "ChangeProposal applied atomically.");
      const receiptLabel = t("proposals.turnReceiptPrefix", "Turn Snapshot Receipt");
      const linkedCommentId = updated.sourceCommentId;
      let undoCommentAction: Pick<ProposalReceiptToastState, "actionLabel" | "onAction"> = {};
      if (linkedCommentId) {
        const targetNodeId = updated.nodeId || updated.nodeIds?.[0];
        if (targetNodeId) {
          void apiElementComment.resolveComment(projectId, targetNodeId, linkedCommentId).catch(() => undefined);
          undoCommentAction = {
            actionLabel: t("proposals.reopenCommentUndo", "Undo"),
            onAction: () => {
              void apiElementComment
                .updateComment(projectId, targetNodeId, linkedCommentId, { anchorStatus: "active" })
                .then(() => {
                  window.dispatchEvent(new CustomEvent("cowok:comment-reopened", {
                    detail: { projectId, nodeId: targetNodeId, commentId: linkedCommentId },
                  }));
                  setReceiptToast(null);
                })
                .catch(() => undefined);
            },
          };
        }
        window.dispatchEvent(new CustomEvent("cowok:comment-resolved", {
          detail: { projectId, nodeId: targetNodeId ?? null, commentId: linkedCommentId },
        }));
      }
      setReceiptToast({ tone: "success", message: applyMsg, ...undoCommentAction });
      window.dispatchEvent(new CustomEvent("cowok:chat-receipt", {
        detail: { projectId, text: `${receiptLabel}: ${updated.title} — ${applyMsg}` },
      }));
    } catch (err) {
      handleProposalError(err, setErrorBanner, setReceiptToast);
    }
  }, [projectId, refreshProposals, setErrorBanner, setProposals, setReceiptToast, t]);

  const rejectProposal = useCallback(async (proposalId: string) => {
    if (!projectId) return;
    try {
      const updated = await apiChangeProposal.rejectProposal(projectId, proposalId);
      setProposals((prev) => replaceProposalItem(prev, updated));
      setReceiptToast({ tone: "success", message: t("proposals.rejectedToast", "ChangeProposal rejected.") });
    } catch (err) {
      handleProposalError(err, null, setReceiptToast);
    }
  }, [projectId, setProposals, setReceiptToast, t]);

  const rebaseAndValidate = useCallback(async (proposalId: string) => {
    if (!projectId) return;
    setIsRevalidating(true);
    try {
      const updated = await apiChangeProposal.validateProposal(projectId, proposalId, { rebaseBaseHashes: true });
      setErrorBanner(null);
      setProposals((prev) => replaceProposalItem(prev, updated));
    } catch (err) {
      handleProposalError(err, setErrorBanner, setReceiptToast);
    } finally {
      setIsRevalidating(false);
    }
  }, [projectId, setErrorBanner, setProposals, setReceiptToast]);

  return { isRevalidating, applyProposal, rejectProposal, rebaseAndValidate };
}

async function runVisualDiffComputation(deps: {
  projectId: string;
  proposalId: string;
  activeProposal: ChangeProposalResponse | null;
  setDiffFrameUrls: React.Dispatch<React.SetStateAction<Record<string, { beforeImageUrl?: string; afterImageUrl?: string }>>>;
  setProposals: React.Dispatch<React.SetStateAction<ChangeProposalResponse[]>>;
  setReceiptToast: ProposalGateActionsDeps["setReceiptToast"];
}) {
  const { projectId, proposalId, activeProposal, setDiffFrameUrls, setProposals, setReceiptToast } = deps;
  try {
    const target = activeProposal?.id === proposalId ? activeProposal : null;
    const frames = target ? rasterizeProposalVisualFrames(target) : { requestPayload: {} };
    if (frames.beforeImageUrl || frames.afterImageUrl) {
      setDiffFrameUrls((prev) => ({ ...prev, [proposalId]: { beforeImageUrl: frames.beforeImageUrl, afterImageUrl: frames.afterImageUrl } }));
    }
    const visualDiff = await apiChangeProposal.computeVisualDiff(projectId, proposalId, frames.requestPayload);
    setProposals((prev) => prev.map((item) => (item.id === proposalId ? { ...item, visualDiff } : item)));
  } catch (err) {
    handleProposalError(err, null, setReceiptToast);
  }
}

async function runTradeoffDecision(deps: {
  projectId: string;
  proposalId: string;
  tradeoffId: string;
  input: DecideTradeoffInput;
  setProposals: React.Dispatch<React.SetStateAction<ChangeProposalResponse[]>>;
  setReceiptToast: ProposalGateActionsDeps["setReceiptToast"];
  refreshProposals: () => Promise<void>;
}) {
  const { projectId, proposalId, tradeoffId, input, setProposals, setReceiptToast, refreshProposals } = deps;
  try {
    const updated = await apiChangeProposal.decideTradeoff(projectId, proposalId, tradeoffId, input);
    setProposals((prev) =>
      prev.map((item) => (item.id !== proposalId ? item : { ...item, tradeoffs: (item.tradeoffs ?? []).map((tr) => (tr.id === updated.id ? updated : tr)) }))
    );
    if (input.decision === "disagreed_instead") void refreshProposals();
  } catch (err) {
    handleProposalError(err, null, setReceiptToast);
  }
}

function useProposalDiffAndTradeoffs(deps: {
  projectId: string | undefined;
  activeProposal: ChangeProposalResponse | null;
  setProposals: React.Dispatch<React.SetStateAction<ChangeProposalResponse[]>>;
  setReceiptToast: ProposalGateActionsDeps["setReceiptToast"];
  refreshProposals: () => Promise<void>;
}) {
  const { projectId, activeProposal, setProposals, setReceiptToast, refreshProposals } = deps;
  const [isComputingDiff, setIsComputingDiff] = useState(false);
  const [diffFrameUrls, setDiffFrameUrls] = useState<Record<string, { beforeImageUrl?: string; afterImageUrl?: string }>>({});

  const computeVisualDiff = useCallback(async (proposalId: string) => {
    if (!projectId) return;
    setIsComputingDiff(true);
    await runVisualDiffComputation({ projectId, proposalId, activeProposal, setDiffFrameUrls, setProposals, setReceiptToast });
    setIsComputingDiff(false);
  }, [activeProposal, projectId, setProposals, setReceiptToast]);

  const decideTradeoff = useCallback(async (tradeoffId: string, input: DecideTradeoffInput) => {
    if (!projectId || !activeProposal) return;
    await runTradeoffDecision({ projectId, proposalId: activeProposal.id, tradeoffId, input, setProposals, setReceiptToast, refreshProposals });
  }, [activeProposal, projectId, refreshProposals, setProposals, setReceiptToast]);

  return { isComputingDiff, computeVisualDiff, decideTradeoff, activeDiffFrames: activeProposal ? diffFrameUrls[activeProposal.id] : undefined };
}

async function executeQuickDraftCreation(deps: {
  opts: NodeProposalControllerOptions;
  t: (key: string, fallback: string) => string;
  setSelectedProposalId: React.Dispatch<React.SetStateAction<string | null>>;
  setReceiptToast: React.Dispatch<React.SetStateAction<ProposalReceiptToastState | null>>;
}) {
  const { opts, t, setSelectedProposalId, setReceiptToast } = deps;
  if (!opts.projectId) return;
  try {
    const scopeText = opts.selectedNodeKey || opts.selectedNodeId || t("proposals.defaultScope", "Checkout Flow");
    const prompt = t("proposals.quickDraftPrompt", "Refine {{scope}} hierarchy & primary CTA contrast").replace("{{scope}}", scopeText);
    const created = await apiChangeProposal.createProposal(
      opts.projectId,
      buildUiChangeProposalInput({ prompt, nodeId: opts.selectedNodeId, screenKey: opts.selectedNodeKey })
    );
    setSelectedProposalId(created.id);
    notifyProposalsChanged(opts.projectId);
  } catch (err) {
    if (err instanceof Error && err.message) setReceiptToast({ tone: "danger", message: err.message });
  }
}

export function useNodeProposalController(opts: NodeProposalControllerOptions) {
  const { t } = useTranslation();
  const { proposals, setProposals, refreshProposals } = useProposalFetchEffect(
    opts.disabled ? undefined : opts.projectId,
    opts.selectedNodeId
  );
  const [selectedProposalId, setSelectedProposalId] = useState<string | null>(null);
  const [isDrawerOpen, setIsDrawerOpen] = useState(false);
  const [isDesignerTalkOpen, setIsDesignerTalkOpen] = useState(false);
  const [errorBanner, setErrorBanner] = useState<ProposalGateErrorBanner | null>(null);
  const [receiptToast, setReceiptToast] = useState<ProposalReceiptToastState | null>(null);

  const activeProposal = proposals.find((p) => p.id === selectedProposalId) ?? proposals[0] ?? null;
  useDesignerTalkToggleHotkey(Boolean(opts.isModalOpen || opts.disabled), opts.isInlineCodeOpen, setIsDesignerTalkOpen);
  const gates = useProposalGateActions({ projectId: opts.projectId, setProposals, setErrorBanner, setReceiptToast, refreshProposals });
  const extras = useProposalDiffAndTradeoffs({ projectId: opts.projectId, activeProposal, setProposals, setReceiptToast, refreshProposals });

  const createQuickDraftProposal = useCallback(
    () => executeQuickDraftCreation({
      opts: { projectId: opts.projectId, selectedNodeId: opts.selectedNodeId, selectedNodeKey: opts.selectedNodeKey, isModalOpen: false, isInlineCodeOpen: false },
      t, setSelectedProposalId, setReceiptToast,
    }),
    [opts.projectId, opts.selectedNodeId, opts.selectedNodeKey, t]
  );

  return {
    proposals, proposalsCount: proposals.length, activeProposal, setSelectedProposalId, createQuickDraftProposal,
    isDrawerOpen, setIsDrawerOpen, isDesignerTalkOpen, setIsDesignerTalkOpen, errorBanner, receiptToast,
    dismissToast: () => setReceiptToast(null), ...gates, ...extras,
  };
}
