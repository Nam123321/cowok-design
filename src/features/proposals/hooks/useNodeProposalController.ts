// @story 05.2
import { useCallback, useEffect, useState } from "react";
import { apiChangeProposal } from "../../../api/change-proposal";
import { useTranslation } from "../../../hooks/useTranslation";
import { HarnessDaemonApiError } from "../../../api/harness-daemon-transport";
import type { ChangeProposalResponse, DecideTradeoffInput } from "../../../lib/validations/change-proposal";
import type { ProposalGateErrorBanner } from "../components/ValidationGateBadge";
import { shouldSuppressProposalHotkey } from "../utils/diff-engine";

export interface NodeProposalControllerOptions {
  projectId?: string;
  selectedNodeId?: string | null;
  isModalOpen: boolean;
  isInlineCodeOpen: boolean;
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

  return { proposals, setProposals, refreshProposals };
}

function useDesignerTalkToggleHotkey(
  isModalOpen: boolean,
  isInlineCodeOpen: boolean,
  setIsDesignerTalkOpen: React.Dispatch<React.SetStateAction<boolean>>
) {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (shouldSuppressProposalHotkey(e, { isModalOpen, isInlineEditorOpen: isInlineCodeOpen })) return;
      if (e.key.toLowerCase() === "t") {
        e.preventDefault();
        setIsDesignerTalkOpen((prev) => !prev);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isInlineCodeOpen, isModalOpen, setIsDesignerTalkOpen]);
}

function useProposalGateActions(
  projectId: string | undefined,
  setProposals: React.Dispatch<React.SetStateAction<ChangeProposalResponse[]>>,
  setErrorBanner: React.Dispatch<React.SetStateAction<ProposalGateErrorBanner | null>>,
  setReceiptToast: React.Dispatch<React.SetStateAction<{ tone: "success" | "danger"; message: string } | null>>
) {
  const { t } = useTranslation();
  const [isRevalidating, setIsRevalidating] = useState(false);

  const applyProposal = useCallback(async (proposalId: string) => {
    if (!projectId) return;
    try {
      const updated = await apiChangeProposal.applyProposal(projectId, proposalId);
      setErrorBanner(null);
      setProposals((prev) => replaceProposalItem(prev, updated));
      setReceiptToast({ tone: "success", message: t("proposals.appliedToast", "ChangeProposal applied atomically.") });
    } catch (err) {
      if (err instanceof HarnessDaemonApiError) {
        setErrorBanner({ status: err.status, code: err.code, message: err.message });
        setReceiptToast({ tone: "danger", message: err.message });
      }
    }
  }, [projectId, setErrorBanner, setProposals, setReceiptToast, t]);

  const rejectProposal = useCallback(async (proposalId: string) => {
    if (!projectId) return;
    try {
      const updated = await apiChangeProposal.rejectProposal(projectId, proposalId);
      setProposals((prev) => replaceProposalItem(prev, updated));
      setReceiptToast({ tone: "success", message: t("proposals.rejectedToast", "ChangeProposal rejected.") });
    } catch (err) {
      if (err instanceof HarnessDaemonApiError) setReceiptToast({ tone: "danger", message: err.message });
    }
  }, [projectId, setProposals, setReceiptToast, t]);

  const rebaseAndValidate = useCallback(async (proposalId: string) => {
    if (!projectId) return;
    setIsRevalidating(true);
    try {
      const updated = await apiChangeProposal.validateProposal(projectId, proposalId, { rebaseBaseHashes: true });
      setErrorBanner(null);
      setProposals((prev) => replaceProposalItem(prev, updated));
    } finally {
      setIsRevalidating(false);
    }
  }, [projectId, setErrorBanner, setProposals]);

  return { isRevalidating, applyProposal, rejectProposal, rebaseAndValidate };
}

function useProposalDiffAndTradeoffs(
  projectId: string | undefined,
  activeProposal: ChangeProposalResponse | null,
  setProposals: React.Dispatch<React.SetStateAction<ChangeProposalResponse[]>>,
  refreshProposals: () => Promise<void>
) {
  const [isComputingDiff, setIsComputingDiff] = useState(false);

  const computeVisualDiff = useCallback(async (proposalId: string) => {
    if (!projectId) return;
    setIsComputingDiff(true);
    try {
      const visualDiff = await apiChangeProposal.computeVisualDiff(projectId, proposalId, {});
      setProposals((prev) => prev.map((item) => (item.id === proposalId ? { ...item, visualDiff } : item)));
    } finally {
      setIsComputingDiff(false);
    }
  }, [projectId, setProposals]);

  const decideTradeoff = useCallback(async (tradeoffId: string, input: DecideTradeoffInput) => {
    if (!projectId || !activeProposal) return;
    const updated = await apiChangeProposal.decideTradeoff(projectId, activeProposal.id, tradeoffId, input);
    setProposals((prev) =>
      prev.map((item) => item.id !== activeProposal.id ? item : { ...item, tradeoffs: (item.tradeoffs ?? []).map((tr) => (tr.id === updated.id ? updated : tr)) })
    );
    if (input.decision === "disagreed_instead") void refreshProposals();
  }, [activeProposal, projectId, refreshProposals, setProposals]);

  return { isComputingDiff, computeVisualDiff, decideTradeoff };
}

export function useNodeProposalController(opts: NodeProposalControllerOptions) {
  const { proposals, setProposals, refreshProposals } = useProposalFetchEffect(opts.projectId, opts.selectedNodeId);
  const [isDrawerOpen, setIsDrawerOpen] = useState(false);
  const [isDesignerTalkOpen, setIsDesignerTalkOpen] = useState(false);
  const [errorBanner, setErrorBanner] = useState<ProposalGateErrorBanner | null>(null);
  const [receiptToast, setReceiptToast] = useState<{ tone: "success" | "danger"; message: string } | null>(null);

  const activeProposal = proposals[0] ?? null;
  useDesignerTalkToggleHotkey(opts.isModalOpen, opts.isInlineCodeOpen, setIsDesignerTalkOpen);
  const gates = useProposalGateActions(opts.projectId, setProposals, setErrorBanner, setReceiptToast);
  const extras = useProposalDiffAndTradeoffs(opts.projectId, activeProposal, setProposals, refreshProposals);

  return {
    proposalsCount: proposals.length,
    activeProposal,
    isDrawerOpen,
    setIsDrawerOpen,
    isDesignerTalkOpen,
    setIsDesignerTalkOpen,
    errorBanner,
    receiptToast,
    dismissToast: () => setReceiptToast(null),
    ...gates,
    ...extras,
  };
}
