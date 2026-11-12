// @story 04.3
// @implements AC-01 (Reactive Hook for KeyedParallelCoordinator)
import { useState, useEffect, useCallback } from "react";
import {
  globalKeyedParallelCoordinator,
  NodeExecutionJob,
  NodeQueueSummary,
  NodeJobStatus,
} from "../store/keyed-parallel-coordinator";

export function useKeyedParallelCoordinator() {
  const [summary, setSummary] = useState<NodeQueueSummary>(() =>
    globalKeyedParallelCoordinator.getSummary()
  );

  useEffect(() => {
    return globalKeyedParallelCoordinator.subscribe((latestSummary) => {
      setSummary(latestSummary);
    });
  }, []);

  const enqueueJob = useCallback(
    (args: {
      projectId: string;
      sessionId: string;
      nodeId: string;
      prompt: string;
    }) => {
      return globalKeyedParallelCoordinator.enqueueJob(args);
    },
    []
  );

  const steerJob = useCallback((jobId: string, instruction: string) => {
    return globalKeyedParallelCoordinator.steerJob(jobId, instruction);
  }, []);

  const pauseJob = useCallback((jobId: string) => {
    return globalKeyedParallelCoordinator.pauseJob(jobId);
  }, []);

  const resumeJob = useCallback((jobId: string) => {
    return globalKeyedParallelCoordinator.resumeJob(jobId);
  }, []);

  const cancelJob = useCallback((jobId: string) => {
    return globalKeyedParallelCoordinator.cancelJob(jobId);
  }, []);

  const completeJob = useCallback((jobId: string, outputSha256?: string) => {
    return globalKeyedParallelCoordinator.completeJob(jobId, outputSha256);
  }, []);

  const getNodeStatus = useCallback(
    (
      nodeId: string
    ): {
      status: NodeJobStatus;
      queuePosition: number;
      jobId?: string;
    } | null => {
      return globalKeyedParallelCoordinator.getNodeStatus(nodeId);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [summary]
  );

  const getActiveJobs = useCallback((): NodeExecutionJob[] => {
    return globalKeyedParallelCoordinator.getActiveJobs();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [summary]);

  return {
    summary,
    enqueueJob,
    steerJob,
    pauseJob,
    resumeJob,
    cancelJob,
    completeJob,
    getNodeStatus,
    getActiveJobs,
  };
}
