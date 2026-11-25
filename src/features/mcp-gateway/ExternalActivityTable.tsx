// @story 11.2
// @implements AC-04 (Spacious Full-Page Live External MCP Activity Audit Log Table with Localized Labels & Semantic Tokens)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { McpActivityStatus, McpExternalActivityDto } from "../../lib/validations/mcp-gateway";

const HEADER_ICON_SIZE = 14;
const SMALL_ICON_SIZE = 11;
const FONT_WEIGHT_SEMIBOLD = 600;
const FILTER_OPTIONS: readonly (McpActivityStatus | "ALL")[] = [
  "ALL",
  "OK",
  "SELF_HEALING_ERROR",
  "DENIED_HUMAN_ONLY",
];

const FILTER_LABELS: Readonly<Record<McpActivityStatus | "ALL", { key: string; fallback: string }>> = {
  ALL: { key: "mcpGateway.filterAll", fallback: "Tất cả" },
  OK: { key: "mcpGateway.filterOk", fallback: "Thành công" },
  SELF_HEALING_ERROR: { key: "mcpGateway.filterSelfHealing", fallback: "Gợi ý tự sửa" },
  DENIED_HUMAN_ONLY: { key: "mcpGateway.filterDenied", fallback: "Bị chặn bảo mật" },
  VALIDATION_ERROR: { key: "mcpGateway.statusValidationError", fallback: "Lỗi tham số" },
};

const STATUS_META: Readonly<
  Record<McpActivityStatus, { key: string; fallback: string; style: React.CSSProperties }>
> = {
  OK: {
    key: "mcpGateway.statusOk",
    fallback: "Thành công",
    style: {
      backgroundColor: "var(--state-success-soft)",
      color: "var(--state-success)",
      border: "1px solid var(--state-success)",
    },
  },
  SELF_HEALING_ERROR: {
    key: "mcpGateway.statusSelfHealing",
    fallback: "Gợi ý tự sửa",
    style: {
      backgroundColor: "var(--state-warning-soft)",
      color: "var(--state-warning)",
      border: "1px solid var(--state-warning)",
    },
  },
  VALIDATION_ERROR: {
    key: "mcpGateway.statusValidationError",
    fallback: "Lỗi tham số",
    style: {
      backgroundColor: "var(--state-warning-soft)",
      color: "var(--state-warning)",
      border: "1px solid var(--state-warning)",
    },
  },
  DENIED_HUMAN_ONLY: {
    key: "mcpGateway.statusDeniedHumanOnly",
    fallback: "Bị chặn bảo mật",
    style: {
      backgroundColor: "var(--state-danger-soft)",
      color: "var(--state-danger)",
      border: "1px solid var(--state-danger)",
    },
  },
};

export interface ExternalActivityTableProps {
  activities: McpExternalActivityDto[];
  loading: boolean;
  statusFilter: McpActivityStatus | "ALL";
  onFilterChange: (next: McpActivityStatus | "ALL") => void;
  onRefresh: () => void;
}

const ActivityRow: React.FC<{ item: McpExternalActivityDto }> = ({ item }) => {
  const { t } = useTranslation();
  const statusInfo = STATUS_META[item.status];
  const actorLabel =
    item.actorKind === "MCP_CLIENT"
      ? t("mcpGateway.actorMcpClient", "Ứng dụng ngoài")
      : t("mcpGateway.actorHuman", "Người dùng");

  return (
    <tr
      data-testid={`mcp-activity-row-${item.id}`}
      style={{ borderBottom: "1px solid var(--border-subtle)" }}
    >
      <td style={{ padding: "8px 12px", fontSize: "11px", color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>
        {item.hostClient}
        <div style={{ fontSize: "9.5px", color: "var(--text-muted)", fontFamily: "var(--font-sans)", marginTop: "2px" }}>
          {actorLabel}
        </div>
      </td>
      <td style={{ padding: "8px 12px", fontSize: "11px", color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>
        {item.toolName || item.methodName}
        <div style={{ fontSize: "9.5px", color: "var(--text-muted)", marginTop: "2px" }}>{item.methodName}</div>
      </td>
      <td style={{ padding: "8px 12px", whiteSpace: "nowrap" }}>
        <span
          data-status={item.status}
          style={{
            display: "inline-flex",
            alignItems: "center",
            fontSize: "10px",
            fontWeight: FONT_WEIGHT_SEMIBOLD,
            padding: "2px 8px",
            borderRadius: "var(--radius-pill)",
            ...statusInfo.style,
          }}
        >
          {t(statusInfo.key, statusInfo.fallback)}
        </span>
      </td>
      <td style={{ padding: "8px 12px", fontSize: "11px", color: "var(--text-secondary)", lineHeight: "1.4" }}>
        {item.summary}
      </td>
      <td style={{ padding: "8px 12px", fontSize: "10.5px", color: "var(--text-muted)", textAlign: "right", fontFamily: "var(--font-mono)", whiteSpace: "nowrap" }}>
        {item.latencyMs}ms
      </td>
    </tr>
  );
};

const REFRESH_BTN_STYLE: React.CSSProperties = {
  padding: "4px 10px",
  fontSize: "10px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-subtle)",
  color: "var(--text-primary)",
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
};

const ActivityFilterButton: React.FC<{
  opt: McpActivityStatus | "ALL";
  active: boolean;
  onSelect: (next: McpActivityStatus | "ALL") => void;
}> = ({ opt, active, onSelect }) => {
  const { t } = useTranslation();
  const meta = FILTER_LABELS[opt];
  return (
    <button
      type="button"
      data-testid={`mcp-activity-filter-${opt}`}
      onClick={() => onSelect(opt)}
      style={{
        padding: "3px 10px",
        fontSize: "10px",
        fontWeight: FONT_WEIGHT_SEMIBOLD,
        borderRadius: "var(--radius-pill)",
        border: active ? "1px solid var(--text-primary)" : "1px solid var(--border-subtle)",
        backgroundColor: active ? "var(--text-primary)" : "var(--bg-subtle)",
        color: active ? "var(--bg-workspace)" : "var(--text-secondary)",
        cursor: "pointer",
      }}
    >
      {t(meta.key, meta.fallback)}
    </button>
  );
};

const ActivityHeaderControls: React.FC<{
  loading: boolean;
  statusFilter: McpActivityStatus | "ALL";
  onFilterChange: (next: McpActivityStatus | "ALL") => void;
  onRefresh: () => void;
}> = ({ loading, statusFilter, onFilterChange, onRefresh }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
      {FILTER_OPTIONS.map((opt) => (
        <ActivityFilterButton key={opt} opt={opt} active={statusFilter === opt} onSelect={onFilterChange} />
      ))}
      <button
        type="button"
        data-testid="mcp-activity-refresh-btn"
        disabled={loading}
        onClick={onRefresh}
        style={{ ...REFRESH_BTN_STYLE, cursor: loading ? "wait" : "pointer" }}
      >
        <BrandIcon name="refresh-cw" size={SMALL_ICON_SIZE} color="currentColor" />
        <span>{t("mcpGateway.refreshBtn", "Làm mới")}</span>
      </button>
    </div>
  );
};

export const ExternalActivityTable: React.FC<ExternalActivityTableProps> = ({
  activities,
  loading,
  statusFilter,
  onFilterChange,
  onRefresh,
}) => {
  const { t } = useTranslation();
  return (
    <section
      data-ff-id="mcp-external-activity-table"
      aria-label={t("mcpGateway.activityTitle", "Nhật ký Hoạt động MCP Trực tiếp")}
      style={{ display: "flex", flexDirection: "column", gap: "10px" }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: "8px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="data-grid" size={HEADER_ICON_SIZE} color="var(--text-primary)" />
          <span style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
            {t("mcpGateway.activityTitle", "Nhật ký Hoạt động MCP Trực tiếp")}
          </span>
        </div>
        <ActivityHeaderControls loading={loading} statusFilter={statusFilter} onFilterChange={onFilterChange} onRefresh={onRefresh} />
      </div>
      <div style={{ border: "1px solid var(--border-subtle)", borderRadius: "var(--radius-xs)", overflowX: "auto", backgroundColor: "var(--bg-surface)" }}>
        {activities.length === 0 ? (
          <div style={{ padding: "16px 12px", fontSize: "11.5px", color: "var(--text-muted)", textAlign: "center" }}>
            {t("mcpGateway.emptyActivity", "Chưa có hoạt động MCP nào từ ứng dụng bên ngoài.")}
          </div>
        ) : (
          <table style={{ width: "100%", borderCollapse: "collapse" }}>
            <thead>
              <tr style={{ backgroundColor: "var(--bg-subtle)", borderBottom: "1px solid var(--border-subtle)", textAlign: "left" }}>
                <th style={{ padding: "8px 12px", fontSize: "10.5px", color: "var(--text-secondary)" }}>{t("mcpGateway.colHost", "Ứng dụng")}</th>
                <th style={{ padding: "8px 12px", fontSize: "10.5px", color: "var(--text-secondary)" }}>{t("mcpGateway.colMethod", "Công cụ / Phương thức")}</th>
                <th style={{ padding: "8px 12px", fontSize: "10.5px", color: "var(--text-secondary)" }}>{t("mcpGateway.colStatus", "Trạng thái")}</th>
                <th style={{ padding: "8px 12px", fontSize: "10.5px", color: "var(--text-secondary)" }}>{t("mcpGateway.colSummary", "Tóm tắt")}</th>
                <th style={{ padding: "8px 12px", fontSize: "10.5px", color: "var(--text-secondary)", textAlign: "right" }}>{t("mcpGateway.colLatency", "Độ trễ")}</th>
              </tr>
            </thead>
            <tbody>
              {activities.map((item) => (
                <ActivityRow key={item.id} item={item} />
              ))}
            </tbody>
          </table>
        )}
      </div>
    </section>
  );
};
