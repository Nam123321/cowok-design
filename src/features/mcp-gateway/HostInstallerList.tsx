// @story 11.2
// @implements AC-03, AC-04 (1-Click External Host MCP Installer Card Grid [5.20] with Safe Backup & 1-Click Rollback)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { McpHostId, McpHostStatusDto } from "../../lib/validations/mcp-gateway";

const ICON_SIZE = 13;
const SMALL_ICON_SIZE = 11;
const SHA_PREVIEW_LEN = 10;
const FONT_WEIGHT_SEMIBOLD = 600;

export interface HostInstallerListProps {
  hosts: McpHostStatusDto[];
  serverUrl: string;
  busyHostId: McpHostId | null;
  loading: boolean;
  onInstall: (hostId: McpHostId) => void;
  onRollback: (hostId: McpHostId, backupId?: string) => void;
}

const CARD_STYLE: React.CSSProperties = {
  padding: "10px 12px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-surface)",
  display: "flex",
  flexDirection: "column",
  justifyContent: "space-between",
  gap: "8px",
  height: "100%",
  boxSizing: "border-box",
};

const FORMAT_PILL_STYLE: React.CSSProperties = {
  fontSize: "9.5px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  padding: "2px 6px",
  borderRadius: "var(--radius-pill)",
  backgroundColor: "var(--bg-subtle)",
  color: "var(--text-secondary)",
  fontFamily: "var(--font-mono)",
};

const BACKUP_PILL_STYLE: React.CSSProperties = {
  fontSize: "9.5px",
  padding: "2px 7px",
  borderRadius: "var(--radius-pill)",
  backgroundColor: "var(--state-success-soft)",
  color: "var(--state-success)",
  border: "1px solid var(--state-success)",
  fontFamily: "var(--font-mono)",
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  width: "fit-content",
};

const HostCardActions: React.FC<{
  host: McpHostStatusDto;
  busy: boolean;
  onInstall: (hostId: McpHostId) => void;
  onRollback: (hostId: McpHostId, backupId?: string) => void;
}> = ({ host, busy, onInstall, onRollback }) => {
  const { t } = useTranslation();
  const cursor = busy ? "wait" : "pointer";
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "6px", marginTop: "2px" }}>
      <button
        type="button"
        className="cowok-btn cowok-btn-primary"
        data-testid={`mcp-install-btn-${host.hostId}`}
        disabled={busy}
        onClick={() => onInstall(host.hostId)}
        style={{ flex: 1, padding: "5px 8px", fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", cursor }}
      >
        {host.installed
          ? t("mcpGateway.reinstallHostBtn", "Cài lại")
          : t("mcpGateway.installHostBtn", "Cài đặt")}
      </button>
      {host.latestBackupId && (
        <button
          type="button"
          className="cowok-btn"
          data-testid={`mcp-rollback-btn-${host.hostId}`}
          disabled={busy}
          onClick={() => onRollback(host.hostId, host.latestBackupId ?? undefined)}
          style={{
            padding: "5px 8px",
            fontSize: "10px",
            fontWeight: FONT_WEIGHT_SEMIBOLD,
            borderRadius: "var(--radius-xs)",
            border: "1px solid var(--border-default)",
            backgroundColor: "var(--bg-subtle)",
            color: "var(--text-primary)",
            cursor,
            display: "inline-flex",
            alignItems: "center",
            gap: "4px",
          }}
        >
          <BrandIcon name="corner-up-left" size={SMALL_ICON_SIZE} color="currentColor" />
          <span>{t("mcpGateway.rollbackBtn", "Khôi phục")}</span>
        </button>
      )}
    </div>
  );
};

const HostInstallerCard: React.FC<{
  host: McpHostStatusDto;
  busy: boolean;
  onInstall: (hostId: McpHostId) => void;
  onRollback: (hostId: McpHostId, backupId?: string) => void;
}> = ({ host, busy, onInstall, onRollback }) => {
  const { t } = useTranslation();
  const shaShort = host.latestAfterSha256 ? host.latestAfterSha256.slice(0, SHA_PREVIEW_LEN) : null;
  return (
    <div data-testid={`mcp-host-card-${host.hostId}`} style={CARD_STYLE}>
      <div style={{ display: "flex", flexDirection: "column", gap: "6px", minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "6px", minWidth: 0 }}>
            <BrandIcon name="hard-drive" size={ICON_SIZE} color="var(--text-secondary)" />
            <span style={{ fontSize: "11.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
              {host.displayName}
            </span>
          </div>
          <span style={FORMAT_PILL_STYLE}>{host.configFormat.toUpperCase()}</span>
        </div>
        <div
          title={host.defaultRelativePath}
          style={{
            fontSize: "10px",
            color: "var(--text-muted)",
            fontFamily: "var(--font-mono)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
            lineHeight: "16px",
          }}
        >
          {host.defaultRelativePath}
        </div>
        {shaShort && (
          <div data-testid={`mcp-backup-pill-${host.hostId}`} style={BACKUP_PILL_STYLE}>
            <BrandIcon name="check-circle" size={SMALL_ICON_SIZE} color="var(--state-success)" />
            <span>{t("mcpGateway.backupChecksumLabel", "Mã kiểm tra bản sao lưu")}: {shaShort}</span>
          </div>
        )}
      </div>
      <HostCardActions host={host} busy={busy} onInstall={onInstall} onRollback={onRollback} />
    </div>
  );
};

export const HostInstallerList: React.FC<HostInstallerListProps> = ({
  hosts,
  serverUrl,
  busyHostId,
  loading,
  onInstall,
  onRollback,
}) => {
  const { t } = useTranslation();
  return (
    <section
      data-ff-id="mcp-host-installer-list"
      aria-label={t("mcpGateway.hostInstallerTitle", "Cài đặt MCP 1 chạm cho Ứng dụng Ngoài")}
      style={{ display: "flex", flexDirection: "column", gap: "8px" }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: "8px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="execution-pipeline" size={ICON_SIZE} color="var(--text-primary)" />
          <span style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
            {t("mcpGateway.hostInstallerTitle", "Cài đặt MCP 1 chạm cho Ứng dụng Ngoài")}
          </span>
        </div>
        {serverUrl && (
          <span style={{ fontSize: "10px", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
            {serverUrl}
          </span>
        )}
      </div>
      {loading && hosts.length === 0 ? (
        <div style={{ fontSize: "11px", color: "var(--text-muted)", padding: "8px" }}>
          {t("mcpGateway.loadingHosts", "Đang tải danh sách ứng dụng...")}
        </div>
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(210px, 1fr))", gap: "10px" }}>
          {hosts.map((host) => (
            <HostInstallerCard
              key={host.hostId}
              host={host}
              busy={busyHostId === host.hostId}
              onInstall={onInstall}
              onRollback={onRollback}
            />
          ))}
        </div>
      )}
    </section>
  );
};
