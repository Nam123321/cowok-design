// @story 04.4
// @implements AC-01, AC-02 (Rust Daemon SSE Event Stream & Job Dispatch Engine)
import { apiSdiEngine } from "@/api/sdi-engine";
import type { DesignIntentResponse } from "@/lib/validations/sdi";
import { apiWorkbenchSettings } from "@/api/workbench-settings";
import type { WorkbenchNode } from "../store/flow-workbench-store";
import type { useKeyedParallelCoordinator } from "./useKeyedParallelCoordinator";
import type { FloatingChatMessage } from "../components/FloatingChatStreamPanel";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const AMBIGUITY_PERCENT_SCALE = 100;
const SINGLE_NODE_COUNT = 1;

export interface StreamSyncParams {
  projectId: string;
  primaryNode: WorkbenchNode | null;
  selectedNodesCount: number;
  streamAbortRef: React.MutableRefObject<AbortController | null>;
  onAppendMsg: (msg: FloatingChatMessage) => void;
  t: (key: string, fallback: string) => string;
}

export function openFloatingChatEventStream(params: StreamSyncParams): AbortController {
  params.streamAbortRef.current?.abort();
  const controller = new AbortController();
  params.streamAbortRef.current = controller;
  const targetNodeId = params.selectedNodesCount === SINGLE_NODE_COUNT ? params.primaryNode?.id : undefined;

  void apiWorkbenchSettings
    .streamJobEvents(params.projectId, {
      nodeId: targetNodeId,
      signal: controller.signal,
      onEvents: (events) => {
        const summaryText = events.map((ev) => `[${ev.eventKind}] ${ev.payloadText || ev.nodeId}`).join("\n");
        if (!summaryText) return;
        params.onAppendMsg({
          id: `sse-${Date.now()}-${events[0]?.seqNo ?? 0}`,
          nodeId: params.primaryNode?.id || "unknown",
          role: "assistant",
          content: summaryText,
          timestamp: Date.now(),
        });
      },
    })
    .catch((err: unknown) => {
      if (controller.signal.aborted || (err instanceof Error && err.name === "AbortError")) return;
      const msg = err instanceof Error ? err.message : "stream_failed";
      params.onAppendMsg({
        id: `sse-err-${Date.now()}`,
        nodeId: params.primaryNode?.id || "unknown",
        role: "assistant",
        content: `${params.t("floatingChat.daemonSyncError", "Không thể đồng bộ với Rust daemon")}: ${msg}`,
        timestamp: Date.now(),
      });
    });
  return controller;
}

export interface NodeJob {
  id: string;
  nodeId: string;
}

export interface DispatchJobsOptions {
  projectId: string;
  sessionId: string;
  primaryNode: WorkbenchNode | null;
  prompt: string;
  jobs: NodeJob[];
  coordinator: ReturnType<typeof useKeyedParallelCoordinator>;
  streamAbortRef: React.MutableRefObject<AbortController | null>;
  onAppendMsg: (msg: FloatingChatMessage) => void;
  t: (key: string, fallback: string) => string;
  formatNumber: (value: number) => string;
}

type DaemonJobStatus = "QUEUED" | "RUNNING" | "PAUSED" | "INTERRUPTED" | "COMPLETED" | "FAILED" | "CANCELLED";

async function postJobEventsForBatch(
  opts: { projectId: string; sessionId: string; jobs: NodeJob[] },
  eventKind: string,
  payloadText: string,
  jobStatus: DaemonJobStatus
) {
  for (const job of opts.jobs) {
    await apiWorkbenchSettings.postJobEvent(opts.projectId, {
      jobId: job.id,
      nodeId: job.nodeId,
      sessionId: opts.sessionId,
      eventKind,
      payloadText,
      jobStatus,
    });
  }
}

function formatIntentSummary(
  intent: DesignIntentResponse,
  t: (key: string, fallback: string) => string,
  formatNumber: (value: number) => string
): string {
  if (intent.status === "clarifying") {
    const clarifyPct = formatNumber(Math.round(intent.ambiguityScore * AMBIGUITY_PERCENT_SCALE));
    const variantTitles = intent.clarificationVariants?.map((v) => v.title).join(" | ");
    return `${t("chat.clarifyPrefix", "Cần làm rõ")} (${clarifyPct}%): ${variantTitles || intent.intentType}`;
  }
  return `${t("floatingChat.compiledIntent", "Đã biên dịch intent")} (${intent.intentType}) · status=${intent.status}`;
}

async function executeDaemonCompilation(opts: DispatchJobsOptions): Promise<void> {
  const batchOpts = { projectId: opts.projectId, sessionId: opts.sessionId, jobs: opts.jobs };
  await postJobEventsForBatch(batchOpts, "started", JSON.stringify({ prompt: opts.prompt }), "RUNNING");

  const intent = await apiSdiEngine.compileIntent(opts.projectId, {
    rawPrompt: opts.prompt,
    targetNodeId: opts.primaryNode?.id,
  });
  const summary = formatIntentSummary(intent, opts.t, opts.formatNumber);
  opts.onAppendMsg({
    id: `resp-${intent.id}`,
    nodeId: opts.primaryNode?.id || "unknown",
    role: "assistant",
    content: summary,
    timestamp: Date.now(),
  });

  await postJobEventsForBatch(batchOpts, "completed", JSON.stringify({ intentId: intent.id, summary }), "COMPLETED");
  opts.jobs.forEach((job) => opts.coordinator.completeJob(job.id));
}

export async function dispatchNodeJobToDaemon(opts: DispatchJobsOptions): Promise<void> {
  if (!UUID_RE.test(opts.projectId)) {
    opts.jobs.forEach((job) => opts.coordinator.failJob(job.id, "NON_UUID_PROJECT"));
    return;
  }

  const controller = openFloatingChatEventStream({
    projectId: opts.projectId,
    primaryNode: opts.primaryNode,
    selectedNodesCount: opts.jobs.length,
    streamAbortRef: opts.streamAbortRef,
    onAppendMsg: opts.onAppendMsg,
    t: opts.t,
  });

  try {
    await executeDaemonCompilation(opts);
  } catch (err) {
    const msg = err instanceof Error ? err.message : opts.t("floatingChat.daemonRequestFailed", "Yêu cầu Daemon thất bại");
    opts.jobs.forEach((job) => opts.coordinator.failJob(job.id, msg));
    opts.onAppendMsg({
      id: `err-${Date.now()}`,
      nodeId: opts.primaryNode?.id || "unknown",
      role: "assistant",
      content: `${opts.t("floatingChat.daemonSyncError", "Không thể đồng bộ với Rust daemon")}: ${msg}`,
      timestamp: Date.now(),
    });
  } finally {
    controller.abort();
  }
}
