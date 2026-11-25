// @story 14.2
// @implements AC-04 (5-Step Activation Funnel Drop-Off, 4 Channels + 5 Plugin Hosts & Feature Adoption Table)
import React from "react";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  AnalyticsFunnelData,
  BreakdownMetric,
  FeatureAdoptionMetric,
  FunnelStepMetric,
} from "../../lib/validations/cloud-admin";

export interface ActivationFunnelTabProps {
  funnel: AnalyticsFunnelData | null;
}

const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const MIN_BAR_WIDTH_PCT = 4;
const ADOPTION_COL_SPAN = 3;

const PANEL_STYLE: React.CSSProperties = {
  padding: "16px",
  borderRadius: "10px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  display: "flex",
  flexDirection: "column",
  gap: "12px",
};

const TITLE_STYLE: React.CSSProperties = {
  margin: 0,
  fontSize: "14px",
  fontWeight: FONT_WEIGHT_BOLD,
  color: "var(--text-primary)",
};

const TH_STYLE: React.CSSProperties = {
  textAlign: "left",
  padding: "8px 10px",
  fontSize: "12px",
  color: "var(--text-muted)",
  borderBottom: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-subtle)",
};

const TD_STYLE: React.CSSProperties = {
  padding: "8px 10px",
  fontSize: "13px",
  color: "var(--text-primary)",
  borderBottom: "1px solid var(--border-subtle)",
};

function FunnelStepRow({ step, maxUsers }: { step: FunnelStepMetric; maxUsers: number }) {
  const { formatNumber } = useFormatter();
  const widthPct = maxUsers > 0 ? Math.max(MIN_BAR_WIDTH_PCT, Math.round((step.users / maxUsers) * 100)) : 0;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "4px" }} data-testid={`funnel-step-${step.stage}`}>
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: "12px" }}>
        <strong style={{ color: "var(--text-primary)" }}>{step.label}</strong>
        <span style={{ color: "var(--text-secondary)" }}>
          {formatNumber(step.users)} users · {formatNumber(step.conversionRatePct, 1)}% conv
          {step.dropOffCount > 0 && (
            <span style={{ color: "var(--state-danger)", marginLeft: "8px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
              (-{formatNumber(step.dropOffCount)} / -{formatNumber(step.dropOffRatePct, 1)}% drop-off)
            </span>
          )}
        </span>
      </div>
      <div style={{ height: "10px", borderRadius: "999px", backgroundColor: "var(--bg-subtle)", overflow: "hidden" }}>
        <div
          style={{
            height: "100%",
            width: `${widthPct}%`,
            backgroundColor: "var(--action-primary)",
            borderRadius: "999px",
          }}
        />
      </div>
    </div>
  );
}

function BreakdownGrid({ title, items, testId }: { title: string; items: BreakdownMetric[]; testId: string }) {
  const { formatNumber } = useFormatter();
  return (
    <section style={PANEL_STYLE} data-testid={testId}>
      <h3 style={TITLE_STYLE}>{title}</h3>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(120px, 1fr))", gap: "8px" }}>
        {items.map((item) => (
          <div
            key={item.key}
            style={{
              padding: "8px 10px",
              borderRadius: "8px",
              backgroundColor: "var(--bg-subtle)",
              border: "1px solid var(--border-subtle)",
            }}
          >
            <div style={{ fontSize: "12px", color: "var(--text-muted)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>{item.key}</div>
            <div style={{ fontSize: "16px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>
              {formatNumber(item.users)}
            </div>
            <div style={{ fontSize: "11px", color: "var(--text-secondary)" }}>
              {formatNumber(item.events)} events
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function FeatureAdoptionTable({ items }: { items: FeatureAdoptionMetric[] }) {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <section style={PANEL_STYLE} data-testid="superadmin-feature-adoption">
      <h3 style={TITLE_STYLE}>{t("superadmin.funnel.featureAdoption", "Feature Usage Frequency")}</h3>
      <table style={{ width: "100%", borderCollapse: "collapse" }}>
        <thead>
          <tr>
            <th style={TH_STYLE}>{t("superadmin.funnel.eventCol", "Feature / Event")}</th>
            <th style={TH_STYLE}>{t("superadmin.funnel.totalEventsCol", "Total Events")}</th>
            <th style={TH_STYLE}>{t("superadmin.funnel.uniqueInstallsCol", "Unique Installs")}</th>
          </tr>
        </thead>
        <tbody>
          {items.length === 0 ? (
            <tr>
              <td colSpan={ADOPTION_COL_SPAN} style={{ ...TD_STYLE, textAlign: "center", color: "var(--text-muted)" }}>
                {t("superadmin.funnel.noEvents", "No telemetry events ingested yet.")}
              </td>
            </tr>
          ) : (
            items.map((item) => (
              <tr key={item.eventName}>
                <td style={TD_STYLE}>{item.eventName}</td>
                <td style={TD_STYLE}>{formatNumber(item.totalEvents)}</td>
                <td style={TD_STYLE}>{formatNumber(item.uniqueInstalls)}</td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </section>
  );
}

export const ActivationFunnelTab: React.FC<ActivationFunnelTabProps> = ({ funnel }) => {
  const { t } = useTranslation();
  const steps = funnel?.funnelSteps ?? [];
  const maxUsers = steps.reduce((acc, s) => Math.max(acc, s.users), 0);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
      <section style={PANEL_STYLE} data-ff-id="superadmin-funnel-chart" data-testid="superadmin-funnel-chart">
        <h3 style={TITLE_STYLE}>
          {t("superadmin.funnel.title", "Activation Funnel (Day-0 -> AI Connected -> Project -> Preview -> Export)")}
        </h3>
        {steps.map((step) => (
          <FunnelStepRow key={step.stage} step={step} maxUsers={maxUsers} />
        ))}
      </section>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(300px, 1fr))", gap: "12px" }}>
        <BreakdownGrid
          title={t("superadmin.funnel.channelsTitle", "4 Distribution Channels")}
          items={funnel?.channelBreakdown ?? []}
          testId="superadmin-channel-breakdown"
        />
        <BreakdownGrid
          title={t("superadmin.funnel.hostsTitle", "5 Plugin Hosts")}
          items={funnel?.hostBreakdown ?? []}
          testId="superadmin-host-breakdown"
        />
      </div>
      <FeatureAdoptionTable items={funnel?.featureAdoption ?? []} />
    </div>
  );
};
