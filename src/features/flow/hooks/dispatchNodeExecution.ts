// @story 04.4
// @implements AC-01, AC-02, AC-03 (Daemon Enqueue, SSE Stream & CAS Dispatch Helper)
import React from "react";
import { apiFlowExecution } from "@/api/flow-execution";
import { apiSdiEngine } from "@/api/sdi-engine";
import { WorkbenchNode } from "../store/flow-workbench-store";
import { globalKeyedParallelCoordinator } from "../store/keyed-parallel-coordinator";
import { useKeyedParallelCoordinator } from "./useKeyedParallelCoordinator";
import type { FloatingChatMessage } from "./useFloatingChatSync";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export interface DispatchExecutionParams {
  projectId: string;
  sessionId: string;
  primaryNode: WorkbenchNode | null;
  selectedNodeIds: string[];
  prompt: string;
  coordinator: ReturnType<typeof useKeyedParallelCoordinator>;
  streamAbortRef: React.MutableRefObject<AbortController | null>;
  onAppendMsg: (msg: FloatingChatMessage) => void;
  t: (key: string, fallback: string) => string;
  formatNumber: (value: number) => string;
}

function startBackgroundSseStream(params: DispatchExecutionParams, controller: AbortController): void {
  void apiFlowExecution.streamJobEvents(params.projectId, {
    nodeId: params.selectedNodeIds.length === 1 ? params.primaryNode?.id : undefined,
    signal: controller.signal,
    onEvents: (events) => {
      const latestSeq = events.reduce((m, ev) => Math.max(m, ev.projectSeq ?? ev.seqNo ?? 0), 0);
      if (latestSeq > 0) globalKeyedParallelCoordinator.updateProjectSeq(latestSeq);
      const text = events.map((ev) => `[${ev.eventKind}] ${ev.payloadText || ev.nodeId}`).join("\n");
      if (text) {
        params.onAppendMsg({
          id: `sse-${Date.now()}-${latestSeq}`,
          nodeId: params.primaryNode?.id || "unknown",
          role: "assistant",
          content: text,
          timestamp: Date.now(),
          projectSeq: latestSeq,
        });
      }
    },
  }).catch(() => { /* stream aborted or offline */ });
}

async function runEnqueueAndCompile(params: DispatchExecutionParams): Promise<void> {
  const enq = await apiFlowExecution.enqueueJobs(params.projectId, {
    sessionId: params.sessionId,
    nodeIds: params.selectedNodeIds,
    prompt: params.prompt,
    mode: params.selectedNodeIds.length > 1 ? "parallel" : "queue",
  });
  globalKeyedParallelCoordinator.reconcileFromDaemon(enq.jobs, apiFlowExecution.getLastProjectSeq(params.projectId));

  const intent = await apiSdiEngine.compileIntent(params.projectId, { rawPrompt: params.prompt, targetNodeId: params.primaryNode?.id });
  const clarifyPct = params.formatNumber(Math.round(intent.ambiguityScore * 100));
  const summary = intent.status === "clarifying"
    ? `${params.t("chat.clarifyPrefix", "Cần làm rõ")} (${clarifyPct}%): ${intent.clarificationVariants?.map((v) => v.title).join(" | ") || intent.intentType}`
    : `${params.t("floatingChat.compiledIntent", "Đã biên dịch intent")} (${intent.intentType}) · status=${intent.status}`;
  params.onAppendMsg({ id: `resp-${intent.id}`, nodeId: params.primaryNode?.id || "unknown", role: "assistant", content: summary, timestamp: Date.now() });

  for (const job of enq.jobs) {
    await apiFlowExecution.postJobEvent(params.projectId, {
      jobId: job.id, nodeId: job.nodeId, sessionId: params.sessionId,
      eventKind: "completed", payloadText: JSON.stringify({ intentId: intent.id, summary }),
      jobStatus: "COMPLETED", expectedFencingToken: job.fencingToken,
    });
    params.coordinator.completeJob(job.id);
  }
}

export async function dispatchNodeExecutionToDaemon(params: DispatchExecutionParams) {
  const localJobs = params.selectedNodeIds.map((nid) =>
    params.coordinator.enqueueJob({ projectId: params.projectId, sessionId: params.sessionId, nodeId: nid, prompt: params.prompt })
  );
  if (!UUID_RE.test(params.projectId)) {
    localJobs.forEach((j) => params.coordinator.failJob(j.id, "NON_UUID_PROJECT"));
    return;
  }

  params.streamAbortRef.current?.abort();
  const controller = new AbortController();
  params.streamAbortRef.current = controller;
  startBackgroundSseStream(params, controller);

  try {
    await runEnqueueAndCompile(params);
  } catch (err) {
    const msg = err instanceof Error ? err.message : params.t("floatingChat.daemonRequestFailed", "Yêu cầu Daemon thất bại");
    localJobs.forEach((j) => params.coordinator.failJob(j.id, msg));
    params.onAppendMsg({ id: `err-${Date.now()}`, nodeId: params.primaryNode?.id || "unknown", role: "assistant", content: `${params.t("floatingChat.daemonSyncError", "Không thể đồng bộ với Rust daemon")}: ${msg}`, timestamp: Date.now() });
  } finally {
    controller.abort();
  }
}

export function dispatchSteerToDaemon(params: {
  projectId: string;
  jobId: string;
  nodeId: string;
  fence: number;
  instruction: string;
  coordinator: ReturnType<typeof useKeyedParallelCoordinator>;
  appendMsg: (m: FloatingChatMessage) => void;
  errorPrefix: string;
}): void {
  if (!UUID_RE.test(params.projectId)) return;
  void apiFlowExecution
    .steerJob(params.projectId, { jobId: params.jobId, expectedFencingToken: params.fence, instruction: params.instruction, pgidReapRequested: true })
    .then((dto) => params.coordinator.steerJob(dto.id, params.instruction, dto.fencingToken))
    .catch((err: unknown) =>
      params.appendMsg({
        id: `steer-err-${Date.now()}`,
        nodeId: params.nodeId,
        role: "assistant",
        content: `${params.errorPrefix}: ${err instanceof Error ? err.message : "steer_failed"}`,
        timestamp: Date.now(),
      })
    );
}

export function dispatchTransitionToDaemon(params: {
  projectId: string;
  kind: "pause" | "resume" | "cancel";
  jobId: string;
  fence: number;
  coordinator: ReturnType<typeof useKeyedParallelCoordinator>;
}): void {
  const { projectId, kind, jobId, fence, coordinator } = params;
  if (kind === "pause") coordinator.pauseJob(jobId);
  else if (kind === "resume") coordinator.resumeJob(jobId);
  else coordinator.cancelJob(jobId);
  if (!UUID_RE.test(projectId)) return;
  const call =
    kind === "pause"
      ? apiFlowExecution.pauseJob(projectId, { jobId, expectedFencingToken: fence })
      : kind === "resume"
        ? apiFlowExecution.resumeJob(projectId, { jobId, expectedFencingToken: fence })
        : apiFlowExecution.cancelJob(projectId, { jobId, expectedFencingToken: fence });
  void call.catch(() => { /* reconciled via SSE / next sync */ });
}
