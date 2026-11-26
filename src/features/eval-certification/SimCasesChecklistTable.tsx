// @story 13.1
// @implements AC-03, AC-04, AC-05 (DataTableView [5.21] for UC-01..UC-25 Checklist Matrix)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { SimCaseResult, SimCaseStatus } from "../../lib/validations/eval-certification";
import {
  formatEvalDuration,
  getSecurityGateLabel,
  getSimCaseEvidenceSummary,
  getSimCaseStatusLabel,
  getSimCaseTitle,
} from "./eval-copy";

export interface SimCasesChecklistTableProps {
  cases: SimCaseResult[];
  locale?: string;
}

const ICON_SIZE_XS = 11;
const ICON_STROKE = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;

const TABLE_CONTAINER_STYLE: React.CSSProperties = {
  border: "1px solid var(--border-subtle)",
  borderRadius: "var(--radius-sm)",
  backgroundColor: "var(--bg-surface)",
  overflowX: "auto",
  maxHeight: "340px",
  overflowY: "auto",
};

const TABLE_STYLE: React.CSSProperties = {
  width: "100%",
  borderCollapse: "collapse",
  fontSize: "11.5px",
  fontFamily: "var(--font-sans)",
};

const TH_STYLE: React.CSSProperties = {
  position: "sticky",
  top: 0,
  zIndex: 2,
  textAlign: "left",
  padding: "7px 8px",
  fontSize: "10.5px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  color: "var(--text-muted)",
  backgroundColor: "var(--bg-elevated)",
  borderBottom: "1px solid var(--border-default)",
  whiteSpace: "nowrap",
};

const TD_STYLE: React.CSSProperties = {
  padding: "8px",
  borderBottom: "1px solid var(--border-subtle)",
  verticalAlign: "top",
  color: "var(--text-primary)",
};

function getStatusBadgeStyle(status: SimCaseStatus): React.CSSProperties {
  const palette =
    status === "PASS"
      ? { color: "var(--state-success)", bg: "var(--state-success-soft)" }
      : status === "GAP"
        ? { color: "var(--state-warning)", bg: "var(--state-warning-soft)" }
        : { color: "var(--state-danger)", bg: "var(--state-danger-soft)" };
  return {
    display: "inline-flex",
    alignItems: "center",
    gap: "4px",
    padding: "2px 7px",
    borderRadius: "var(--radius-pill)",
    backgroundColor: palette.bg,
    color: palette.color,
    fontSize: "10.5px",
    fontWeight: FONT_WEIGHT_SEMIBOLD,
    whiteSpace: "nowrap",
  };
}

export const SimCasesChecklistTable: React.FC<SimCasesChecklistTableProps> = ({
  cases,
  locale,
}) => {
  const { t, locale: activeLocale } = useTranslation(locale);
  const fmt = useFormatter({ locale: locale || activeLocale });

  if (cases.length === 0) {
    return (
      <div
        data-testid="eval-sim-cases-table"
        style={{
          padding: "20px 12px",
          textAlign: "center",
          borderRadius: "var(--radius-sm)",
          border: "1px dashed var(--border-default)",
          backgroundColor: "var(--bg-workspace)",
          color: "var(--text-muted)",
          fontSize: "11.5px",
        }}
      >
        {t("evalCertification.noMatchingCases", "Không có kịch bản nào khớp bộ lọc hiện tại.")}
      </div>
    );
  }

  return (
    <div data-testid="eval-sim-cases-table" style={TABLE_CONTAINER_STYLE}>
      <table style={TABLE_STYLE}>
        <thead>
          <tr>
            <th style={TH_STYLE}>{t("evalCertification.colCaseId", "Mã")}</th>
            <th style={TH_STYLE}>{t("evalCertification.colJourney", "Hành trình & Bằng chứng")}</th>
            <th style={TH_STYLE}>{t("evalCertification.colStatus", "Kết quả")}</th>
            <th style={{ ...TH_STYLE, textAlign: "right" }}>
              {t("evalCertification.colDuration", "Thời gian")}
            </th>
          </tr>
        </thead>
        <tbody>
          {cases.map((item) => {
            const title = getSimCaseTitle(item, t);
            return (
              <tr
                key={item.caseId}
                tabIndex={0}
                data-testid={`eval-case-row-${item.caseId}`}
                data-milestone={item.milestone}
                data-status={item.status}
              >
                <td style={{ ...TD_STYLE, whiteSpace: "nowrap" }}>
                  <div style={{ display: "flex", flexDirection: "column", gap: "3px" }}>
                    <span
                      style={{
                        fontFamily: "var(--font-mono)",
                        fontWeight: FONT_WEIGHT_SEMIBOLD,
                        fontSize: "11px",
                        color: "var(--text-primary)",
                      }}
                    >
                      {item.caseId}
                    </span>
                    <span
                      style={{
                        display: "inline-flex",
                        alignItems: "center",
                        width: "fit-content",
                        padding: "1px 5px",
                        borderRadius: "var(--radius-xs)",
                        backgroundColor: "var(--bg-subtle)",
                        color: "var(--text-secondary)",
                        fontSize: "10px",
                        fontFamily: "var(--font-mono)",
                      }}
                    >
                      {item.milestone}
                    </span>
                  </div>
                </td>
                <td style={TD_STYLE}>
                  <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
                    <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "5px" }}>
                      <span style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
                        {title}
                      </span>
                      {item.mandatorySecurityGate && (
                        <span
                          data-testid={`eval-case-security-pill-${item.caseId}`}
                          title={
                            item.securityGateKey
                              ? getSecurityGateLabel(item.securityGateKey, t)
                              : t("evalCertification.mandatorySecurityBadge", "Cổng bảo mật bắt buộc")
                          }
                          style={{
                            display: "inline-flex",
                            alignItems: "center",
                            gap: "3px",
                            padding: "1px 6px",
                            borderRadius: "var(--radius-pill)",
                            backgroundColor:
                              item.status === "PASS"
                                ? "var(--state-info-soft)"
                                : "var(--state-danger-soft)",
                            color:
                              item.status === "PASS"
                                ? "var(--state-info)"
                                : "var(--state-danger)",
                            fontSize: "10px",
                            fontWeight: FONT_WEIGHT_SEMIBOLD,
                          }}
                        >
                          <BrandIcon
                            name="shield-check"
                            size={ICON_SIZE_XS}
                            strokeWidth={ICON_STROKE}
                            color="currentColor"
                          />
                          <span>{t("evalCertification.mandatorySecurityShort", "Bảo mật bắt buộc")}</span>
                        </span>
                      )}
                    </div>
                    <span style={{ fontSize: "11px", color: "var(--text-secondary)", lineHeight: 1.35 }}>
                      {getSimCaseEvidenceSummary(item, t, fmt.formatNumber)}
                    </span>
                  </div>
                </td>
                <td style={{ ...TD_STYLE, whiteSpace: "nowrap" }}>
                  <span style={getStatusBadgeStyle(item.status)}>
                    <BrandIcon
                      name={item.status === "PASS" ? "check" : "shield-alert"}
                      size={ICON_SIZE_XS}
                      strokeWidth={ICON_STROKE}
                      color="currentColor"
                    />
                    <span>{getSimCaseStatusLabel(item.status, t)}</span>
                  </span>
                </td>
                <td
                  style={{
                    ...TD_STYLE,
                    textAlign: "right",
                    whiteSpace: "nowrap",
                    fontFamily: "var(--font-mono)",
                    fontVariantNumeric: "tabular-nums",
                    color: "var(--text-secondary)",
                    fontSize: "11px",
                  }}
                >
                  {formatEvalDuration(item.durationMs, fmt.formatNumber, t)}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};
