// @story 11.2
// @implements AC-03, AC-04 (State & Actions Controller Hook for Open MCP Host Installer, 4-Tier Permission Matrix & Live External Activity Audit)
import { useCallback, useEffect, useRef, useState } from "react";
import { apiMcpGateway } from "../../api/mcp-gateway";
import { useTranslation } from "../../hooks/useTranslation";
import type {
  McpActivityStatus,
  McpExternalActivityData,
  McpExternalActivityDto,
  McpHostId,
  McpHostStatusDto,
  McpToolPolicyDto,
} from "../../lib/validations/mcp-gateway";

type TranslateFn = (key: string, defaultVal?: string) => string;

export interface McpGatewayControllerState {
  hosts: McpHostStatusDto[];
  policies: McpToolPolicyDto[];
  activities: McpExternalActivityDto[];
  serverUrl: string;
  loading: boolean;
  error: string | null;
  feedback: string | null;
  busyHostId: McpHostId | null;
  busyToolName: string | null;
  statusFilter: McpActivityStatus | "ALL";
  setStatusFilter: (next: McpActivityStatus | "ALL") => void;
  refresh: () => Promise<void>;
  installHost: (hostId: McpHostId) => Promise<void>;
  rollbackHost: (hostId: McpHostId, backupId?: string) => Promise<void>;
  toggleTool: (policy: McpToolPolicyDto, nextEnabled: boolean) => Promise<void>;
}

const DEFAULT_ACTIVITY_LIMIT = 50;
const SHA_FEEDBACK_LEN = 10;

function formatErr(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback;
}

function interpolate(template: string, vars: Record<string, string>): string {
  return Object.entries(vars).reduce(
    (acc, [key, value]) => acc.replace(`{{${key}}}`, value),
    template
  );
}

interface ActionDeps {
  refresh: () => Promise<void>;
  setError: (v: string | null) => void;
  setFeedback: (v: string | null) => void;
  t: TranslateFn;
}

function useInstallHostAction(deps: ActionDeps, setBusyHostId: (v: McpHostId | null) => void) {
  const { refresh, setError, setFeedback, t } = deps;
  return useCallback(
    async (hostId: McpHostId) => {
      setBusyHostId(hostId);
      setError(null);
      setFeedback(null);
      try {
        const res = await apiMcpGateway.installOrRollbackHost({ hostId, action: "install" });
        setFeedback(
          interpolate(t("mcpGateway.installSuccess", "Installed into {{host}} (backup {{sha}})"), {
            host: res.displayName,
            sha: res.beforeSha256.slice(0, SHA_FEEDBACK_LEN),
          })
        );
        await refresh();
      } catch (err) {
        const fallback = interpolate(
          t("mcpGateway.installFailed", "Failed to install MCP config for {{host}}"),
          { host: hostId }
        );
        setError(formatErr(err, fallback));
      } finally {
        setBusyHostId(null);
      }
    },
    [refresh, setBusyHostId, setError, setFeedback, t]
  );
}

function useRollbackHostAction(deps: ActionDeps, setBusyHostId: (v: McpHostId | null) => void) {
  const { refresh, setError, setFeedback, t } = deps;
  return useCallback(
    async (hostId: McpHostId, backupId?: string) => {
      setBusyHostId(hostId);
      setError(null);
      setFeedback(null);
      try {
        const res = await apiMcpGateway.installOrRollbackHost({
          hostId,
          action: "rollback",
          ...(backupId ? { backupId } : {}),
        });
        setFeedback(
          interpolate(t("mcpGateway.rollbackSuccess", "Rolled back {{host}} config cleanly"), {
            host: res.displayName,
          })
        );
        await refresh();
      } catch (err) {
        const fallback = interpolate(
          t("mcpGateway.rollbackFailed", "Failed to roll back MCP config for {{host}}"),
          { host: hostId }
        );
        setError(formatErr(err, fallback));
      } finally {
        setBusyHostId(null);
      }
    },
    [refresh, setBusyHostId, setError, setFeedback, t]
  );
}

function useToolToggleAction(deps: ActionDeps) {
  const { refresh, setError, setFeedback, t } = deps;
  const [busyToolName, setBusyToolName] = useState<string | null>(null);

  const toggleTool = useCallback(
    async (policy: McpToolPolicyDto, nextEnabled: boolean) => {
      if (policy.lockedHumanOnly || policy.permissionTier === "HUMAN_ONLY") {
        setError(
          t(
            "mcpGateway.humanOnlyToggleDenied",
            "HUMAN_ONLY tools cannot be enabled for external MCP clients (EC-LEARN-001)"
          )
        );
        return;
      }
      setBusyToolName(policy.toolName);
      setError(null);
      setFeedback(null);
      try {
        await apiMcpGateway.toggleToolPolicy({
          toolName: policy.toolName,
          enabled: nextEnabled,
          mcpCallable: nextEnabled,
        });
        const stateLabel = nextEnabled
          ? t("mcpGateway.toolEnabled", "Enabled")
          : t("mcpGateway.toolDisabled", "Disabled");
        setFeedback(
          interpolate(t("mcpGateway.toggleToolSuccess", "Updated {{tool}}: {{state}}"), {
            tool: policy.toolName,
            state: stateLabel,
          })
        );
        await refresh();
      } catch (err) {
        const fallback = interpolate(t("mcpGateway.toggleToolFailed", "Failed to toggle {{tool}}"), {
          tool: policy.toolName,
        });
        setError(formatErr(err, fallback));
      } finally {
        setBusyToolName(null);
      }
    },
    [refresh, setError, setFeedback, t]
  );

  return { busyToolName, toggleTool };
}

function useActivityLoader(
  enabled: boolean,
  statusFilter: McpActivityStatus | "ALL",
  setError: (v: string | null) => void,
  t: TranslateFn
) {
  const [data, setData] = useState<McpExternalActivityData | null>(null);
  const [loading, setLoading] = useState(false);
  const requestSeqRef = useRef(0);

  const refresh = useCallback(async () => {
    const seq = ++requestSeqRef.current;
    setLoading(true);
    setError(null);
    try {
      const next = await apiMcpGateway.getExternalActivity({
        limit: DEFAULT_ACTIVITY_LIMIT,
        ...(statusFilter !== "ALL" ? { status: statusFilter } : {}),
      });
      if (seq !== requestSeqRef.current) return;
      setData(next);
    } catch (err) {
      if (seq !== requestSeqRef.current) return;
      setError(formatErr(err, t("mcpGateway.loadActivityFailed", "Failed to load MCP gateway activity")));
    } finally {
      if (seq === requestSeqRef.current) setLoading(false);
    }
  }, [setError, statusFilter, t]);

  useEffect(() => {
    if (!enabled) return;
    void refresh();
  }, [enabled, refresh]);

  return { data, loading, refresh };
}

export function useMcpGatewayController(enabled: boolean): McpGatewayControllerState {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<McpActivityStatus | "ALL">("ALL");
  const [busyHostId, setBusyHostId] = useState<McpHostId | null>(null);
  const { data, loading, refresh } = useActivityLoader(enabled, statusFilter, setError, t);
  const deps: ActionDeps = { refresh, setError, setFeedback, t };
  const installHost = useInstallHostAction(deps, setBusyHostId);
  const rollbackHost = useRollbackHostAction(deps, setBusyHostId);
  const { busyToolName, toggleTool } = useToolToggleAction(deps);

  return {
    hosts: data?.hosts ?? [],
    policies: data?.policies ?? [],
    activities: data?.activities ?? [],
    serverUrl: data?.serverUrl ?? "",
    loading,
    error,
    feedback,
    busyHostId,
    busyToolName,
    statusFilter,
    setStatusFilter,
    refresh,
    installHost,
    rollbackHost,
    toggleTool,
  };
}
