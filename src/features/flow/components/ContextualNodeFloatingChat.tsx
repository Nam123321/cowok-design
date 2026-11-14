// @story 04.3
// @implements AC-02, AC-01 (Contextual Multi-Node Floating Chat v2)
import React, { useState, useRef, useEffect, useCallback } from "react";
import { apiSdiEngine } from "@/api/sdi-engine";
import { apiWorkbenchSettings } from "@/api/workbench-settings";
import { useTranslation } from "@/hooks/useTranslation";
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
}

const MIN_WIDTH = 360;
const MAX_WIDTH = 900;
const MIN_HEIGHT = 220;
const MAX_HEIGHT = 850;
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export const ContextualNodeFloatingChat: React.FC<ContextualNodeFloatingChatProps> = ({
  projectId,
  sessionId,
  selectedNodes,
  onDeselectNode,
  onClearSelection,
  onOpenLeftChat,
  onFocusNode,
}) => {
  const { t } = useTranslation();
  const coordinator = useKeyedParallelCoordinator();
  const [isCollapsed, setIsCollapsed] = useState(false);
  const [showReviewSummary, setShowReviewSummary] = useState(false);
  const [messages, setMessages] = useState<FloatingChatMessage[]>([]);
  const [inputPrompt, setInputPrompt] = useState("");
  const [steerDraft, setSteerDraft] = useState("");
  const [isSteering, setIsSteering] = useState(false);
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);
  const [size, setSize] = useState<{ width: number; height: number }>({ width: 480, height: 420 });
  const [isDragging, setIsDragging] = useState(false);
  const dragStartRef = useRef({ mouseX: 0, mouseY: 0, startX: 0, startY: 0 });
  const containerRef = useRef<HTMLDivElement>(null);
  const historyEndRef = useRef<HTMLDivElement>(null);
  const streamAbortRef = useRef<AbortController | null>(null);

  const primaryNode = selectedNodes[0] || null;
  const isMulti = selectedNodes.length > 1;
  const primaryNodeStatus = primaryNode ? coordinator.getNodeStatus(primaryNode.id) : null;
  const isRunning = primaryNodeStatus?.status === "running";
  const isQueued = primaryNodeStatus?.status === "queued";
  const isPaused = primaryNodeStatus?.status === "paused";
  const activeJobId = primaryNodeStatus?.jobId;

  useEffect(() => {
    if (typeof historyEndRef.current?.scrollIntoView === "function") {
      historyEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages, isRunning]);

  useEffect(() => {
    return () => {
      streamAbortRef.current?.abort();
    };
  }, []);

  useEffect(() => {
    if (position === null && typeof window !== "undefined") {
      setPosition({
        x: Math.max(12, Math.round(window.innerWidth / 2 - size.width / 2)),
        y: Math.max(60, window.innerHeight - size.height - 24),
      });
    }
  }, [position, size]);

  const handleHeaderMouseDown = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    setIsDragging(true);
    const currX = position ? position.x : Math.round(window.innerWidth / 2 - size.width / 2);
    const currY = position ? position.y : window.innerHeight - size.height - 24;
    dragStartRef.current = { mouseX: e.clientX, mouseY: e.clientY, startX: currX, startY: currY };
  };

  useEffect(() => {
    if (!isDragging) return;
    const onMouseMove = (e: MouseEvent) => {
      const rawX = dragStartRef.current.startX + (e.clientX - dragStartRef.current.mouseX);
      const rawY = dragStartRef.current.startY + (e.clientY - dragStartRef.current.mouseY);
      const maxX = Math.max(12, window.innerWidth - size.width - 12);
      const maxY = Math.max(54, window.innerHeight - (isCollapsed ? 50 : size.height) - 12);
      setPosition({ x: Math.min(maxX, Math.max(12, rawX)), y: Math.min(maxY, Math.max(54, rawY)) });
    };
    const onMouseUp = () => setIsDragging(false);
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [isDragging, size, isCollapsed]);

  const handleResizeStart = (e: React.MouseEvent, direction: "se" | "e" | "s" | "w" | "n") => {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX;
    const startY = e.clientY;
    const startW = size.width;
    const startH = size.height;
    const startPosX = position ? position.x : 0;
    const startPosY = position ? position.y : 0;

    const onMouseMove = (ev: MouseEvent) => {
      const dx = ev.clientX - startX;
      const dy = ev.clientY - startY;
      let newW = startW;
      let newH = startH;
      let newPosX = startPosX;
      let newPosY = startPosY;
      if (direction.includes("e")) newW = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startW + dx));
      if (direction.includes("w")) {
        newW = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startW - dx));
        newPosX = startPosX + (startW - newW);
      }
      if (direction.includes("s")) newH = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, startH + dy));
      if (direction.includes("n")) {
        newH = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, startH - dy));
        newPosY = startPosY + (startH - newH);
      }
      setSize({ width: newW, height: newH });
      if (newPosX !== startPosX || newPosY !== startPosY) setPosition({ x: newPosX, y: newPosY });
    };
    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  const handleRedockToBottom = () => {
    if (typeof window === "undefined") return;
    const targetW = 480;
    const targetH = 420;
    setSize({ width: targetW, height: targetH });
    setPosition({
      x: Math.round(window.innerWidth / 2 - targetW / 2),
      y: window.innerHeight - targetH - 24,
    });
  };

  const dispatchJobsToDaemon = useCallback(
    async (prompt: string, jobs: Array<{ id: string; nodeId: string }>) => {
      if (!UUID_RE.test(projectId)) return;
      streamAbortRef.current?.abort();
      const controller = new AbortController();
      streamAbortRef.current = controller;

      void apiWorkbenchSettings
        .streamJobEvents(projectId, {
          nodeId: primaryNode?.id,
          signal: controller.signal,
          onEvents: (events) => {
            const summaryText = events
              .map((ev) => `[${ev.eventKind}] ${ev.payloadText || ev.nodeId}`)
              .join("\n");
            if (!summaryText) return;
            setMessages((prev) => [
              ...prev,
              {
                id: `sse-${Date.now()}-${events[0]?.seqNo ?? 0}`,
                nodeId: primaryNode?.id || "unknown",
                role: "assistant",
                content: summaryText,
                timestamp: Date.now(),
              },
            ]);
          },
        })
        .catch(() => {});

      try {
        for (const job of jobs) {
          await apiWorkbenchSettings.postJobEvent(projectId, {
            jobId: job.id,
            nodeId: job.nodeId,
            sessionId,
            eventKind: "started",
            payloadText: JSON.stringify({ prompt }),
            jobStatus: "RUNNING",
          });
        }
        const intent = await apiSdiEngine.compileIntent(projectId, {
          rawPrompt: prompt,
          targetNodeId: primaryNode?.id,
        });
        const statusSummary =
          intent.status === "clarifying"
            ? `Cần làm rõ (ambiguity ${Math.round(intent.ambiguityScore * 100)}%): ${
                intent.clarificationVariants?.map((v) => v.title).join(" | ") || intent.intentType
              }`
            : `Đã biên dịch intent (${intent.intentType}) · status=${intent.status}`;
        setMessages((prev) => [
          ...prev,
          {
            id: `resp-${intent.id}`,
            nodeId: primaryNode?.id || "unknown",
            role: "assistant",
            content: statusSummary,
            timestamp: Date.now(),
          },
        ]);
        for (const job of jobs) {
          await apiWorkbenchSettings.postJobEvent(projectId, {
            jobId: job.id,
            nodeId: job.nodeId,
            sessionId,
            eventKind: "completed",
            payloadText: JSON.stringify({ intentId: intent.id, summary: statusSummary }),
            jobStatus: "COMPLETED",
          });
          coordinator.completeJob(job.id);
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : "Daemon request failed";
        setMessages((prev) => [
          ...prev,
          {
            id: `err-${Date.now()}`,
            nodeId: primaryNode?.id || "unknown",
            role: "assistant",
            content: `⚠ Không thể đồng bộ với Rust daemon: ${msg}`,
            timestamp: Date.now(),
          },
        ]);
      } finally {
        controller.abort();
      }
    },
    [projectId, sessionId, primaryNode, coordinator]
  );

  const handleSendMessage = () => {
    if (!inputPrompt.trim() || selectedNodes.length === 0) return;
    const prompt = inputPrompt.trim();
    setInputPrompt("");

    setMessages((prev) => [
      ...prev,
      {
        id: `msg-${Date.now()}`,
        nodeId: primaryNode?.id || "unknown",
        role: "user",
        content: prompt,
        timestamp: Date.now(),
      },
    ]);

    const jobs = selectedNodes.map((node) =>
      coordinator.enqueueJob({ projectId, sessionId, nodeId: node.id, prompt })
    );
    void dispatchJobsToDaemon(prompt, jobs);
  };

  const handleSteerSubmit = () => {
    if (!steerDraft.trim() || !activeJobId) return;
    const instruction = steerDraft.trim();
    coordinator.steerJob(activeJobId, instruction);
    setMessages((prev) => [
      ...prev,
      {
        id: `steer-${Date.now()}`,
        nodeId: primaryNode?.id || "unknown",
        role: "user",
        content: `🧭 Chỉ dẫn thêm (Steer): ${instruction}`,
        timestamp: Date.now(),
      },
    ]);
    if (UUID_RE.test(projectId) && primaryNode) {
      void apiWorkbenchSettings
        .postJobEvent(projectId, {
          jobId: activeJobId,
          nodeId: primaryNode.id,
          sessionId,
          eventKind: "steered",
          payloadText: JSON.stringify({ instruction }),
          jobStatus: "RUNNING",
        })
        .catch(() => {});
    }
    setSteerDraft("");
    setIsSteering(false);
  };

  if (selectedNodes.length === 0) return null;

  if (isCollapsed) {
    return (
      <FloatingChatCollapsedPill
        containerRef={containerRef}
        pillX={position ? position.x : 24}
        pillY={position ? position.y : 60}
        isDragging={isDragging}
        isMulti={isMulti}
        selectedCount={selectedNodes.length}
        primaryNode={primaryNode}
        primaryNodeStatus={primaryNodeStatus}
        showReviewSummary={showReviewSummary}
        onHeaderMouseDown={handleHeaderMouseDown}
        onExpand={() => setIsCollapsed(false)}
        onToggleReviewSummary={() => setShowReviewSummary((prev) => !prev)}
        onCloseReviewSummary={() => setShowReviewSummary(false)}
      />
    );
  }

  const winX = position ? position.x : 24;
  const winY = position ? position.y : 60;

  return (
    <div
      ref={containerRef}
      className="contextual-node-floating-chat"
      data-testid="contextual-node-floating-chat"
      style={{
        position: "fixed",
        left: `${winX}px`,
        top: `${winY}px`,
        width: `${size.width}px`,
        height: `${size.height}px`,
        minWidth: `${MIN_WIDTH}px`,
        maxWidth: `${MAX_WIDTH}px`,
        minHeight: `${MIN_HEIGHT}px`,
        maxHeight: `${MAX_HEIGHT}px`,
        backgroundColor: "var(--bg-elevated)",
        border: "1.5px solid var(--action-primary)",
        borderRadius: "var(--radius-lg)",
        boxShadow: "var(--shadow-lg)",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        zIndex: 55,
        boxSizing: "border-box",
      }}
    >
      <div
        className="fc-drag-header"
        onMouseDown={handleHeaderMouseDown}
        style={{
          height: "36px",
          minHeight: "36px",
          padding: "0 10px",
          backgroundColor: "var(--bg-surface)",
          borderBottom: "1px solid var(--border-subtle)",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          cursor: isDragging ? "grabbing" : "grab",
          userSelect: "none",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: "6px", minWidth: 0, flex: 1 }}>
          <span style={{ color: "var(--text-muted)", fontSize: "12px", letterSpacing: "-1px" }}>⋮⋮</span>
          <span style={{ fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--text-muted)", backgroundColor: "var(--bg-canvas)", padding: "1px 4px", borderRadius: "var(--radius-xs)" }}>
            {projectId.slice(0, 8)}
          </span>
          <span style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {isMulti ? `🎯 ${selectedNodes.length} Màn hình đang chọn` : `🎯 ${primaryNode?.screenKey}: ${primaryNode?.title}`}
          </span>
          {isRunning && (
            <span style={{ fontSize: "9px", fontWeight: 700, color: "var(--action-primary)", backgroundColor: "var(--bg-selected)", padding: "1px 5px", borderRadius: "var(--radius-xs)" }}>
              RUNNING
            </span>
          )}
          {isQueued && (
            <span style={{ fontSize: "9px", fontWeight: 700, color: "var(--state-warning)", backgroundColor: "var(--bg-subtle)", padding: "1px 5px", borderRadius: "var(--radius-xs)" }}>
              QUEUED #{primaryNodeStatus?.queuePosition}
            </span>
          )}
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <button type="button" onClick={handleRedockToBottom} title={t("flow.redockBottom", "Đưa về đáy chuẩn (⌖)")} style={{ border: "none", background: "transparent", cursor: "pointer", fontSize: "12px", color: "var(--text-muted)", padding: "2px 4px" }}>⌖</button>
          {onOpenLeftChat && (
            <button type="button" onClick={onOpenLeftChat} title={t("flow.openLeftChat", "Mở sang khung chat trái")} style={{ border: "none", background: "transparent", cursor: "pointer", fontSize: "11px", color: "var(--text-muted)", padding: "2px 4px" }}>◧</button>
          )}
          <button type="button" onClick={() => setIsCollapsed(true)} title="Thu gọn thành Pill (Esc)" style={{ border: "none", background: "transparent", cursor: "pointer", fontSize: "11px", color: "var(--text-muted)", padding: "2px 4px" }}>▾</button>
          <button type="button" onClick={onClearSelection} title="Đóng khung chat" style={{ border: "none", background: "transparent", cursor: "pointer", fontSize: "12px", color: "var(--text-muted)", padding: "2px 4px" }}>✕</button>
        </div>
      </div>

      <FloatingChatStreamPanel
        projectId={projectId}
        sessionId={sessionId}
        selectedNodes={selectedNodes}
        primaryNode={primaryNode}
        isMulti={isMulti}
        messages={messages}
        historyEndRef={historyEndRef}
        isRunning={isRunning}
        isQueued={isQueued}
        isPaused={isPaused}
        activeJobId={activeJobId}
        isSteering={isSteering}
        steerDraft={steerDraft}
        onDeselectNode={onDeselectNode}
        onFocusNode={onFocusNode}
        onPauseJob={(id) => coordinator.pauseJob(id)}
        onResumeJob={(id) => coordinator.resumeJob(id)}
        onCancelJob={(id) => coordinator.cancelJob(id)}
        onToggleSteer={() => setIsSteering((p) => !p)}
        onSteerDraftChange={setSteerDraft}
        onSteerSubmit={handleSteerSubmit}
      />

      <div className="fc-footer" style={{ padding: "6px 10px", backgroundColor: "var(--bg-elevated)", borderTop: "1px solid var(--border-subtle)", display: "flex", flexDirection: "column", gap: "6px" }}>
        <div className="fc-composer-box" style={{ display: "flex", flexDirection: "column", gap: "4px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-sm)", padding: "6px 8px" }}>
          <textarea
            value={inputPrompt}
            onChange={(e) => setInputPrompt(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSendMessage();
              }
            }}
            placeholder={isMulti ? "Yêu cầu Agent xử lý đồng loạt các màn hình đang chọn..." : `Yêu cầu chỉnh sửa ${primaryNode?.title}... (Enter gửi)`}
            rows={2}
            style={{ width: "100%", resize: "none", border: "none", outline: "none", backgroundColor: "transparent", color: "var(--text-primary)", fontFamily: "inherit", fontSize: "12px", lineHeight: 1.4, boxSizing: "border-box" }}
          />
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", paddingTop: "2px" }}>
            <span style={{ fontSize: "10px", color: "var(--text-muted)", display: "flex", alignItems: "center", gap: "3px" }}>
              🤖 Cowok Swarm · {isMulti ? "Song song" : "FIFO per-node"}
            </span>
            <button type="button" onClick={handleSendMessage} style={{ padding: "3px 10px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", fontSize: "11px", fontWeight: 600, border: "none", cursor: "pointer" }}>
              {isQueued ? "⏳ Xếp hàng ↵" : "Gửi AI ↵"}
            </button>
          </div>
        </div>
      </div>

      <div style={{ position: "absolute", top: "6px", bottom: "6px", right: 0, width: "6px", cursor: "ew-resize" }} onMouseDown={(e) => handleResizeStart(e, "e")} />
      <div style={{ position: "absolute", top: "6px", bottom: "6px", left: 0, width: "6px", cursor: "ew-resize" }} onMouseDown={(e) => handleResizeStart(e, "w")} />
      <div style={{ position: "absolute", top: 0, left: "6px", right: "6px", height: "6px", cursor: "ns-resize" }} onMouseDown={(e) => handleResizeStart(e, "n")} />
      <div style={{ position: "absolute", bottom: "2px", right: "2px", width: "14px", height: "14px", cursor: "nwse-resize", display: "grid", placeItems: "center", color: "var(--text-muted)", fontSize: "10px", userSelect: "none" }} onMouseDown={(e) => handleResizeStart(e, "se")}>
        ◢
      </div>
    </div>
  );
};
