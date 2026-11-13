// @story 04.2
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CliDiagnosticItem } from "../../../lib/validations/workbench-settings";

interface OnboardingStepsContentProps {
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
}

const cardStyle = (active: boolean): React.CSSProperties => ({
  padding: "12px",
  borderRadius: "var(--radius-sm)",
  border: `1px solid ${active ? "var(--action-primary)" : "var(--border-default)"}`,
  backgroundColor: active ? "var(--bg-selected)" : "var(--bg-subtle)",
  cursor: "pointer",
});

export const OnboardingStepsContent: React.FC<OnboardingStepsContentProps> = (p) => {
  const { t } = useTranslation();

  if (p.currentStep === 1) {
    return (
      <div>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "14px" }}>
          <div>
            <h3 style={{ margin: 0, fontSize: "13.5px", fontWeight: 600 }}>
              {t("onboarding.step1Title", "1. CLI Doctor: Kiểm tra các Coding Agent trên máy")}
            </h3>
            <p style={{ margin: "4px 0 0 0", fontSize: "11.5px", color: "var(--text-secondary)" }}>
              {t("onboarding.step1Desc", "Quét tự động các CLI được hỗ trợ qua loopback daemon (tối đa 1.5s/probe)")}
            </p>
          </div>
          <button
            type="button"
            onClick={p.onRefreshDoctor}
            disabled={p.loadingDoctor}
            style={{
              padding: "6px 12px",
              borderRadius: "var(--radius-xs)",
              border: "1px solid var(--border-default)",
              backgroundColor: "transparent",
              color: "var(--text-primary)",
              fontSize: "11px",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: "6px",
            }}
          >
            <BrandIcon name="refresh-cw" size={11} color="currentColor" />
            <span>{p.loadingDoctor ? t("onboarding.scanning", "Đang quét...") : t("onboarding.rescan", "Quét lại")}</span>
          </button>
        </div>

        {p.loadingDoctor ? (
          <div style={{ textAlign: "center", padding: "40px 0", color: "var(--text-secondary)" }}>
            <p style={{ fontSize: "12px" }}>{t("onboarding.scanningLocal", "Đang quét hệ thống CLI cục bộ...")}</p>
          </div>
        ) : (
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
            {p.cliItems.map((cli) => {
              const isSelected = p.selectedCli === cli.id;
              return (
                <div
                  key={cli.id}
                  onClick={() => cli.installed && p.onSelectCli(cli.id)}
                  style={{
                    ...cardStyle(isSelected),
                    padding: "10px 12px",
                    cursor: cli.installed ? "pointer" : "default",
                    opacity: cli.installed ? 1 : 0.55,
                    transition: "all var(--duration-fast)",
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                    <span style={{ fontSize: "12px", fontWeight: 600 }}>{cli.name}</span>
                    <span
                      style={{
                        fontSize: "10px",
                        padding: "2px 6px",
                        borderRadius: "var(--radius-xs)",
                        backgroundColor: cli.installed ? "var(--state-success-soft)" : "var(--bg-hover)",
                        color: cli.installed ? "var(--state-success)" : "var(--text-secondary)",
                        fontWeight: 500,
                      }}
                    >
                      {cli.installed
                        ? cli.authenticated
                          ? t("onboarding.ready", "Sẵn sàng")
                          : t("onboarding.authNeeded", "Cần đăng nhập")
                        : t("onboarding.notInstalled", "Chưa cài")}
                    </span>
                  </div>
                  <div style={{ marginTop: "6px", fontSize: "10.5px", color: "var(--text-secondary)" }}>
                    <code>{cli.command}</code> {cli.version ? `• ${cli.version}` : ""}
                  </div>
                </div>
              );
            })}
          </div>
        )}
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
              <input type="radio" name="permissionLevel" checked={isChecked} onChange={() => p.onChangePermissionLevel(item.id)} style={{ marginTop: "2px" }} />
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
