// @story 04.1, 04.2
// @implements AC-03
import { useCallback, useEffect, useState } from "react";
import { getGlobalLocale, setGlobalLocale } from "../../../hooks/useTranslation";
import type { ThemeMode } from "../components/WorkbenchTopNav";
import type { PersistedWorkbenchSettings } from "../types";

export const WORKBENCH_SETTINGS_STORAGE_KEY = "cowok:workbench:settings";

export function loadPersistedSettings(): Partial<PersistedWorkbenchSettings> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(WORKBENCH_SETTINGS_STORAGE_KEY);
    return raw ? (JSON.parse(raw) as Partial<PersistedWorkbenchSettings>) : {};
  } catch {
    return {};
  }
}

export function savePersistedSettings(patch: Partial<PersistedWorkbenchSettings>): void {
  if (typeof window === "undefined") return;
  try {
    const prev = loadPersistedSettings();
    const safeNext: Partial<PersistedWorkbenchSettings> = {
      theme: patch.theme ?? prev.theme ?? "warm",
      locale: patch.locale ?? prev.locale ?? "vi",
      fontSize: patch.fontSize ?? prev.fontSize ?? "13px",
      defaultModel: patch.defaultModel ?? prev.defaultModel ?? "claude-sonnet-4-6",
      byosEndpoint: patch.byosEndpoint ?? prev.byosEndpoint ?? "http://127.0.0.1:43110/v1",
      autoContextSlice: patch.autoContextSlice ?? prev.autoContextSlice ?? true,
    };
    window.localStorage.setItem(WORKBENCH_SETTINGS_STORAGE_KEY, JSON.stringify(safeNext));
  } catch {
    // Storage restricted in test sandbox
  }
}

export function useWorkbenchSettings() {
  const initial = loadPersistedSettings();
  const [themeMode, setThemeModeState] = useState<ThemeMode>(initial.theme || "warm");
  const [currentLocale, setCurrentLocaleState] = useState<string>(initial.locale || getGlobalLocale());
  const [currentFontSize, setCurrentFontSizeState] = useState(initial.fontSize || "13px");
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [settingsActiveTab, setSettingsActiveTab] = useState<"general" | "models" | "mcp" | "automations">("general");

  const setThemeMode = useCallback((t: ThemeMode) => { setThemeModeState(t); savePersistedSettings({ theme: t }); }, []);
  const setCurrentLocale = useCallback((l: string) => { setCurrentLocaleState(l); setGlobalLocale(l); savePersistedSettings({ locale: l }); }, []);
  const setCurrentFontSize = useCallback((s: string) => { setCurrentFontSizeState(s); savePersistedSettings({ fontSize: s }); }, []);
  const openSettings = useCallback((tab: "general" | "models" | "mcp" | "automations" = "general") => { setSettingsActiveTab(tab); setIsSettingsOpen(true); }, []);

  useEffect(() => {
    if (typeof document !== "undefined") {
      document.documentElement.setAttribute("data-theme", themeMode);
      document.documentElement.dataset.theme = themeMode;
    }
  }, [themeMode]);

  return {
    themeMode, setThemeMode, currentLocale, setCurrentLocale, currentFontSize, setCurrentFontSize,
    isSettingsOpen, setIsSettingsOpen, settingsActiveTab, setSettingsActiveTab, openSettings,
  };
}
