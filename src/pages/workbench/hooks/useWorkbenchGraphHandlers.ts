import { useCallback } from "react";
import { createNewFlowWorkbenchStore, FlowEdgeType } from "../../../features/flow/store/flow-workbench-store";
import {
  NEW_NODE_BASE_X,
  NEW_NODE_BASE_Y,
  NEW_NODE_STEP_X,
  NEW_NODE_STEP_Y,
  MODULO_COLS,
  MODULO_ROWS,
  getConditionLabel,
} from "../constants";

type WorkbenchStoreInstance = ReturnType<typeof createNewFlowWorkbenchStore>;

function buildNewNodePayload(
  nextIdx: number,
  customTitle?: string,
  customX?: number,
  customY?: number
) {
  const key = `SCR-0${nextIdx}-${String(Date.now()).slice(-5)}`;
  const id =
    typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `00000000-0000-4000-8000-${String(nextIdx).padStart(12, "0")}`;
  return {
    id,
    screenKey: key,
    title: customTitle || `Màn hình Mới ${nextIdx}`,
    routePath: `/screen-${nextIdx}`,
    posX: customX !== undefined ? customX : NEW_NODE_BASE_X + (nextIdx % MODULO_COLS) * NEW_NODE_STEP_X,
    posY: customY !== undefined ? customY : NEW_NODE_BASE_Y + (nextIdx % MODULO_ROWS) * NEW_NODE_STEP_Y,
    note: "Vừa khởi tạo",
  };
}

export function useWorkbenchGraphHandlers(store: WorkbenchStoreInstance, nodeCount: number) {
  const handleUpdatePosition = useCallback(
    (id: string, x: number, y: number) => store.updateNodePosition(id, x, y),
    [store]
  );

  const handleSetViewport = useCallback(
    (viewport: any) => store.setViewport(viewport),
    [store]
  );

  const handleAddNode = useCallback(
    (title?: string, posX?: number, posY?: number) => {
      store.addNode(buildNewNodePayload(nodeCount + 1, title, posX, posY));
    },
    [store, nodeCount]
  );

  const handleAddTransition = useCallback(
    (sourceId: string, targetId: string, edgeType: FlowEdgeType = "main") => {
      const res = store.addTransition({
        sourceNodeId: sourceId,
        targetNodeId: targetId,
        triggerEvent: "onClick",
        edgeType,
        conditionLabel: getConditionLabel(edgeType),
      });
      if (!res.success && res.error) {
        alert(res.error);
      }
    },
    [store]
  );

  const handleReverseTransition = useCallback(
    (id: string) => {
      const res = store.reverseTransition(id);
      if (!res.success && res.error) {
        alert(res.error);
      }
    },
    [store]
  );

  return {
    handleUpdatePosition,
    handleSetViewport,
    handleAddNode,
    handleAddTransition,
    handleReverseTransition,
  };
}
