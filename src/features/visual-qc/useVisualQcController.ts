// @story 07.1
// @implements AC-01, AC-02, AC-03, AC-04
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { runVisualQc, runVisualQcAutoFix } from "../../api/visual-qc";
import { useTranslation } from "../../hooks/useTranslation";
import {
  type FlowAuditEdge,
  type FlowAuditNode,
  MAX_AUTOFIX_ATTEMPTS,
  type QcAutoFixRequest,
  type QcAutoFixResponse,
  type QcAutoFixResponseData,
  type QcIssue,
  type QcRunRequest,
  type QcRunResponse,
  type QcRunResponseData,
  type QcTargetKind,
} from "../../lib/validations/visual-qc";

const SLIDE_VIEWPORT_PX = 1920;
const DESKTOP_VIEWPORT_PX = 1440;
const LAPTOP_VIEWPORT_PX = 1024;
const TABLET_VIEWPORT_PX = 768;
const MOBILE_VIEWPORT_PX = 390;

export interface VisualQcPort {
  runVisualQc: (payload: QcRunRequest) => Promise<QcRunResponse>;
  runAutoFix: (payload: QcAutoFixRequest) => Promise<QcAutoFixResponse>;
}

const DEFAULT_QC_PORT: VisualQcPort = { runVisualQc, runAutoFix: runVisualQcAutoFix };

export interface UseVisualQcControllerParams {
  projectId: string;
  targetId: string;
  targetKind?: QcTargetKind;
  html: string;
  constraintsJson?: string;
  fixtureRef?: string;
  flowNodes?: FlowAuditNode[];
  flowEdges?: FlowAuditEdge[];
  onHtmlPatched?: (nextHtml: string) => void;
  port?: VisualQcPort;
}

export interface VisualQcControllerState {
  runResult: QcRunResponseData | null;
  lastFixResult: QcAutoFixResponseData | null;
  rollbackHtml: string | null;
  activeViewport: number;
  selectedIssueId: string | null;
  isRunning: boolean;
  isFixing: boolean;
  isOverlayVisible: boolean;
  isReadinessOpen: boolean;
  error: string | null;
  agentAttemptsUsed: number;
  isAgentFixLocked: boolean;
  deterministicIssues: QcIssue[];
  visibleViewportIssues: QcIssue[];
  setActiveViewport: (viewport: number) => void;
  setSelectedIssueId: (id: string | null) => void;
  setIsOverlayVisible: (open: boolean) => void;
  setIsReadinessOpen: (open: boolean) => void;
  runQc: (opts?: { forceRefresh?: boolean; forceNoBrowser?: boolean }) => Promise<QcRunResponseData | null>;
  runDeterministicFix: (issueIds?: string[]) => Promise<QcAutoFixResponseData | null>;
  runAgentFix: (issueIds?: string[]) => Promise<QcAutoFixResponseData | null>;
  rollbackLastFix: () => void;
}

function defaultViewportsForKind(kind: QcTargetKind): number[] {
  if (kind === "PRESENTATION_SLIDE") return [SLIDE_VIEWPORT_PX];
  return [DESKTOP_VIEWPORT_PX, LAPTOP_VIEWPORT_PX, TABLET_VIEWPORT_PX, MOBILE_VIEWPORT_PX];
}

function extractErrorMessage(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback;
}

function resolveFixTargetIds(
  runResult: QcRunResponseData,
  deterministicIssues: QcIssue[],
  mode: "DETERMINISTIC_LINT_FIX" | "AGENT_PATCH",
  explicitIds?: string[]
): string[] {
  if (explicitIds && explicitIds.length > 0) return explicitIds;
  const list = mode === "DETERMINISTIC_LINT_FIX" ? deterministicIssues : runResult.issues;
  return list.map((i) => i.id);
}

type TranslateFn = (key: string, fallback?: string) => string;

function isEditableEventTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toUpperCase();
  const ceAttr = target.getAttribute("contenteditable");
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    target.isContentEditable ||
    ceAttr === "true" ||
    ceAttr === ""
  );
}

function useUndoShortcut(rollbackHtml: string | null, rollbackLastFix: () => void): void {
  const latestRef = useRef({ rollbackHtml, rollbackLastFix });
  latestRef.current = { rollbackHtml, rollbackLastFix };

  useEffect(() => {
    if (typeof window === "undefined") return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (latestRef.current.rollbackHtml === null) return;
      if (isEditableEventTarget(e.target)) return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z" && !e.shiftKey) {
        e.preventDefault();
        latestRef.current.rollbackLastFix();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}

function useRunQcAction(args: {
  params: UseVisualQcControllerParams;
  targetKind: QcTargetKind;
  port: VisualQcPort;
  t: TranslateFn;
  setIsRunning: (v: boolean) => void;
  setError: (v: string | null) => void;
  setRunResult: (v: QcRunResponseData | null) => void;
  setAgentAttemptsUsed: (v: number) => void;
}) {
  return useCallback(
    async (opts?: { forceRefresh?: boolean; forceNoBrowser?: boolean }) => {
      args.setIsRunning(true);
      args.setError(null);
      try {
        const res = await args.port.runVisualQc({
          projectId: args.params.projectId,
          targetKind: args.targetKind,
          targetId: args.params.targetId,
          html: args.params.html,
          constraintsJson: args.params.constraintsJson ?? "{}",
          viewports: defaultViewportsForKind(args.targetKind),
          fixtureRef: args.params.fixtureRef ?? "default",
          flowNodes: args.params.flowNodes ?? [],
          flowEdges: args.params.flowEdges ?? [],
          forceRefresh: opts?.forceRefresh ?? false,
          forceNoBrowser: opts?.forceNoBrowser ?? false,
        });
        args.setRunResult(res.data);
        args.setAgentAttemptsUsed(res.data.autofixAttempt);
        return res.data;
      } catch (err) {
        args.setError(
          extractErrorMessage(err, args.t("visualQc.errRunFailed", "Visual QC audit failed"))
        );
        return null;
      } finally {
        args.setIsRunning(false);
      }
    },
    [args]
  );
}

interface QcAutoFixActionArgs {
  projectId: string;
  html: string;
  port: VisualQcPort;
  t: TranslateFn;
  runResult: QcRunResponseData | null;
  deterministicIssues: QcIssue[];
  agentAttemptsUsed: number;
  onHtmlPatched?: (nextHtml: string) => void;
  setIsFixing: (v: boolean) => void;
  setError: (v: string | null) => void;
  setLastFixResult: (v: QcAutoFixResponseData | null) => void;
  setRollbackHtml: (v: string | null) => void;
  setAgentAttemptsUsed: (v: number) => void;
}

function useQcAutoFixAction(args: QcAutoFixActionArgs) {
  return useCallback(
    async (mode: "DETERMINISTIC_LINT_FIX" | "AGENT_PATCH", issueIds?: string[]) => {
      if (!args.runResult) return null;
      const targets = resolveFixTargetIds(args.runResult, args.deterministicIssues, mode, issueIds);
      if (targets.length === 0) return null;
      const nextAttempt = mode === "AGENT_PATCH" ? args.agentAttemptsUsed + 1 : 1;
      if (mode === "AGENT_PATCH" && nextAttempt > MAX_AUTOFIX_ATTEMPTS) {
        args.setError(
          args.t(
            "visualQc.errAgentLocked",
            "Maximum 3 Auto-Fix attempts reached. Restore snapshot or edit manually."
          )
        );
        return null;
      }
      args.setIsFixing(true);
      args.setError(null);
      try {
        const res = await args.port.runAutoFix({
          projectId: args.projectId, qcRunId: args.runResult.qcRunId,
          issueIds: targets, mode, attemptNumber: nextAttempt, currentHtml: args.html,
        });
        args.setLastFixResult(res.data);
        args.setRollbackHtml(res.data.previousHtml);
        if (mode === "AGENT_PATCH") args.setAgentAttemptsUsed(res.data.attemptNumber);
        if (res.data.patchedHtml !== args.html) args.onHtmlPatched?.(res.data.patchedHtml);
        return res.data;
      } catch (err) {
        args.setError(extractErrorMessage(err, args.t("visualQc.errFixFailed", "Auto-Fix failed")));
        return null;
      } finally {
        args.setIsFixing(false);
      }
    },
    [args]
  );
}

export function useVisualQcController(params: UseVisualQcControllerParams): VisualQcControllerState {
  const { t } = useTranslation();
  const targetKind = params.targetKind ?? "APP_SCREEN";
  const port = params.port ?? DEFAULT_QC_PORT;
  const initialVp = targetKind === "PRESENTATION_SLIDE" ? SLIDE_VIEWPORT_PX : DESKTOP_VIEWPORT_PX;
  const [runResult, setRunResult] = useState<QcRunResponseData | null>(null);
  const [lastFixResult, setLastFixResult] = useState<QcAutoFixResponseData | null>(null);
  const [rollbackHtml, setRollbackHtml] = useState<string | null>(null);
  const [activeViewport, setActiveViewport] = useState(initialVp);
  const [selectedIssueId, setSelectedIssueId] = useState<string | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [isFixing, setIsFixing] = useState(false);
  const [isOverlayVisible, setIsOverlayVisible] = useState(true);
  const [isReadinessOpen, setIsReadinessOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [agentAttemptsUsed, setAgentAttemptsUsed] = useState(0);

  const deterministicIssues = useMemo(() => (runResult?.issues ?? []).filter((i) => i.deterministicFixable), [runResult]);
  const visibleViewportIssues = useMemo(
    () => (runResult?.issues ?? []).filter((i) => i.viewportWidth === activeViewport || i.category === "flow"),
    [runResult, activeViewport]
  );
  const runQc = useRunQcAction({ params, targetKind, port, t, setIsRunning, setError, setRunResult, setAgentAttemptsUsed });
  const executeFix = useQcAutoFixAction({
    projectId: params.projectId, html: params.html, port, t, runResult, deterministicIssues,
    agentAttemptsUsed, onHtmlPatched: params.onHtmlPatched, setIsFixing, setError,
    setLastFixResult, setRollbackHtml, setAgentAttemptsUsed,
  });
  const rollbackLastFix = useCallback(() => {
    if (rollbackHtml === null) return;
    params.onHtmlPatched?.(rollbackHtml);
    setRollbackHtml(null);
  }, [rollbackHtml, params]);

  useUndoShortcut(rollbackHtml, rollbackLastFix);
  return {
    runResult, lastFixResult, rollbackHtml, activeViewport, selectedIssueId, isRunning, isFixing,
    isOverlayVisible, isReadinessOpen, error, agentAttemptsUsed,
    isAgentFixLocked: agentAttemptsUsed >= MAX_AUTOFIX_ATTEMPTS,
    deterministicIssues, visibleViewportIssues, setActiveViewport, setSelectedIssueId,
    setIsOverlayVisible, setIsReadinessOpen, runQc, rollbackLastFix,
    runDeterministicFix: (ids) => executeFix("DETERMINISTIC_LINT_FIX", ids),
    runAgentFix: (ids) => executeFix("AGENT_PATCH", ids),
  };
}
