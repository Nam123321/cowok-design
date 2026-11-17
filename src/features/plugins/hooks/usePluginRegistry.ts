// @story 08.4
// @implements AC-01, AC-02, AC-04, AC-05
import { useCallback, useEffect, useState } from "react";
import { apiPluginCenter } from "../../../api/plugin-center";
import type {
  CronScheduleStatusDto,
  KnowledgeCardDto,
  PluginSummaryDto,
} from "../../../lib/validations/plugins";

export interface PluginRegistryState {
  plugins: PluginSummaryDto[];
  learnCards: KnowledgeCardDto[];
  schedulesByPlugin: Record<string, CronScheduleStatusDto | null>;
  loading: boolean;
  error: string | null;
  actionPendingId: string | null;
  actionFeedback: string | null;
}

const MIN_RATIONALE_LENGTH = 4;

type LifecycleTarget = "VALIDATED" | "INSTALLED" | "ACTIVE" | "DEPRECATED" | "ARCHIVED";

async function loadSchedulesForPlugins(
  plugins: PluginSummaryDto[]
): Promise<Record<string, CronScheduleStatusDto | null>> {
  const schedulesByPlugin: Record<string, CronScheduleStatusDto | null> = {};
  await Promise.all(
    plugins.map(async (p) => {
      try {
        schedulesByPlugin[p.id] = await apiPluginCenter.getPluginSchedule(p.id, p.projectId);
      } catch {
        schedulesByPlugin[p.id] = null;
      }
    })
  );
  return schedulesByPlugin;
}

async function fetchPluginRegistrySnapshot(projectId: string, scope: "mcp" | "automations") {
  const [plugins, learnCards] = await Promise.all([
    apiPluginCenter.getPluginRegistry({ projectId }),
    scope === "mcp" ? apiPluginCenter.listLearnCards(projectId) : Promise.resolve([]),
  ]);
  const schedulesByPlugin =
    scope === "automations" ? await loadSchedulesForPlugins(plugins) : {};
  return { plugins, learnCards, schedulesByPlugin };
}

type ActionRunner = (actionId: string, fn: () => Promise<void>, successMessage: string) => Promise<void>;

function buildStagingActions(runAction: ActionRunner) {
  return {
    promoteStaging: (plugin: PluginSummaryDto, reason: string) =>
      runAction(
        `promote:${plugin.id}`,
        async () => {
          const cleanReason = reason.trim();
          if (cleanReason.length < MIN_RATIONALE_LENGTH) {
            throw new Error("Rationale must be at least 4 characters");
          }
          await apiPluginCenter.promotePluginStaging(plugin.id, {
            projectId: plugin.projectId,
            expectedRevision: plugin.revision,
            reason: cleanReason,
          });
        },
        `Promoted ${plugin.stagingVersion || "candidate"} for ${plugin.pluginSlug}`
      ),
    discardStaging: (plugin: PluginSummaryDto, reason: string) =>
      runAction(
        `discard:${plugin.id}`,
        async () => {
          const cleanReason = reason.trim();
          if (cleanReason.length < MIN_RATIONALE_LENGTH) {
            throw new Error("Rationale must be at least 4 characters");
          }
          await apiPluginCenter.discardPluginStaging(plugin.id, {
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
    transitionPlugin: (plugin: PluginSummaryDto, targetState: LifecycleTarget) =>
      runAction(
        plugin.id,
        async () => {
          await apiPluginCenter.transitionPlugin(plugin.id, {
            projectId: plugin.projectId,
            targetState,
            expectedRevision: plugin.revision,
            reason: `Workbench transition -> ${targetState}`,
          });
        },
        `${plugin.pluginSlug} -> ${targetState}`
      ),
    triggerSyncNow: (plugin: PluginSummaryDto, refreshAuth = false) =>
      runAction(
        `sync:${plugin.id}`,
        async () => {
          await apiPluginCenter.triggerPluginSyncNow(plugin.id, {
            projectId: plugin.projectId,
            refreshAuth,
          });
        },
        `Triggered sync for ${plugin.pluginSlug}`
      ),
    saveSchedule: (plugin: PluginSummaryDto, cronExpression: string, upstreamRepoUrl: string) =>
      runAction(
        `schedule:${plugin.id}`,
        async () => {
          await apiPluginCenter.upsertPluginSchedule(plugin.id, {
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
        `approve:${card.id}`,
        async () => {
          const cleanReason = reason.trim();
          if (cleanReason.length < MIN_RATIONALE_LENGTH) {
            throw new Error("Rationale must be at least 4 characters");
          }
          await apiPluginCenter.approveLearnCard(card.projectId, card.id, {
            expectedRevision: card.revision,
            reason: cleanReason,
          });
        },
        `Accepted LEARN card: ${card.title}`
      ),
    rejectLearnCard: (card: KnowledgeCardDto, reason: string) =>
      runAction(
        `reject:${card.id}`,
        async () => {
          const cleanReason = reason.trim();
          if (cleanReason.length < MIN_RATIONALE_LENGTH) {
            throw new Error("Rationale must be at least 4 characters");
          }
          await apiPluginCenter.rejectLearnCard(card.projectId, card.id, {
            expectedRevision: card.revision,
            reason: cleanReason,
          });
        },
        `Rejected LEARN card: ${card.title}`
      ),
  };
}

function usePluginSseSubscription(projectId: string, refreshAll: () => Promise<void>) {
  useEffect(() => {
    void refreshAll();
    if (typeof window === "undefined" || !projectId) {
      return undefined;
    }
    const controller = new AbortController();
    void apiPluginCenter
      .subscribePluginEvents(projectId, {
        signal: controller.signal,
        onRefresh: () => {
          void refreshAll();
        },
      })
      .catch(() => {
        // Ignore stream abort or transient disconnect during teardown
      });
    return () => {
      controller.abort();
    };
  }, [projectId, refreshAll]);
}

export function usePluginRegistry(projectId: string, scope: "mcp" | "automations") {
  const [state, setState] = useState<PluginRegistryState>({
    plugins: [],
    learnCards: [],
    schedulesByPlugin: {},
    loading: true,
    error: null,
    actionPendingId: null,
    actionFeedback: null,
  });

  const refreshAll = useCallback(async () => {
    setState((prev) => ({ ...prev, loading: true, error: null }));
    try {
      const snap = await fetchPluginRegistrySnapshot(projectId, scope);
      setState((prev) => ({ ...prev, ...snap, loading: false, error: null }));
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Failed to load plugin ecosystem";
      setState((prev) => ({ ...prev, loading: false, error: msg }));
    }
  }, [projectId, scope]);

  usePluginSseSubscription(projectId, refreshAll);

  const runAction = useCallback(
    async (actionId: string, fn: () => Promise<void>, successMessage: string) => {
      setState((prev) => ({ ...prev, actionPendingId: actionId, actionFeedback: null, error: null }));
      try {
        await fn();
        setState((prev) => ({ ...prev, actionPendingId: null, actionFeedback: successMessage }));
        await refreshAll();
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : "Plugin action rejected";
        setState((prev) => ({ ...prev, actionPendingId: null, error: msg }));
      }
    },
    [refreshAll]
  );

  return {
    ...state,
    refreshAll,
    ...buildLifecycleActions(runAction),
    ...buildStagingActions(runAction),
    ...buildLearnActions(runAction),
  };
}
