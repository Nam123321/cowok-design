// @story 04.3, 04.5
// @implements Clean-Room EV-001 & EV-002 (Shared BYOS Auto-Detection & Quota Sync Hook)
import { useCallback, useEffect, useMemo, useState } from "react";
import { apiWorkbenchSettings } from "../api/workbench-settings";
import type {
  CliDiagnosticItem,
  DoctorModelOption,
  ProviderUsageDto,
} from "../lib/validations/workbench-settings";

export interface DetectedProviderCatalogState {
  tools: CliDiagnosticItem[];
  readyTools: CliDiagnosticItem[];
  setupTools: CliDiagnosticItem[];
  moreTools: CliDiagnosticItem[];
  usageProviders: ProviderUsageDto[];
  checkedAtEpochMs: number | null;
  loading: boolean;
  error: string | null;
  probingId: string | null;
  refresh: (opts?: { active?: boolean; refreshUsage?: boolean }) => Promise<void>;
  probeTool: (cliId: string) => Promise<void>;
}

const BUILTIN_MODEL_MODELS: DoctorModelOption[] = [
  { id: "cowok-swarm", label: "Cowok Swarm · Multi-Agent (Antigravity 2.0)", providerId: "cowok", recommended: true },
  { id: "claude-37", label: "Claude Sonnet 4 / 3.7", providerId: "claude", recommended: false },
  { id: "gemini-25", label: "Gemini 2.5 Pro", providerId: "gemini", recommended: false },
  { id: "openai-gpt4o", label: "GPT-4o / o3-mini", providerId: "codex", recommended: false },
  { id: "deepseek-r1", label: "DeepSeek R1 (Ollama Local)", providerId: "ollama", recommended: false },
];

export function collectSelectableModels(
  readyTools: CliDiagnosticItem[],
  usageProviders: ProviderUsageDto[]
): Array<DoctorModelOption & { providerName: string; ready: boolean; accountLabel?: string | null }> {
  const seen = new Set<string>();
  const out: Array<DoctorModelOption & { providerName: string; ready: boolean; accountLabel?: string | null }> = [];
  const usageById = new Map(usageProviders.map((u) => [u.id, u]));

  for (const tool of readyTools) {
    const usage = usageById.get(tool.id);
    const list = usage?.models?.length ? usage.models : tool.models ?? [];
    for (const m of list) {
      if (seen.has(m.id)) continue;
      seen.add(m.id);
      out.push({
        ...m,
        providerName: tool.name,
        ready: true,
        accountLabel: tool.accountLabel ?? usage?.accountLabel ?? null,
      });
    }
  }
  for (const preset of BUILTIN_MODEL_MODELS) {
    if (seen.has(preset.id)) continue;
    seen.add(preset.id);
    out.push({
      ...preset,
      providerName: preset.providerId || "Cowok",
      ready: preset.id === "cowok-swarm",
      accountLabel: null,
    });
  }
  return out;
}

export function useDoctorProvidersUsage(autoLoad = true): DetectedProviderCatalogState {
  const [tools, setTools] = useState<CliDiagnosticItem[]>([]);
  const [usageProviders, setUsageProviders] = useState<ProviderUsageDto[]>([]);
  const [checkedAtEpochMs, setCheckedAtEpochMs] = useState<number | null>(null);
  const [loading, setLoading] = useState<boolean>(autoLoad);
  const [error, setError] = useState<string | null>(null);
  const [probingId, setProbingId] = useState<string | null>(null);

  const refresh = useCallback(async (opts?: { active?: boolean; refreshUsage?: boolean }) => {
    setLoading(true);
    setError(null);
    try {
      const [matrixRes, usageRes] = await Promise.all([
        apiWorkbenchSettings.getDoctorCliMatrix(opts?.active ? "active" : "passive"),
        apiWorkbenchSettings.getDoctorUsage(Boolean(opts?.refreshUsage)).catch(() => null),
      ]);
      setTools(matrixRes.clis || matrixRes.tools || []);
      if (usageRes) {
        setUsageProviders(usageRes.providers || []);
        setCheckedAtEpochMs(usageRes.checkedAtEpochMs);
      } else {
        setCheckedAtEpochMs(Date.now());
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "provider_scan_failed");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (autoLoad) void refresh({ active: false, refreshUsage: false });
  }, [autoLoad, refresh]);

  const probeTool = useCallback(async (cliId: string) => {
    setProbingId(cliId);
    try {
      const updated = await apiWorkbenchSettings.probeCliTool(cliId);
      setTools((prev) => prev.map((t) => (t.id === cliId ? { ...t, ...updated } : t)));
      const usageRes = await apiWorkbenchSettings.getDoctorUsage(true).catch(() => null);
      if (usageRes) {
        setUsageProviders(usageRes.providers || []);
        setCheckedAtEpochMs(usageRes.checkedAtEpochMs);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "cli_probe_failed");
    } finally {
      setProbingId(null);
    }
  }, []);

  const readyTools = useMemo(
    () => tools.filter((t) => t.status === "ready" || (t.installed && t.authenticated)),
    [tools]
  );
  const setupTools = useMemo(
    () => tools.filter((t) => t.installed && t.status !== "ready" && !t.authenticated),
    [tools]
  );
  const moreTools = useMemo(() => tools.filter((t) => !t.installed), [tools]);

  return {
    tools,
    readyTools,
    setupTools,
    moreTools,
    usageProviders,
    checkedAtEpochMs,
    loading,
    error,
    probingId,
    refresh,
    probeTool,
  };
}

export function useSyncedDefaultModel(
  fallback: { id: string; label: string },
  readPersisted: () => { defaultModel?: string },
  presets: readonly { id: string; label: string }[]
): [{ id: string; label: string }, (m: { id: string; label: string }) => void] {
  const resolveById = useCallback(
    (id?: string): { id: string; label: string } => {
      if (!id) return fallback;
      return presets.find((p) => p.id === id) ?? { id, label: id };
    },
    [fallback, presets]
  );
  const [selected, setSelected] = useState<{ id: string; label: string }>(() =>
    resolveById(readPersisted().defaultModel)
  );
  useEffect(() => {
    const sync = () => setSelected(resolveById(readPersisted().defaultModel));
    window.addEventListener("cowok:settings-changed", sync);
    window.addEventListener("storage", sync);
    return () => {
      window.removeEventListener("cowok:settings-changed", sync);
      window.removeEventListener("storage", sync);
    };
  }, [readPersisted, resolveById]);
  return [selected, setSelected];
}
