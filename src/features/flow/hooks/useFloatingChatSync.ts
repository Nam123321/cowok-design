// @story 04.4
// @implements AC-01, AC-02, AC-03 (Floating Chat Sync, 8-Dir Resize Geometry & Shared Chat Bridge)
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { apiFlowExecution } from "@/api/flow-execution";
import { globalKeyedParallelCoordinator, type NodeStatusSnapshot } from "../store/keyed-parallel-coordinator";
import { sharedChatStore, toSessionChatKey } from "../store/shared-chat-sync";
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
export type ResizeDirection = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

export interface FloatingChatMessage {
  id: string;
  nodeId: string;
  role: "user" | "assistant";
  content: string;
  timestamp: number;
  projectSeq?: number;
  fencingToken?: number;
}

function clampBound(val: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, val));
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
      setAnchor({ x: clampBound(rawX, 12, maxX), bottom: clampBound(rawBottom, 12, maxBottom) });
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

const FLOATING_CHAT_GEOMETRY_KEY = "cowok_floating_chat_geometry";

interface StoredFloatingGeometry {
  x?: number;
  bottom?: number;
  width?: number;
  height?: number | null;
}

function readStoredFloatingGeometry(): StoredFloatingGeometry | null {
  if (typeof window === "undefined") return null;
  try {
    const saved = localStorage.getItem(FLOATING_CHAT_GEOMETRY_KEY);
    return saved ? (JSON.parse(saved) as StoredFloatingGeometry) : null;
  } catch (err) {
    console.warn("[floating-chat] failed to read geometry from localStorage", err);
    return null;
  }
}

function writeStoredFloatingGeometry(payload: StoredFloatingGeometry | null) {
  if (typeof window === "undefined") return;
  try {
    if (payload === null) {
      localStorage.removeItem(FLOATING_CHAT_GEOMETRY_KEY);
    } else {
      localStorage.setItem(FLOATING_CHAT_GEOMETRY_KEY, JSON.stringify(payload));
    }
  } catch (err) {
    console.warn("[floating-chat] failed to update geometry in localStorage", err);
  }
}

function resolveInitialAnchor(): { x: number; bottom: number } | null {
  const parsed = readStoredFloatingGeometry();
  if (!parsed || typeof parsed.x !== "number" || typeof parsed.bottom !== "number") return null;
  return {
    x: clampBound(parsed.x, 12, Math.max(12, window.innerWidth - 360)),
    bottom: clampBound(parsed.bottom, 12, Math.max(12, window.innerHeight - 60)),
  };
}

function resolveInitialWidth(): number {
  const parsed = readStoredFloatingGeometry();
  if (!parsed || typeof parsed.width !== "number") return FLOATING_CHAT_BOUNDS.DEFAULT_WIDTH;
  return clampBound(parsed.width, FLOATING_CHAT_BOUNDS.MIN_WIDTH, FLOATING_CHAT_BOUNDS.MAX_WIDTH);
}

function resolveInitialHeight(): number | null {
  const parsed = readStoredFloatingGeometry();
  if (!parsed || typeof parsed.height !== "number") return null;
  const maxAllowed = Math.min(FLOATING_CHAT_BOUNDS.MAX_HEIGHT, window.innerHeight - 24);
  return clampBound(parsed.height, FLOATING_CHAT_BOUNDS.MIN_HEIGHT, maxAllowed);
}

export function useFloatingWindowGeometry(isCollapsed: boolean) {
  const [anchor, setAnchor] = useState<{ x: number; bottom: number } | null>(resolveInitialAnchor);
  const [width, setWidth] = useState<number>(resolveInitialWidth);
  const [customHeight, setCustomHeight] = useState<number | null>(resolveInitialHeight);
  const [isDragging, setIsDragging] = useState(false);
  const dragStartRef = useRef({ mouseX: 0, mouseY: 0, startX: 0, startBottom: FLOATING_CHAT_BOUNDS.DEFAULT_BOTTOM as number });

  useEffect(() => {
    if (anchor === null && typeof window !== "undefined") {
      setAnchor({ x: Math.max(12, Math.round(window.innerWidth / 2 - width / 2)), bottom: FLOATING_CHAT_BOUNDS.DEFAULT_BOTTOM });
    }
  }, [anchor, width]);

  useEffect(() => {
    if (anchor) writeStoredFloatingGeometry({ x: anchor.x, bottom: anchor.bottom, width, height: customHeight });
  }, [anchor, width, customHeight]);

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
    writeStoredFloatingGeometry(null);
  };

  return { anchor, setAnchor, width, setWidth, customHeight, setCustomHeight, isDragging, handleHeaderMouseDown, handleRedockToBottom };
}

export function computeResizedBounds(
  direction: ResizeDirection,
  dx: number,
  dy: number,
  state: { width: number; height: number; x: number; bottom: number }
): { width?: number; height?: number; x?: number; bottom?: number } {
  const out: { width?: number; height?: number; x?: number; bottom?: number } = {};
  if (direction.includes("e")) {
    out.width = clampBound(state.width + dx, FLOATING_CHAT_BOUNDS.MIN_WIDTH, FLOATING_CHAT_BOUNDS.MAX_WIDTH);
  } else if (direction.includes("w")) {
    const rawW = state.width - dx;
    const maxGrowLeft = Math.max(0, state.x - 12);
    const nextW = dx < 0
      ? Math.min(clampBound(rawW, FLOATING_CHAT_BOUNDS.MIN_WIDTH, FLOATING_CHAT_BOUNDS.MAX_WIDTH), state.width + maxGrowLeft)
      : clampBound(rawW, FLOATING_CHAT_BOUNDS.MIN_WIDTH, FLOATING_CHAT_BOUNDS.MAX_WIDTH);
    out.width = nextW;
    out.x = Math.max(12, state.x - (nextW - state.width));
  }
  if (direction.includes("n")) {
    out.height = clampBound(state.height - dy, FLOATING_CHAT_BOUNDS.MIN_HEIGHT, FLOATING_CHAT_BOUNDS.MAX_HEIGHT);
  } else if (direction.includes("s")) {
    const rawH = clampBound(state.height + dy, FLOATING_CHAT_BOUNDS.MIN_HEIGHT, FLOATING_CHAT_BOUNDS.MAX_HEIGHT);
    const maxGrowDown = Math.max(0, state.bottom - 12);
    const clampedH = dy > 0 ? Math.min(rawH, state.height + maxGrowDown) : rawH;
    out.height = clampedH;
    out.bottom = Math.max(12, state.bottom - (clampedH - state.height));
  }
  return out;
}

export function startResizeDrag(
  e: React.MouseEvent,
  direction: ResizeDirection,
  state: { width: number; height: number; x: number; bottom: number },
  onApply: (next: { width?: number; height?: number; x?: number; bottom?: number }) => void
) {
  e.preventDefault();
  e.stopPropagation();
  const startX = e.clientX;
  const startY = e.clientY;
  const onMouseMove = (ev: MouseEvent) => onApply(computeResizedBounds(direction, ev.clientX - startX, ev.clientY - startY, state));
  const onMouseUp = () => {
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
  };
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
}

export function useFloatingSharedMessages(projectId: string, sessionId: string, selectedNodeIds: string[]): FloatingChatMessage[] {
  const snap = useSyncExternalStore(sharedChatStore.subscribe, sharedChatStore.getSnapshot, sharedChatStore.getSnapshot);
  const scopeKey = toSessionChatKey(projectId, sessionId);
  const nodeKey = selectedNodeIds.join(",");
  return useMemo(() => {
    const allow = new Set(selectedNodeIds);
    return sharedChatStore.getMessages(scopeKey)
      .filter((m) => !m.nodeId || allow.size === 0 || allow.has(m.nodeId))
      .map((m) => ({ id: m.id, nodeId: m.nodeId || "session", role: m.sender === "human" ? "user" : "assistant", content: m.text, timestamp: m.createdAtMs, projectSeq: m.projectSeq, fencingToken: m.fencingToken }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [snap.version, scopeKey, nodeKey]);
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
        globalKeyedParallelCoordinator.reconcileFromDaemon(res.jobs, seq, { projectId: params.projectId, sessionId: params.sessionId });
        setSyncError(null);
        params.onDaemonSynced?.(seq);
      } catch (err) {
        if (active) setSyncError(err instanceof Error ? err.message : "sync_failed");
      }
    };
    void syncDaemonSessions();
    return () => { active = false; };
  }, [params.projectId, params.sessionId]);

  useEffect(() => {
    const ref = params.streamAbortRef;
    return () => { ref.current?.abort(); };
  }, [params.projectId, params.streamAbortRef]);
}
