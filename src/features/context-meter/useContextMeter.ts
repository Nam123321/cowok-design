// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import { useCallback, useEffect, useRef, useState } from "react";
import { assembleContext, getContextTelemetry } from "../../api/context";
import type {
  ContextAssembleData,
  ContextTelemetryItem,
  DomOutlineNode,
} from "../../lib/validations/context";
import { CONTEXT_LIMITS, type ContextApiError } from "../../lib/validations/context-errors";
import type {
  ContextAdapterMode,
  ContextModelProfile,
  ContextReasonCode,
  PortalId,
  TaskHorizon,
} from "../../lib/validations/context-types";

const UUID_V4_LOOSE_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const DEFAULT_HISTORY_LIMIT = 10;
const MAX_DOM_NODES_CAPTURED = 48;
const MAX_DOM_SUMMARY_CHARS = 80;

export interface ContextMeterSnapshot {
  telemetryId: string;
  projectId: string;
  portalId: PortalId;
  horizon: TaskHorizon;
  modelProfile: ContextModelProfile;
  adapterMode: ContextAdapterMode;
  reasonCode: ContextReasonCode;
  staticPrefixSha256: string;
  outlineTokens: number;
  textTotalTokens: number;
  imageTokens: number;
  ssotDomRatio: number;
  latencyMs: number;
}

export interface ContextMeterState {
  status: "empty" | "loading" | "ready" | "error";
  portalId: PortalId;
  horizon: TaskHorizon;
  modelProfile: ContextModelProfile;
  snapshot: ContextMeterSnapshot | null;
  historyCount: number;
  error: ContextApiError["error"] | null;
}

function collectLiveDomOutlineNodes(): DomOutlineNode[] {
  if (typeof document === "undefined") return [];
  const cap = Math.min(MAX_DOM_NODES_CAPTURED, CONTEXT_LIMITS.MAX_DOM_OUTLINE_NODES);
  const elements = Array.from(document.querySelectorAll("[data-ff-id]"));
  const nodes: DomOutlineNode[] = [];
  for (const el of elements) {
    if (nodes.length >= cap) break;
    const ffId = (el.getAttribute("data-ff-id") || "").trim().slice(0, CONTEXT_LIMITS.MAX_DOM_FF_ID_LEN);
    const rawTag = (el.tagName ? el.tagName.toLowerCase() : "div").trim().slice(0, CONTEXT_LIMITS.MAX_DOM_TAG_LEN);
    if (!ffId || !rawTag) continue;
    const role = (el.getAttribute("role") || "").trim().slice(0, CONTEXT_LIMITS.MAX_DOM_ROLE_LEN) || undefined;
    const rawText = (el.getAttribute("aria-label") || el.textContent || "").trim().replace(/\s+/g, " ");
    const summary = rawText ? rawText.slice(0, MAX_DOM_SUMMARY_CHARS) : undefined;
    nodes.push({ ffId, tag: rawTag, ...(role ? { role } : {}), ...(summary ? { summary } : {}) });
  }
  return nodes;
}

function fromAssembleData(d: ContextAssembleData): ContextMeterSnapshot {
  return {
    telemetryId: d.telemetryId, projectId: d.projectId, portalId: d.portalId, horizon: d.horizon,
    modelProfile: d.modelProfile, adapterMode: d.adapterMode, reasonCode: d.reasonCode,
    staticPrefixSha256: d.staticPrefixSha256, outlineTokens: d.tokenUsage.outline,
    textTotalTokens: d.tokenUsage.textTotal, imageTokens: d.tokenUsage.image,
    ssotDomRatio: d.tokenUsage.ssotDomRatio, latencyMs: d.latencyMs,
  };
}

function fromTelemetryItem(head: ContextTelemetryItem): ContextMeterSnapshot {
  return {
    telemetryId: head.telemetryId, projectId: head.projectId, portalId: head.portalId, horizon: head.horizon,
    modelProfile: head.modelProfile, adapterMode: head.adapterMode, reasonCode: head.reasonCode,
    staticPrefixSha256: head.staticPrefixSha256, outlineTokens: head.tokenUsage.outline,
    textTotalTokens: head.tokenUsage.textTotal, imageTokens: head.tokenUsage.image,
    ssotDomRatio: head.tokenUsage.ssotDomRatio, latencyMs: head.latencyMs,
  };
}

async function fetchLatestHistorySnapshot(params: {
  projectId: string;
  portalId: PortalId;
  horizon: TaskHorizon;
  modelProfile: ContextModelProfile;
}): Promise<
  | { ok: true; count: number; snapshot: ContextMeterSnapshot | null }
  | { ok: false; error: ContextApiError["error"] }
> {
  const out = await getContextTelemetry({
    projectId: params.projectId, portalId: params.portalId,
    modelProfile: params.modelProfile, limit: DEFAULT_HISTORY_LIMIT,
  });
  if (!out.ok) return { ok: false, error: out.error };
  const items = out.data.items;
  const matching = items.find((item) => item.horizon === params.horizon);
  return { ok: true, count: items.length, snapshot: matching ? fromTelemetryItem(matching) : null };
}

async function executeContextAssemble(params: {
  projectId: string;
  portalId: PortalId;
  horizon: TaskHorizon;
  modelProfile: ContextModelProfile;
  intentText?: string;
}): Promise<
  | { ok: true; snapshot: ContextMeterSnapshot }
  | { ok: false; error: ContextApiError["error"] }
> {
  const domOutlineNodes = collectLiveDomOutlineNodes();
  const out = await assembleContext({
    projectId: params.projectId,
    portalId: params.portalId,
    horizon: params.horizon,
    modelProfile: params.modelProfile,
    intent: (params.intentText || `Assemble ${params.portalId} workspace context`).trim(),
    requestedImageTokens: 0,
    ...(domOutlineNodes.length > 0 ? { domOutlineNodes } : {}),
    inspectOnly: false,
  });
  return out.ok ? { ok: true, snapshot: fromAssembleData(out.data) } : { ok: false, error: out.error };
}

function isSnapshotInScope(
  snapshot: ContextMeterSnapshot | null,
  scope: { projectId?: string; portalId: PortalId; horizon: TaskHorizon; modelProfile: ContextModelProfile }
): snapshot is ContextMeterSnapshot {
  return Boolean(
    snapshot && scope.projectId && snapshot.projectId === scope.projectId &&
    snapshot.portalId === scope.portalId && snapshot.horizon === scope.horizon &&
    snapshot.modelProfile === scope.modelProfile
  );
}

function useScopedMeterSelections(clearSnapshotState: () => void) {
  const [portalId, setPortalIdState] = useState<PortalId>("DeveloperPortal");
  const [horizon, setHorizonState] = useState<TaskHorizon>("mid");
  const [modelProfile, setModelProfileState] = useState<ContextModelProfile>("A");
  const setPortalId = useCallback((nextPortal: PortalId) => {
    clearSnapshotState(); setPortalIdState(nextPortal);
  }, [clearSnapshotState]);
  const setHorizon = useCallback((nextHorizon: TaskHorizon) => {
    clearSnapshotState(); setHorizonState(nextHorizon);
  }, [clearSnapshotState]);
  const setModelProfile = useCallback((nextProfile: ContextModelProfile) => {
    clearSnapshotState(); setModelProfileState(nextProfile);
  }, [clearSnapshotState]);
  return { portalId, setPortalId, horizon, setHorizon, modelProfile, setModelProfile };
}

export function useContextMeter(projectId?: string) {
  const [status, setStatus] = useState<ContextMeterState["status"]>("empty");
  const [snapshot, setSnapshot] = useState<ContextMeterSnapshot | null>(null);
  const [historyCount, setHistoryCount] = useState<number>(0);
  const [error, setError] = useState<ContextApiError["error"] | null>(null);
  const requestSeqRef = useRef(0);
  const clearSnapshotState = useCallback(() => {
    setSnapshot(null); setHistoryCount(0); setError(null);
  }, []);
  const { portalId, setPortalId, horizon, setHorizon, modelProfile, setModelProfile } =
    useScopedMeterSelections(clearSnapshotState);
  const hasValidProject = Boolean(projectId && UUID_V4_LOOSE_RE.test(projectId));

  const loadHistory = useCallback(async () => {
    const seq = ++requestSeqRef.current;
    const scope = { projectId, portalId, horizon, modelProfile };
    setSnapshot((prev) => (isSnapshotInScope(prev, scope) ? prev : null));
    if (!hasValidProject || !projectId) {
      setStatus("empty"); return clearSnapshotState();
    }
    setStatus("loading"); setError(null);
    const res = await fetchLatestHistorySnapshot({ projectId, portalId, horizon, modelProfile });
    if (seq !== requestSeqRef.current) return;
    if (!res.ok) {
      clearSnapshotState(); setStatus("error"); return setError(res.error);
    }
    setHistoryCount(res.count); setSnapshot(res.snapshot); setStatus(res.snapshot ? "ready" : "empty");
  }, [clearSnapshotState, hasValidProject, horizon, modelProfile, portalId, projectId]);

  useEffect(() => { void loadHistory(); }, [loadHistory]);

  const assembleNow = useCallback(async (intentText?: string) => {
    const seq = ++requestSeqRef.current;
    if (!hasValidProject || !projectId) return setStatus("empty");
    setStatus("loading"); setError(null);
    const res = await executeContextAssemble({ projectId, portalId, horizon, modelProfile, intentText });
    if (seq !== requestSeqRef.current) return;
    if (!res.ok) { setStatus("error"); return setError(res.error); }
    setSnapshot(res.snapshot); setHistoryCount((c) => c + 1); setStatus("ready");
  }, [hasValidProject, horizon, modelProfile, portalId, projectId]);

  const scopedSnapshot = isSnapshotInScope(snapshot, { projectId, portalId, horizon, modelProfile })
    ? snapshot
    : null;
  return {
    status, portalId, setPortalId, horizon, setHorizon, modelProfile, setModelProfile,
    snapshot: scopedSnapshot, historyCount, error, hasValidProject, refreshTelemetry: loadHistory, assembleNow,
  };
}
