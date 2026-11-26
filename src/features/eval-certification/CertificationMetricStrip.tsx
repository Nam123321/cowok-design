// @story 13.1
// @implements AC-02, AC-03, AC-04 (MetricLine [5.10] KPI Telemetry Strip for M1..M5 Certification)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { EvalBenchmarkRunDto } from "../../lib/validations/eval-certification";
import { formatEvalDuration } from "./eval-copy";

export interface CertificationMetricStripProps {
  run: EvalBenchmarkRunDto | null;
  locale?: string;
}

const MIN_SIM_PASS_THRESHOLD = 22;
const MIN_FAULT_CATCH_PERCENT = 95;
const ICON_SIZE_XS = 12;
const ICON_STROKE = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;

const STRIP_GRID_STYLE: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
  gap: "8px",
};

const CELL_STYLE: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "3px",
  padding: "8px 10px",
  borderRadius: "var(--radius-sm)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-workspace)",
  minWidth: 0,
};

const LABEL_ROW_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "6px",
  fontSize: "10.5px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  color: "var(--text-muted)",
};

const VALUE_STYLE: React.CSSProperties = {
  fontSize: "15px",
  fontWeight: FONT_WEIGHT_BOLD,
  fontFamily: "var(--font-mono)",
  fontVariantNumeric: "tabular-nums",
  lineHeight: 1.25,
};

const SUBTEXT_STYLE: React.CSSProperties = {
  fontSize: "10.5px",
  color: "var(--text-secondary)",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

interface MetricCardItem {
  id: string;
  label: string;
  value: string;
  subtext: string;
  icon: string;
  accentColor: string;
}

export const CertificationMetricStrip: React.FC<CertificationMetricStripProps> = ({
  run,
  locale,
}) => {
  const { t, locale: activeLocale } = useTranslation(locale);
  const fmt = useFormatter({ locale: locale || activeLocale, currency: "USD" });

  if (!run) {
    return (
      <div
        data-testid="eval-metric-strip"
        style={{
          padding: "10px 12px",
          borderRadius: "var(--radius-sm)",
          border: "1px dashed var(--border-default)",
          backgroundColor: "var(--bg-workspace)",
          color: "var(--text-muted)",
          fontSize: "11.5px",
        }}
      >
        {t("evalCertification.metricEmpty", "Chưa có lượt đánh giá nào. Nhấn Chạy kiểm định để bắt đầu.")}
      </div>
    );
  }

  const simOk =
    run.simCasesPassed >= MIN_SIM_PASS_THRESHOLD &&
    run.simCasesBlocked === 0 &&
    run.simCasesGap === 0 &&
    run.securityGatesPassed;
  const liftPositive = run.ablation.liftPercent > 0 && run.ablation.regressionsCount === 0;
  const faultOk =
    run.faultInjection.catchRatePercent >= MIN_FAULT_CATCH_PERCENT &&
    run.faultInjection.builderScorerIsolated;
  const liftSign = run.ablation.liftPercent > 0 ? "+" : "";

  const items: MetricCardItem[] = [
    {
      id: "sim",
      label: t("evalCertification.metricSimLabel", "Hành trình người dùng"),
      value: `${fmt.formatNumber(run.simCasesPassed)}/${fmt.formatNumber(run.simCasesTotal)}`,
      subtext: t("evalCertification.metricSimSub", "Mục tiêu >=22 · Chặn {{blocked}} · Thiếu {{gap}}", {
        blocked: fmt.formatNumber(run.simCasesBlocked),
        gap: fmt.formatNumber(run.simCasesGap),
      }),
      icon: simOk ? "check-circle" : "shield-alert",
      accentColor: simOk ? "var(--state-success)" : "var(--state-danger)",
    },
    {
      id: "ablation",
      label: t("evalCertification.metricAblationLabel", "Chênh lệch Đối chứng"),
      value: `${liftSign}${fmt.formatNumber(run.ablation.liftPercent, 1)}%`,
      subtext: t("evalCertification.metricAblationSub", "{{briefs}} kịch bản · {{reg}} hồi quy", {
        briefs: fmt.formatNumber(run.ablation.briefsEvaluated),
        reg: fmt.formatNumber(run.ablation.regressionsCount),
      }),
      icon: "layers",
      accentColor: liftPositive ? "var(--state-success)" : "var(--state-warning)",
    },
    {
      id: "fault",
      label: t("evalCertification.metricFaultLabel", "Bắt lỗi Độc lập"),
      value: `${fmt.formatNumber(run.faultInjection.catchRatePercent, 1)}%`,
      subtext: t("evalCertification.metricFaultSub", "Phát hiện {{caught}}/{{injected}} lỗi giả lập", {
        caught: fmt.formatNumber(run.faultInjection.caughtCount),
        injected: fmt.formatNumber(run.faultInjection.injectedCount),
      }),
      icon: "shield-check",
      accentColor: faultOk ? "var(--state-success)" : "var(--state-danger)",
    },
    {
      id: "sla",
      label: t("evalCertification.metricSlaLabel", "Thời gian & Chi phí"),
      value: `${formatEvalDuration(run.durationMs, fmt.formatNumber, t)} · ${fmt.formatCurrency(run.estimatedCostUsd, "USD")}`,
      subtext: run.offlineEnforced
        ? t("evalCertification.metricOfflineMode", "Chế độ ngoại tuyến hoàn toàn")
        : t("evalCertification.metricLocalMode", "Kiểm định cục bộ toàn diện"),
      icon: "clock",
      accentColor: "var(--action-primary)",
    },
  ];

  return (
    <section
      aria-label={t("evalCertification.metricStripAria", "Chỉ số tổng quan chứng nhận")}
      data-testid="eval-metric-strip"
      style={STRIP_GRID_STYLE}
    >
      {items.map((item) => (
        <div key={item.id} data-testid={`eval-metric-${item.id}`} style={CELL_STYLE}>
          <div style={LABEL_ROW_STYLE}>
            <span>{item.label}</span>
            <BrandIcon name={item.icon} size={ICON_SIZE_XS} strokeWidth={ICON_STROKE} color={item.accentColor} />
          </div>
          <div style={{ ...VALUE_STYLE, color: item.accentColor }}>{item.value}</div>
          <div style={SUBTEXT_STYLE} title={item.subtext}>
            {item.subtext}
          </div>
        </div>
      ))}
    </section>
  );
};
