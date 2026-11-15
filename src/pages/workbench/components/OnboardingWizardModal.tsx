// @story 04.2, 04.3
// @implements Gate H6 (Prioritize BYOS CLI Doctor BEFORE BYOK, Auto-Connect Zero-Click UX)
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
  onDaemonSyncWarning?: (message: string) => void;
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
  onDaemonSyncWarning,
}) => {
  const { t } = useTranslation();
  const [currentStep, setCurrentStep] = useState<number>(1);
  const [loadingDoctor, setLoadingDoctor] = useState<boolean>(true);
  const [doctorError, setDoctorError] = useState<string | null>(null);
  const [cliItems, setCliItems] = useState<CliDiagnosticItem[]>([]);
  const [selectedCli, setSelectedCli] = useState<string>("agy");
  const [storageMode, setStorageMode] = useState<"vault_0600" | "ram_only">("vault_0600");
  const [permissionLevel, setPermissionLevel] = useState<"auto_sandbox" | "ask_risky" | "ask_all">("auto_sandbox");
  const [workspaceMode, setWorkspaceMode] = useState<"sandboxed" | "linked">("sandboxed");
  const [isFinishing, setIsFinishing] = useState<boolean>(false);

  // Step 1 BYOS & BYOK enhancements
  const [autoConnectedCliIds, setAutoConnectedCliIds] = useState<string[]>([]);
  const [probingCliId, setProbingCliId] = useState<string | null>(null);
  const [byokProvider, setByokProvider] = useState<string>("openai");
  const [byokApiKey, setByokApiKey] = useState<string>("");
  const [isVerifyingByok, setIsVerifyingByok] = useState<boolean>(false);
  const [byokVerifyStatus, setByokVerifyStatus] = useState<{ valid: boolean; message: string } | null>(null);
  const [, setIsStep1Connected] = useState<boolean>(false);

  const runDoctorScan = async () => {
    setLoadingDoctor(true);
    setDoctorError(null);
    try {
      const data = await apiWorkbenchSettings.scanDoctor();
      const items = data.clis || data.tools || [];
      setCliItems(items);

      // Auto-detect & auto-connect ready local BYOS subscriptions (Zero-Click UX Pattern)
      const readyTools = items.filter((c) => c.installed && (c.authenticated || c.status === "ready"));
      const readyIds = readyTools.map((c) => c.id);
      setAutoConnectedCliIds(readyIds);

      if (readyIds.length > 0) {
        setIsStep1Connected(true);
        setSelectedCli(readyIds[0]);
      } else {
        const firstInstalled = items.find((c) => c.installed);
        if (firstInstalled) {
          setSelectedCli(firstInstalled.id);
        }
      }
    } catch (err) {
      const detail = err instanceof Error ? `: ${err.message}` : "";
      setDoctorError(`${t("onboarding.doctorError", "Không thể quét trạng thái CLI từ daemon")}${detail}`);
    } finally {
      setLoadingDoctor(false);
    }
  };

  const handleProbeCli = async (cliId: string) => {
    setProbingCliId(cliId);
    try {
      const updated = await apiWorkbenchSettings.probeCliTool(cliId);
      setCliItems((prev) =>
        prev.map((item) => (item.id === cliId ? { ...item, ...updated } : item))
      );
      if (updated.status === "ready" || updated.authenticated) {
        setAutoConnectedCliIds((prev) => Array.from(new Set([...prev, cliId])));
        setIsStep1Connected(true);
      }
    } catch (err) {
      const detail = err instanceof Error ? `: ${err.message}` : "";
      setDoctorError(`${t("onboarding.probeError", "Không thể kiểm tra CLI trên daemon")}${detail}`);
    } finally {
      setProbingCliId(null);
    }
  };

  const handleVerifyByok = async () => {
    if (!byokApiKey.trim()) return;
    setIsVerifyingByok(true);
    setByokVerifyStatus(null);
    try {
      const res = await apiWorkbenchSettings.verifyProviderKey({
        providerId: byokProvider,
        apiKey: byokApiKey.trim(),
        endpointUrl: "",
      });
      setByokVerifyStatus({
        valid: res.valid,
        message:
          res.message ||
          (res.valid
            ? t("onboarding.keyValid", "API Key hợp lệ!")
            : t("onboarding.keyInvalid", "API Key không hợp lệ")),
      });
      if (res.valid) {
        setIsStep1Connected(true);
        setSelectedCli(byokProvider);
      }
    } catch (err) {
      setByokVerifyStatus({
        valid: false,
        message: err instanceof Error ? err.message : t("onboarding.verifyError", "Lỗi xác thực"),
      });
    } finally {
      setIsVerifyingByok(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      void runDoctorScan();
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const canonicalModelMap: Record<string, string> = {
    claude: "claude-37",
    anthropic: "claude-37",
    gemini: "gemini-25",
    agy: "gemini-25",
    codex: "openai-gpt4o",
    openai: "openai-gpt4o",
    deepseek: "deepseek-r1",
    ollama: "deepseek-r1",
    openrouter: "byos-custom",
    byos: "byos-custom",
  };

  const handleFinish = async () => {
    if (isFinishing) return;
    setIsFinishing(true);
    setDoctorError(null);
    try {
      await apiWorkbenchSettings.updateVaultConfig({ vaultMode: storageMode });

      let resolvedCredentialRef = `vault://${storageMode}/${selectedCli}`;
      const trimmedKey = byokApiKey.trim();
      if (trimmedKey) {
        const storedKey = await apiWorkbenchSettings.upsertVaultKey({
          providerId: byokProvider,
          apiKey: trimmedKey,
          vaultMode: storageMode,
        });
        if (storedKey.vaultRef) {
          resolvedCredentialRef = storedKey.vaultRef;
        }
      }

      const canonicalModelId = canonicalModelMap[selectedCli] || "cowok-swarm";
      await apiWorkbenchSettings.upsertAgentSetting({
        providerId: selectedCli,
        modelId: canonicalModelId,
        endpointUrl: "",
        credentialRef: resolvedCredentialRef,
        status: permissionLevel,
        permissionMode: permissionLevel,
        vaultMode: storageMode,
        onboardingDone: true,
        isDefault: true,
      });
      onComplete({
        preferredCli: selectedCli,
        storageMode,
        permissionLevel,
        workspaceMode,
      });
      onClose();
    } catch (err) {
      const detail = err instanceof Error ? `: ${err.message}` : "";
      const msg = `${t("settings.saveDaemonWarning", "Không thể lưu cấu hình lên daemon SQLite V7")}${detail}`;
      setDoctorError(msg);
      onDaemonSyncWarning?.(msg);
    } finally {
      setIsFinishing(false);
    }
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "var(--bg-overlay-strong)",
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
        {/* Centered Welcome Header Block */}
        <div
          style={{
            padding: "20px 24px",
            borderBottom: "1px solid var(--border-default)",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            textAlign: "center",
            position: "relative",
            backgroundColor: "var(--bg-subtle)",
            gap: "8px",
          }}
        >
          <div
            style={{
              position: "absolute",
              top: "16px",
              right: "20px",
              padding: "4px 10px",
              borderRadius: "var(--radius-pill)",
              backgroundColor: "var(--bg-selected)",
              color: "var(--action-primary)",
              fontSize: "11px",
              fontWeight: 600,
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              lineHeight: 1,
            }}
          >
            {t("onboarding.stepLabel", "Bước")} {currentStep} / {STEP_COUNT}
          </div>

          <div
            style={{
              width: "36px",
              height: "36px",
              borderRadius: "var(--radius-xs)",
              backgroundColor: "var(--action-primary)",
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              color: "var(--action-primary-fg)",
            }}
          >
            <BrandIcon name="terminal" size={18} color="currentColor" />
          </div>

          <div style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", textAlign: "center" }}>
            <h2 style={{ margin: 0, fontSize: "15px", fontWeight: 600, textAlign: "center", lineHeight: "1.3" }}>
              {t("workbench.onboardingTitle", "Chào mừng đến Cowok-Design (Day-0 Setup)")}
            </h2>
            <p style={{ margin: "4px 0 0 0", fontSize: "11.5px", color: "var(--text-secondary)", textAlign: "center" }}>
              {t("workbench.onboardingSubtitle", "Thiết lập môi trường siêu tốc trong <60 giây")}
            </p>
          </div>
        </div>

        {/* Body Step Content */}
        <div style={{ padding: "24px", overflowY: "auto", flex: 1, minHeight: "320px" }}>
          {doctorError && (
            <div
              role="alert"
              data-testid="onboarding-daemon-warning"
              style={{
                marginBottom: "12px",
                padding: "8px 12px",
                borderRadius: "var(--radius-xs)",
                backgroundColor: "var(--bg-subtle)",
                border: "1px solid var(--state-warning)",
                color: "var(--state-warning)",
                fontSize: "11.5px",
              }}
            >
              {doctorError}
            </div>
          )}
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
            autoConnectedCliIds={autoConnectedCliIds}
            probingCliId={probingCliId}
            onProbeCli={handleProbeCli}
            byokProvider={byokProvider}
            onChangeByokProvider={setByokProvider}
            byokApiKey={byokApiKey}
            onChangeByokApiKey={setByokApiKey}
            isVerifyingByok={isVerifyingByok}
            onVerifyByok={() => void handleVerifyByok()}
            byokVerifyStatus={byokVerifyStatus}
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
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              lineHeight: 1,
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
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  lineHeight: 1,
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
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  lineHeight: 1,
                }}
              >
                {t("onboarding.nextBtn", "Tiếp tục")}
              </button>
            ) : (
              <button
                type="button"
                disabled={isFinishing}
                onClick={() => void handleFinish()}
                style={{
                  padding: "7px 20px",
                  borderRadius: "var(--radius-xs)",
                  border: "none",
                  backgroundColor: "var(--action-primary)",
                  color: "var(--action-primary-fg)",
                  fontSize: "12px",
                  fontWeight: 600,
                  cursor: isFinishing ? "wait" : "pointer",
                  opacity: isFinishing ? 0.7 : 1,
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  lineHeight: 1,
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
