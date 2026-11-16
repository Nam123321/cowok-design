// @story 04.4
// @implements AC-01, AC-02, AC-03 (Daemon Enqueue, Lifecycle SSE Stream & CAS Dispatch Helper)
import React from "react";
import { apiFlowExecution } from "@/api/flow-execution";
import { apiSdiEngine } from "@/api/sdi-engine";
import { WorkbenchNode } from "../store/flow-workbench-store";
import { globalKeyedParallelCoordinator } from "../store/keyed-parallel-coordinator";
import { sharedChatStore, toSessionChatKey } from "../store/shared-chat-sync";
import { useKeyedParallelCoordinator } from "./useKeyedParallelCoordinator";
import type { FloatingChatMessage } from "./useFloatingChatSync";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const activeStreams = new Map<string, AbortController>();

export interface DispatchExecutionParams {
  projectId: string;
  sessionId: string;
  primaryNode: WorkbenchNode | null;
  selectedNodeIds: string[];
  prompt: string;
  coordinator: ReturnType<typeof useKeyedParallelCoordinator>;
  streamAbortRef?: React.MutableRefObject<AbortController | null>;
  onAppendMsg: (msg: FloatingChatMessage) => void;
  t: (key: string, fallback: string) => string;
  formatNumber: (value: number) => string;
}

export async function syncCoordinatorFromDaemon(
  projectId: string,
  coordinator?: Pick<ReturnType<typeof useKeyedParallelCoordinator>, "syncFromDaemon">
): Promise<void> {
  if (!UUID_RE.test(projectId)) return;
  try {
    if (coordinator) await coordinator.syncFromDaemon(projectId);
    else {
      const res = await apiFlowExecution.listNodeSessions(projectId);
      globalKeyedParallelCoordinator.reconcileFromDaemon(res.jobs, apiFlowExecution.getLastProjectSeq(projectId), { projectId });
    }
    sharedChatStore.bumpSubSessionsVersion();
  } catch { /* ignore when daemon offline */ }
}

export function ensureProjectSseStream(
  projectId: string,
  sessionId: string,
  streamAbortRef?: React.MutableRefObject<AbortController | null>,
  onAppendMsg?: (msg: FloatingChatMessage) => void
): void {
  if (!UUID_RE.test(projectId)) return;
  const existing = activeStreams.get(projectId);
  if (existing && !existing.signal.aborted) {
    if (streamAbortRef) streamAbortRef.current = existing;
    return;
  }
  const controller = new AbortController();
  activeStreams.set(projectId, controller);
  if (streamAbortRef) streamAbortRef.current = controller;

  void apiFlowExecution.streamJobEvents(projectId, {
    signal: controller.signal,
    onEvents: (events) => {
      const latestSeq = events.reduce((m, ev) => Math.max(m, ev.projectSeq ?? ev.seqNo ?? 0), 0);
      if (latestSeq > 0) globalKeyedParallelCoordinator.updateProjectSeq(latestSeq);
      for (const ev of events) {
        if (!ev.payloadText) continue;
        const msg: FloatingChatMessage = {
          id: `sse-${ev.id}`,
          nodeId: ev.nodeId,
          role: "assistant",
          content: `[${ev.eventKind}] ${ev.payloadText}`,
          timestamp: Date.now(),
          projectSeq: ev.projectSeq ?? ev.seqNo,
        };
        if (onAppendMsg) onAppendMsg(msg);
        else sharedChatStore.appendMessage(toSessionChatKey(projectId, sessionId), {
          id: msg.id, sender: "agent", text: msg.content, timestamp: `${String(new Date().getHours()).padStart(2, "0")}:${String(new Date().getMinutes()).padStart(2, "0")}:${String(new Date().getSeconds()).padStart(2, "0")}`,
          createdAtMs: msg.timestamp, nodeId: ev.nodeId, jobId: ev.jobId, projectSeq: msg.projectSeq,
        });
      }
      if (events.some((ev) => ev.jobStatus)) {
        void syncCoordinatorFromDaemon(projectId);
      }
    },
  }).catch(() => { /* stream aborted or offline */ }).finally(() => {
    if (activeStreams.get(projectId) === controller) activeStreams.delete(projectId);
  });
}

async function runEnqueueAndCompile(params: DispatchExecutionParams): Promise<void> {
  const enq = await apiFlowExecution.enqueueJobs(params.projectId, {
    sessionId: params.sessionId,
    nodeIds: params.selectedNodeIds,
    prompt: params.prompt,
    mode: params.selectedNodeIds.length > 1 ? "parallel" : "queue",
  });
  globalKeyedParallelCoordinator.reconcileFromDaemon(enq.jobs, apiFlowExecution.getLastProjectSeq(params.projectId));
  sharedChatStore.bumpSubSessionsVersion();

  const intent = await apiSdiEngine.compileIntent(params.projectId, { rawPrompt: params.prompt, targetNodeId: params.primaryNode?.id });
  const clarifyPct = params.formatNumber(Math.round(intent.ambiguityScore * 100));
  const summary = intent.status === "clarifying"
    ? `${params.t("chat.clarifyPrefix", "Cần làm rõ")} (${clarifyPct}%): ${intent.clarificationVariants?.map((v) => v.title).join(" | ") || intent.intentType}`
    : `${params.t("floatingChat.compiledIntent", "Đã biên dịch intent")} (${intent.intentType}) · status=${intent.status}`;
  params.onAppendMsg({ id: `resp-${intent.id}`, nodeId: params.primaryNode?.id || "unknown", role: "assistant", content: summary, timestamp: Date.now() });
}

export async function dispatchNodeExecutionToDaemon(params: DispatchExecutionParams) {
  if (!UUID_RE.test(params.projectId)) {
    const localJobs = params.selectedNodeIds.map((nid) =>
      params.coordinator.enqueueJob({ projectId: params.projectId, sessionId: params.sessionId, nodeId: nid, prompt: params.prompt })
    );
    localJobs.forEach((j) => params.coordinator.failJob(j.id, "NON_UUID_PROJECT"));
    return;
  }

  ensureProjectSseStream(params.projectId, params.sessionId, params.streamAbortRef, params.onAppendMsg);
  try {
    await runEnqueueAndCompile(params);
  } catch (err) {
    const msg = err instanceof Error ? err.message : params.t("floatingChat.daemonRequestFailed", "Yêu cầu Daemon thất bại");
    params.onAppendMsg({ id: `err-${Date.now()}`, nodeId: params.primaryNode?.id || "unknown", role: "assistant", content: `${params.t("floatingChat.daemonSyncError", "Không thể đồng bộ với Rust daemon")}: ${msg}`, timestamp: Date.now() });
    await syncCoordinatorFromDaemon(params.projectId, params.coordinator);
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
    .then((dto) => {
      globalKeyedParallelCoordinator.reconcileSingleJob(dto, apiFlowExecution.getLastProjectSeq(params.projectId));
      params.coordinator.steerJob(dto.id, params.instruction, dto.fencingToken);
      sharedChatStore.bumpSubSessionsVersion();
    })
    .catch((err: unknown) => {
      const msg = err instanceof Error ? err.message : "steer_failed";
      const prefix = msg.includes("409") || msg.includes("STALE_FENCING_TOKEN") ? "409 STALE_FENCING_TOKEN" : params.errorPrefix;
      params.appendMsg({ id: `steer-err-${Date.now()}`, nodeId: params.nodeId, role: "assistant", content: `${prefix}: ${msg}`, timestamp: Date.now() });
      void syncCoordinatorFromDaemon(params.projectId, params.coordinator);
    });
}

export function dispatchTransitionToDaemon(params: {
  projectId: string;
  kind: "pause" | "resume" | "cancel";
  jobId: string;
  fence: number;
  coordinator: Pick<ReturnType<typeof useKeyedParallelCoordinator>, "pauseJob" | "resumeJob" | "cancelJob" | "syncFromDaemon">;
  onErrorMsg?: (msg: string) => void;
}): void {
  const { projectId, kind, jobId, fence, coordinator, onErrorMsg } = params;
  if (!UUID_RE.test(projectId)) {
    if (kind === "pause") coordinator.pauseJob(jobId);
    else if (kind === "resume") coordinator.resumeJob(jobId);
    else coordinator.cancelJob(jobId);
    return;
  }
  const transitions = {
    pause: () => apiFlowExecution.pauseJob(projectId, { jobId, expectedFencingToken: fence }),
    resume: () => apiFlowExecution.resumeJob(projectId, { jobId, expectedFencingToken: fence }),
    cancel: () => apiFlowExecution.cancelJob(projectId, { jobId, expectedFencingToken: fence }),
  };
  const call = transitions[kind]();

  void call
    .then((dto) => {
      globalKeyedParallelCoordinator.reconcileSingleJob(dto, apiFlowExecution.getLastProjectSeq(projectId));
      if (kind === "cancel") void syncCoordinatorFromDaemon(projectId, coordinator);
      else sharedChatStore.bumpSubSessionsVersion();
    })
    .catch((err: unknown) => {
      const msg = err instanceof Error ? err.message : `${kind}_failed`;
      onErrorMsg?.(msg);
      void syncCoordinatorFromDaemon(projectId, coordinator);
    });
}
