// @story 04.1
import React, { useEffect, useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { apiWorkbenchSettings } from "../../../api/workbench-settings";
import type { McpServerDto } from "../../../lib/validations/workbench-settings";

interface SettingsMcpAutomationsPaneProps {
  selectedTab: "mcp" | "automations";
}

const DEFAULT_SYSTEM_MCPS = [
  { id: "sys-falkordb", name: "cowok-falkordb", desc: "127.0.0.1:6380 • Knowledge Graph & Vector DB", status: "ONLINE" },
  { id: "sys-fs", name: "local-filesystem", desc: "Native Workspace & In-Place Writeback", status: "ONLINE" },
  { id: "sys-cdp", name: "browser-devtools", desc: "Chrome DevTools Protocol (CDP Automation)", status: "ONLINE" },
  { id: "sys-github", name: "github-mcp-server", desc: "GitHub SCM, PRs & Release Pipeline", status: "CONNECTED" },
];

const McpHostCredentialsCard: React.FC = () => {
  const { t } = useTranslation();
  const [isCopiedKey, setIsCopiedKey] = useState(false);
  const [isCopiedEndpoint, setIsCopiedEndpoint] = useState(false);
  const [hostApiKey, setHostApiKey] = useState("cwk_local_mcp_dev_••••••••••••••••");
  const hostEndpoint = "http://127.0.0.1:3100/mcp";

  const handleCopy = (text: string, type: "key" | "endpoint") => {
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(text);
      if (type === "key") { setIsCopiedKey(true); setTimeout(() => setIsCopiedKey(false), 2000); }
      else { setIsCopiedEndpoint(true); setTimeout(() => setIsCopiedEndpoint(false), 2000); }
    }
  };
  const handleRegenerateKey = () => {
    const bytes = new Uint8Array(16);
    if (typeof globalThis.crypto?.getRandomValues === "function") {
      globalThis.crypto.getRandomValues(bytes);
    }
    const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
    setHostApiKey(`cwk_local_mcp_dev_${hex}`);
  };

  return (
    <div style={{ padding: "12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)" }}>
      <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)", marginBottom: "3px" }}>{t("settings.mcpHostTitle", "Máy chủ MCP Cowok-Design (Host MCP Server)")}</div>
      <div style={{ fontSize: "10.5px", color: "var(--text-muted)", marginBottom: "10px", lineHeight: "1.35" }}>{t("settings.mcpHostDesc", "Cung cấp Endpoint và API Key để các công cụ ngoài gọi vào Cowok-Design")}</div>
      <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
        <div>
          <label style={{ display: "block", fontSize: "10.5px", color: "var(--text-muted)", marginBottom: "3px" }}>{t("settings.mcpEndpointLabel", "MCP Server Endpoint URL")}</label>
          <div style={{ display: "flex", gap: "6px" }}>
            <input type="text" readOnly value={hostEndpoint} className="cowok-input" style={{ flex: 1, height: "28px", padding: "0 8px", fontSize: "11px", fontFamily: "var(--font-mono)", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", border: "1px solid var(--border-default)" }} />
            <button type="button" onClick={() => handleCopy(hostEndpoint, "endpoint")} className="cowok-btn cowok-btn-secondary" style={{ height: "28px", padding: "0 10px", fontSize: "10.5px", borderRadius: "var(--radius-xs)", cursor: "pointer", whiteSpace: "nowrap" }}>
              {isCopiedEndpoint ? t("settings.copiedKey", "Đã sao chép!") : t("settings.copyKeyBtn", "Sao chép")}
            </button>
          </div>
        </div>
        <div>
          <label style={{ display: "block", fontSize: "10.5px", color: "var(--text-muted)", marginBottom: "3px" }}>{t("settings.mcpApiKeyLabel", "Khóa API MCP của Cowok-Design")}</label>
          <div style={{ display: "flex", gap: "6px" }}>
            <input type="text" readOnly value={hostApiKey} className="cowok-input" style={{ flex: 1, height: "28px", padding: "0 8px", fontSize: "11px", fontFamily: "var(--font-mono)", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", border: "1px solid var(--border-default)" }} />
            <button type="button" onClick={() => handleCopy(hostApiKey, "key")} className="cowok-btn cowok-btn-secondary" style={{ height: "28px", padding: "0 10px", fontSize: "10.5px", borderRadius: "var(--radius-xs)", cursor: "pointer", whiteSpace: "nowrap" }}>
              {isCopiedKey ? t("settings.copiedKey", "Đã sao chép!") : t("settings.copyKeyBtn", "Sao chép Key")}
            </button>
            <button type="button" onClick={handleRegenerateKey} className="cowok-btn cowok-btn-ghost" style={{ height: "28px", padding: "0 8px", fontSize: "10.5px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", cursor: "pointer", whiteSpace: "nowrap" }}>
              {t("settings.regenerateKeyBtn", "Tạo lại Key")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

const McpClientRegistryList: React.FC = () => {
  const { t } = useTranslation();
  const [servers, setServers] = useState<McpServerDto[]>([]);
  const [adding, setAdding] = useState(false);
  const [newName, setNewName] = useState("");
  const [newCommand, setNewCommand] = useState("");
  const [mcpError, setMcpError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void apiWorkbenchSettings.listMcpServers()
      .then((rows) => {
        if (active) {
          setServers(rows);
          setMcpError(null);
        }
      })
      .catch((err) => {
        if (!active) return;
        const detail = err instanceof Error ? `: ${err.message}` : "";
        setMcpError(`${t("settings.mcpDaemonError", "Không thể kết nối daemon MCP")}${detail}`);
      });
    return () => { active = false; };
  }, [t]);

  const handleAddServer = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newName.trim() || !newCommand.trim()) {
      setMcpError(t("settings.mcpEmptyValidation", "Vui lòng nhập đầy đủ tên và lệnh/URL MCP."));
      return;
    }
    setMcpError(null);
    try {
      const saved = await apiWorkbenchSettings.upsertMcpServer({
        serverSlug: newName.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-"),
        displayName: newName.trim(),
        transportKind: newCommand.trim().startsWith("http") ? "http_sse" : "stdio",
        commandOrUrl: newCommand.trim(),
        permissionTier: "read_only",
        enabled: true,
      });
      setServers((prev) => [...prev.filter((s) => s.id !== saved.id), saved]);
      setNewName("");
      setNewCommand("");
      setAdding(false);
    } catch (err) {
      const detail = err instanceof Error ? `: ${err.message}` : "";
      setMcpError(`${t("settings.mcpDaemonError", "Không thể lưu MCP server lên daemon")}${detail}`);
    }
  };

  const handleDeleteServer = async (id: string) => {
    setMcpError(null);
    const prevSnapshot = servers;
    setServers((prev) => prev.filter((s) => s.id !== id));
    try {
      await apiWorkbenchSettings.deleteMcpServer(id);
    } catch (err) {
      setServers(prevSnapshot);
      const detail = err instanceof Error ? `: ${err.message}` : "";
      setMcpError(`${t("settings.mcpDaemonError", "Không thể xóa MCP server trên daemon")}${detail}`);
    }
  };

  const displayedRows = servers.length > 0
    ? servers.map((s) => ({
        id: s.id,
        name: s.displayName,
        desc: `${s.transportKind.toUpperCase()} • ${s.commandOrUrl}`,
        status: s.lastHealthStatus.toUpperCase(),
        removable: true,
      }))
    : DEFAULT_SYSTEM_MCPS.map((m) => ({ ...m, removable: false }));

  return (
    <div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: "6px" }}>
        <label style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-secondary)" }}>
          {t("settings.mcpClientTitle", "Danh sách Máy chủ MCP đã đăng ký (Client MCPs)")}
        </label>
        <div style={{ display: "flex", gap: "6px" }}>
          <button type="button" onClick={() => setAdding((v) => !v)} className="cowok-btn cowok-btn-primary" style={{ padding: "4px 10px", fontSize: "10.5px", fontWeight: 600, borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
            {adding ? t("settings.cancelBtn", "Hủy") : t("settings.addMcpBtn", "+ Thêm MCP Server")}
          </button>
        </div>
      </div>
      {mcpError && (
        <div
          role="alert"
          data-testid="settings-mcp-error"
          style={{
            padding: "6px 10px",
            marginBottom: "8px",
            borderRadius: "var(--radius-xs)",
            backgroundColor: "var(--bg-subtle)",
            border: "1px solid var(--state-danger)",
            color: "var(--state-danger)",
            fontSize: "11px",
          }}
        >
          {mcpError}
        </div>
      )}
      {adding && (
        <form onSubmit={handleAddServer} style={{ display: "flex", gap: "6px", marginBottom: "8px" }}>
          <input type="text" value={newName} onChange={(e) => setNewName(e.target.value)} placeholder={t("settings.mcpNamePlaceholder", "Tên MCP (vd: custom-figma)")} className="cowok-input" style={{ flex: 1, height: "28px", padding: "0 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)" }} />
          <input type="text" value={newCommand} onChange={(e) => setNewCommand(e.target.value)} placeholder={t("settings.mcpCmdPlaceholder", "Lệnh / URL (vd: npx -y ...)")} className="cowok-input" style={{ flex: 2, height: "28px", padding: "0 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)" }} />
          <button type="submit" className="cowok-btn cowok-btn-primary" style={{ height: "28px", padding: "0 10px", fontSize: "10.5px", fontWeight: 600, borderRadius: "var(--radius-xs)", cursor: "pointer" }}>{t("settings.saveBtn", "Lưu")}</button>
        </form>
      )}
      <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
        {displayedRows.map((mcp) => (
          <div key={mcp.id} style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
            <div>
              <div style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>{mcp.name}</div>
              <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>{mcp.desc}</div>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
              <span style={{ fontSize: "9.5px", fontWeight: 600, padding: "2px 6px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--state-success-soft)", color: "var(--state-success)", border: "1px solid var(--border-subtle)" }}>{mcp.status}</span>
              {mcp.removable && (
                <button type="button" onClick={() => void handleDeleteServer(mcp.id)} aria-label={t("common.delete", "Xóa")} style={{ border: "none", background: "transparent", color: "var(--text-muted)", fontSize: "10px", cursor: "pointer" }}>×</button>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export const SettingsMcpAutomationsPane: React.FC<SettingsMcpAutomationsPaneProps> = ({ selectedTab }) => {
  const { t } = useTranslation();
  if (selectedTab === "mcp") {
    return (
      <>
        <McpHostCredentialsCard />
        <McpClientRegistryList />
      </>
    );
  }
  return (
    <>
      <div>
        <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)", marginBottom: "3px" }}>{t("settings.automationsTitle", "Tác vụ nền & Tự động hóa định kỳ")}</div>
        <div style={{ fontSize: "11px", color: "var(--text-muted)", marginBottom: "12px", lineHeight: "1.4" }}>{t("settings.automationsDesc", "Theo dõi tiến trình Rust SQLite WAL Daemon, cron triggers và webhook callbacks")}</div>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
        <div style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div>
            <div style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)" }}>{t("settings.daemonLoopbackTitle", "Cowok Rust SQLite WAL Daemon (Loopback)")}</div>
            <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>{t("settings.daemonLoopbackSubtitle", "127.0.0.1 • Dual-Ingress HMAC-SHA256 v2")}</div>
          </div>
          <span style={{ fontSize: "9.5px", fontWeight: 600, padding: "2px 6px", borderRadius: "10px", backgroundColor: "var(--state-success-soft)", color: "var(--state-success)", border: "1px solid var(--border-subtle)" }}>RUNNING</span>
        </div>
        <div style={{ padding: "8px 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div>
            <div style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)" }}>{t("settings.autoSnapshotTitle", "Auto-Snapshot State & Graph AST")}</div>
            <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>{t("settings.autoSnapshotDesc", "Mỗi 5 phút • Tự động ghi nhận delta đồ thị 2D DAG")}</div>
          </div>
          <span style={{ fontSize: "9.5px", fontWeight: 600, padding: "2px 6px", borderRadius: "10px", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)" }}>SCHEDULED</span>
        </div>
      </div>
    </>
  );
};
