// @story 04.1, 08.4
import { useEffect, useState } from "react";
import { apiWorkbenchSettings } from "../../../api/workbench-settings";
import { useTranslation } from "../../../hooks/useTranslation";
import type { McpServerDto } from "../../../lib/validations/workbench-settings";
import { loadPersistedSettings } from "./useWorkbenchSessionSettings";

export type SettingsTab = "general" | "models" | "mcp" | "automations";

function applyDefaultAgentSettingRow(
  rows: Awaited<ReturnType<typeof apiWorkbenchSettings.listAgentSettings>>,
  setters: {
    setDefaultModel: (v: string) => void;
    setByosEndpoint: (v: string) => void;
    setVaultMode: (v: string) => void;
    setPermissionMode: (v: string) => void;
    setOnboardingDone: (v: boolean) => void;
  }
) {
  if (rows.length === 0) return;
  const def = rows.find((r) => r.isDefault) || rows[0];
  if (def.modelId) setters.setDefaultModel(def.modelId);
  if (def.endpointUrl) setters.setByosEndpoint(def.endpointUrl);
  const nextVault = def.vaultMode || def.credentialRef;
  if (nextVault) setters.setVaultMode(nextVault);
  const nextPerm = def.permissionMode || def.status;
  if (nextPerm) setters.setPermissionMode(nextPerm);
  if (typeof def.onboardingDone === "boolean") setters.setOnboardingDone(def.onboardingDone);
}

function useSettingsModalSyncEffect(params: {
  isOpen: boolean;
  onClose: () => void;
  setSyncWarning: (v: string | null) => void;
  setDefaultModel: (v: string) => void;
  setByosEndpoint: (v: string) => void;
  setVaultMode: (v: string) => void;
  setPermissionMode: (v: string) => void;
  setOnboardingDone: (v: boolean) => void;
}) {
  const { isOpen, onClose, setSyncWarning, setDefaultModel, setByosEndpoint, setVaultMode, setPermissionMode, setOnboardingDone } = params;
  useEffect(() => {
    if (!isOpen) return;
    let active = true;
    void apiWorkbenchSettings
      .listAgentSettings()
      .then((rows) => {
        if (!active) return;
        setSyncWarning(null);
        applyDefaultAgentSettingRow(rows, { setDefaultModel, setByosEndpoint, setVaultMode, setPermissionMode, setOnboardingDone });
      })
      .catch((err: unknown) => {
        if (!active) return;
        setSyncWarning(err instanceof Error ? err.message : "daemon_sync_failed");
      });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      active = false;
      window.removeEventListener("keydown", onKey);
    };
  }, [isOpen, onClose, setSyncWarning, setDefaultModel, setByosEndpoint, setVaultMode, setPermissionMode, setOnboardingDone]);
}

export function useSettingsDialogState(isOpen: boolean, onClose: () => void, activeTab: SettingsTab) {
  const initial = loadPersistedSettings();
  const [selectedTab, setSelectedTab] = useState<SettingsTab>(activeTab);
  const [defaultModel, setDefaultModel] = useState(initial.defaultModel || "cowok-swarm");
  const [byosEndpoint, setByosEndpoint] = useState(initial.byosEndpoint || "");
  const [vaultMode, setVaultMode] = useState<string>("vault_0600");
  const [permissionMode, setPermissionMode] = useState<string>("auto_sandbox");
  const [onboardingDone, setOnboardingDone] = useState<boolean>(false);
  const [ephemeralKeys, setEphemeralKeys] = useState({
    gemini: "",
    claude: "",
    openai: "",
    openrouter: "",
    deepseek: "",
    byosToken: "",
  });
  const [showKeys, setShowKeys] = useState(false);
  const [savedFeedback, setSavedFeedback] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [syncWarning, setSyncWarning] = useState<string | null>(null);

  useEffect(() => {
    if (activeTab) setSelectedTab(activeTab);
  }, [activeTab]);

  useSettingsModalSyncEffect({
    isOpen,
    onClose,
    setSyncWarning,
    setDefaultModel,
    setByosEndpoint,
    setVaultMode,
    setPermissionMode,
    setOnboardingDone,
  });

  return {
    selectedTab, setSelectedTab, defaultModel, setDefaultModel, byosEndpoint, setByosEndpoint,
    vaultMode, setVaultMode, permissionMode, setPermissionMode, onboardingDone, setOnboardingDone,
    ephemeralKeys, setEphemeralKeys, showKeys, setShowKeys, savedFeedback, setSavedFeedback,
    isSaving, setIsSaving, syncWarning, setSyncWarning,
  };
}

export function useMcpServersState() {
  const { t } = useTranslation();
  const [servers, setServers] = useState<McpServerDto[]>([]);
  const [mcpError, setMcpError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void apiWorkbenchSettings
      .listMcpServers()
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
    return () => {
      active = false;
    };
  }, [t]);

  return { servers, setServers, mcpError, setMcpError };
}

export function useAutomationsHealthState(selectedTab: "mcp" | "automations") {
  const [daemonHealth, setDaemonHealth] = useState<"checking" | "online" | "offline">("checking");

  useEffect(() => {
    if (selectedTab !== "automations") return;
    let active = true;
    void apiWorkbenchSettings
      .getDoctorCliMatrix("passive")
      .then(() => {
        if (active) setDaemonHealth("online");
      })
      .catch(() => {
        if (active) setDaemonHealth("offline");
      });
    return () => {
      active = false;
    };
  }, [selectedTab]);

  return daemonHealth;
}
