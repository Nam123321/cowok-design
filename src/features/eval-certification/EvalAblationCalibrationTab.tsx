// @story 13.1
// @implements AC-02, AC-04, AC-05 (Paired Ablation, design-craft SemVer Gate, 3-Provider Calibration & Fault Injection)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { EvalBenchmarkRunDto } from "../../lib/validations/eval-certification";
import { getProviderLabel } from "./eval-copy";

export interface EvalAblationCalibrationTabProps {
  run: EvalBenchmarkRunDto | null;
  locale?: string;
}

const ICON_SIZE_SM = 13;
const ICON_STROKE = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;

const CARD_STYLE: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "8px",
  padding: "10px 12px",
  borderRadius: "var(--radius-sm)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-workspace)",
};

const CARD_HEADER_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "8px",
};

const KV_GRID_STYLE: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
  gap: "8px",
};

const STAT_BOX_STYLE: React.CSSProperties = {
  padding: "7px 8px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-surface)",
  display: "flex",
  flexDirection: "column",
  gap: "2px",
};

function buildPassPillStyle(passed: boolean): React.CSSProperties {
  return {
    display: "inline-flex",
    alignItems: "center",
    gap: "4px",
    padding: "2px 8px",
    borderRadius: "var(--radius-pill)",
    backgroundColor: passed ? "var(--state-success-soft)" : "var(--state-danger-soft)",
    color: passed ? "var(--state-success)" : "var(--state-danger)",
    fontSize: "10.5px",
    fontWeight: FONT_WEIGHT_SEMIBOLD,
    whiteSpace: "nowrap",
  };
}

export const EvalAblationCalibrationTab: React.FC<EvalAblationCalibrationTabProps> = ({
  run,
  locale,
}) => {
  const { t, locale: activeLocale } = useTranslation(locale);
  const fmt = useFormatter({ locale: locale || activeLocale });

  if (!run) {
    return (
      <div
        data-testid="eval-ablation-tab-panel"
        style={{
          padding: "18px 12px",
          textAlign: "center",
          borderRadius: "var(--radius-sm)",
          border: "1px dashed var(--border-default)",
          color: "var(--text-muted)",
          fontSize: "11.5px",
        }}
      >
        {t("evalCertification.ablationEmpty", "Chạy kiểm định để xem đối chứng kỹ năng và hiệu chuẩn.")}
      </div>
    );
  }

  const { ablation, calibration, faultInjection } = run;
  const liftPrefix = ablation.liftPercent > 0 ? "+" : "";

  return (
    <div
      data-testid="eval-ablation-tab-panel"
      style={{ display: "flex", flexDirection: "column", gap: "10px" }}
    >
      <section data-testid="eval-ablation-card" style={CARD_STYLE}>
        <div style={CARD_HEADER_STYLE}>
          <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: FONT_WEIGHT_SEMIBOLD, fontSize: "12px", color: "var(--text-primary)" }}>
            <BrandIcon name="layers" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="var(--action-primary)" />
            <span>{t("evalCertification.ablationTitle", "Đối chứng Bắt cặp & Cổng Phát hành Kỹ năng")}</span>
          </div>
          <span style={buildPassPillStyle(ablation.designCraftSemverPassed)}>
            {ablation.designCraftSemverPassed
              ? t("evalCertification.semverGatePass", "Cổng SemVer Đạt ({{ver}})", { ver: ablation.designCraftVersion })
              : t("evalCertification.semverGateFail", "Cổng SemVer Chặn ({{ver}})", { ver: ablation.designCraftVersion })}
          </span>
        </div>

        <div style={KV_GRID_STYLE}>
          <div style={STAT_BOX_STYLE}>
            <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
              {t("evalCertification.ablationBriefs", "Kịch bản đối chứng")}
            </span>
            <span style={{ fontSize: "13px", fontWeight: FONT_WEIGHT_BOLD, fontFamily: "var(--font-mono)" }}>
              {t("evalCertification.ablationBriefsVal", "{{total}} ({{story}} kể chuyện dữ liệu)", {
                total: fmt.formatNumber(ablation.briefsEvaluated),
                story: fmt.formatNumber(ablation.dataStorytellingBriefsCount),
              })}
            </span>
          </div>
          <div style={STAT_BOX_STYLE}>
            <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
              {t("evalCertification.ablationLift", "Mức cải thiện thực tế")}
            </span>
            <span
              style={{
                fontSize: "13px",
                fontWeight: FONT_WEIGHT_BOLD,
                fontFamily: "var(--font-mono)",
                color: ablation.liftPercent > 0 ? "var(--state-success)" : "var(--state-danger)",
              }}
            >
              {`${liftPrefix}${fmt.formatNumber(ablation.liftPercent, 1)}%`}
            </span>
          </div>
          <div style={STAT_BOX_STYLE}>
            <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
              {t("evalCertification.ablationScores", "Điểm gốc → Có kỹ năng")}
            </span>
            <span style={{ fontSize: "12.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, fontFamily: "var(--font-mono)" }}>
              {`${fmt.formatNumber(ablation.baselineMeanScore, 1)} → ${fmt.formatNumber(ablation.withSkillMeanScore, 1)}`}
            </span>
          </div>
          <div style={STAT_BOX_STYLE}>
            <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
              {t("evalCertification.ablationRegressions", "Số ca hồi quy")}
            </span>
            <span
              style={{
                fontSize: "13px",
                fontWeight: FONT_WEIGHT_BOLD,
                fontFamily: "var(--font-mono)",
                color: ablation.regressionsCount === 0 ? "var(--state-success)" : "var(--state-danger)",
              }}
            >
              {fmt.formatNumber(ablation.regressionsCount)}
            </span>
          </div>
        </div>
      </section>

      <section data-testid="eval-calibration-card" style={CARD_STYLE}>
        <div style={CARD_HEADER_STYLE}>
          <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: FONT_WEIGHT_SEMIBOLD, fontSize: "12px", color: "var(--text-primary)" }}>
            <BrandIcon name="activity" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="var(--action-primary)" />
            <span>{t("evalCertification.calibrationTitle", "Hiệu chuẩn 3 Tầng Định tuyến Quyết định")}</span>
          </div>
          <span style={buildPassPillStyle(calibration.passed && calibration.totalFloatLeaks === 0)}>
            {t("evalCertification.floatLeakBadge", "{{count}} rò rỉ xác suất", {
              count: fmt.formatNumber(calibration.totalFloatLeaks),
            })}
          </span>
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
          {calibration.providers.map((prov) => (
            <div
              key={prov.providerId}
              data-testid={`eval-provider-row-${prov.providerId}`}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: "8px",
                padding: "7px 8px",
                borderRadius: "var(--radius-xs)",
                border: "1px solid var(--border-subtle)",
                backgroundColor: "var(--bg-surface)",
                fontSize: "11px",
              }}
            >
              <div style={{ display: "flex", flexDirection: "column", gap: "2px", minWidth: 0 }}>
                <span style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
                  {getProviderLabel(prov.providerId, t)}
                </span>
                <span style={{ fontSize: "10.5px", color: "var(--text-muted)" }}>
                  {t("evalCertification.providerCasesCount", "{{count}} tình huống kiểm thử", {
                    count: fmt.formatNumber(prov.casesEvaluated),
                  })}
                </span>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: "8px", flexShrink: 0 }}>
                <span style={{ fontFamily: "var(--font-mono)", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
                  {`${fmt.formatNumber(prov.accuracyPercent, 1)}%`}
                </span>
                <span style={buildPassPillStyle(prov.passed && prov.floatLeaksCount === 0)}>
                  {prov.passed
                    ? t("evalCertification.statusPass", "Đạt")
                    : t("evalCertification.statusFail", "Chưa đạt")}
                </span>
              </div>
            </div>
          ))}
        </div>
      </section>

      <section data-testid="eval-fault-injection-card" style={CARD_STYLE}>
        <div style={CARD_HEADER_STYLE}>
          <div style={{ display: "flex", alignItems: "center", gap: "6px", fontWeight: FONT_WEIGHT_SEMIBOLD, fontSize: "12px", color: "var(--text-primary)" }}>
            <BrandIcon name="shield-check" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="var(--state-success)" />
            <span>{t("evalCertification.faultTitle", "Tiêm lỗi & Giám khảo Độc lập")}</span>
          </div>
          <span style={buildPassPillStyle(faultInjection.passed)}>
            {`${fmt.formatNumber(faultInjection.catchRatePercent, 1)}%`}
          </span>
        </div>

        <div style={KV_GRID_STYLE}>
          <div style={STAT_BOX_STYLE}>
            <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
              {t("evalCertification.faultCaughtRatio", "Lỗi đã chặn / Tiêm vào")}
            </span>
            <span style={{ fontSize: "13px", fontWeight: FONT_WEIGHT_BOLD, fontFamily: "var(--font-mono)" }}>
              {`${fmt.formatNumber(faultInjection.caughtCount)}/${fmt.formatNumber(faultInjection.injectedCount)}`}
            </span>
          </div>
          <div style={STAT_BOX_STYLE}>
            <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>
              {t("evalCertification.faultIsolationLabel", "Tách biệt Tạo & Chấm điểm")}
            </span>
            <span
              style={{
                fontSize: "12px",
                fontWeight: FONT_WEIGHT_SEMIBOLD,
                color: faultInjection.builderScorerIsolated
                  ? "var(--state-success)"
                  : "var(--state-danger)",
              }}
            >
              {faultInjection.builderScorerIsolated
                ? t("evalCertification.faultIsolatedYes", "Độc lập hoàn toàn")
                : t("evalCertification.faultIsolatedNo", "Cảnh báo trùng vai trò")}
            </span>
          </div>
        </div>
      </section>
    </div>
  );
};
