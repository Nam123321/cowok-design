// @story 04.1
// @implements AC-06, AC-08
import { apiFlowWorkbench, FlowApiError } from "../../../api/flow-workbench";
import type { WorkbenchNode } from "./flow-workbench-types";

export interface CoordinateSyncTarget {
  projectId: string;
  isDeploying: () => boolean;
  isPersistedNode: (nodeId: string) => boolean;
  getNodeLayoutVersion: (nodeId: string) => number;
  onLayoutSynced: (updates: Array<{ nodeId: string; layoutVersion: number }>, graphLayoutVersion?: number) => void;
  onStaleLayoutResync: () => Promise<void>;
}

export class CoordinateSyncQueue {
  private pending = new Map<string, { posX: number; posY: number }>();
  private timer: ReturnType<typeof setTimeout> | null = null;
  private inflight = false;
  private readonly debounceMs: number;

  constructor(debounceMs = 120) {
    this.debounceMs = debounceMs;
  }

  public enqueueMove(
    target: CoordinateSyncTarget,
    nodeId: string,
    posX: number,
    posY: number
  ): "ephemeral_local_only" | "queued_persisted" {
    if (!target.isPersistedNode(nodeId)) {
      return "ephemeral_local_only";
    }
    this.pending.set(nodeId, { posX, posY });
    this.scheduleFlush(target);
    return "queued_persisted";
  }

  public async flushBatchNow(
    target: CoordinateSyncTarget,
    nodes: WorkbenchNode[]
  ): Promise<"noop" | "synced" | "resynced"> {
    const persisted = nodes.filter((n) => target.isPersistedNode(n.id));
    if (persisted.length === 0 || !target.projectId) return "noop";
    for (const p of persisted) {
      this.pending.delete(p.id);
    }
    try {
      const res = await apiFlowWorkbench.batchUpdateNodePositions(target.projectId, {
        positions: persisted.map((n) => ({
          nodeId: n.id,
          posX: n.posX,
          posY: n.posY,
          expectedLayoutVersion: target.getNodeLayoutVersion(n.id),
        })),
      });
      target.onLayoutSynced(
        res.nodes.map((n) => ({ nodeId: n.id, layoutVersion: n.layoutVersion })),
        res.layoutVersion
      );
      return "synced";
    } catch (err) {
      if (err instanceof FlowApiError && err.status === 409) {
        await target.onStaleLayoutResync();
        return "resynced";
      }
      return "noop";
    }
  }

  private scheduleFlush(target: CoordinateSyncTarget): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      void this.drainQueue(target);
    }, this.debounceMs);
  }

  public async drainQueue(target: CoordinateSyncTarget): Promise<void> {
    if (this.inflight || this.pending.size === 0 || !target.projectId) return;
    if (target.isDeploying()) {
      this.scheduleFlush(target);
      return;
    }

    this.inflight = true;
    const entries = Array.from(this.pending.entries());
    this.pending.clear();

    try {
      if (entries.length === 1) {
        const [nodeId, coords] = entries[0];
        const updated = await apiFlowWorkbench.updateNode(target.projectId, nodeId, {
          posX: coords.posX,
          posY: coords.posY,
          expectedLayoutVersion: target.getNodeLayoutVersion(nodeId),
        });
        target.onLayoutSynced([{ nodeId, layoutVersion: updated.layoutVersion }]);
      } else {
        const res = await apiFlowWorkbench.batchUpdateNodePositions(target.projectId, {
          positions: entries.map(([nodeId, coords]) => ({
            nodeId,
            posX: coords.posX,
            posY: coords.posY,
            expectedLayoutVersion: target.getNodeLayoutVersion(nodeId),
          })),
        });
        target.onLayoutSynced(
          res.nodes.map((n) => ({ nodeId: n.id, layoutVersion: n.layoutVersion })),
          res.layoutVersion
        );
      }
    } catch (err) {
      if (err instanceof FlowApiError && err.status === 409) {
        await target.onStaleLayoutResync();
      }
    } finally {
      this.inflight = false;
      if (this.pending.size > 0) {
        this.scheduleFlush(target);
      }
    }
  }
}
