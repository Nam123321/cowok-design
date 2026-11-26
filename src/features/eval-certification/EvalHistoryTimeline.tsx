// @story 13.1
// @implements AC-04 (Run Timeline [PAT-03] Immutable Certification Run Ledger)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { EvalBenchmarkRunDto } from "../../lib/validations/eval-certification";
import { formatEvalDuration, getTierLabel, getVerdictLabel } from "./eval-copy";

export interface EvalHistoryTimelineProps {
  runs: EvalBenchmarkRunDto[];
  selectedRunId: string | null;
  onSelectRun: (runId: string) => void;
  locale?: string;
}

const ICON_SIZE_XS = 12;
const ICON_STROKE = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;

export const EvalHistoryTimeline: React.FC<EvalHistoryTimelineProps> = ({
  runs,
  selectedRunId,
  onSelectRun,
  locale,
}) => {
  const { t, locale: activeLocale } = useTranslation(locale);
  const fmt = useFormatter({ locale: locale || activeLocale, currency: "USD" });

  if (runs.length === 0) {
    return (
      <div
        data-testid="eval-history-timeline"
        style={{
          padding: "20px 12px",
          textAlign: "center",
          borderRadius: "var(--radius-sm)",
          border: "1px dashed var(--border-default)",
          color: "var(--text-muted)",
          fontSize: "11.5px",
        }}
      >
        {t("evalCertification.historyEmpty", "Chưa có lịch sử phiên chứng nhận nào được lưu.")}
      </div>
    );
  }

  return (
    <div
      data-testid="eval-history-timeline"
      style={{ display: "flex", flexDirection: "column", gap: "8px" }}
    >
      {runs.map((run) => {
        const isSelected = run.runId === selectedRunId;
        const isPass = run.verdict === "PASS";
        const dateStr = `${fmt.formatDate(run.createdAt)} ${fmt.formatTime(run.createdAt)}`;
        return (
          <button
            key={run.runId}
            type="button"
            data-testid={`eval-history-item-${run.runId}`}
            aria-pressed={isSelected}
            onClick={() => onSelectRun(run.runId)}
            style={{
              textAlign: "left",
              display: "flex",
              flexDirection: "column",
              gap: "6px",
              padding: "10px 12px",
              borderRadius: "var(--radius-sm)",
              border: isSelected
                ? "1px solid var(--action-primary)"
                : "1px solid var(--border-subtle)",
              backgroundColor: isSelected ? "var(--bg-selected)" : "var(--bg-workspace)",
              cursor: "pointer",
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: "8px",
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <BrandIcon
                  name={isPass ? "check-circle" : "shield-alert"}
                  size={ICON_SIZE_XS}
                  strokeWidth={ICON_STROKE}
                  color={isPass ? "var(--state-success)" : "var(--state-danger)"}
                />
                <span style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
                  {getTierLabel(run.tier, t)}
                </span>
              </div>
              <span
                style={{
                  padding: "2px 7px",
                  borderRadius: "var(--radius-pill)",
                  backgroundColor: isPass ? "var(--state-success-soft)" : "var(--state-danger-soft)",
                  color: isPass ? "var(--state-success)" : "var(--state-danger)",
                  fontSize: "10px",
                  fontWeight: FONT_WEIGHT_SEMIBOLD,
                }}
              >
                {getVerdictLabel(run.verdict, t)}
              </span>
            </div>

            <div
              style={{
                display: "flex",
                flexWrap: "wrap",
                alignItems: "center",
                justifyContent: "space-between",
                gap: "8px",
                fontSize: "11px",
                color: "var(--text-secondary)",
                fontFamily: "var(--font-mono)",
                fontVariantNumeric: "tabular-nums",
              }}
            >
              <span>
                {`${fmt.formatNumber(run.simCasesPassed)}/${fmt.formatNumber(run.simCasesTotal)} · ${formatEvalDuration(run.durationMs, fmt.formatNumber, t)} · ${fmt.formatCurrency(run.estimatedCostUsd, "USD")}`}
              </span>
              <span style={{ color: "var(--text-muted)", fontFamily: "var(--font-sans)" }}>
                {dateStr}
              </span>
            </div>
          </button>
        );
      })}
    </div>
  );
};
