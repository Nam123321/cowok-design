// @story 04.1, 04.2
// @implements AC-03 (Shared Workbench Settings Storage Helper — FSD Lower Layer)
import { getGlobalLocale, setGlobalLocale } from "../hooks/useTranslation";

export type WorkbenchThemeMode = "warm" | "dim" | "night" | "night-low" | "monochrome";

export interface PersistedWorkbenchSettingsData {
  theme: WorkbenchThemeMode;
  locale: string;
  fontSize: string;
  defaultModel: string;
  byosEndpoint: string;
  autoContextSlice: boolean;
}

export const WORKBENCH_SETTINGS_STORAGE_KEY = "cowok:workbench:settings";

export function loadPersistedSettings(): Partial<PersistedWorkbenchSettingsData> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(WORKBENCH_SETTINGS_STORAGE_KEY);
    return raw ? (JSON.parse(raw) as Partial<PersistedWorkbenchSettingsData>) : {};
  } catch {
    return {};
  }
}

export function savePersistedSettings(patch: Partial<PersistedWorkbenchSettingsData>): void {
  if (typeof window === "undefined") return;
  try {
    const prev = loadPersistedSettings();
    const defaultByos =
      (typeof import.meta !== "undefined" && import.meta.env?.VITE_DEFAULT_BYOS_ENDPOINT) || "";
    const safeNext: Partial<PersistedWorkbenchSettingsData> = {
      theme: patch.theme ?? prev.theme ?? "warm",
      locale: patch.locale ?? prev.locale ?? "vi",
      fontSize: patch.fontSize ?? prev.fontSize ?? "13px",
      defaultModel: patch.defaultModel ?? prev.defaultModel ?? "cowok-swarm",
      byosEndpoint: patch.byosEndpoint ?? prev.byosEndpoint ?? defaultByos,
      autoContextSlice: patch.autoContextSlice ?? prev.autoContextSlice ?? true,
    };
    window.localStorage.setItem(WORKBENCH_SETTINGS_STORAGE_KEY, JSON.stringify(safeNext));
    window.dispatchEvent(new CustomEvent("cowok:settings-changed", { detail: safeNext }));
  } catch {
    // Storage restricted in test sandbox
  }
}

export { getGlobalLocale, setGlobalLocale };
