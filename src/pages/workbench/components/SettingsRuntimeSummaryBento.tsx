// @story 04.3
// @implements Shorten Runtime Settings Bento (Info-Only Summary When Default, Expandable Override)
import React, { useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";

export interface SettingsRuntimeSummaryBentoProps {
  storageMode: string;
  permissionLevel: string;
  onChangeStorageMode: (mode: string) => void;
  onChangePermissionLevel: (level: string) => void;
}

const DEFAULT_VAULT = "vault_0600";
const DEFAULT_PERM = "auto_sandbox";

const choiceBtnStyle = (active: boolean): React.CSSProperties => ({
  padding: "7px 8px",
  fontSize: "11px",
  fontWeight: active ? 600 : 400,
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  borderBottom: active ? "2px solid var(--action-primary)" : "1px solid var(--border-subtle)",
  backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
  color: "var(--text-primary)",
  cursor: "pointer",
});

export const SettingsRuntimeSummaryBento: React.FC<SettingsRuntimeSummaryBentoProps> = ({
  storageMode,
  permissionLevel,
  onChangeStorageMode,
  onChangePermissionLevel,
}) => {
  const { t } = useTranslation();
  const isDefaultRuntime = storageMode === DEFAULT_VAULT && permissionLevel === DEFAULT_PERM;
  const [expanded, setExpanded] = useState<boolean>(!isDefaultRuntime);

  const storageLabel =
    storageMode === "ram_only"
      ? t("settings.vaultStorageRamOnly", "RAM Only (Chỉ giữ trong phiên)")
      : t("settings.vaultStorageOsKeychain", "OS Keychain / Daemon 0600 Vault");
  const permLabel =
    permissionLevel === "ask_risky"
      ? t("settings.permAskRisky", "Hỏi khi rủi ro")
      : permissionLevel === "ask_all"
        ? t("settings.permAskAll", "Hỏi tất cả")
        : t("settings.permAutoSandbox", "Tự động trong Hộp cát (Sandbox)");

  return (
    <div data-testid="runtime-bento-summary" style={{ padding: "10px 12px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "8px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
        <div style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)" }}>
          {t("settings.runtimeBentoTitle", "3. Cài đặt Runtime (Lưu trữ & Phê duyệt)")}
          {isDefaultRuntime && (
            <span style={{ marginLeft: "6px", fontSize: "10px", fontWeight: 500, color: "var(--text-muted)" }}>
              · {t("settings.runtimeDefaultTag", "Mặc định")}
            </span>
          )}
        </div>
        <button
          type="button"
          data-testid="runtime-customize-btn"
          onClick={() => setExpanded((v) => !v)}
          style={{ border: "none", background: "transparent", fontSize: "11px", fontWeight: 600, color: "var(--action-primary)", cursor: "pointer", padding: 0 }}
        >
          {expanded ? `${t("settings.collapseBento", "Thu gọn")} ▲` : `${t("settings.customizeRuntimeBtn", "Tùy chỉnh")} ▾`}
        </button>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
        <div style={{ padding: "7px 9px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "2px" }}>
          <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>{t("settings.vaultStorageShort", "Cơ chế lưu trữ")}</span>
          <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-primary)" }}>{storageLabel}</span>
        </div>
        <div style={{ padding: "7px 9px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-workspace)", border: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "2px" }}>
          <span style={{ fontSize: "10px", color: "var(--text-muted)" }}>{t("settings.permissionShort", "Cơ chế phê duyệt")}</span>
          <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-primary)" }}>{permLabel}</span>
        </div>
      </div>

      {expanded && (
        <div data-testid="runtime-bento-controls" style={{ display: "flex", flexDirection: "column", gap: "8px", paddingTop: "4px", borderTop: "1px solid var(--border-subtle)" }}>
          <div>
            <label style={{ display: "block", fontSize: "10.5px", fontWeight: 600, color: "var(--text-secondary)", marginBottom: "5px" }}>
              {t("settings.vaultStorageLabel", "Cơ chế lưu trữ Secret & Vault:")}
            </label>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
              <button type="button" onClick={() => onChangeStorageMode("vault_0600")} style={choiceBtnStyle(storageMode === "vault_0600")}>
                {t("settings.vaultStorageOsKeychain", "OS Keychain / Daemon 0600 Vault")}
              </button>
              <button type="button" onClick={() => onChangeStorageMode("ram_only")} style={choiceBtnStyle(storageMode === "ram_only")}>
                {t("settings.vaultStorageRamOnly", "RAM Only (Chỉ giữ trong phiên)")}
              </button>
            </div>
          </div>
          <div>
            <label style={{ display: "block", fontSize: "10.5px", fontWeight: 600, color: "var(--text-secondary)", marginBottom: "5px" }}>
              {t("settings.permissionLabel", "Cấp độ phê duyệt tự động (Permission Approval):")}
            </label>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "6px" }}>
              {[
                { id: "auto_sandbox", label: t("settings.permAutoSandbox", "Tự động Sandbox") },
                { id: "ask_risky", label: t("settings.permAskRisky", "Hỏi khi rủi ro") },
                { id: "ask_all", label: t("settings.permAskAll", "Hỏi tất cả") },
              ].map((lvl) => (
                <button key={lvl.id} type="button" onClick={() => onChangePermissionLevel(lvl.id)} style={choiceBtnStyle(permissionLevel === lvl.id)}>
                  {lvl.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
