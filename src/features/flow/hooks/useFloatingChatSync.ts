// @story 04.4
// @implements AC-01, AC-02, AC-03 (Floating Chat Sync, Geometry & Daemon CAS Dispatch)
import { useEffect, useRef, useState } from "react";
import { apiFlowExecution } from "@/api/flow-execution";
import { globalKeyedParallelCoordinator, type NodeStatusSnapshot } from "../store/keyed-parallel-coordinator";
export { dispatchNodeExecutionToDaemon, dispatchSteerToDaemon, dispatchTransitionToDaemon } from "./dispatchNodeExecution";

export const FLOATING_CHAT_BOUNDS = {
  DEFAULT_WIDTH: 440,
  MIN_WIDTH: 360,
  MAX_WIDTH: 900,
  MIN_HEIGHT: 480,
  MAX_HEIGHT: 850,
  DEFAULT_BOTTOM: 20,
} as const;

export const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export interface FloatingChatMessage {
  id: string;
  nodeId: string;
  role: "user" | "assistant";
  content: string;
  timestamp: number;
  projectSeq?: number;
  fencingToken?: number;
}

function useFloatingDragListeners(params: {
  isDragging: boolean;
  width: number;
  isCollapsed: boolean;
  dragStartRef: React.MutableRefObject<{ mouseX: number; mouseY: number; startX: number; startBottom: number }>;
  setAnchor: (next: { x: number; bottom: number }) => void;
  setIsDragging: (next: boolean) => void;
}) {
  const { isDragging, width, isCollapsed, dragStartRef, setAnchor, setIsDragging } = params;
  useEffect(() => {
    if (!isDragging) return;
    const onMouseMove = (e: MouseEvent) => {
      const rawX = dragStartRef.current.startX + (e.clientX - dragStartRef.current.mouseX);
      const rawBottom = dragStartRef.current.startBottom - (e.clientY - dragStartRef.current.mouseY);
      const maxX = Math.max(12, window.innerWidth - width - 12);
      const maxBottom = Math.max(12, window.innerHeight - (isCollapsed ? 60 : 220));
      setAnchor({ x: Math.min(maxX, Math.max(12, rawX)), bottom: Math.min(maxBottom, Math.max(12, rawBottom)) });
    };
    const onMouseUp = () => setIsDragging(false);
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [isDragging, width, isCollapsed, dragStartRef, setAnchor, setIsDragging]);
}

export function useFloatingWindowGeometry(isCollapsed: boolean) {
  const [anchor, setAnchor] = useState<{ x: number; bottom: number } | null>(null);
  const [width, setWidth] = useState<number>(FLOATING_CHAT_BOUNDS.DEFAULT_WIDTH);
  const [customHeight, setCustomHeight] = useState<number | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const dragStartRef = useRef({ mouseX: 0, mouseY: 0, startX: 0, startBottom: FLOATING_CHAT_BOUNDS.DEFAULT_BOTTOM as number });

  useEffect(() => {
    if (anchor === null && typeof window !== "undefined") {
      setAnchor({ x: Math.max(12, Math.round(window.innerWidth / 2 - width / 2)), bottom: FLOATING_CHAT_BOUNDS.DEFAULT_BOTTOM });
    }
  }, [anchor, width]);

  useFloatingDragListeners({ isDragging, width, isCollapsed, dragStartRef, setAnchor, setIsDragging });

  const handleHeaderMouseDown = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    setIsDragging(true);
    const currX = anchor ? anchor.x : Math.round(window.innerWidth / 2 - width / 2);
    const currBottom = anchor ? anchor.bottom : FLOATING_CHAT_BOUNDS.DEFAULT_BOTTOM;
    dragStartRef.current = { mouseX: e.clientX, mouseY: e.clientY, startX: currX, startBottom: currBottom };
  };

  const handleRedockToBottom = () => {
    if (typeof window === "undefined") return;
    setWidth(FLOATING_CHAT_BOUNDS.DEFAULT_WIDTH);
    setCustomHeight(null);
    setAnchor({ x: Math.max(12, Math.round(window.innerWidth / 2 - FLOATING_CHAT_BOUNDS.DEFAULT_WIDTH / 2)), bottom: FLOATING_CHAT_BOUNDS.DEFAULT_BOTTOM });
  };

  return { anchor, setAnchor, width, setWidth, customHeight, setCustomHeight, isDragging, handleHeaderMouseDown, handleRedockToBottom };
}

export function startResizeDrag(
  e: React.MouseEvent,
  direction: "e" | "w" | "n",
  state: { width: number; height: number; x: number; bottom: number },
  onApply: (next: { width?: number; height?: number; x?: number }) => void
) {
  e.preventDefault();
  e.stopPropagation();
  const startX = e.clientX;
  const startY = e.clientY;
  const onMouseMove = (ev: MouseEvent) => {
    const dx = ev.clientX - startX;
    const dy = ev.clientY - startY;
    if (direction === "e") onApply({ width: Math.min(FLOATING_CHAT_BOUNDS.MAX_WIDTH, Math.max(FLOATING_CHAT_BOUNDS.MIN_WIDTH, state.width + dx)) });
    if (direction === "w") {
      const nextW = Math.min(FLOATING_CHAT_BOUNDS.MAX_WIDTH, Math.max(FLOATING_CHAT_BOUNDS.MIN_WIDTH, state.width - dx));
      onApply({ width: nextW, x: state.x + (state.width - nextW) });
    }
    if (direction === "n") onApply({ height: Math.min(FLOATING_CHAT_BOUNDS.MAX_HEIGHT, Math.max(FLOATING_CHAT_BOUNDS.MIN_HEIGHT, state.height - dy)) });
  };
  const onMouseUp = () => {
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
  };
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
}

export function useFloatingChatEffects(params: {
  projectId: string;
  sessionId: string;
  messages: FloatingChatMessage[];
  primaryStatus: NodeStatusSnapshot | null;
  historyEndRef: React.RefObject<HTMLDivElement | null>;
  streamAbortRef: React.MutableRefObject<AbortController | null>;
  onDaemonSynced?: (projectSeq: number) => void;
}) {
  const [, setSyncError] = useState<string | null>(null);

  useEffect(() => {
    params.historyEndRef.current?.scrollIntoView?.({ behavior: "smooth" });
  }, [params.messages, params.primaryStatus?.status, params.historyEndRef]);

  useEffect(() => {
    if (!UUID_RE.test(params.projectId)) return;
    let active = true;
    const syncDaemonSessions = async () => {
      try {
        const res = await apiFlowExecution.listNodeSessions(params.projectId, params.sessionId);
        if (!active) return;
        const seq = apiFlowExecution.getLastProjectSeq(params.projectId);
        globalKeyedParallelCoordinator.reconcileFromDaemon(res.jobs, seq);
        setSyncError(null);
        params.onDaemonSynced?.(seq);
      } catch (err) {
        if (active) setSyncError(err instanceof Error ? err.message : "sync_failed");
      }
    };
    void syncDaemonSessions();
    return () => { active = false; };
  }, [params.projectId, params.sessionId]);

  useEffect(() => () => {
    params.streamAbortRef.current?.abort();
  }, [params.streamAbortRef]);
}
