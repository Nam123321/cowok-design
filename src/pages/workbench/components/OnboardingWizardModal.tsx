// @story 04.2
// @implements AC-02 (Day-0 Onboarding Wizard & CLI Doctor)
import React, { useState, useEffect } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { apiWorkbenchSettings } from "../../../api/workbench-settings";
import type { CliDiagnosticItem } from "../../../lib/validations/workbench-settings";
import { OnboardingStepsContent } from "./OnboardingStepsContent";

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
      const data = await apiWorkbenchSettings.scanDoctor();
      setCliItems(data.clis || []);
      const firstReady = data.clis?.find((c) => c.installed);
      if (firstReady) {
        setSelectedCli(firstReady.id);
      }
    } catch {
      // Fallback empty when daemon is starting
    } finally {
      setLoadingDoctor(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      void runDoctorScan();
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const handleFinish = () => {
    localStorage.setItem("cowok_first_run_completed", "true");
    localStorage.setItem("cowok_default_agent", selectedCli);
    localStorage.setItem("cowok_storage_mode", storageMode);
    localStorage.setItem("cowok_permission_level", permissionLevel);
    void apiWorkbenchSettings
      .upsertAgentSetting({
        providerId: selectedCli,
        modelId: selectedCli,
        endpointUrl: "",
        credentialRef: storageMode,
        status: permissionLevel,
        isDefault: true,
      })
      .catch(() => {
        // Optimistic persistence until daemon reconnects
      });
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
          backgroundColor: "var(--bg-surface)",
          borderRadius: "var(--radius-lg)",
          border: "1px solid var(--border-default)",
          boxShadow: "var(--shadow-lg)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          color: "var(--text-primary)",
          fontFamily: "var(--font-sans)",
        }}
      >
        {/* Header */}
        <div
          style={{
            padding: "18px 24px",
            borderBottom: "1px solid var(--border-default)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            backgroundColor: "var(--bg-subtle)",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <div
              style={{
                width: "28px",
                height: "28px",
                borderRadius: "var(--radius-xs)",
                backgroundColor: "var(--action-primary)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                color: "var(--action-primary-fg)",
              }}
            >
              <BrandIcon name="terminal" size={16} color="currentColor" />
            </div>
            <div>
              <h2 style={{ margin: 0, fontSize: "15px", fontWeight: 600 }}>
                {t("workbench.onboardingTitle", "Chào mừng đến Cowok-Design (Day-0 Setup)")}
              </h2>
              <p style={{ margin: "2px 0 0 0", fontSize: "11.5px", color: "var(--text-secondary)" }}>
                {t("workbench.onboardingSubtitle", "Thiết lập môi trường siêu tốc trong <60 giây")}
              </p>
            </div>
          </div>
          <div
            style={{
              padding: "4px 10px",
              borderRadius: "var(--radius-pill)",
              backgroundColor: "var(--bg-selected)",
              color: "var(--action-primary)",
              fontSize: "11px",
              fontWeight: 600,
            }}
          >
            {t("onboarding.stepLabel", "Bước")} {currentStep} / {STEP_COUNT}
          </div>
        </div>

        {/* Body Step Content */}
        <div style={{ padding: "24px", overflowY: "auto", flex: 1, minHeight: "320px" }}>
          <OnboardingStepsContent
            currentStep={currentStep}
            loadingDoctor={loadingDoctor}
            cliItems={cliItems}
            selectedCli={selectedCli}
            onSelectCli={setSelectedCli}
            onRefreshDoctor={() => void runDoctorScan()}
            workspaceMode={workspaceMode}
            onChangeWorkspaceMode={setWorkspaceMode}
            storageMode={storageMode}
            onChangeStorageMode={setStorageMode}
            permissionLevel={permissionLevel}
            onChangePermissionLevel={setPermissionLevel}
          />
        </div>

        {/* Footer Navigation */}
        <div
          style={{
            padding: "14px 24px",
            borderTop: "1px solid var(--border-default)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            backgroundColor: "var(--bg-subtle)",
          }}
        >
          <button
            type="button"
            onClick={onClose}
            style={{
              padding: "7px 14px",
              borderRadius: "var(--radius-xs)",
              border: "none",
              backgroundColor: "transparent",
              color: "var(--text-secondary)",
              fontSize: "12px",
              cursor: "pointer",
            }}
          >
            {t("onboarding.skipBtn", "Bỏ qua (Dùng mặc định)")}
          </button>

          <div style={{ display: "flex", gap: "8px" }}>
            {currentStep > 1 && (
              <button
                type="button"
                onClick={() => setCurrentStep((s) => s - 1)}
                style={{
                  padding: "7px 16px",
                  borderRadius: "var(--radius-xs)",
                  border: "1px solid var(--border-default)",
                  backgroundColor: "transparent",
                  color: "var(--text-primary)",
                  fontSize: "12px",
                  cursor: "pointer",
                }}
              >
                {t("onboarding.backBtn", "Quay lại")}
              </button>
            )}

            {currentStep < STEP_COUNT ? (
              <button
                type="button"
                onClick={() => setCurrentStep((s) => s + 1)}
                style={{
                  padding: "7px 18px",
                  borderRadius: "var(--radius-xs)",
                  border: "none",
                  backgroundColor: "var(--action-primary)",
                  color: "var(--action-primary-fg)",
                  fontSize: "12px",
                  fontWeight: 600,
                  cursor: "pointer",
                }}
              >
                {t("onboarding.nextBtn", "Tiếp tục")}
              </button>
            ) : (
              <button
                type="button"
                onClick={handleFinish}
                style={{
                  padding: "7px 20px",
                  borderRadius: "var(--radius-xs)",
                  border: "none",
                  backgroundColor: "var(--action-primary)",
                  color: "var(--action-primary-fg)",
                  fontSize: "12px",
                  fontWeight: 600,
                  cursor: "pointer",
                  display: "flex",
                  alignItems: "center",
                  gap: "6px",
                }}
              >
                <BrandIcon name="play" size={12} color="currentColor" />
                <span>{t("onboarding.finishBtn", "Hoàn tất & Mở Canvas (<60s)")}</span>
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
