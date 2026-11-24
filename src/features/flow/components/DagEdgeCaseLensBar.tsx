// @story 05.5
// @implements AC-01, AC-02, AC-03 ([Show Edge-Case Flows] Lens Bar, <=8 Happy-Path Cluster Badge & Real 3-Tier Flow Synthesizer Trigger)
import React, { useEffect, useRef, useState } from "react";
import { apiFlowSynthesize } from "@/api/flow-synthesize";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import type {
  SynthesizedFlowPlan,
  SynthesizedFlowSummary,
} from "@/lib/validations/flow-synthesize";
import type { WorkbenchNode } from "../store/flow-workbench-store";

const MAX_DEFAULT_HAPPY_PATH_CLUSTERS = 8;
const ICON_SIZE_XS = 11;
const STROKE_WIDTH = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const LENS_BAR_Z_INDEX = 20;
const DISABLED_OPACITY = 0.65;

export interface DagEdgeCaseLensBarProps {
  projectId?: string;
  nodes: WorkbenchNode[];
  showEdgeCaseFlows: boolean;
  onToggleEdgeCaseFlows: (next: boolean) => void;
  primaryClusterCount: number;
  edgeCaseBranchCount: number;
  lastSummary: SynthesizedFlowSummary | null;
  onSynthesized: (plan: SynthesizedFlowPlan) => void;
  isReadOnly?: boolean;
}

function buildSynthesizePayload(projectId: string, nodes: WorkbenchNode[], fallbackBrief: string) {
  const screens = nodes
    .filter((n) => !n.screenKey.startsWith("fmea-"))
    .map((n) => ({
      screenKey: n.screenKey,
      title: n.title,
      routePath: n.routePath || "/",
    }));
  const briefSummary =
    screens.length > 0
      ? `3-tier flow synthesis for ${screens.map((s) => s.title).join(", ")}`
      : fallbackBrief;
  return {
    projectId,
    briefSummary,
    maxDefaultClusterNodes: MAX_DEFAULT_HAPPY_PATH_CLUSTERS,
    includeFmeaBranches: true,
    screens,
  };
}

const ClusterBadge: React.FC<{
  primaryCount: number;
  formatNumber: (n: number) => string;
  label: string;
}> = ({ primaryCount, formatNumber, label }) => {
  const ratio = `${formatNumber(primaryCount)}/${formatNumber(MAX_DEFAULT_HAPPY_PATH_CLUSTERS)}`;
  return (
    <span
      data-ff-id="dag-happy-path-cluster"
      data-testid="dag-happy-path-cluster"
      title={`${label}: ${ratio}`}
      aria-label={`${label}: ${ratio}`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "4px",
        height: "24px",
        fontSize: "11px",
        fontWeight: FONT_WEIGHT_SEMIBOLD,
        padding: "0 7px",
        borderRadius: "var(--radius-sm)",
        backgroundColor: "var(--bg-workspace)",
        color: "var(--text-primary)",
        border: "1px solid var(--border-default)",
        fontFamily: "var(--font-mono)",
        whiteSpace: "nowrap",
        flexShrink: 0,
      }}
    >
      <BrandIcon name="layers" size={ICON_SIZE_XS} strokeWidth={STROKE_WIDTH} color="currentColor" />
      <span>{ratio}</span>
    </span>
  );
};

const EdgeCaseLensToggle: React.FC<{
  checked: boolean;
  edgeCaseCount: number;
  label: string;
  title: string;
  formatNumber: (n: number) => string;
  onToggle: () => void;
}> = ({ checked, edgeCaseCount, label, title, formatNumber, onToggle }) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    title={title}
    data-ff-id="show-edge-case-flows-toggle"
    data-testid="show-edge-case-flows-toggle"
    onClick={onToggle}
    style={{
      display: "inline-flex",
      alignItems: "center",
      gap: "5px",
      height: "24px",
      fontSize: "11px",
      fontWeight: FONT_WEIGHT_SEMIBOLD,
      padding: "0 8px",
      borderRadius: "var(--radius-sm)",
      cursor: "pointer",
      border: checked ? "1.5px dashed var(--state-warning)" : "1px solid var(--border-default)",
      backgroundColor: checked ? "var(--state-warning-soft)" : "var(--bg-workspace)",
      color: checked ? "var(--state-warning)" : "var(--text-secondary)",
      fontFamily: "var(--font-sans)",
      whiteSpace: "nowrap",
      flexShrink: 0,
    }}
  >
    <BrandIcon name="git-branch" size={ICON_SIZE_XS} strokeWidth={STROKE_WIDTH} color="currentColor" />
    <span>{label}</span>
    <span
      data-testid="edge-case-branch-count"
      style={{
        fontSize: "10px",
        fontFamily: "var(--font-mono)",
        padding: "0px 4px",
        borderRadius: "var(--radius-xs)",
        backgroundColor: "var(--bg-surface)",
        color: "var(--text-primary)",
      }}
    >
      {formatNumber(edgeCaseCount)}
    </span>
  </button>
);

const SynthesizeActionButton: React.FC<{
  disabled: boolean;
  label: string;
  title: string;
  onClick: () => void;
}> = ({ disabled, label, title, onClick }) => (
  <button
    type="button"
    title={title}
    data-ff-id="synthesize-3tier-flow-btn"
    data-testid="synthesize-3tier-flow-btn"
    disabled={disabled}
    onClick={onClick}
    style={{
      display: "inline-flex",
      alignItems: "center",
      gap: "5px",
      height: "24px",
      fontSize: "11px",
      fontWeight: FONT_WEIGHT_SEMIBOLD,
      padding: "0 9px",
      borderRadius: "var(--radius-sm)",
      border: "1px solid var(--action-primary)",
      backgroundColor: "var(--action-primary)",
      color: "var(--action-primary-fg)",
      cursor: disabled ? "not-allowed" : "pointer",
      opacity: disabled ? DISABLED_OPACITY : 1,
      fontFamily: "var(--font-sans)",
      whiteSpace: "nowrap",
      flexShrink: 0,
    }}
  >
    <BrandIcon name="sparkles" size={ICON_SIZE_XS} strokeWidth={STROKE_WIDTH} color="currentColor" />
    <span>{label}</span>
  </button>
);

const VerdictShieldBadge: React.FC<{
  summary: SynthesizedFlowSummary;
  formatNumber: (n: number) => string;
  substatesLabel: string;
}> = ({ summary, formatNumber, substatesLabel }) => {
  const pct = `${formatNumber(summary.substateCoveragePercent)}%`;
  const fullTitle = `${summary.decisionKind} · ${pct} ${substatesLabel}`;
  return (
    <span
      data-testid="dag-verdict-shield-badge"
      title={fullTitle}
      aria-label={fullTitle}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "4px",
        height: "24px",
        fontSize: "10px",
        fontFamily: "var(--font-mono)",
        color: "var(--text-secondary)",
        padding: "0 6px",
        borderRadius: "var(--radius-xs)",
        backgroundColor: "var(--bg-workspace)",
        border: "1px solid var(--border-default)",
        whiteSpace: "nowrap",
        flexShrink: 0,
      }}
    >
      <BrandIcon name="shield" size={ICON_SIZE_XS} strokeWidth={STROKE_WIDTH} color="currentColor" />
      <span>{`${summary.decisionKind} · ${pct}`}</span>
    </span>
  );
};

const LENS_BAR_STYLE: React.CSSProperties = {
  position: "absolute",
  top: "12px",
  left: "12px",
  maxWidth: "calc(100% - 64px)",
  height: "34px",
  boxSizing: "border-box",
  zIndex: LENS_BAR_Z_INDEX,
  display: "inline-flex",
  flexDirection: "row",
  flexWrap: "nowrap",
  alignItems: "center",
  gap: "6px",
  padding: "4px 6px",
  overflowX: "auto",
  borderRadius: "var(--radius-md)",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  boxShadow: "var(--shadow-sm)",
  pointerEvents: "auto",
};

interface SynthesizeActionParams {
  projectId: string;
  nodes: WorkbenchNode[];
  isReadOnly: boolean;
  defaultBrief: string;
  genericError: string;
  onSynthesized: (plan: SynthesizedFlowPlan) => void;
}

interface SynthesizeGuardRefs {
  activeProjectRef: React.MutableRefObject<string>;
  reqSeqRef: React.MutableRefObject<number>;
  abortRef: React.MutableRefObject<AbortController | null>;
}

function isRequestStillCurrent(
  refs: SynthesizeGuardRefs,
  seq: number,
  targetProject: string,
  signal: AbortSignal
): boolean {
  return (
    seq === refs.reqSeqRef.current &&
    refs.activeProjectRef.current === targetProject &&
    !signal.aborted
  );
}

function useFlowSynthesizeAction(params: SynthesizeActionParams) {
  const [isSynthesizing, setIsSynthesizing] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const refs: SynthesizeGuardRefs = {
    activeProjectRef: useRef(params.projectId),
    reqSeqRef: useRef(0),
    abortRef: useRef<AbortController | null>(null),
  };

  useEffect(() => {
    refs.activeProjectRef.current = params.projectId;
    refs.reqSeqRef.current += 1;
    refs.abortRef.current?.abort();
    refs.abortRef.current = null;
    setIsSynthesizing(false);
    setErrorMessage(null);
    return () => {
      refs.abortRef.current?.abort();
      refs.abortRef.current = null;
    };
  }, [params.projectId]);

  const runSynthesize = async () => {
    if (!params.projectId || isSynthesizing || params.isReadOnly) return;
    const seq = ++refs.reqSeqRef.current;
    const targetProject = params.projectId;
    refs.abortRef.current?.abort();
    const controller = new AbortController();
    refs.abortRef.current = controller;
    setIsSynthesizing(true);
    setErrorMessage(null);
    try {
      const payload = buildSynthesizePayload(targetProject, params.nodes, params.defaultBrief);
      const response = await apiFlowSynthesize.synthesize(payload, controller.signal);
      if (isRequestStillCurrent(refs, seq, targetProject, controller.signal)) {
        params.onSynthesized(response.data);
      }
    } catch (err) {
      if (isRequestStillCurrent(refs, seq, targetProject, controller.signal)) {
        setErrorMessage(err instanceof Error && err.message ? err.message : params.genericError);
      }
    } finally {
      if (seq === refs.reqSeqRef.current && refs.activeProjectRef.current === targetProject) {
        setIsSynthesizing(false);
      }
    }
  };

  return { isSynthesizing, errorMessage, runSynthesize };
}

export const DagEdgeCaseLensBar: React.FC<DagEdgeCaseLensBarProps> = ({
  projectId,
  nodes,
  showEdgeCaseFlows,
  onToggleEdgeCaseFlows,
  primaryClusterCount,
  edgeCaseBranchCount,
  lastSummary,
  onSynthesized,
  isReadOnly = false,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const resolvedProjectId = projectId?.trim() || "";
  const { isSynthesizing, errorMessage, runSynthesize } = useFlowSynthesizeAction({
    projectId: resolvedProjectId,
    nodes,
    isReadOnly,
    defaultBrief: t("flowSynthesize.defaultBrief", "End-to-end 3-tier interactive product flow"),
    genericError: t("flowSynthesize.errorGeneric", "Không thể tổng hợp luồng 3 tầng"),
    onSynthesized,
  });
  const fullActionLabel = isSynthesizing
    ? t("flowSynthesize.synthesizing", "Đang tổng hợp...")
    : t("flowSynthesize.synthesizeAction", "Tổng hợp luồng 3 tầng");
  const shortActionLabel = isSynthesizing
    ? t("flowSynthesize.synthesizing", "Đang tổng hợp...")
    : t("flowSynthesize.synthesizeShort", "Tổng hợp 3 tầng");

  return (
    <div data-testid="dag-edge-case-lens-bar" onMouseDown={(e) => e.stopPropagation()} style={LENS_BAR_STYLE}>
      <ClusterBadge primaryCount={primaryClusterCount} formatNumber={formatNumber} label={t("flowSynthesize.happyPathClusterLabel", "Cụm Happy-Path")} />
      <EdgeCaseLensToggle checked={showEdgeCaseFlows} edgeCaseCount={edgeCaseBranchCount} label={t("flowSynthesize.edgeCaseShort", "Ngoại lệ")} title={t("flowSynthesize.showEdgeCaseFlows", "Show Edge-Case Flows")} formatNumber={formatNumber} onToggle={() => onToggleEdgeCaseFlows(!showEdgeCaseFlows)} />
      {!isReadOnly && <SynthesizeActionButton disabled={isSynthesizing || !resolvedProjectId} label={shortActionLabel} title={fullActionLabel} onClick={() => void runSynthesize()} />}
      {lastSummary && <VerdictShieldBadge summary={lastSummary} formatNumber={formatNumber} substatesLabel={t("flowSynthesize.substatesLabel", "4-SubState")} />}
      {errorMessage && (
        <span role="alert" data-testid="dag-synthesize-error" style={{ fontSize: "11px", color: "var(--state-danger)", fontFamily: "var(--font-sans)", whiteSpace: "nowrap", flexShrink: 0 }}>
          {errorMessage}
        </span>
      )}
    </div>
  );
};
