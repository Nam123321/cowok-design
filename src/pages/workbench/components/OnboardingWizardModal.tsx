// @story 04.2
// @implements AC-02 (Day-0 Onboarding Wizard & CLI Doctor)
import React, { useState, useEffect } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { CliDiagnosticItem } from "../../../app/api/doctor/route";

export interface OnboardingWizardModalProps {
  isOpen: boolean;
  onClose: () => void;
  onComplete: (config: OnboardingResult) => void;
}

export interface OnboardingResult {
  preferredCli: string;
  storageMode: "vault_0600" | "ram_only";
  permissionLevel: "auto_sandbox" | "ask_risky" | "ask_all";
  workspaceMode: "sandboxed" | "linked";
}

const STEP_COUNT = 3;

export const OnboardingWizardModal: React.FC<OnboardingWizardModalProps> = ({
  isOpen,
  onClose,
  onComplete,
}) => {
  const { t } = useTranslation();
  const [currentStep, setCurrentStep] = useState<number>(1);
  const [loadingDoctor, setLoadingDoctor] = useState<boolean>(true);
  const [cliItems, setCliItems] = useState<CliDiagnosticItem[]>([]);
  const [selectedCli, setSelectedCli] = useState<string>("agy");
  const [storageMode, setStorageMode] = useState<"vault_0600" | "ram_only">("vault_0600");
  const [permissionLevel, setPermissionLevel] = useState<"auto_sandbox" | "ask_risky" | "ask_all">("auto_sandbox");
  const [workspaceMode, setWorkspaceMode] = useState<"sandboxed" | "linked">("sandboxed");

  const runDoctorScan = async () => {
    setLoadingDoctor(true);
    try {
      const res = await fetch("/api/doctor");
      if (res.ok) {
        const data = await res.json();
        setCliItems(data.clis || []);
        const firstReady = data.clis?.find((c: CliDiagnosticItem) => c.installed);
        if (firstReady) {
          setSelectedCli(firstReady.id);
        }
      }
    } catch {
      // Fallback empty
    } finally {
      setLoadingDoctor(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      runDoctorScan();
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const handleFinish = () => {
    localStorage.setItem("cowok_first_run_completed", "true");
    localStorage.setItem("cowok_default_agent", selectedCli);
    localStorage.setItem("cowok_storage_mode", storageMode);
    localStorage.setItem("cowok_permission_level", permissionLevel);
    onComplete({
      preferredCli: selectedCli,
      storageMode,
      permissionLevel,
      workspaceMode,
    });
    onClose();
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0, 0, 0, 0.72)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 10000,
        backdropFilter: "blur(6px)",
      }}
    >
      <div
        style={{
          width: "620px",
          maxWidth: "94vw",
          maxHeight: "88vh",
          backgroundColor: "var(--bg-surface, #1e1e24)",
          borderRadius: "12px",
          border: "1px solid var(--border-default, #2e2e38)",
          boxShadow: "0 20px 50px rgba(0,0,0,0.5)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          color: "var(--text-primary, #e2e8f0)",
          fontFamily: "var(--font-sans, system-ui, -apple-system, sans-serif)",
        }}
      >
        {/* Header */}
        <div
          style={{
            padding: "18px 24px",
            borderBottom: "1px solid var(--border-default, #2e2e38)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            backgroundColor: "var(--bg-subtle, #18181d)",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <div
              style={{
                width: "28px",
                height: "28px",
                borderRadius: "6px",
                backgroundColor: "var(--action-primary, #3b82f6)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                color: "#fff",
              }}
            >
              <BrandIcon name="terminal" size={16} color="currentColor" />
            </div>
            <div>
              <h2 style={{ margin: 0, fontSize: "15px", fontWeight: 600 }}>
                {t("workbench.onboardingTitle", "Chào mừng đến Cowok-Design (Day-0 Setup)")}
              </h2>
              <p style={{ margin: "2px 0 0 0", fontSize: "11.5px", color: "var(--text-secondary, #94a3b8)" }}>
                {t("workbench.onboardingSubtitle", "Thiết lập môi trường siêu tốc trong <60 giây")}
              </p>
            </div>
          </div>
          {/* Step pill */}
          <div
            style={{
              padding: "4px 10px",
              borderRadius: "20px",
              backgroundColor: "rgba(59, 130, 246, 0.15)",
              color: "var(--action-primary, #3b82f6)",
              fontSize: "11px",
              fontWeight: 600,
            }}
          >
            Bước {currentStep} / {STEP_COUNT}
          </div>
        </div>

        {/* Body Step Content */}
        <div style={{ padding: "24px", overflowY: "auto", flex: 1, minHeight: "320px" }}>
          {currentStep === 1 && (
            <div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "14px" }}>
                <div>
                  <h3 style={{ margin: 0, fontSize: "13.5px", fontWeight: 600 }}>
                    1. CLI Doctor: Kiểm tra các Coding Agent trên máy
                  </h3>
                  <p style={{ margin: "4px 0 0 0", fontSize: "11.5px", color: "var(--text-secondary, #94a3b8)" }}>
                    Quét tự động các CLI được hỗ trợ qua loopback daemon (tối đa 1.5s/probe)
                  </p>
                </div>
                <button
                  type="button"
                  onClick={runDoctorScan}
                  disabled={loadingDoctor}
                  style={{
                    padding: "6px 12px",
                    borderRadius: "6px",
                    border: "1px solid var(--border-default, #2e2e38)",
                    backgroundColor: "transparent",
                    color: "var(--text-primary, #e2e8f0)",
                    fontSize: "11px",
                    cursor: "pointer",
                    display: "flex",
                    alignItems: "center",
                    gap: "6px",
                  }}
                >
                  <BrandIcon name="refresh-cw" size={11} color="currentColor" />
                  <span>{loadingDoctor ? "Đang quét..." : "Quét lại"}</span>
                </button>
              </div>

              {loadingDoctor ? (
                <div style={{ textAlign: "center", padding: "40px 0", color: "var(--text-secondary, #94a3b8)" }}>
                  <p style={{ fontSize: "12px" }}>Đang quét hệ thống CLI cục bộ...</p>
                </div>
              ) : (
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
                  {cliItems.map((cli) => {
                    const isSelected = selectedCli === cli.id;
                    return (
                      <div
                        key={cli.id}
                        onClick={() => cli.installed && setSelectedCli(cli.id)}
                        style={{
                          padding: "10px 12px",
                          borderRadius: "8px",
                          border: `1px solid ${
                            isSelected
                              ? "var(--action-primary, #3b82f6)"
                              : "var(--border-default, #2e2e38)"
                          }`,
                          backgroundColor: isSelected
                            ? "rgba(59, 130, 246, 0.08)"
                            : "var(--bg-subtle, #18181d)",
                          cursor: cli.installed ? "pointer" : "default",
                          opacity: cli.installed ? 1 : 0.55,
                          transition: "all var(--duration-fast, 0.15s)",
                        }}
                      >
                        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                          <span style={{ fontSize: "12px", fontWeight: 600 }}>{cli.name}</span>
                          <span
                            style={{
                              fontSize: "10px",
                              padding: "2px 6px",
                              borderRadius: "4px",
                              backgroundColor: cli.installed ? "rgba(34, 197, 94, 0.15)" : "rgba(148, 163, 184, 0.12)",
                              color: cli.installed ? "#4ade80" : "var(--text-secondary, #94a3b8)",
                              fontWeight: 500,
                            }}
                          >
                            {cli.installed ? "Sẵn sàng" : "Chưa cài"}
                          </span>
                        </div>
                        <div style={{ marginTop: "6px", fontSize: "10.5px", color: "var(--text-secondary, #94a3b8)" }}>
                          <code>{cli.command}</code> {cli.version ? `• ${cli.version}` : ""}
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          )}

          {currentStep === 2 && (
            <div>
              <h3 style={{ margin: 0, fontSize: "13.5px", fontWeight: 600 }}>
                2. Không gian làm việc & Cơ chế bảo mật lưu trữ
              </h3>
              <p style={{ margin: "4px 0 16px 0", fontSize: "11.5px", color: "var(--text-secondary, #94a3b8)" }}>
                Định cấu hình cách thức lưu trữ dữ liệu dự án và token bảo mật
              </p>

              <div style={{ marginBottom: "16px" }}>
                <label style={{ display: "block", fontSize: "11.5px", fontWeight: 600, marginBottom: "8px" }}>
                  Chế độ Workspace:
                </label>
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
                  <div
                    onClick={() => setWorkspaceMode("sandboxed")}
                    style={{
                      padding: "12px",
                      borderRadius: "8px",
                      border: `1px solid ${
                        workspaceMode === "sandboxed"
                          ? "var(--action-primary, #3b82f6)"
                          : "var(--border-default, #2e2e38)"
                      }`,
                      backgroundColor:
                        workspaceMode === "sandboxed"
                          ? "rgba(59, 130, 246, 0.08)"
                          : "var(--bg-subtle, #18181d)",
                      cursor: "pointer",
                    }}
                  >
                    <div style={{ fontSize: "12px", fontWeight: 600 }}>Sandbox Tự Quản Lý (Khuyên dùng)</div>
                    <div style={{ fontSize: "11px", color: "var(--text-secondary, #94a3b8)", marginTop: "4px" }}>
                      Tự động phân vùng isolated trong <code>.cowok-design/projects/</code> với HMAC token
                    </div>
                  </div>
                  <div
                    onClick={() => setWorkspaceMode("linked")}
                    style={{
                      padding: "12px",
                      borderRadius: "8px",
                      border: `1px solid ${
                        workspaceMode === "linked"
                          ? "var(--action-primary, #3b82f6)"
                          : "var(--border-default, #2e2e38)"
                      }`,
                      backgroundColor:
                        workspaceMode === "linked"
                          ? "rgba(59, 130, 246, 0.08)"
                          : "var(--bg-subtle, #18181d)",
                      cursor: "pointer",
                    }}
                  >
                    <div style={{ fontSize: "12px", fontWeight: 600 }}>Liên Kết Thư Mục Local</div>
                    <div style={{ fontSize: "11px", color: "var(--text-secondary, #94a3b8)", marginTop: "4px" }}>
                      Chọn một thư mục mã nguồn hiện có trên máy (hỗ trợ Git repo)
                    </div>
                  </div>
                </div>
              </div>

              <div>
                <label style={{ display: "block", fontSize: "11.5px", fontWeight: 600, marginBottom: "8px" }}>
                  Cơ chế lưu trữ Secret & API Keys:
                </label>
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
                  <div
                    onClick={() => setStorageMode("vault_0600")}
                    style={{
                      padding: "12px",
                      borderRadius: "8px",
                      border: `1px solid ${
                        storageMode === "vault_0600"
                          ? "var(--action-primary, #3b82f6)"
                          : "var(--border-default, #2e2e38)"
                      }`,
                      backgroundColor:
                        storageMode === "vault_0600"
                          ? "rgba(59, 130, 246, 0.08)"
                          : "var(--bg-subtle, #18181d)",
                      cursor: "pointer",
                    }}
                  >
                    <div style={{ fontSize: "12px", fontWeight: 600 }}>Daemon 0600 Vault (Bền vững)</div>
                    <div style={{ fontSize: "11px", color: "var(--text-secondary, #94a3b8)", marginTop: "4px" }}>
                      Lưu trong tệp mã hóa quyền 0600 của Daemon, giữ nguyên qua các lần khởi động lại
                    </div>
                  </div>
                  <div
                    onClick={() => setStorageMode("ram_only")}
                    style={{
                      padding: "12px",
                      borderRadius: "8px",
                      border: `1px solid ${
                        storageMode === "ram_only"
                          ? "var(--action-primary, #3b82f6)"
                          : "var(--border-default, #2e2e38)"
                      }`,
                      backgroundColor:
                        storageMode === "ram_only"
                          ? "rgba(59, 130, 246, 0.08)"
                          : "var(--bg-subtle, #18181d)",
                      cursor: "pointer",
                    }}
                  >
                    <div style={{ fontSize: "12px", fontWeight: 600 }}>RAM Only (Tạm thời)</div>
                    <div style={{ fontSize: "11px", color: "var(--text-secondary, #94a3b8)", marginTop: "4px" }}>
                      Chỉ lưu trên bộ nhớ phiên duyệt web, tự hủy khi đóng tab
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}

          {currentStep === 3 && (
            <div>
              <h3 style={{ margin: 0, fontSize: "13.5px", fontWeight: 600 }}>
                3. Quyền hạn thực thi & Khởi chạy Workbench
              </h3>
              <p style={{ margin: "4px 0 16px 0", fontSize: "11.5px", color: "var(--text-secondary, #94a3b8)" }}>
                Chọn mức độ tự động phê duyệt khi Agent can thiệp vào mã nguồn và file hệ thống
              </p>

              <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
                {[
                  {
                    id: "auto_sandbox",
                    title: "Tự động trong Sandbox (Khuyên dùng)",
                    desc: "Tự động ghi các file preview, HTML/CSS, và snapshot trong phạm vi dự án an toàn.",
                  },
                  {
                    id: "ask_risky",
                    title: "Hỏi khi thao tác rủi ro cao",
                    desc: "Chỉ hiện hộp thoại xác nhận khi ghi đè mã nguồn gốc hoặc chạy lệnh terminal.",
                  },
                  {
                    id: "ask_all",
                    title: "Luôn hỏi xác nhận (Nghiêm ngặt)",
                    desc: "Yêu cầu click Duyệt thủ công cho mọi thay đổi DAG, patch và code generation.",
                  },
                ].map((item) => {
                  const isChecked = permissionLevel === item.id;
                  return (
                    <div
                      key={item.id}
                      onClick={() => setPermissionLevel(item.id as any)}
                      style={{
                        padding: "12px 14px",
                        borderRadius: "8px",
                        border: `1px solid ${
                          isChecked ? "var(--action-primary, #3b82f6)" : "var(--border-default, #2e2e38)"
                        }`,
                        backgroundColor: isChecked ? "rgba(59, 130, 246, 0.08)" : "var(--bg-subtle, #18181d)",
                        cursor: "pointer",
                        display: "flex",
                        alignItems: "flex-start",
                        gap: "12px",
                      }}
                    >
                      <input
                        type="radio"
                        name="permissionLevel"
                        checked={isChecked}
                        onChange={() => setPermissionLevel(item.id as any)}
                        style={{ marginTop: "2px" }}
                      />
                      <div>
                        <div style={{ fontSize: "12px", fontWeight: 600 }}>{item.title}</div>
                        <div style={{ fontSize: "11px", color: "var(--text-secondary, #94a3b8)", marginTop: "2px" }}>
                          {item.desc}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>

        {/* Footer Navigation */}
        <div
          style={{
            padding: "14px 24px",
            borderTop: "1px solid var(--border-default, #2e2e38)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            backgroundColor: "var(--bg-subtle, #18181d)",
          }}
        >
          <button
            type="button"
            onClick={onClose}
            style={{
              padding: "7px 14px",
              borderRadius: "6px",
              border: "none",
              backgroundColor: "transparent",
              color: "var(--text-secondary, #94a3b8)",
              fontSize: "12px",
              cursor: "pointer",
            }}
          >
            Bỏ qua (Dùng mặc định)
          </button>

          <div style={{ display: "flex", gap: "8px" }}>
            {currentStep > 1 && (
              <button
                type="button"
                onClick={() => setCurrentStep((s) => s - 1)}
                style={{
                  padding: "7px 16px",
                  borderRadius: "6px",
                  border: "1px solid var(--border-default, #2e2e38)",
                  backgroundColor: "transparent",
                  color: "var(--text-primary, #e2e8f0)",
                  fontSize: "12px",
                  cursor: "pointer",
                }}
              >
                Quay lại
              </button>
            )}

            {currentStep < STEP_COUNT ? (
              <button
                type="button"
                onClick={() => setCurrentStep((s) => s + 1)}
                style={{
                  padding: "7px 18px",
                  borderRadius: "6px",
                  border: "none",
                  backgroundColor: "var(--action-primary, #3b82f6)",
                  color: "#fff",
                  fontSize: "12px",
                  fontWeight: 600,
                  cursor: "pointer",
                }}
              >
                Tiếp tục
              </button>
            ) : (
              <button
                type="button"
                onClick={handleFinish}
                style={{
                  padding: "7px 20px",
                  borderRadius: "6px",
                  border: "none",
                  backgroundColor: "var(--action-primary, #3b82f6)",
                  color: "#fff",
                  fontSize: "12px",
                  fontWeight: 600,
                  cursor: "pointer",
                  display: "flex",
                  alignItems: "center",
                  gap: "6px",
                }}
              >
                <BrandIcon name="play" size={12} color="#fff" />
                <span>Hoàn tất & Mở Canvas (&lt;60s)</span>
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
