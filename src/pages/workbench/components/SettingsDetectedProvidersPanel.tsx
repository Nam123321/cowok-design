// @story 04.3, 11.1
// @implements Bento 1: Auto-Detected Local BYOS / CLI Catalog (Auto-Connect Ready + Inline Setup Flow)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CliDiagnosticItem } from "../../../lib/validations/workbench-settings";

export interface SettingsDetectedProvidersPanelProps {
  readyTools: CliDiagnosticItem[];
  setupTools: CliDiagnosticItem[];
  moreTools: CliDiagnosticItem[];
  connectedIds: string[];
  loading: boolean;
  probingId: string | null;
  onToggleConnect: (tool: CliDiagnosticItem) => void;
  onRecheckAll: () => void;
  onProbeTool: (cliId: string) => void;
}

const SETUP_HINT_CMD: Record<string, string> = {
  codex: "codex login",
  claude: "claude",
  gemini: "gemini",
  agy: "agy",
  ollama: "ollama serve",
  copilot: "gh auth login",
  opencode: "opencode",
  qwen: "qwen",
  pi: "pi",
};

const ReadyBentoTile: React.FC<{
  tool: CliDiagnosticItem;
  connected: boolean;
  onClick: () => void;
}> = ({ tool, connected, onClick }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      data-testid={`provider-row-${tool.id}`}
      onClick={onClick}
      style={{
        padding: "8px 10px",
        borderRadius: "var(--radius-xs)",
        border: connected ? "1px solid var(--state-success)" : "1px solid var(--border-default)",
        backgroundColor: connected ? "var(--state-success-soft)" : "var(--bg-workspace)",
        textAlign: "left",
        cursor: "pointer",
        display: "flex",
        flexDirection: "column",
        gap: "3px",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
        <span style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {tool.name}
        </span>
        <span style={{ fontSize: "10px", fontWeight: 600, color: connected ? "var(--state-success)" : "var(--text-muted)", flexShrink: 0 }}>
          {connected ? `✓ ${t("settings.connectedShort", "Đã nối")}` : t("settings.providerReady", "Sẵn sàng")}
        </span>
      </div>
      <span style={{ fontSize: "10px", color: "var(--text-muted)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
        {tool.accountLabel || `${tool.command} (${t("settings.detectedLocal", "Phát hiện trên máy")})`}
      </span>
    </button>
  );
};

const SetupDrawerCard: React.FC<{
  tool: CliDiagnosticItem;
  probing: boolean;
  onProbe: () => void;
  onClose: () => void;
}> = ({ tool, probing, onProbe, onClose }) => {
  const { t } = useTranslation();
  const cmdHint = SETUP_HINT_CMD[tool.id] || tool.command;

  return (
    <div data-testid={`setup-flow-${tool.id}`} style={{ padding: "9px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "6px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
        <span style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)" }}>
          {t("settings.setupTitlePrefix", "Thiết lập")} {tool.name}
        </span>
        <button type="button" onClick={onClose} style={{ border: "none", background: "transparent", color: "var(--text-muted)", fontSize: "12px", cursor: "pointer" }}>×</button>
      </div>
      <div style={{ fontSize: "10.5px", color: "var(--text-secondary)", lineHeight: 1.4 }}>
        {tool.authReason || t("settings.setupStepInstruction", "Chạy lệnh bên dưới trên terminal rồi bấm Kiểm tra lại để tự động kết nối.")}
      </div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", padding: "5px 8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-canvas)", border: "1px solid var(--border-subtle)" }}>
        <code style={{ fontSize: "10.5px", fontFamily: "var(--font-mono)", color: "var(--text-primary)" }}>{cmdHint}</code>
        <button
          type="button"
          data-testid={`probe-setup-btn-${tool.id}`}
          onClick={onProbe}
          disabled={probing}
          style={{ height: "22px", padding: "0 9px", borderRadius: "var(--radius-pill)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--action-primary)", fontSize: "10.5px", fontWeight: 600, cursor: "pointer" }}
        >
          {probing ? "..." : t("settings.probeBtn", "Kiểm tra")}
        </button>
      </div>
      <div style={{ fontSize: "10px", color: "var(--text-muted)" }}>
        {t("settings.story11ByosNote", "Điều phối chạy tiến trình cô lập đa CLI ($COWOK_SESSION_DIR) thuộc Story 11.1.")}
      </div>
    </div>
  );
};

export const SettingsDetectedProvidersPanel: React.FC<SettingsDetectedProvidersPanelProps> = (p) => {
  const { t } = useTranslation();
  const [showUnconfigured, setShowUnconfigured] = useState(false);
  const [setupTarget, setSetupTarget] = useState<CliDiagnosticItem | null>(null);

  const pendingTools = [...p.setupTools, ...p.moreTools];

  return (
    <div data-testid="detected-byos-panel" style={{ padding: "10px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "8px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
        <div>
          <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)" }}>
            {t("settings.detectedByosTitle", "1. LLM & Coding Agents (BYOS)")}
          </div>
          <div style={{ fontSize: "10.5px", color: "var(--text-muted)" }}>
            {t("settings.detectedByosDesc", "Tự động phát hiện CLI trên máy và kết nối sẵn — click để bật/tắt hoặc thiết lập mới.")}
          </div>
        </div>
        <button
          type="button"
          data-testid="settings-recheck-providers-btn"
          onClick={p.onRecheckAll}
          disabled={p.loading}
          style={{ height: "26px", padding: "0 10px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", fontSize: "11px", fontWeight: 500, cursor: p.loading ? "wait" : "pointer", display: "inline-flex", alignItems: "center", gap: "5px", flexShrink: 0 }}
        >
          <BrandIcon name="refresh" size={11} color="currentColor" />
          <span>{p.loading ? t("settings.checkingProviders", "Đang quét...") : t("settings.checkAgain", "Kiểm tra lại")}</span>
        </button>
      </div>

      {p.readyTools.length > 0 ? (
        <div data-testid="ready-byos-list" style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "6px" }}>
          {p.readyTools.map((tool) => (
            <ReadyBentoTile
              key={tool.id}
              tool={tool}
              connected={p.connectedIds.includes(tool.id)}
              onClick={() => p.onToggleConnect(tool)}
            />
          ))}
        </div>
      ) : (
        !p.loading && (
          <div style={{ fontSize: "11px", color: "var(--text-muted)", padding: "2px" }}>
            {t("settings.noLocalByosDetected", "Chưa phát hiện CLI AI nào sẵn sàng trên máy; mở danh mục bên dưới để thiết lập hoặc dùng API Gateway.")}
          </div>
        )
      )}

      {pendingTools.length > 0 && (
        <div>
          <button
            type="button"
            data-testid="toggle-more-providers-btn"
            onClick={() => setShowUnconfigured((v) => !v)}
            style={{ width: "100%", padding: "6px 9px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)", color: "var(--text-secondary)", fontSize: "11px", fontWeight: 500, display: "flex", alignItems: "center", justifyContent: "space-between", cursor: "pointer" }}
          >
            <span>{t("settings.moreAiProviders", "Thiết lập thêm LLM / CLI khác")} ({pendingTools.length})</span>
            <span>{showUnconfigured ? "▲" : "▼"}</span>
          </button>

          {showUnconfigured && (
            <div data-testid="setup-byos-list" style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "5px", marginTop: "6px" }}>
              {pendingTools.map((tool) => {
                const needAuth = tool.installed;
                return (
                  <button
                    key={tool.id}
                    type="button"
                    data-testid={`provider-row-${tool.id}`}
                    onClick={() => setSetupTarget(tool)}
                    style={{ padding: "6px 8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", fontSize: "10.5px", color: "var(--text-primary)", cursor: "pointer", textAlign: "left" }}
                  >
                    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{tool.name}</span>
                    <span style={{ fontSize: "9.5px", color: needAuth ? "var(--state-warning)" : "var(--text-muted)", flexShrink: 0 }}>
                      {needAuth ? t("settings.setUpBtn", "Thiết lập") : tool.command}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      )}

      {setupTarget && (
        <SetupDrawerCard
          tool={setupTarget}
          probing={p.probingId === setupTarget.id}
          onProbe={() => p.onProbeTool(setupTarget.id)}
          onClose={() => setSetupTarget(null)}
        />
      )}
    </div>
  );
};
