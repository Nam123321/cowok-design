// @story 08.4
// @implements AC-01, AC-02, AC-04, AC-05
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
} from "react";
import { pluginCenterClient } from "../../plugin-center/client";
import type { PluginLifecycleTarget, PluginSummaryDto } from "../../../lib/validations/plugins";
import type {
  CronScheduleStatusDto,
  KnowledgeCardDto,
} from "../../../lib/validations/plugins-learn-cron";

export type PluginActionScope = "plugin" | "learn" | "cron";

export interface PluginRegistryState {
  plugins: PluginSummaryDto[];
  learnCards: KnowledgeCardDto[];
  schedulesByPlugin: Record<string, CronScheduleStatusDto | null>;
  loading: boolean;
  error: string | null;
  pluginError: string | null;
  learnError: string | null;
  cronError: string | null;
  actionPendingId: string | null;
  actionFeedback: string | null;
}

const MIN_RATIONALE_LENGTH = 4;
const SSE_RECONNECT_BASE_MS = 1000;
const SSE_RECONNECT_MAX_MS = 15_000;
const MAX_SSE_BACKOFF_EXPONENT = 5;

function toErrMessage(err: unknown, fallback = "Operation failed"): string {
  return err instanceof Error ? err.message : fallback;
}

function isNotFoundError(err: unknown): boolean {
  const e = err as { status?: number; code?: string; message?: string } | null;
  if (e?.status === 404 || e?.code === "NOT_FOUND") return true;
  return err instanceof Error && /\b404\b|NOT_FOUND/i.test(err.message);
}

async function loadSchedulesForPlugins(
  plugins: PluginSummaryDto[]
): Promise<{ schedulesByPlugin: Record<string, CronScheduleStatusDto | null>; scheduleError: string | null }> {
  const schedulesByPlugin: Record<string, CronScheduleStatusDto | null> = {};
  const persistedPlugins = plugins.filter((p) => p.sourceType !== "builtin" || Boolean(p.upstreamRepoUrl));
  const results = await Promise.allSettled(
    persistedPlugins.map(async (p) => {
      try {
        return { pluginId: p.id, schedule: await pluginCenterClient.getPluginSchedule(p.id, p.projectId) };
      } catch (err: unknown) {
        if (isNotFoundError(err)) {
          return { pluginId: p.id, schedule: null };
        }
        throw err;
      }
    })
  );
  let scheduleError: string | null = null;
  for (let i = 0; i < results.length; i += 1) {
    const res = results[i];
    if (res.status === "fulfilled") {
      schedulesByPlugin[res.value.pluginId] = res.value.schedule;
    } else if (!scheduleError) {
      scheduleError = toErrMessage(res.reason, `Failed to load schedule for ${persistedPlugins[i]?.pluginSlug || "plugin"}`);
    }
  }
  return { schedulesByPlugin, scheduleError };
}

async function fetchPluginRegistrySnapshot(projectId: string, scope: "mcp" | "automations") {
  const [pluginsRes, cardsRes] = await Promise.allSettled([
    pluginCenterClient.getPluginRegistry({ projectId }),
    scope === "mcp" ? pluginCenterClient.listLearnCards(projectId) : Promise.resolve(null),
  ]);
  const plugins = pluginsRes.status === "fulfilled" ? pluginsRes.value : [];
  const learnCards = cardsRes.status === "fulfilled" ? cardsRes.value : null;
  const pluginError = pluginsRes.status === "rejected" ? toErrMessage(pluginsRes.reason, "Failed to load plugins") : null;
  const learnError = cardsRes.status === "rejected" ? toErrMessage(cardsRes.reason, "Failed to load LEARN cards") : null;
  let schedulesByPlugin: Record<string, CronScheduleStatusDto | null> | null = null;
  let cronError: string | null = pluginError;
  if (scope === "automations" && pluginsRes.status === "fulfilled") {
    const schedResult = await loadSchedulesForPlugins(plugins);
    schedulesByPlugin = schedResult.schedulesByPlugin;
    cronError = schedResult.scheduleError;
  }
  const error = pluginError ?? learnError ?? (scope === "automations" ? cronError : null);
  return { plugins, learnCards, schedulesByPlugin, error, pluginError, learnError, cronError };
}

type ActionRunner = (
  scope: PluginActionScope,
  actionId: string,
  fn: () => Promise<void>,
  successMessage: string
) => Promise<boolean>;

function buildStagingActions(runAction: ActionRunner) {
  return {
    promoteStaging: (plugin: PluginSummaryDto, reason: string) =>
      runAction(
        "plugin",
        `promote:${plugin.id}`,
        async () => {
          const cleanReason = reason.trim();
          if (cleanReason.length < MIN_RATIONALE_LENGTH) {
            throw new Error("Rationale must be at least 4 characters");
          }
          await pluginCenterClient.promotePluginStaging(plugin.id, {
            projectId: plugin.projectId,
            expectedRevision: plugin.revision,
            reason: cleanReason,
          });
        },
        `Promoted ${plugin.stagingVersion || "candidate"} for ${plugin.pluginSlug}`
      ),
    discardStaging: (plugin: PluginSummaryDto, reason: string) =>
      runAction(
        "plugin",
        `discard:${plugin.id}`,
        async () => {
          const cleanReason = reason.trim();
          if (cleanReason.length < MIN_RATIONALE_LENGTH) {
            throw new Error("Rationale must be at least 4 characters");
          }
          await pluginCenterClient.discardPluginStaging(plugin.id, {
            projectId: plugin.projectId,
            expectedRevision: plugin.revision,
            reason: cleanReason,
          });
        },
        `Discarded staged candidate for ${plugin.pluginSlug}`
      ),
  };
}

function buildLifecycleActions(runAction: ActionRunner) {
  return {
    transitionPlugin: (plugin: PluginSummaryDto, targetState: PluginLifecycleTarget) =>
      runAction(
        "plugin",
        plugin.id,
        async () => {
          await pluginCenterClient.transitionPlugin(plugin.id, {
            projectId: plugin.projectId,
            targetState,
            expectedRevision: plugin.revision,
            reason: `Workbench transition -> ${targetState}`,
          });
        },
        `${plugin.pluginSlug} -> ${targetState}`
      ),
    quarantinePlugin: (plugin: PluginSummaryDto, reason: string) =>
      runAction(
        "plugin",
        `quarantine:${plugin.id}`,
        async () => {
          const cleanReason = reason.trim();
          if (cleanReason.length < MIN_RATIONALE_LENGTH) {
            throw new Error("Quarantine reason must be at least 4 characters");
          }
          await pluginCenterClient.quarantinePlugin(plugin.id, {
            projectId: plugin.projectId,
            expectedRevision: plugin.revision,
            reason: cleanReason,
          });
        },
        `Quarantined ${plugin.pluginSlug}`
      ),
  };
}

function buildCronActions(runAction: ActionRunner) {
  return {
    triggerSyncNow: (plugin: PluginSummaryDto, refreshAuth = false) =>
      runAction(
        "cron",
        `sync:${plugin.id}`,
        async () => {
          await pluginCenterClient.triggerPluginSyncNow(plugin.id, {
            projectId: plugin.projectId,
            refreshAuth,
          });
        },
        `Triggered sync for ${plugin.pluginSlug}`
      ),
    saveSchedule: (plugin: PluginSummaryDto, cronExpression: string, upstreamRepoUrl: string) =>
      runAction(
        "cron",
        `schedule:${plugin.id}`,
        async () => {
          await pluginCenterClient.upsertPluginSchedule(plugin.id, {
            projectId: plugin.projectId,
            cronExpression: cronExpression.trim(),
            upstreamRepoUrl: upstreamRepoUrl.trim(),
          });
        },
        `Saved cron schedule for ${plugin.pluginSlug}`
      ),
  };
}

function buildLearnActions(runAction: ActionRunner) {
  return {
    approveLearnCard: (card: KnowledgeCardDto, reason: string) =>
      runAction(
        "learn",
        `approve:${card.id}`,
        async () => {
          const cleanReason = reason.trim();
          if (cleanReason.length < MIN_RATIONALE_LENGTH) {
            throw new Error("Rationale must be at least 4 characters");
          }
          await pluginCenterClient.approveLearnCard(card.projectId, card.id, {
            expectedRevision: card.revision,
            reason: cleanReason,
          });
        },
        `Accepted LEARN card: ${card.title}`
      ),
    rejectLearnCard: (card: KnowledgeCardDto, reason: string) =>
      runAction(
        "learn",
        `reject:${card.id}`,
        async () => {
          const cleanReason = reason.trim();
          if (cleanReason.length < MIN_RATIONALE_LENGTH) {
            throw new Error("Rationale must be at least 4 characters");
          }
          await pluginCenterClient.rejectLearnCard(card.projectId, card.id, {
            expectedRevision: card.revision,
            reason: cleanReason,
          });
        },
        `Rejected LEARN card: ${card.title}`
      ),
  };
}

function waitAbortable(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    if (signal.aborted) {
      resolve();
      return;
    }
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    const onAbort = () => {
      clearTimeout(timer);
      resolve();
    };
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

function usePluginSseSubscription(
  projectId: string,
  onEventRefresh: () => void,
  onResetState: () => void,
  onStreamError: (msg: string) => void
) {
  useEffect(() => {
    onResetState();
    onEventRefresh();
    if (typeof window === "undefined" || !projectId) {
      return undefined;
    }
    const controller = new AbortController();
    let attempt = 0;
    const connect = async () => {
      while (!controller.signal.aborted) {
        try {
          await pluginCenterClient.subscribePluginEvents(projectId, {
            signal: controller.signal,
            onConnected: () => {
              attempt = 0;
            },
            onRefresh: () => {
              attempt = 0;
              onEventRefresh();
            },
          });
        } catch (err: unknown) {
          if (controller.signal.aborted) break;
          const status = (err as { status?: number } | null)?.status;
          if (status === 401 || status === 403 || status === 404) {
            onStreamError(toErrMessage(err, "Plugin event stream unavailable"));
            break;
          }
        }
        if (controller.signal.aborted) break;
        const delayMs = Math.min(SSE_RECONNECT_MAX_MS, SSE_RECONNECT_BASE_MS * 2 ** attempt);
        attempt = Math.min(attempt + 1, MAX_SSE_BACKOFF_EXPONENT);
        await waitAbortable(delayMs, controller.signal);
      }
    };
    void connect();
    return () => {
      controller.abort();
    };
  }, [projectId, onEventRefresh, onResetState, onStreamError]);
}

const INITIAL_REGISTRY_STATE: PluginRegistryState = {
  plugins: [],
  learnCards: [],
  schedulesByPlugin: {},
  loading: true,
  error: null,
  pluginError: null,
  learnError: null,
  cronError: null,
  actionPendingId: null,
  actionFeedback: null,
};

interface RegistryRefreshParams {
  projectId: string;
  scope: "mcp" | "automations";
  reqId: number;
  requestIdRef: MutableRefObject<number>;
  setState: Dispatch<SetStateAction<PluginRegistryState>>;
  silent?: boolean;
}

async function executeRegistryRefresh({
  projectId,
  scope,
  reqId,
  requestIdRef,
  setState,
  silent,
}: RegistryRefreshParams): Promise<void> {
  if (!silent) {
    setState((prev) => ({ ...prev, loading: true, error: null, pluginError: null, learnError: null, cronError: null }));
  }
  try {
    const snap = await fetchPluginRegistrySnapshot(projectId, scope);
    if (requestIdRef.current !== reqId) return;
    setState((prev) => ({
      ...prev,
      plugins: snap.plugins,
      learnCards: snap.learnCards ?? prev.learnCards,
      schedulesByPlugin: snap.schedulesByPlugin ?? prev.schedulesByPlugin,
      error: snap.error,
      pluginError: snap.pluginError,
      learnError: snap.learnError,
      cronError: snap.cronError,
      loading: false,
    }));
  } catch (err: unknown) {
    if (requestIdRef.current !== reqId) return;
    const msg = toErrMessage(err, "Failed to load plugin ecosystem");
    setState((prev) => ({ ...prev, loading: false, error: msg, pluginError: msg, cronError: msg }));
  }
}

const SCOPE_ERROR_FIELD: Record<PluginActionScope, "pluginError" | "learnError" | "cronError"> = {
  plugin: "pluginError",
  learn: "learnError",
  cron: "cronError",
};

async function executeScopedAction(params: {
  actionScope: PluginActionScope;
  actionId: string;
  fn: () => Promise<void>;
  successMessage: string;
  setState: Dispatch<SetStateAction<PluginRegistryState>>;
  refreshAll: (opts?: { silent?: boolean }) => Promise<void>;
}): Promise<boolean> {
  const { actionScope, actionId, fn, successMessage, setState, refreshAll } = params;
  const errField = SCOPE_ERROR_FIELD[actionScope];
  setState((prev) => ({
    ...prev,
    actionPendingId: actionId,
    actionFeedback: null,
    error: null,
    [errField]: null,
  }));
  try {
    await fn();
    setState((prev) => ({ ...prev, actionPendingId: null, actionFeedback: successMessage }));
    await refreshAll({ silent: true });
    return true;
  } catch (err: unknown) {
    const msg = toErrMessage(err, "Plugin action rejected");
    setState((prev) => ({ ...prev, actionPendingId: null, error: msg, [errField]: msg }));
    return false;
  }
}

export function usePluginRegistry(projectId: string, scope: "mcp" | "automations") {
  const [state, setState] = useState<PluginRegistryState>(INITIAL_REGISTRY_STATE);
  const requestIdRef = useRef(0);
  const scopeRef = useRef(scope);
  scopeRef.current = scope;

  const refreshAll = useCallback(
    async (opts?: { silent?: boolean }) => {
      const reqId = requestIdRef.current + 1;
      requestIdRef.current = reqId;
      await executeRegistryRefresh({ projectId, scope: scopeRef.current, reqId, requestIdRef, setState, silent: opts?.silent });
    },
    [projectId]
  );

  useEffect(() => {
    void refreshAll({ silent: true });
  }, [scope, refreshAll]);

  const resetForProjectChange = useCallback(() => {
    requestIdRef.current += 1;
    setState(INITIAL_REGISTRY_STATE);
  }, []);

  const handleStreamError = useCallback((msg: string) => {
    setState((prev) => ({ ...prev, error: prev.error ?? msg }));
  }, []);

  const handleSseRefresh = useCallback(() => {
    void refreshAll({ silent: true });
  }, [refreshAll]);

  usePluginSseSubscription(projectId, handleSseRefresh, resetForProjectChange, handleStreamError);

  const runAction = useCallback<ActionRunner>(
    (actionScope, actionId, fn, successMessage) =>
      executeScopedAction({ actionScope, actionId, fn, successMessage, setState, refreshAll }),
    [refreshAll]
  );

  return {
    ...state,
    refreshAll,
    ...buildLifecycleActions(runAction),
    ...buildCronActions(runAction),
    ...buildStagingActions(runAction),
    ...buildLearnActions(runAction),
  };
}
