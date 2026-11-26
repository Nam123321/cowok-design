// @story 13.1
// @implements AC-03, AC-04, AC-05 (AdvancedFilterBar [5.22] for 25-Case Sim-User Checklist)
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { MilestoneSummary } from "../../lib/validations/eval-certification";
import { getMilestoneDescription, getMilestoneLabel } from "./eval-copy";
import type { MilestoneFilterValue, StatusFilterValue } from "./useEvalCertification";

export interface MilestoneStatusPillsProps {
  milestones: MilestoneSummary[];
  activeMilestone: MilestoneFilterValue;
  onSelectMilestone: (value: MilestoneFilterValue) => void;
  locale?: string;
}

export interface SimCasesFilterBarProps {
  searchQuery: string;
  statusFilter: StatusFilterValue;
  securityOnly: boolean;
  totalMatching: number;
  totalCases: number;
  onSearchChange: (value: string) => void;
  onStatusChange: (value: StatusFilterValue) => void;
  onToggleSecurityOnly: () => void;
  locale?: string;
}

const ICON_SIZE_XS = 11;
const ICON_STROKE = 1.85;
const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;

const PILLS_SCROLL_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "6px",
  overflowX: "auto",
  paddingBottom: "2px",
};

const FILTER_ROW_STYLE: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  alignItems: "center",
  gap: "6px",
  padding: "8px",
  borderRadius: "var(--radius-sm)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-subtle)",
};

const INPUT_WRAP_STYLE: React.CSSProperties = {
  flex: "1 1 160px",
  display: "inline-flex",
  alignItems: "center",
  gap: "6px",
  height: "28px",
  padding: "0 8px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-surface)",
  color: "var(--text-secondary)",
};

const SEARCH_INPUT_STYLE: React.CSSProperties = {
  border: "none",
  outline: "none",
  backgroundColor: "transparent",
  color: "var(--text-primary)",
  fontSize: "11.5px",
  width: "100%",
  fontFamily: "var(--font-sans)",
};

const SELECT_WRAP_STYLE: React.CSSProperties = {
  position: "relative",
  display: "inline-flex",
  alignItems: "center",
};

const SELECT_STYLE: React.CSSProperties = {
  height: "28px",
  padding: "0 26px 0 9px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-surface)",
  color: "var(--text-primary)",
  fontSize: "11px",
  fontFamily: "var(--font-sans)",
  cursor: "pointer",
  appearance: "none",
  WebkitAppearance: "none",
};

const SELECT_CHEVRON_STYLE: React.CSSProperties = {
  position: "absolute",
  right: "8px",
  display: "inline-flex",
  alignItems: "center",
  pointerEvents: "none",
  color: "var(--text-muted)",
};

function buildPillStyle(active: boolean, failed: boolean): React.CSSProperties {
  return {
    display: "inline-flex",
    alignItems: "center",
    gap: "5px",
    height: "26px",
    padding: "0 9px",
    borderRadius: "var(--radius-pill)",
    border: active
      ? "1px solid var(--action-primary)"
      : failed
        ? "1px solid var(--state-danger)"
        : "1px solid var(--border-subtle)",
    backgroundColor: active
      ? "var(--bg-selected)"
      : failed
        ? "var(--state-danger-soft)"
        : "var(--bg-workspace)",
    color: active
      ? "var(--action-primary)"
      : failed
        ? "var(--state-danger)"
        : "var(--text-secondary)",
    fontSize: "11px",
    fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
    whiteSpace: "nowrap",
    cursor: "pointer",
    flexShrink: 0,
  };
}

export const MilestoneStatusPills: React.FC<MilestoneStatusPillsProps> = ({
  milestones,
  activeMilestone,
  onSelectMilestone,
  locale,
}) => {
  const { t, locale: activeLocale } = useTranslation(locale);
  const fmt = useFormatter({ locale: locale || activeLocale });
  const [showGuide, setShowGuide] = useState(false);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
        <div
          role="group"
          aria-label={t("evalCertification.milestonePillsAria", "Lọc theo cột mốc M1 đến M5")}
          data-testid="eval-milestone-pills"
          style={{ ...PILLS_SCROLL_STYLE, flex: 1, minWidth: 0 }}
        >
          <button
            type="button"
            data-testid="eval-milestone-pill-ALL"
            aria-pressed={activeMilestone === "ALL"}
            onClick={() => onSelectMilestone("ALL")}
            style={buildPillStyle(activeMilestone === "ALL", false)}
          >
            <span>{t("evalCertification.milestoneAll", "Tất cả")}</span>
          </button>
          {milestones.map((ms) => {
            const isActive = activeMilestone === ms.milestone;
            const isFailed = ms.status === "FAIL" || !ms.securityGatePassed;
            const title = `${ms.milestone} · ${getMilestoneLabel(ms.milestone, t)} — ${getMilestoneDescription(ms.milestone, t)}`;
            return (
              <button
                key={ms.milestone}
                type="button"
                data-testid={`eval-milestone-pill-${ms.milestone}`}
                aria-pressed={isActive}
                title={title}
                onClick={() => onSelectMilestone(ms.milestone)}
                style={buildPillStyle(isActive, isFailed)}
              >
                <BrandIcon
                  name={ms.securityGatePassed ? "shield-check" : "shield-alert"}
                  size={ICON_SIZE_XS}
                  strokeWidth={ICON_STROKE}
                  color={ms.securityGatePassed ? "var(--state-success)" : "var(--state-danger)"}
                />
                <span>{ms.milestone}</span>
                <span style={{ fontFamily: "var(--font-mono)", fontVariantNumeric: "tabular-nums" }}>
                  {`${fmt.formatNumber(ms.passed)}/${fmt.formatNumber(ms.total)}`}
                </span>
              </button>
            );
          })}
        </div>

        <button
          type="button"
          data-testid="eval-milestone-info-toggle"
          aria-expanded={showGuide}
          aria-controls="eval-milestone-guide-panel"
          aria-label={t("evalCertification.milestoneInfoBtn", "Giải thích ý nghĩa M1–M5")}
          title={t("evalCertification.milestoneInfoBtn", "Giải thích ý nghĩa M1–M5")}
          onClick={() => setShowGuide((prev) => !prev)}
          style={{
            width: "26px",
            height: "26px",
            borderRadius: "var(--radius-pill)",
            border: showGuide
              ? "1px solid var(--action-primary)"
              : "1px solid var(--border-subtle)",
            backgroundColor: showGuide ? "var(--bg-selected)" : "var(--bg-workspace)",
            color: showGuide ? "var(--action-primary)" : "var(--text-secondary)",
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            cursor: "pointer",
            flexShrink: 0,
          }}
        >
          <BrandIcon name="help-circle" size={13} strokeWidth={ICON_STROKE} color="currentColor" />
        </button>
      </div>

      {showGuide && (
        <section
          id="eval-milestone-guide-panel"
          data-testid="eval-milestone-guide"
          aria-label={t("evalCertification.milestoneGuideTitle", "Ý nghĩa 5 cột mốc chứng nhận (M1–M5)")}
          style={{
            padding: "9px 10px",
            borderRadius: "var(--radius-sm)",
            border: "1px solid var(--border-subtle)",
            backgroundColor: "var(--bg-subtle)",
            display: "flex",
            flexDirection: "column",
            gap: "6px",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
            <span style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
              {t("evalCertification.milestoneGuideTitle", "Ý nghĩa 5 cột mốc chứng nhận (M1–M5)")}
            </span>
            <button
              type="button"
              data-testid="eval-milestone-guide-close"
              aria-label={t("common.close", "Đóng")}
              onClick={() => setShowGuide(false)}
              style={{
                border: "none",
                backgroundColor: "transparent",
                color: "var(--text-muted)",
                cursor: "pointer",
                padding: "2px",
                display: "inline-flex",
                alignItems: "center",
              }}
            >
              <BrandIcon name="close" size={ICON_SIZE_XS} strokeWidth={ICON_STROKE} color="currentColor" />
            </button>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "5px" }}>
            {milestones.map((ms) => (
              <div
                key={`guide-${ms.milestone}`}
                style={{ display: "flex", alignItems: "flex-start", gap: "7px", fontSize: "11px", lineHeight: 1.4 }}
              >
                <span
                  style={{
                    fontFamily: "var(--font-mono)",
                    fontWeight: FONT_WEIGHT_SEMIBOLD,
                    fontSize: "10.5px",
                    color: "var(--action-primary)",
                    backgroundColor: "var(--bg-selected)",
                    padding: "1px 5px",
                    borderRadius: "var(--radius-xs)",
                    flexShrink: 0,
                  }}
                >
                  {ms.milestone}
                </span>
                <div style={{ color: "var(--text-secondary)", minWidth: 0 }}>
                  <strong style={{ color: "var(--text-primary)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
                    {getMilestoneLabel(ms.milestone, t)}:
                  </strong>{" "}
                  <span>{getMilestoneDescription(ms.milestone, t)}</span>
                </div>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
};

export const SimCasesFilterBar: React.FC<SimCasesFilterBarProps> = ({
  searchQuery,
  statusFilter,
  securityOnly,
  totalMatching,
  totalCases,
  onSearchChange,
  onStatusChange,
  onToggleSecurityOnly,
  locale,
}) => {
  const { t, locale: activeLocale } = useTranslation(locale);
  const fmt = useFormatter({ locale: locale || activeLocale });

  return (
    <div data-testid="eval-cases-filter-bar" style={FILTER_ROW_STYLE}>
      <div style={INPUT_WRAP_STYLE}>
        <BrandIcon name="search" size={ICON_SIZE_XS} strokeWidth={ICON_STROKE} color="var(--text-muted)" />
        <input
          type="search"
          data-testid="eval-case-search-input"
          aria-label={t("evalCertification.searchPlaceholder", "Tìm theo mã hoặc mô tả kịch bản...")}
          placeholder={t("evalCertification.searchPlaceholder", "Tìm theo mã hoặc mô tả kịch bản...")}
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          style={SEARCH_INPUT_STYLE}
        />
      </div>

      <div style={SELECT_WRAP_STYLE}>
        <select
          data-testid="eval-case-status-select"
          aria-label={t("evalCertification.statusFilterAria", "Lọc theo trạng thái kịch bản")}
          value={statusFilter}
          onChange={(e) => onStatusChange(e.target.value as StatusFilterValue)}
          style={SELECT_STYLE}
        >
          <option value="ALL">{t("evalCertification.statusAll", "Mọi trạng thái")}</option>
          <option value="PASS">{t("evalCertification.statusPass", "Đạt")}</option>
          <option value="FAIL">{t("evalCertification.statusFail", "Chưa đạt")}</option>
          <option value="BLOCKED">{t("evalCertification.statusBlocked", "Bị chặn")}</option>
          <option value="GAP">{t("evalCertification.statusGap", "Thiếu bước")}</option>
        </select>
        <span style={SELECT_CHEVRON_STYLE} aria-hidden="true">
          <BrandIcon name="chevron-down" size={ICON_SIZE_XS} strokeWidth={ICON_STROKE} color="currentColor" />
        </span>
      </div>

      <button
        type="button"
        data-testid="eval-security-only-toggle"
        aria-pressed={securityOnly}
        onClick={onToggleSecurityOnly}
        style={buildPillStyle(securityOnly, false)}
      >
        <BrandIcon name="filter" size={ICON_SIZE_XS} strokeWidth={ICON_STROKE} color="currentColor" />
        <span>{t("evalCertification.securityOnlyBtn", "Cổng bảo mật")}</span>
      </button>

      <span
        data-testid="eval-cases-count-label"
        style={{
          marginLeft: "auto",
          fontSize: "10.5px",
          fontFamily: "var(--font-mono)",
          color: "var(--text-muted)",
          fontVariantNumeric: "tabular-nums",
        }}
      >
        {t("evalCertification.matchingCount", "{{shown}}/{{total}}", {
          shown: fmt.formatNumber(totalMatching),
          total: fmt.formatNumber(totalCases),
        })}
      </span>
    </div>
  );
};
