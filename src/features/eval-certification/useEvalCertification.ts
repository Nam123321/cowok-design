// @story 13.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Workbench M1..M5 Certification Controller Hook)
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  apiEvalCertification,
  EvalCertificationApiError,
  type EvalCertificationApiClient,
} from "../../api/eval-certification";
import type { CowokToastTone } from "../../components/ui/CowokToast";
import type {
  EvalBenchmarkRunDto,
  EvalHistoryData,
  EvalMilestoneId,
  EvalTier,
  SimCaseResult,
  SimCaseStatus,
} from "../../lib/validations/eval-certification";
import {
  formatEvalErrorMessage,
  getMilestoneLabel,
  getSecurityGateLabel,
  getSimCaseTitle,
  getTierLabel,
  type TranslateFn,
} from "./eval-copy";

export type EvalDrawerTab = "cases" | "ablation" | "history";
export type MilestoneFilterValue = "ALL" | EvalMilestoneId;
export type StatusFilterValue = "ALL" | SimCaseStatus;

export interface EvalToastState {
  tone: CowokToastTone;
  message: string;
}

export interface UseEvalCertificationOptions {
  isOpen: boolean;
  projectId?: string;
  t: TranslateFn;
  client?: Pick<EvalCertificationApiClient, "runEval" | "getEvalHistory">;
}

const DEFAULT_HISTORY_LIMIT = 20;

interface SimCaseFilterArgs {
  item: SimCaseResult;
  milestoneFilter: MilestoneFilterValue;
  statusFilter: StatusFilterValue;
  securityOnly: boolean;
  normalizedQuery: string;
  t: TranslateFn;
}

function matchesSimCaseFilters(args: SimCaseFilterArgs): boolean {
  const { item, milestoneFilter, statusFilter, securityOnly, normalizedQuery, t } = args;
  if (milestoneFilter !== "ALL" && item.milestone !== milestoneFilter) return false;
  if (statusFilter !== "ALL" && item.status !== statusFilter) return false;
  if (securityOnly && !item.mandatorySecurityGate) return false;
  if (!normalizedQuery) return true;
  const gateLabel = item.securityGateKey ? getSecurityGateLabel(item.securityGateKey, t) : "";
  const hay = `${item.caseId} ${item.milestone} ${getSimCaseTitle(item, t)} ${getMilestoneLabel(item.milestone, t)} ${gateLabel}`.toLowerCase();
  return hay.includes(normalizedQuery);
}

export function useEvalCertification(opts: UseEvalCertificationOptions) {
  const { isOpen, projectId, t, client = apiEvalCertification } = opts;
  const [history, setHistory] = useState<EvalHistoryData | null>(null);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [selectedTier, setSelectedTier] = useState<EvalTier>("pr");
  const [activeTab, setActiveTab] = useState<EvalDrawerTab>("cases");
  const [milestoneFilter, setMilestoneFilter] = useState<MilestoneFilterValue>("ALL");
  const [statusFilter, setStatusFilter] = useState<StatusFilterValue>("ALL");
  const [securityOnly, setSecurityOnly] = useState<boolean>(false);
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [isLoadingHistory, setIsLoadingHistory] = useState<boolean>(false);
  const [isRunning, setIsRunning] = useState<boolean>(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [conflictActive, setConflictActive] = useState<boolean>(false);
  const [tamperOverride, setTamperOverride] = useState<{ active: boolean; reason: string | null }>({
    active: false,
    reason: null,
  });
  const [toast, setToast] = useState<EvalToastState | null>(null);
  const loadSeqRef = useRef(0);
  const isRunningRef = useRef(false);
  const tRef = useRef<TranslateFn>(t);
  tRef.current = t;

  const loadHistory = useCallback(
    async (options?: { preserveTamper?: boolean; selectLatest?: boolean }) => {
      const seq = loadSeqRef.current + 1;
      loadSeqRef.current = seq;
      setIsLoadingHistory(true);
      setLoadError(null);
      try {
        const resp = await client.getEvalHistory({ limit: DEFAULT_HISTORY_LIMIT });
        if (loadSeqRef.current !== seq) return;
        setHistory(resp.data);
        setConflictActive(false);
        if (!options?.preserveTamper || !resp.data.sealedIntegrityOk) {
          setTamperOverride({
            active: !resp.data.sealedIntegrityOk,
            reason: resp.data.tamperReason,
          });
        }
        if (resp.data.latestRun) {
          const latestId = resp.data.latestRun.runId;
          setSelectedRunId((prev) => {
            if (options?.selectLatest) return latestId;
            if (prev && resp.data.runs.some((r) => r.runId === prev)) return prev;
            return latestId;
          });
        }
      } catch (err) {
        if (loadSeqRef.current === seq) {
          setLoadError(formatEvalErrorMessage(err, tRef.current));
        }
      } finally {
        if (loadSeqRef.current === seq) {
          setIsLoadingHistory(false);
        }
      }
    },
    [client]
  );

  useEffect(() => {
    if (isOpen) {
      void loadHistory();
    } else {
      setToast(null);
    }
  }, [isOpen, loadHistory]);

  const handleRunEval = useCallback(
    async (tierOverride?: EvalTier) => {
      if (isRunningRef.current) return;
      isRunningRef.current = true;
      loadSeqRef.current += 1;
      setIsLoadingHistory(false);
      const tierToRun = tierOverride ?? selectedTier;
      setIsRunning(true);
      setConflictActive(false);
      try {
        const resp = await client.runEval({
          tier: tierToRun,
          projectId,
          runSimulatedUsers: true,
        });
        const newRun = resp.data;
        const tr = tRef.current;
        setHistory((prev) => {
          const prevRuns = prev?.runs ?? [];
          const deduped = [newRun, ...prevRuns.filter((r) => r.runId !== newRun.runId)];
          return {
            sealedIntegrityOk: !newRun.tamperDetected,
            sealedManifestSha256: newRun.sealedManifestSha256,
            manifestPinSource: newRun.manifestPinSource,
            tamperReason: newRun.tamperReason,
            latestRun: newRun,
            runs: deduped,
            totalRuns: Math.max(deduped.length, (prev?.totalRuns ?? 0) + 1),
          };
        });
        setSelectedRunId(newRun.runId);
        setTamperOverride({ active: newRun.tamperDetected, reason: newRun.tamperReason });
        const tone: CowokToastTone =
          newRun.verdict === "PASS" ? "success" : "danger";
        const message =
          newRun.verdict === "PASS"
            ? tr("evalCertification.runSuccessToast", "Đã hoàn tất {{tier}} — Đạt chứng nhận.", {
                tier: getTierLabel(tierToRun, tr),
              })
            : tr(
                "evalCertification.runFailedToast",
                "Đã hoàn tất {{tier}} — Có tiêu chí chưa đạt chuẩn.",
                { tier: getTierLabel(tierToRun, tr) }
              );
        setToast({ tone, message });
      } catch (err) {
        const tr = tRef.current;
        if (err instanceof EvalCertificationApiError && err.isAlreadyRunning) {
          setConflictActive(true);
          setToast({ tone: "warning", message: formatEvalErrorMessage(err, tr) });
        } else if (err instanceof EvalCertificationApiError && err.isTamperError) {
          const rawReason =
            typeof err.details?.tamperReason === "string" ? err.details.tamperReason : err.message;
          setTamperOverride({ active: true, reason: rawReason });
          setToast({ tone: "danger", message: formatEvalErrorMessage(err, tr) });
          void loadHistory({ preserveTamper: true, selectLatest: true });
        } else {
          setToast({ tone: "danger", message: formatEvalErrorMessage(err, tr) });
        }
      } finally {
        isRunningRef.current = false;
        setIsRunning(false);
      }
    },
    [client, loadHistory, projectId, selectedTier]
  );

  const selectedRun: EvalBenchmarkRunDto | null = useMemo(() => {
    if (!history) return null;
    if (selectedRunId) {
      const found = history.runs.find((r) => r.runId === selectedRunId);
      if (found) return found;
    }
    return history.latestRun ?? history.runs[0] ?? null;
  }, [history, selectedRunId]);

  const filteredSimCases = useMemo(() => {
    if (!selectedRun) return [];
    const normalizedQuery = searchQuery.trim().toLowerCase();
    return selectedRun.simCases.filter((item) =>
      matchesSimCaseFilters({
        item,
        milestoneFilter,
        statusFilter,
        securityOnly,
        normalizedQuery,
        t,
      })
    );
  }, [milestoneFilter, searchQuery, securityOnly, selectedRun, statusFilter, t]);

  const selectMilestonePill = useCallback((milestone: MilestoneFilterValue) => {
    setMilestoneFilter(milestone);
    setActiveTab("cases");
  }, []);

  const selectHistoryRun = useCallback((runId: string) => {
    setSelectedRunId(runId);
    setActiveTab("cases");
  }, []);

  const liveTamperActive = Boolean(
    tamperOverride.active || history?.sealedIntegrityOk === false
  );
  const selectedRunTampered = Boolean(
    selectedRun?.tamperDetected || selectedRun?.verdict === "SEALED_EVAL_TAMPER_ERROR"
  );
  const tamperActive = liveTamperActive || selectedRunTampered;
  const tamperReason = liveTamperActive
    ? (tamperOverride.reason ?? history?.tamperReason ?? selectedRun?.tamperReason ?? null)
    : (selectedRun?.tamperReason ?? null);

  return {
    history,
    selectedRun,
    selectedRunId,
    selectedTier,
    setSelectedTier,
    activeTab,
    setActiveTab,
    milestoneFilter,
    setMilestoneFilter,
    selectMilestonePill,
    statusFilter,
    setStatusFilter,
    securityOnly,
    setSecurityOnly,
    searchQuery,
    setSearchQuery,
    filteredSimCases,
    isLoadingHistory,
    isRunning,
    loadError,
    conflictActive,
    liveTamperActive,
    selectedRunTampered,
    tamperActive,
    tamperReason,
    toast,
    dismissToast: () => setToast(null),
    loadHistory,
    handleRunEval,
    selectHistoryRun,
  };
}
