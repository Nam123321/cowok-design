// @story 14.2
// @implements AC-04 (DESIGN.md [5.10] Metric Line — DAU/WAU/MAU, Open/P0 Tickets & Active Crash Groups)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { AnalyticsFunnelData } from "../../lib/validations/cloud-admin";

export interface SuperAdminKpiStripProps {
  funnel: AnalyticsFunnelData | null;
  openTicketFallback: number;
  p0Fallback: number;
  crashFallback: number;
}

const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const KPI_LINE_HEIGHT = 1.2;
const ICON_SIZE_SM = 14;

const STRIP_STYLE: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(auto-fit, minmax(168px, 1fr))",
  gap: "12px",
  padding: "12px 16px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "10px",
};

const CELL_STYLE: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: "4px",
  padding: "8px 10px",
  backgroundColor: "var(--bg-subtle)",
  borderRadius: "8px",
  border: "1px solid var(--border-subtle)",
};

const LABEL_ROW_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "6px",
  fontSize: "12px",
  color: "var(--text-muted)",
  fontWeight: FONT_WEIGHT_MEDIUM,
};

const VALUE_STYLE: React.CSSProperties = {
  fontSize: "20px",
  fontWeight: FONT_WEIGHT_BOLD,
  color: "var(--text-primary)",
  lineHeight: KPI_LINE_HEIGHT,
};

interface KpiMetricItem {
  id: string;
  label: string;
  value: string;
  icon: string;
  accent: string;
  sublabel?: string;
}

function buildKpiItems(
  props: SuperAdminKpiStripProps,
  t: (k: string, d?: string) => string,
  formatNumber: (v: number) => string
): KpiMetricItem[] {
  const { funnel, openTicketFallback, p0Fallback, crashFallback } = props;
  const openTickets = funnel?.openTickets ?? openTicketFallback;
  const p0Tickets = funnel?.p0Tickets ?? p0Fallback;
  const crashes = funnel?.activeCrashGroups ?? crashFallback;
  return [
    { id: "dau", label: t("superadmin.kpi.dau", "DAU (24h)"), value: formatNumber(funnel?.dau ?? 0), icon: "activity", accent: "var(--action-primary)" },
    { id: "wau", label: t("superadmin.kpi.wau", "WAU (7d)"), value: formatNumber(funnel?.wau ?? 0), icon: "users", accent: "var(--state-info)" },
    { id: "mau", label: t("superadmin.kpi.mau", "MAU (30d)"), value: formatNumber(funnel?.mau ?? 0), icon: "globe", accent: "var(--state-success)" },
    {
      id: "open-tickets",
      label: t("superadmin.kpi.openTickets", "Open Helpdesk"),
      value: formatNumber(openTickets),
      icon: "message",
      accent: p0Tickets > 0 ? "var(--state-danger)" : "var(--state-warning)",
      sublabel: `${formatNumber(p0Tickets)} P0`,
    },
    {
      id: "crashes",
      label: t("superadmin.kpi.activeCrashes", "Active Crashes"),
      value: formatNumber(crashes),
      icon: "shield-alert",
      accent: crashes > 0 ? "var(--state-danger)" : "var(--state-success)",
    },
  ];
}

function KpiMetricCard({ item }: { item: KpiMetricItem }) {
  return (
    <div style={CELL_STYLE} data-testid={`kpi-card-${item.id}`}>
      <div style={LABEL_ROW_STYLE}>
        <BrandIcon name={item.icon} size={ICON_SIZE_SM} color={item.accent} />
        <span>{item.label}</span>
      </div>
      <div style={VALUE_STYLE}>{item.value}</div>
      {item.sublabel && (
        <span style={{ fontSize: "11px", color: item.accent, fontWeight: FONT_WEIGHT_SEMIBOLD }}>
          {item.sublabel}
        </span>
      )}
    </div>
  );
}

export const SuperAdminKpiStrip: React.FC<SuperAdminKpiStripProps> = (props) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const items = buildKpiItems(props, t, formatNumber);

  return (
    <section
      aria-label={t("superadmin.kpi.regionLabel", "Cloud Control Plane KPIs")}
      data-ff-id="superadmin-kpi-metric-line"
      data-testid="superadmin-kpi-metric-line"
      style={STRIP_STYLE}
    >
      {items.map((item) => (
        <KpiMetricCard key={item.id} item={item} />
      ))}
    </section>
  );
};
