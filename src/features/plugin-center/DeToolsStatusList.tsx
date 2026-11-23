// @story 08.5
// @implements AC-01 (7 Built-in DE Tools Status List <=25ms, $0 execution)
import React, { useState } from "react";
import { DeRuntimeApiError, runBuiltInDe } from "../../api/de-runtime";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import {
  type BuiltInDeTool,
  type DeRunReportDto,
  type DeToolResultDto,
} from "../../lib/validations/de-runtime";
import "../plugins/plugins.css";

const ICON_SIZE = 14;
const SLA_BUDGET_MS = 25;

const ALL_DE_TOOLS: ReadonlyArray<{ id: BuiltInDeTool; labelKey: string; fallback: string }> = [
  { id: "scan-ui", labelKey: "deRuntime.toolScanUi", fallback: "scan-ui (DOM/A11y Audit)" },
  { id: "intake", labelKey: "deRuntime.toolIntake", fallback: "intake (KIND_CAP Quotas)" },
  { id: "tokens", labelKey: "deRuntime.toolTokens", fallback: "tokens (OKLCH Auto-Contrast)" },
  { id: "nfa-packs", labelKey: "deRuntime.toolNfaPacks", fallback: "nfa-packs (Linear NFA <=25ms)" },
  { id: "archify", labelKey: "deRuntime.toolArchify", fallback: "archify (JSON-IR -> SVG)" },
  { id: "get_artifact", labelKey: "deRuntime.toolGetArtifact", fallback: "get_artifact (BFS + O_NOFOLLOW)" },
  { id: "notes-check", labelKey: "deRuntime.toolNotesCheck", fallback: "notes-check (Slide Pacing)" },
];

export interface DeToolsStatusListProps {
  projectId: string;
}

const DeToolRow: React.FC<{
  toolId: BuiltInDeTool;
  label: string;
  result?: DeToolResultDto;
}> = ({ toolId, label, result }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  let badgeClass = "plugin-eco-badge";
  let dotClass = "plugin-eco-status-dot";
  let badgeText = t("deRuntime.statusReady", "Sẵn sàng");
  if (result) {
    badgeClass = result.passed
      ? "plugin-eco-badge plugin-eco-badge-active"
      : "plugin-eco-badge plugin-eco-badge-danger";
    dotClass = result.passed
      ? "plugin-eco-status-dot plugin-eco-status-dot--active"
      : "plugin-eco-status-dot plugin-eco-status-dot--danger";
    badgeText = result.passed
      ? t("deRuntime.statusPass", "Đạt")
      : `${t("deRuntime.statusFail", "Lỗi")} (${formatNumber(result.findings.length)})`;
  }
  return (
    <div className="plugin-eco-chip-card" data-testid={`de-tool-row-${toolId}`}>
      <div className="plugin-eco-card-row">
        <span className="plugin-eco-slug">{label}</span>
        <span className={badgeClass} data-testid={`de-tool-status-${toolId}`}>
          <span className={dotClass} aria-hidden="true" />
          <span>{badgeText}</span>
        </span>
      </div>
      {result && (
        <div className="plugin-eco-meta">
          {formatNumber(result.durationMs)}ms ({formatNumber(result.durationMicros)}us) •{" "}
          {formatNumber(result.findings.length)} {t("deRuntime.findingsUnit", "cảnh báo")}
        </div>
      )}
    </div>
  );
};

const DeToolsHeader: React.FC<{ running: boolean; onRun: () => void }> = ({ running, onRun }) => {
  const { t } = useTranslation();
  const { formatCurrency, formatNumber } = useFormatter();
  return (
    <div className="plugin-eco-header">
      <div className="plugin-eco-title-group">
        <BrandIcon name="sparkles" size={ICON_SIZE} color="var(--action-primary)" />
        <h3 className="plugin-eco-title">
          {t("deRuntime.deToolsTitle", "7 Built-in Deterministic Engineering (DE) Tools")}
        </h3>
      </div>
      <div className="plugin-eco-actions">
        <span className="plugin-eco-badge plugin-eco-badge-de-sla" data-testid="de-sla-badge">
          {formatCurrency(0)} • &lt;={formatNumber(SLA_BUDGET_MS)}ms SLA
        </span>
        <button
          type="button"
          className="plugin-eco-btn plugin-eco-btn-primary"
          data-testid="btn-run-de-suite"
          disabled={running}
          onClick={onRun}
        >
          {running
            ? t("deRuntime.runningDeBtn", "Running DE Suite...")
            : t("deRuntime.runDeBtn", "Run 7 DE Tools")}
        </button>
      </div>
    </div>
  );
};

const DeToolsInputs: React.FC<{
  targetPath: string;
  sampleHtml: string;
  onTargetChange: (v: string) => void;
  onSampleChange: (v: string) => void;
}> = ({ targetPath, sampleHtml, onTargetChange, onSampleChange }) => {
  const { t } = useTranslation();
  return (
    <div className="plugin-eco-inline-bar">
      <input
        type="text"
        className="plugin-eco-input"
        data-testid="input-de-target-path"
        value={targetPath}
        onChange={(e) => onTargetChange(e.target.value)}
        placeholder={t("deRuntime.targetPathPlaceholder", "Target artifact path")}
      />
      <input
        type="text"
        className="plugin-eco-rationale-input"
        data-testid="input-de-sample-content"
        value={sampleHtml}
        onChange={(e) => onSampleChange(e.target.value)}
        placeholder={t("deRuntime.sampleContentPlaceholder", "Artifact content preview")}
      />
    </div>
  );
};

function useDeToolsController(projectId: string) {
  const [targetPath, setTargetPath] = useState("screens/home.tsx");
  const [sampleHtml, setSampleHtml] = useState(
    "<main><h1>Workspace</h1><p>Semantic tokens ready.</p></main>"
  );
  const [running, setRunning] = useState(false);
  const [report, setReport] = useState<DeRunReportDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  const runSuite = async () => {
    setRunning(true);
    setError(null);
    try {
      const res = await runBuiltInDe({
        projectId,
        tools: ALL_DE_TOOLS.map((tool) => tool.id),
        targetArtifactPath: targetPath.trim() || "screens/home.tsx",
        contentOverride: sampleHtml,
      });
      setReport(res.data);
    } catch (err) {
      setError(err instanceof DeRuntimeApiError ? `[${err.code}] ${err.message}` : String(err));
    } finally {
      setRunning(false);
    }
  };

  return { targetPath, sampleHtml, running, report, error, setTargetPath, setSampleHtml, runSuite };
}

export const DeToolsStatusList: React.FC<DeToolsStatusListProps> = ({ projectId }) => {
  const { t } = useTranslation();
  const { formatCurrency, formatNumber } = useFormatter();
  const ctrl = useDeToolsController(projectId);
  const resultMap = new Map(ctrl.report?.results.map((r) => [r.tool, r]) ?? []);

  return (
    <section
      className="plugin-eco-section"
      data-ff-id="de-tools-status-list"
      data-testid="de-tools-status-list"
    >
      <DeToolsHeader running={ctrl.running} onRun={() => void ctrl.runSuite()} />
      <DeToolsInputs
        targetPath={ctrl.targetPath}
        sampleHtml={ctrl.sampleHtml}
        onTargetChange={ctrl.setTargetPath}
        onSampleChange={ctrl.setSampleHtml}
      />
      {ctrl.error && (
        <div className="plugin-eco-error-box" role="alert" data-testid="de-tools-error">
          <span>{ctrl.error}</span>
        </div>
      )}
      {ctrl.report && (
        <div className="plugin-eco-meta" data-testid="de-suite-summary">
          {t("deRuntime.summaryTotalLabel", "Tổng thời gian")}: {formatNumber(ctrl.report.totalDurationMs)}ms •{" "}
          {t("deRuntime.summaryCostLabel", "Chi phí")}: {formatCurrency(ctrl.report.costUsd)} •{" "}
          {ctrl.report.passed
            ? t("deRuntime.summaryAllPassed", "Tất cả đạt")
            : t("deRuntime.summaryFindings", "Phát hiện cảnh báo")}
        </div>
      )}
      <div className="plugin-eco-chip-grid">
        {ALL_DE_TOOLS.map((tool) => (
          <DeToolRow
            key={tool.id}
            toolId={tool.id}
            label={t(tool.labelKey, tool.fallback)}
            result={resultMap.get(tool.id)}
          />
        ))}
      </div>
    </section>
  );
};
