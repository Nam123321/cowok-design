// @story 04.2, 04.3
// @implements Gate H6 (Prioritize BYOS CLI Doctor BEFORE BYOK, Auto-Connect Zero-Click UX)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CliDiagnosticItem } from "../../../lib/validations/workbench-settings";

export interface OnboardingStepsContentProps {
  currentStep: number;
  loadingDoctor: boolean;
  cliItems: CliDiagnosticItem[];
  selectedCli: string;
  onSelectCli: (id: string) => void;
  onRefreshDoctor: () => void;
  workspaceMode: "sandboxed" | "linked";
  onChangeWorkspaceMode: (mode: "sandboxed" | "linked") => void;
  storageMode: "vault_0600" | "ram_only";
  onChangeStorageMode: (mode: "vault_0600" | "ram_only") => void;
  permissionLevel: "auto_sandbox" | "ask_risky" | "ask_all";
  onChangePermissionLevel: (level: "auto_sandbox" | "ask_risky" | "ask_all") => void;
  // Step 1 BYOS & BYOK enhancements
  autoConnectedCliIds?: string[];
  probingCliId?: string | null;
  onProbeCli?: (id: string) => void;
  byokProvider?: string;
  onChangeByokProvider?: (provider: string) => void;
  byokApiKey?: string;
  onChangeByokApiKey?: (key: string) => void;
  isVerifyingByok?: boolean;
  onVerifyByok?: () => void;
  byokVerifyStatus?: { valid: boolean; message: string } | null;
}

const cardStyle = (active: boolean): React.CSSProperties => ({
  padding: "12px",
  borderRadius: "var(--radius-sm)",
  border: `1px solid ${active ? "var(--action-primary)" : "var(--border-default)"}`,
  backgroundColor: active ? "var(--bg-selected)" : "var(--bg-subtle)",
  cursor: "pointer",
  transition: "all var(--duration-fast, 150ms)",
});

export const OnboardingStepsContent: React.FC<OnboardingStepsContentProps> = (p) => {
  const { t } = useTranslation();
  const [isCliExpanded, setIsCliExpanded] = useState<boolean>(true);

  if (p.currentStep === 1) {
    const autoConnectedIds = p.autoConnectedCliIds || [];
    const autoConnectedTools = p.cliItems.filter((c) => autoConnectedIds.includes(c.id));
    const autoConnectedNames = autoConnectedTools.map((c) => c.name).join(", ");

    const byokProvider = p.byokProvider || "openai";
    const byokApiKey = p.byokApiKey || "";
    const isVerifyingByok = p.isVerifyingByok || false;

    const getPlaceholder = (provider: string) => {
      switch (provider) {
        case "anthropic":
          return "sk-ant-api03-...";
        case "gemini":
          return "AIzaSy...";
        case "deepseek":
          return "sk-...";
        case "openrouter":
          return "sk-or-...";
        default:
          return "sk-...";
      }
    };

    return (
      <div style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
        {/* 1. Primary Section: Bring Your Own Subscription (BYOS) Local CLI Doctor */}
        <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", gap: "12px" }}>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <h3 style={{ margin: 0, fontSize: "13.5px", fontWeight: 600, color: "var(--text-primary)" }}>
                  {t("onboarding.step1Title", "1. Kết nối CLI sẵn có trên máy — Bring Your Own Subscription (BYOS)")}
                </h3>
              </div>
              <p style={{ margin: "4px 0 0 0", fontSize: "11.5px", color: "var(--text-secondary)", lineHeight: "1.4" }}>
                {t(
                  "onboarding.step1Desc",
                  "Tận dụng gói thuê bao AI đã đăng nhập sẵn trên máy (Claude Code, OpenAI Codex, Gemini CLI, Cursor, v.v.)"
                )}
              </p>
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
              <button
                type="button"
                data-testid="cli-doctor-accordion-toggle"
                onClick={() => setIsCliExpanded((prev) => !prev)}
                style={{
                  height: "28px",
                  padding: "0 10px",
                  borderRadius: "var(--radius-xs)",
                  border: "1px solid var(--border-default)",
                  backgroundColor: "transparent",
                  color: "var(--text-secondary)",
                  fontSize: "11px",
                  cursor: "pointer",
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  lineHeight: 1,
                  gap: "4px",
                }}
                title={t("onboarding.byosAccordionToggle", "Thu gọn / Mở rộng danh sách CLI")}
              >
                <BrandIcon name={isCliExpanded ? "chevron-up" : "chevron-down"} size={11} color="currentColor" />
                <span>{isCliExpanded ? t("settings.sizeCompact", "Thu gọn") : t("settings.sizeDefault", "Mở rộng")}</span>
              </button>

              <button
                type="button"
                onClick={p.onRefreshDoctor}
                disabled={p.loadingDoctor}
                style={{
                  height: "28px",
                  padding: "0 10px",
                  borderRadius: "var(--radius-xs)",
                  border: "1px solid var(--border-default)",
                  backgroundColor: "transparent",
                  color: "var(--text-primary)",
                  fontSize: "11px",
                  cursor: p.loadingDoctor ? "wait" : "pointer",
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  lineHeight: 1,
                  gap: "6px",
                }}
              >
                <BrandIcon name="refresh-cw" size={11} color="currentColor" />
                <span>{p.loadingDoctor ? t("onboarding.scanning", "Đang quét...") : t("onboarding.rescan", "Quét lại")}</span>
              </button>
            </div>
          </div>

          {/* Prominent Auto-Connect Banner (Zero-Click UX Pattern) */}
          {autoConnectedIds.length > 0 && (
            <div
              data-testid="byos-auto-connect-banner"
              style={{
                padding: "10px 14px",
                borderRadius: "var(--radius-sm)",
                backgroundColor: "var(--state-success-soft)",
                border: "1px solid var(--state-success)",
                color: "var(--state-success)",
                fontSize: "12px",
                fontWeight: 500,
                display: "flex",
                alignItems: "center",
                gap: "8px",
              }}
            >
              <BrandIcon name="check-circle" size={16} color="currentColor" />
              <div style={{ flex: 1 }}>
                <span>
                  {t(
                    "onboarding.byosAutoConnectedNotice",
                    "Đã tự động phát hiện & kết nối {{count}} gói thuê bao local BYOS: {{names}}"
                  )
                    .replace("{{count}}", String(autoConnectedIds.length))
                    .replace("{{names}}", autoConnectedNames || "Local CLIs")}
                </span>
              </div>
            </div>
          )}

          {/* Collapsible CLI Tool Grid (Expanded by default) */}
          {isCliExpanded && (
            <>
              {p.loadingDoctor ? (
                <div style={{ textAlign: "center", padding: "32px 0", color: "var(--text-secondary)" }}>
                  <p style={{ fontSize: "12px", margin: 0 }}>
                    {t("onboarding.scanningLocal", "Đang quét hệ thống CLI cục bộ...")}
                  </p>
                </div>
              ) : (
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
                  {p.cliItems.map((cli) => {
                    const isAutoConnected = autoConnectedIds.includes(cli.id);
                    const isSelected = p.selectedCli === cli.id;
                    const isReady = cli.installed && (cli.authenticated || isAutoConnected);
                    const isProbing = p.probingCliId === cli.id;

                    return (
                      <div
                        key={cli.id}
                        data-testid={`cli-tool-card-${cli.id}`}
                        onClick={() => cli.installed && p.onSelectCli(cli.id)}
                        style={{
                          ...cardStyle(isSelected),
                          padding: "10px 12px",
                          cursor: cli.installed ? "pointer" : "default",
                          opacity: cli.installed ? 1 : 0.6,
                          display: "flex",
                          flexDirection: "column",
                          gap: "6px",
                        }}
                      >
                        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
                          <span style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)" }}>
                            {cli.name}
                          </span>

                          {isAutoConnected ? (
                            <span
                              style={{
                                fontSize: "10px",
                                padding: "3px 8px",
                                borderRadius: "var(--radius-pill)",
                                backgroundColor: "var(--state-success)",
                                color: "var(--action-primary-fg)",
                                fontWeight: 600,
                                display: "inline-flex",
                                alignItems: "center",
                                justifyContent: "center",
                                lineHeight: 1,
                              }}
                            >
                              {t("onboarding.autoConnectedBadge", "Đã tự động kết nối")}
                            </span>
                          ) : (
                            <span
                              style={{
                                fontSize: "10px",
                                padding: "2px 6px",
                                borderRadius: "var(--radius-xs)",
                                backgroundColor: isReady
                                  ? "var(--state-success-soft)"
                                  : cli.installed
                                  ? "var(--bg-hover)"
                                  : "var(--bg-subtle)",
                                color: isReady
                                  ? "var(--state-success)"
                                  : "var(--text-secondary)",
                                fontWeight: 500,
                                display: "inline-flex",
                                alignItems: "center",
                                justifyContent: "center",
                                lineHeight: 1,
                              }}
                            >
                              {cli.installed
                                ? cli.authenticated
                                  ? t("onboarding.ready", "Sẵn sàng")
                                  : t("onboarding.authNeeded", "Cần đăng nhập")
                                : t("onboarding.notInstalled", "Chưa cài")}
                            </span>
                          )}
                        </div>

                        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginTop: "2px" }}>
                          <div style={{ fontSize: "10.5px", color: "var(--text-secondary)" }}>
                            <code>{cli.command}</code> {cli.version ? `• ${cli.version}` : ""}
                          </div>

                          {cli.installed && p.onProbeCli && (
                            <button
                              type="button"
                              data-testid={`cli-probe-btn-${cli.id}`}
                              disabled={isProbing}
                              onClick={(e) => {
                                e.stopPropagation();
                                p.onProbeCli?.(cli.id);
                              }}
                              style={{
                                padding: "2px 8px",
                                height: "22px",
                                borderRadius: "var(--radius-xs)",
                                border: "1px solid var(--border-default)",
                                backgroundColor: "var(--bg-workspace)",
                                color: "var(--text-primary)",
                                fontSize: "10px",
                                cursor: isProbing ? "wait" : "pointer",
                                display: "inline-flex",
                                alignItems: "center",
                                justifyContent: "center",
                                lineHeight: 1,
                                gap: "3px",
                              }}
                            >
                              <BrandIcon name="play" size={8} color="currentColor" />
                              <span>{isProbing ? t("onboarding.probing", "Đang thử...") : t("onboarding.probeBtn", "Kiểm tra")}</span>
                            </button>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </>
          )}
        </div>

        {/* 2. Secondary Section: Paste API Key Directly (BYOK) */}
        <div
          style={{
            paddingTop: "14px",
            borderTop: "1px solid var(--border-default)",
            display: "flex",
            flexDirection: "column",
            gap: "10px",
          }}
        >
          <div>
            <h4 style={{ margin: 0, fontSize: "12.5px", fontWeight: 600, color: "var(--text-primary)" }}>
              {t("onboarding.byokSectionTitle", "2. Hoặc dán API Key trực tiếp (BYOK)")}
            </h4>
            <p style={{ margin: "3px 0 0 0", fontSize: "11px", color: "var(--text-secondary)" }}>
              {t(
                "onboarding.byokSectionDesc",
                "Nhập khóa API nhà cung cấp (OpenAI, Claude, Gemini, v.v.) để kích hoạt nhanh"
              )}
            </p>
          </div>

          <div style={{ display: "flex", gap: "8px", alignItems: "center", flexWrap: "wrap" }}>
            <select
              data-testid="byok-provider-select"
              value={byokProvider}
              onChange={(e) => p.onChangeByokProvider?.(e.target.value)}
              className="cowok-input"
              style={{
                height: "34px",
                padding: "0 10px",
                fontSize: "12px",
                borderRadius: "var(--radius-xs)",
                border: "1px solid var(--border-default)",
                backgroundColor: "var(--bg-workspace)",
                color: "var(--text-primary)",
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                lineHeight: 1,
              }}
            >
              <option value="openai">{t("settings.modelOptionOpenAi4o", "OpenAI")}</option>
              <option value="anthropic">{t("settings.modelOptionClaude37", "Anthropic Claude")}</option>
              <option value="gemini">{t("settings.modelOptionGemini25", "Google Gemini")}</option>
              <option value="deepseek">{t("settings.modelOptionDeepseekR1", "DeepSeek")}</option>
              <option value="openrouter">{t("settings.modelOptionByos", "OpenRouter")}</option>
            </select>

            <div style={{ display: "flex", flex: 1, minWidth: "220px", gap: "6px" }}>
              <input
                type="password"
                data-testid="byok-api-key-input"
                value={byokApiKey}
                onChange={(e) => p.onChangeByokApiKey?.(e.target.value)}
                placeholder={getPlaceholder(byokProvider)}
                className="cowok-input"
                style={{
                  flex: 1,
                  minWidth: 0,
                  height: "34px",
                  padding: "0 12px",
                  fontSize: "11.5px",
                  fontFamily: "var(--font-mono)",
                  borderRadius: "var(--radius-xs)",
                  border: "1px solid var(--border-default)",
                  backgroundColor: "var(--bg-workspace)",
                  color: "var(--text-primary)",
                  boxSizing: "border-box",
                }}
              />

              <button
                type="button"
                data-testid="byok-verify-btn"
                disabled={isVerifyingByok || !byokApiKey.trim()}
                onClick={p.onVerifyByok}
                style={{
                  height: "34px",
                  padding: "0 16px",
                  borderRadius: "var(--radius-xs)",
                  border: "none",
                  backgroundColor: "var(--action-primary)",
                  color: "var(--action-primary-fg)",
                  fontSize: "11.5px",
                  fontWeight: 600,
                  cursor: isVerifyingByok || !byokApiKey.trim() ? "not-allowed" : "pointer",
                  opacity: isVerifyingByok || !byokApiKey.trim() ? 0.6 : 1,
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  lineHeight: 1,
                  whiteSpace: "nowrap",
                }}
              >
                {isVerifyingByok ? t("onboarding.verifying", "Đang kiểm tra...") : t("onboarding.verifyBtn", "Xác thực Key")}
              </button>
            </div>
          </div>

          {p.byokVerifyStatus && (
            <div
              style={{
                fontSize: "11px",
                color: p.byokVerifyStatus.valid ? "var(--state-success)" : "var(--state-danger)",
                display: "flex",
                alignItems: "center",
                gap: "6px",
              }}
            >
              <BrandIcon name={p.byokVerifyStatus.valid ? "check-circle" : "alert-triangle"} size={13} color="currentColor" />
              <span>{p.byokVerifyStatus.message}</span>
            </div>
          )}
        </div>
      </div>
    );
  }

  if (p.currentStep === 2) {
    return (
      <div>
        <h3 style={{ margin: 0, fontSize: "13.5px", fontWeight: 600 }}>
          {t("onboarding.step2Title", "2. Không gian làm việc & Cơ chế bảo mật lưu trữ")}
        </h3>
        <p style={{ margin: "4px 0 16px 0", fontSize: "11.5px", color: "var(--text-secondary)" }}>
          {t("onboarding.step2Desc", "Định cấu hình cách thức lưu trữ dữ liệu dự án và token bảo mật")}
        </p>

        <div style={{ marginBottom: "16px" }}>
          <label style={{ display: "block", fontSize: "11.5px", fontWeight: 600, marginBottom: "8px" }}>
            {t("onboarding.workspaceModeLabel", "Chế độ Workspace:")}
          </label>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
            <div onClick={() => p.onChangeWorkspaceMode("sandboxed")} style={cardStyle(p.workspaceMode === "sandboxed")}>
              <div style={{ fontSize: "12px", fontWeight: 600 }}>{t("onboarding.wsSandboxed", "Sandbox Tự Quản Lý (Khuyên dùng)")}</div>
              <div style={{ fontSize: "11px", color: "var(--text-secondary)", marginTop: "4px" }}>
                {t("onboarding.wsSandboxedDesc", "Tự động phân vùng isolated trong .cowok-design/projects/ với HMAC token")}
              </div>
            </div>
            <div onClick={() => p.onChangeWorkspaceMode("linked")} style={cardStyle(p.workspaceMode === "linked")}>
              <div style={{ fontSize: "12px", fontWeight: 600 }}>{t("onboarding.wsLinked", "Liên Kết Thư Mục Local")}</div>
              <div style={{ fontSize: "11px", color: "var(--text-secondary)", marginTop: "4px" }}>
                {t("onboarding.wsLinkedDesc", "Chọn một thư mục mã nguồn hiện có trên máy (hỗ trợ Git repo)")}
              </div>
            </div>
          </div>
        </div>

        <div>
          <label style={{ display: "block", fontSize: "11.5px", fontWeight: 600, marginBottom: "8px" }}>
            {t("onboarding.storageLabel", "Cơ chế lưu trữ Secret & API Keys:")}
          </label>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
            <div onClick={() => p.onChangeStorageMode("vault_0600")} style={cardStyle(p.storageMode === "vault_0600")}>
              <div style={{ fontSize: "12px", fontWeight: 600 }}>{t("onboarding.vaultTitle", "Daemon 0600 Vault (Bền vững)")}</div>
              <div style={{ fontSize: "11px", color: "var(--text-secondary)", marginTop: "4px" }}>
                {t("onboarding.vaultDesc", "Lưu trong SQLite / Vault quyền 0600 của Daemon, giữ nguyên qua các lần khởi động lại")}
              </div>
            </div>
            <div onClick={() => p.onChangeStorageMode("ram_only")} style={cardStyle(p.storageMode === "ram_only")}>
              <div style={{ fontSize: "12px", fontWeight: 600 }}>{t("onboarding.ramOnlyTitle", "RAM Only (Tạm thời)")}</div>
              <div style={{ fontSize: "11px", color: "var(--text-secondary)", marginTop: "4px" }}>
                {t("onboarding.ramOnlyDesc", "Chỉ giữ trong phiên làm việc hiện tại, tự hủy khi đóng tab")}
              </div>
            </div>
          </div>
        </div>
      </div>
    );
  }

  const permissionItems: Array<{ id: "auto_sandbox" | "ask_risky" | "ask_all"; title: string; desc: string }> = [
    {
      id: "auto_sandbox",
      title: t("onboarding.permAuto", "Tự động trong Sandbox (Khuyên dùng)"),
      desc: t("onboarding.permAutoDesc", "Tự động ghi các file preview, HTML/CSS, và snapshot trong phạm vi dự án an toàn."),
    },
    {
      id: "ask_risky",
      title: t("onboarding.permRisky", "Hỏi khi thao tác rủi ro cao"),
      desc: t("onboarding.permRiskyDesc", "Chỉ hiện hộp thoại xác nhận khi ghi đè mã nguồn gốc hoặc chạy lệnh terminal."),
    },
    {
      id: "ask_all",
      title: t("onboarding.permAll", "Luôn hỏi xác nhận (Nghiêm ngặt)"),
      desc: t("onboarding.permAllDesc", "Yêu cầu click Duyệt thủ công cho mọi thay đổi DAG, patch và code generation."),
    },
  ];

  return (
    <div>
      <h3 style={{ margin: 0, fontSize: "13.5px", fontWeight: 600 }}>
        {t("onboarding.step3Title", "3. Quyền hạn thực thi & Khởi chạy Workbench")}
      </h3>
      <p style={{ margin: "4px 0 16px 0", fontSize: "11.5px", color: "var(--text-secondary)" }}>
        {t("onboarding.step3Desc", "Chọn mức độ tự động phê duyệt khi Agent can thiệp vào mã nguồn và file hệ thống")}
      </p>
      <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
        {permissionItems.map((item) => {
          const isChecked = p.permissionLevel === item.id;
          return (
            <div
              key={item.id}
              onClick={() => p.onChangePermissionLevel(item.id)}
              style={{ ...cardStyle(isChecked), padding: "12px 14px", display: "flex", alignItems: "flex-start", gap: "12px" }}
            >
              <input
                type="radio"
                name="permissionLevel"
                checked={isChecked}
                onChange={() => p.onChangePermissionLevel(item.id)}
                style={{
                  marginTop: "2px",
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  lineHeight: 1,
                }}
              />
              <div>
                <div style={{ fontSize: "12px", fontWeight: 600 }}>{item.title}</div>
                <div style={{ fontSize: "11px", color: "var(--text-secondary)", marginTop: "2px" }}>{item.desc}</div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
