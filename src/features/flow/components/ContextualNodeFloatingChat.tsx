// @story 04.3
// @implements AC-02, AC-01 (Contextual Multi-Node Floating Chat v2 — Unified Sidebar UX & Progressive Height)
import React, { useState, useRef, useEffect, useCallback } from "react";
import { apiSdiEngine } from "@/api/sdi-engine";
import { apiWorkbenchSettings } from "@/api/workbench-settings";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import { AgentChatComposer } from "@/pages/workbench/components/AgentChatComposer";
import { WorkbenchNode } from "../store/flow-workbench-store";
import { useKeyedParallelCoordinator } from "../hooks/useKeyedParallelCoordinator";
import { FloatingChatCollapsedPill } from "./FloatingChatCollapsedPill";
import { FloatingChatStreamPanel, type FloatingChatMessage } from "./FloatingChatStreamPanel";

export interface ContextualNodeFloatingChatProps {
  projectId: string;
  sessionId: string;
  selectedNodes: WorkbenchNode[];
  onDeselectNode: (nodeId: string) => void;
  onClearSelection: () => void;
  onOpenLeftChat?: () => void;
  onFocusNode?: (nodeId: string) => void;
  onAttachContext?: (content: string) => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
  defaultExpanded?: boolean;
}

const DEFAULT_WIDTH = 440;
const MIN_WIDTH = 360;
const MAX_WIDTH = 860;
const MIN_HEIGHT = 180;
const MAX_HEIGHT = 640;
const DEFAULT_BOTTOM_OFFSET = 20;
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function useFloatingWindowGeometry(isCollapsed: boolean) {
  const [anchor, setAnchor] = useState<{ x: number; bottom: number } | null>(null);
  const [width, setWidth] = useState<number>(DEFAULT_WIDTH);
  const [customHeight, setCustomHeight] = useState<number | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const dragStartRef = useRef({ mouseX: 0, mouseY: 0, startX: 0, startBottom: DEFAULT_BOTTOM_OFFSET });

  useEffect(() => {
    if (anchor === null && typeof window !== "undefined") {
      setAnchor({ x: Math.max(12, Math.round(window.innerWidth / 2 - width / 2)), bottom: DEFAULT_BOTTOM_OFFSET });
    }
  }, [anchor, width]);

  const handleHeaderMouseDown = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    setIsDragging(true);
    const currX = anchor ? anchor.x : Math.round(window.innerWidth / 2 - width / 2);
    const currBottom = anchor ? anchor.bottom : DEFAULT_BOTTOM_OFFSET;
    dragStartRef.current = { mouseX: e.clientX, mouseY: e.clientY, startX: currX, startBottom: currBottom };
  };

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
  }, [isDragging, width, isCollapsed]);

  const handleRedockToBottom = () => {
    if (typeof window === "undefined") return;
    setWidth(DEFAULT_WIDTH);
    setCustomHeight(null);
    setAnchor({ x: Math.max(12, Math.round(window.innerWidth / 2 - DEFAULT_WIDTH / 2)), bottom: DEFAULT_BOTTOM_OFFSET });
  };

  return { anchor, setAnchor, width, setWidth, customHeight, setCustomHeight, isDragging, handleHeaderMouseDown, handleRedockToBottom };
}

function startResizeDrag(
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
    if (direction === "e") onApply({ width: Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, state.width + dx)) });
    if (direction === "w") {
      const nextW = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, state.width - dx));
      onApply({ width: nextW, x: state.x + (state.width - nextW) });
    }
    if (direction === "n") {
      onApply({ height: Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, state.height - dy)) });
    }
  };
  const onMouseUp = () => {
    window.removeEventListener("mousemove", onMouseMove);
    window.removeEventListener("mouseup", onMouseUp);
  };
  window.addEventListener("mousemove", onMouseMove);
  window.addEventListener("mouseup", onMouseUp);
}

const FloatingChatHeader: React.FC<{
  projectId: string;
  titleText: string;
  isMulti: boolean;
  isDragging: boolean;
  isRunning: boolean;
  isQueued: boolean;
  queuePosition?: number;
  onMouseDown: (e: React.MouseEvent) => void;
  onRedock: () => void;
  onOpenLeftChat?: () => void;
  onCollapse: () => void;
  onClose: () => void;
}> = (p) => {
  const { t } = useTranslation();
  return (
    <div
      className="fc-drag-header"
      onMouseDown={p.onMouseDown}
      style={{ height: "36px", minHeight: "36px", padding: "0 10px 0 12px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", justifyContent: "space-between", cursor: p.isDragging ? "grabbing" : "grab", userSelect: "none", flexShrink: 0 }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: "6px", minWidth: 0, flex: 1 }}>
        <BrandIcon name="crosshair" size={13} color="var(--action-primary)" style={{ flexShrink: 0 }} />
        <span style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{p.titleText}</span>
        <span style={{ fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--text-muted)", backgroundColor: "var(--bg-subtle)", padding: "1px 5px", borderRadius: "var(--radius-xs)", flexShrink: 0 }}>
          {p.isMulti ? t("floatingChat.parallelMode", "Song song") : t("floatingChat.fifoMode", "FIFO per-node")}
        </span>
        {p.isRunning && <span style={{ fontSize: "9px", fontWeight: 700, color: "var(--action-primary)", backgroundColor: "var(--bg-selected)", padding: "1px 5px", borderRadius: "var(--radius-xs)" }}>{t("floatingChat.runningBadge", "RUNNING")}</span>}
        {p.isQueued && <span style={{ fontSize: "9px", fontWeight: 700, color: "var(--state-warning)", backgroundColor: "var(--bg-subtle)", padding: "1px 5px", borderRadius: "var(--radius-xs)" }}>{t("floatingChat.queuedBadge", "QUEUED")} #{p.queuePosition}</span>}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "3px" }}>
        <button type="button" onClick={p.onRedock} title={t("flow.redockBottom", "Khôi phục vị trí đáy mặc định")} aria-label={t("flow.redockBottom", "Khôi phục vị trí đáy mặc định")} style={{ width: "24px", height: "24px", border: "none", background: "transparent", cursor: "pointer", color: "var(--text-muted)", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)" }}>
          <BrandIcon name="fit-view" size={12} color="currentColor" />
        </button>
        {p.onOpenLeftChat && (
          <button type="button" onClick={p.onOpenLeftChat} title={t("flow.openLeftChat", "Mở sang khung chat trái")} aria-label={t("flow.openLeftChat", "Mở sang khung chat trái")} style={{ width: "24px", height: "24px", border: "none", background: "transparent", cursor: "pointer", color: "var(--text-muted)", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)" }}>
            <BrandIcon name="panel-left-open" size={12} color="currentColor" />
          </button>
        )}
        <button type="button" onClick={p.onCollapse} title={t("floatingChat.collapsePill", "Thu gọn thành Pill (Esc)")} aria-label={t("floatingChat.collapsePill", "Thu gọn thành Pill (Esc)")} style={{ width: "24px", height: "24px", border: "none", background: "transparent", cursor: "pointer", color: "var(--text-muted)", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)" }}>
          <BrandIcon name="chevron-down" size={12} color="currentColor" />
        </button>
        <button type="button" onClick={p.onClose} title={t("floatingChat.closeChat", "Đóng khung chat")} aria-label={t("floatingChat.closeChat", "Đóng khung chat")} style={{ width: "24px", height: "24px", border: "none", background: "transparent", cursor: "pointer", color: "var(--text-muted)", display: "inline-flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)" }}>
          <BrandIcon name="close" size={12} color="currentColor" />
        </button>
      </div>
    </div>
  );
};

async function dispatchNodeJobToDaemon(params: {
  projectId: string;
  sessionId: string;
  primaryNode: WorkbenchNode | null;
  prompt: string;
  jobs: Array<{ id: string; nodeId: string }>;
  coordinator: ReturnType<typeof useKeyedParallelCoordinator>;
  streamAbortRef: React.MutableRefObject<AbortController | null>;
  onAppendMsg: (msg: FloatingChatMessage) => void;
  t: (key: string, fallback: string) => string;
  formatNumber: (value: number) => string;
}) {
  if (!UUID_RE.test(params.projectId)) return;
  params.streamAbortRef.current?.abort();
  const controller = new AbortController();
  params.streamAbortRef.current = controller;

  void apiWorkbenchSettings
    .streamJobEvents(params.projectId, {
      nodeId: params.primaryNode?.id,
      signal: controller.signal,
      onEvents: (events) => {
        const summaryText = events.map((ev) => `[${ev.eventKind}] ${ev.payloadText || ev.nodeId}`).join("\n");
        if (!summaryText) return;
        params.onAppendMsg({ id: `sse-${Date.now()}-${events[0]?.seqNo ?? 0}`, nodeId: params.primaryNode?.id || "unknown", role: "assistant", content: summaryText, timestamp: Date.now() });
      },
    })
    .catch(() => {});

  try {
    for (const job of params.jobs) {
      await apiWorkbenchSettings.postJobEvent(params.projectId, { jobId: job.id, nodeId: job.nodeId, sessionId: params.sessionId, eventKind: "started", payloadText: JSON.stringify({ prompt: params.prompt }), jobStatus: "RUNNING" });
    }
    const intent = await apiSdiEngine.compileIntent(params.projectId, { rawPrompt: params.prompt, targetNodeId: params.primaryNode?.id });
    const clarifyPct = params.formatNumber(Math.round(intent.ambiguityScore * 100));
    const summary = intent.status === "clarifying"
      ? `${params.t("chat.clarifyPrefix", "Cần làm rõ")} (${clarifyPct}%): ${intent.clarificationVariants?.map((v) => v.title).join(" | ") || intent.intentType}`
      : `${params.t("floatingChat.compiledIntent", "Đã biên dịch intent")} (${intent.intentType}) · status=${intent.status}`;
    params.onAppendMsg({ id: `resp-${intent.id}`, nodeId: params.primaryNode?.id || "unknown", role: "assistant", content: summary, timestamp: Date.now() });
    for (const job of params.jobs) {
      await apiWorkbenchSettings.postJobEvent(params.projectId, { jobId: job.id, nodeId: job.nodeId, sessionId: params.sessionId, eventKind: "completed", payloadText: JSON.stringify({ intentId: intent.id, summary }), jobStatus: "COMPLETED" });
      params.coordinator.completeJob(job.id);
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : params.t("floatingChat.daemonRequestFailed", "Yêu cầu Daemon thất bại");
    params.onAppendMsg({ id: `err-${Date.now()}`, nodeId: params.primaryNode?.id || "unknown", role: "assistant", content: `${params.t("floatingChat.daemonSyncError", "Không thể đồng bộ với Rust daemon")}: ${msg}`, timestamp: Date.now() });
  } finally {
    controller.abort();
  }
}

const FloatingChatResizeHandles: React.FC<{
  resizeCtx: { width: number; height: number; x: number; bottom: number };
  onApply: (next: { width?: number; height?: number; x?: number }) => void;
}> = ({ resizeCtx, onApply }) => (
  <>
    <div style={{ position: "absolute", top: "6px", bottom: "6px", right: 0, width: "6px", cursor: "ew-resize" }} onMouseDown={(e) => startResizeDrag(e, "e", resizeCtx, onApply)} />
    <div style={{ position: "absolute", top: "6px", bottom: "6px", left: 0, width: "6px", cursor: "ew-resize" }} onMouseDown={(e) => startResizeDrag(e, "w", resizeCtx, onApply)} />
    <div style={{ position: "absolute", top: 0, left: "6px", right: "6px", height: "6px", cursor: "ns-resize" }} onMouseDown={(e) => startResizeDrag(e, "n", resizeCtx, onApply)} />
  </>
);

function useFloatingChatController(props: ContextualNodeFloatingChatProps) {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const coordinator = useKeyedParallelCoordinator();
  const [isCollapsed, setIsCollapsed] = useState(!props.defaultExpanded);
  const [showReviewSummary, setShowReviewSummary] = useState(false);
  const [messages, setMessages] = useState<FloatingChatMessage[]>([]);
  const [inputPrompt, setInputPrompt] = useState("");
  const [steerDraft, setSteerDraft] = useState("");
  const [isSteering, setIsSteering] = useState(false);
  const geo = useFloatingWindowGeometry(isCollapsed);
  const containerRef = useRef<HTMLDivElement>(null);
  const historyEndRef = useRef<HTMLDivElement>(null);
  const streamAbortRef = useRef<AbortController | null>(null);
  const primaryNode = props.selectedNodes[0] || null;
  const primaryStatus = primaryNode ? coordinator.getNodeStatus(primaryNode.id) : null;

  useEffect(() => { historyEndRef.current?.scrollIntoView?.({ behavior: "smooth" }); }, [messages, primaryStatus?.status]);
  useEffect(() => () => { streamAbortRef.current?.abort(); }, []);

  const appendMsg = useCallback((m: FloatingChatMessage) => setMessages((prev) => [...prev, m]), []);
  const handleSendMessage = () => {
    if (!inputPrompt.trim() || props.selectedNodes.length === 0) return;
    const prompt = inputPrompt.trim();
    setInputPrompt("");
    appendMsg({ id: `msg-${Date.now()}`, nodeId: primaryNode?.id || "unknown", role: "user", content: prompt, timestamp: Date.now() });
    const jobs = props.selectedNodes.map((n) => coordinator.enqueueJob({ projectId: props.projectId, sessionId: props.sessionId, nodeId: n.id, prompt }));
    void dispatchNodeJobToDaemon({ projectId: props.projectId, sessionId: props.sessionId, primaryNode, prompt, jobs, coordinator, streamAbortRef, onAppendMsg: appendMsg, t, formatNumber });
  };
  const handleSteerSubmit = () => {
    if (!steerDraft.trim() || !primaryStatus?.jobId) return;
    const instruction = steerDraft.trim();
    coordinator.steerJob(primaryStatus.jobId, instruction);
    appendMsg({ id: `steer-${Date.now()}`, nodeId: primaryNode?.id || "unknown", role: "user", content: `${t("floatingChat.steerPrefix", "Chỉ dẫn thêm (Steer)")}: ${instruction}`, timestamp: Date.now() });
    if (UUID_RE.test(props.projectId) && primaryNode) {
      void apiWorkbenchSettings.postJobEvent(props.projectId, { jobId: primaryStatus.jobId, nodeId: primaryNode.id, sessionId: props.sessionId, eventKind: "steered", payloadText: JSON.stringify({ instruction }), jobStatus: "RUNNING" }).catch(() => {});
    }
    setSteerDraft("");
    setIsSteering(false);
  };
  return { coordinator, isCollapsed, setIsCollapsed, showReviewSummary, setShowReviewSummary, messages, inputPrompt, setInputPrompt, steerDraft, setSteerDraft, isSteering, setIsSteering, geo, containerRef, historyEndRef, primaryNode, primaryStatus, handleSendMessage, handleSteerSubmit };
}

export const ContextualNodeFloatingChat: React.FC<ContextualNodeFloatingChatProps> = (props) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const c = useFloatingChatController(props);
  if (props.selectedNodes.length === 0) return null;

  const isMulti = props.selectedNodes.length > 1;
  const winX = c.geo.anchor ? c.geo.anchor.x : 24;
  const winBottom = c.geo.anchor ? c.geo.anchor.bottom : DEFAULT_BOTTOM_OFFSET;

  if (c.isCollapsed) {
    return (
      <FloatingChatCollapsedPill
        containerRef={c.containerRef} pillX={winX} pillBottom={winBottom} isDragging={c.geo.isDragging}
        isMulti={isMulti} selectedCount={props.selectedNodes.length} primaryNode={c.primaryNode}
        primaryNodeStatus={c.primaryStatus} showReviewSummary={c.showReviewSummary}
        onHeaderMouseDown={c.geo.handleHeaderMouseDown} onExpand={() => c.setIsCollapsed(false)}
        onToggleReviewSummary={() => c.setShowReviewSummary((prev) => !prev)}
        onCloseReviewSummary={() => c.setShowReviewSummary(false)} onClearSelection={props.onClearSelection}
      />
    );
  }

  const titleText = isMulti
    ? `${formatNumber(props.selectedNodes.length)} ${t("floatingChat.screensSelectedSuffix", "Màn hình đang chọn")}`
    : `${c.primaryNode?.screenKey}: ${c.primaryNode?.title}`;
  const resizeCtx = { width: c.geo.width, height: c.geo.customHeight ?? c.containerRef.current?.offsetHeight ?? MIN_HEIGHT, x: winX, bottom: winBottom };
  const applyResize = (next: { width?: number; height?: number; x?: number }) => {
    if (next.width !== undefined) c.geo.setWidth(next.width);
    if (next.height !== undefined) c.geo.setCustomHeight(next.height);
    if (next.x !== undefined) c.geo.setAnchor({ x: next.x, bottom: winBottom });
  };
  const composerPlaceholder = isMulti
    ? t("floatingChat.multiNodePlaceholder", "Yêu cầu Agent xử lý đồng loạt các màn hình đang chọn...")
    : `${t("floatingChat.askAgentAboutPrefix", "Hỏi Agent về")} ${c.primaryNode?.title}... ${t("floatingChat.enterHintSuffix", "(Enter gửi, Shift+Enter xuống dòng)")}`;

  return (
    <div
      ref={c.containerRef}
      className="contextual-node-floating-chat"
      data-testid="contextual-node-floating-chat"
      style={{
        position: "fixed", left: `${winX}px`, bottom: `${winBottom}px`,
        width: `${c.geo.width}px`, minWidth: `${MIN_WIDTH}px`, maxWidth: `${MAX_WIDTH}px`,
        height: c.geo.customHeight ? `${c.geo.customHeight}px` : "auto",
        maxHeight: "min(560px, calc(100vh - 96px))",
        backgroundColor: "var(--bg-canvas)", border: "1px solid var(--border-default)",
        borderRadius: "var(--radius-lg)", boxShadow: "var(--shadow-lg)",
        display: "flex", flexDirection: "column", overflow: "hidden", zIndex: 55, boxSizing: "border-box",
      }}
    >
      <FloatingChatHeader
        projectId={props.projectId} titleText={titleText} isMulti={isMulti} isDragging={c.geo.isDragging}
        isRunning={c.primaryStatus?.status === "running"} isQueued={c.primaryStatus?.status === "queued"} queuePosition={c.primaryStatus?.queuePosition}
        onMouseDown={c.geo.handleHeaderMouseDown} onRedock={c.geo.handleRedockToBottom}
        onOpenLeftChat={props.onOpenLeftChat} onCollapse={() => c.setIsCollapsed(true)} onClose={props.onClearSelection}
      />
      <FloatingChatStreamPanel
        projectId={props.projectId} sessionId={props.sessionId} selectedNodes={props.selectedNodes}
        primaryNode={c.primaryNode} isMulti={isMulti} messages={c.messages} historyEndRef={c.historyEndRef}
        isRunning={c.primaryStatus?.status === "running"} isQueued={c.primaryStatus?.status === "queued"}
        isPaused={c.primaryStatus?.status === "paused"} activeJobId={c.primaryStatus?.jobId}
        isSteering={c.isSteering} steerDraft={c.steerDraft} onDeselectNode={props.onDeselectNode}
        onFocusNode={props.onFocusNode} onPauseJob={(id) => c.coordinator.pauseJob(id)}
        onResumeJob={(id) => c.coordinator.resumeJob(id)} onCancelJob={(id) => c.coordinator.cancelJob(id)}
        onToggleSteer={() => c.setIsSteering((p) => !p)} onSteerDraftChange={c.setSteerDraft} onSteerSubmit={c.handleSteerSubmit}
      />
      <AgentChatComposer
        inputValue={c.inputPrompt} onInputChange={c.setInputPrompt} onSend={c.handleSendMessage}
        attachmentChips={[]} onOpenSettings={props.onOpenSettings} testIdPrefix="floating-chat-composer" compact
        placeholder={composerPlaceholder}
      />
      <FloatingChatResizeHandles resizeCtx={resizeCtx} onApply={applyResize} />
    </div>
  );
};
