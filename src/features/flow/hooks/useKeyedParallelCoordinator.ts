// @story 04.3, 04.4
// @implements AC-01 (Reactive Hook + Daemon Sync for KeyedParallelCoordinator)
import { useState, useEffect, useCallback } from "react";
import { apiFlowExecution } from "@/api/flow-execution";
import {
  globalKeyedParallelCoordinator,
  type NodeExecutionJob,
  type NodeQueueSummary,
  type NodeStatusSnapshot,
  type ScopeFilter,
} from "../store/keyed-parallel-coordinator";
import type { NodeExecutionJobDto } from "@/lib/validations/flow-execution";

export function useKeyedParallelCoordinator() {
  const [summary, setSummary] = useState<NodeQueueSummary>(() => globalKeyedParallelCoordinator.getSummary());

  useEffect(() => globalKeyedParallelCoordinator.subscribe(setSummary), []);

  const enqueueJob = useCallback((args: { projectId: string; sessionId: string; nodeId: string; prompt: string }) => globalKeyedParallelCoordinator.enqueueJob(args), []);
  const steerJob = useCallback((jobId: string, instruction: string, nextToken?: number) => globalKeyedParallelCoordinator.steerJob(jobId, instruction, nextToken), []);
  const pauseJob = useCallback((jobId: string, nextToken?: number) => globalKeyedParallelCoordinator.pauseJob(jobId, nextToken), []);
  const resumeJob = useCallback((jobId: string, nextToken?: number) => globalKeyedParallelCoordinator.resumeJob(jobId, nextToken), []);
  const cancelJob = useCallback((jobId: string, reason?: string) => globalKeyedParallelCoordinator.cancelJob(jobId, reason), []);
  const failJob = useCallback((jobId: string, reason?: string) => globalKeyedParallelCoordinator.failJob(jobId, reason), []);
  const completeJob = useCallback((jobId: string, outputSha256?: string) => globalKeyedParallelCoordinator.completeJob(jobId, outputSha256), []);
  const reconcileSingleJob = useCallback((dto: NodeExecutionJobDto, projectSeq?: number) => globalKeyedParallelCoordinator.reconcileSingleJob(dto, projectSeq), []);

  const syncFromDaemon = useCallback(async (projectId: string, sessionId?: string) => {
    const res = await apiFlowExecution.listNodeSessions(projectId, sessionId);
    globalKeyedParallelCoordinator.reconcileFromDaemon(res.jobs, apiFlowExecution.getLastProjectSeq(projectId), { projectId, sessionId });
    return res;
  }, []);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const getNodeStatus = useCallback((nodeId: string, scope?: ScopeFilter): NodeStatusSnapshot | null => globalKeyedParallelCoordinator.getNodeStatus(nodeId, scope), [summary]);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const getActiveJobs = useCallback((): NodeExecutionJob[] => globalKeyedParallelCoordinator.getActiveJobs(), [summary]);

  return { summary, enqueueJob, steerJob, pauseJob, resumeJob, cancelJob, failJob, completeJob, reconcileSingleJob, syncFromDaemon, getNodeStatus, getActiveJobs };
}
