// @story 04.1, 04.2
// @implements AC-03
import { useCallback, useEffect, useState } from "react";
import {
  getGlobalLocale,
  loadPersistedSettings,
  savePersistedSettings,
  setGlobalLocale,
  WORKBENCH_SETTINGS_STORAGE_KEY,
} from "../../../utils/workbench-settings-storage";
import type { ThemeMode } from "../components/WorkbenchTopNav";
import type { PersistedWorkbenchSettings } from "../types";

export { WORKBENCH_SETTINGS_STORAGE_KEY, loadPersistedSettings, savePersistedSettings };

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
