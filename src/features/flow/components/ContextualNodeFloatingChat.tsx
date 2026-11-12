// @story 04.3
// @implements AC-02, AC-01 (Contextual Multi-Node Floating Chat v2)
import React, { useState, useRef, useEffect, useCallback } from "react";
import { useTranslation } from "@/hooks/useTranslation";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { WorkbenchNode } from "../store/flow-workbench-store";
import { useKeyedParallelCoordinator } from "../hooks/useKeyedParallelCoordinator";

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

interface ChatMessage {
  id: string;
  nodeId: string;
  role: "user" | "assistant";
  content: string;
  timestamp: number;
}

const MIN_WIDTH = 360;
const MAX_WIDTH = 900;
const MIN_HEIGHT = 220;
const MAX_HEIGHT = 850;

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
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputPrompt, setInputPrompt] = useState("");
  const [steerDraft, setSteerDraft] = useState("");
  const [isSteering, setIsSteering] = useState(false);

  // Position and Size state
  const [position, setPosition] = useState<{ x: number; y: number } | null>(null);
  const [size, setSize] = useState<{ width: number; height: number }>({
    width: 480,
    height: 420,
  });

  const [isDragging, setIsDragging] = useState(false);
  const dragStartRef = useRef<{ mouseX: number; mouseY: number; startX: number; startY: number }>({
    mouseX: 0,
    mouseY: 0,
    startX: 0,
    startY: 0,
  });

  const containerRef = useRef<HTMLDivElement>(null);
  const historyEndRef = useRef<HTMLDivElement>(null);

  // Primary active node (first selected)
  const primaryNode = selectedNodes[0] || null;
  const isMulti = selectedNodes.length > 1;

  // Active job for the primary node (if any)
  const primaryNodeStatus = primaryNode ? coordinator.getNodeStatus(primaryNode.id) : null;
  const isRunning = primaryNodeStatus?.status === "running";
  const isQueued = primaryNodeStatus?.status === "queued";
  const isPaused = primaryNodeStatus?.status === "paused";
  const activeJobId = primaryNodeStatus?.jobId;

  // Auto-scroll chat history
  useEffect(() => {
    if (typeof historyEndRef.current?.scrollIntoView === "function") {
      historyEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [messages, isRunning]);

  // Center window initially
  useEffect(() => {
    if (position === null && typeof window !== "undefined") {
      const initialX = Math.max(12, Math.round(window.innerWidth / 2 - size.width / 2));
      const initialY = Math.max(60, window.innerHeight - size.height - 24);
      setPosition({ x: initialX, y: initialY });
    }
  }, [position, size]);

  // Handle Dragging
  const handleHeaderMouseDown = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    setIsDragging(true);
    const currX = position ? position.x : Math.round(window.innerWidth / 2 - size.width / 2);
    const currY = position ? position.y : window.innerHeight - size.height - 24;
    dragStartRef.current = {
      mouseX: e.clientX,
      mouseY: e.clientY,
      startX: currX,
      startY: currY,
    };
  };

  useEffect(() => {
    if (!isDragging) return;

    const onMouseMove = (e: MouseEvent) => {
      const dx = e.clientX - dragStartRef.current.mouseX;
      const dy = e.clientY - dragStartRef.current.mouseY;
      const rawX = dragStartRef.current.startX + dx;
      const rawY = dragStartRef.current.startY + dy;

      const maxX = Math.max(12, window.innerWidth - size.width - 12);
      const maxY = Math.max(54, window.innerHeight - (isCollapsed ? 50 : size.height) - 12);

      setPosition({
        x: Math.min(maxX, Math.max(12, rawX)),
        y: Math.min(maxY, Math.max(54, rawY)),
      });
    };

    const onMouseUp = () => {
      setIsDragging(false);
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [isDragging, size, isCollapsed]);

  // Handle Resize Corner / Edges
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

      if (direction.includes("e")) {
        newW = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, startW + dx));
      }
      if (direction.includes("w")) {
        const potentialW = startW - dx;
        newW = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, potentialW));
        newPosX = startPosX + (startW - newW);
      }
      if (direction.includes("s")) {
        newH = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, startH + dy));
      }
      if (direction.includes("n")) {
        const potentialH = startH - dy;
        newH = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, potentialH));
        newPosY = startPosY + (startH - newH);
      }

      setSize({ width: newW, height: newH });
      if (newPosX !== startPosX || newPosY !== startPosY) {
        setPosition({ x: newPosX, y: newPosY });
      }
    };

    const onMouseUp = () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  const handleRedockToBottom = () => {
    if (typeof window !== "undefined") {
      const targetW = 480;
      const targetH = 420;
      setSize({ width: targetW, height: targetH });
      setPosition({
        x: Math.round(window.innerWidth / 2 - targetW / 2),
        y: window.innerHeight - targetH - 24,
      });
    }
  };

  const handleSendMessage = () => {
    if (!inputPrompt.trim() || selectedNodes.length === 0) return;
    const prompt = inputPrompt.trim();
    setInputPrompt("");

    // Add to local message stream
    const userMsg: ChatMessage = {
      id: `msg-${Date.now()}`,
      nodeId: primaryNode?.id || "unknown",
      role: "user",
      content: prompt,
      timestamp: Date.now(),
    };
    setMessages((prev) => [...prev, userMsg]);

    // Enqueue execution jobs for all selected nodes
    for (const node of selectedNodes) {
      coordinator.enqueueJob({
        projectId,
        sessionId,
        nodeId: node.id,
        prompt,
      });
    }

    // Simulate Agent response completion after small delay in test/prototype
    setTimeout(() => {
      setMessages((prev) => [
        ...prev,
        {
          id: `resp-${Date.now()}`,
          nodeId: primaryNode?.id || "unknown",
          role: "assistant",
          content: `✓ Đã tiếp nhận yêu cầu cho ${
            selectedNodes.length === 1 ? primaryNode?.title : `${selectedNodes.length} màn hình`
          }. Đang điều phối thực thi.`,
          timestamp: Date.now(),
        },
      ]);
    }, 600);
  };

  const handleSteerSubmit = () => {
    if (!steerDraft.trim() || !activeJobId) return;
    coordinator.steerJob(activeJobId, steerDraft.trim());
    setMessages((prev) => [
      ...prev,
      {
        id: `steer-${Date.now()}`,
        nodeId: primaryNode?.id || "unknown",
        role: "user",
        content: `🧭 Chỉ dẫn thêm (Steer): ${steerDraft.trim()}`,
        timestamp: Date.now(),
      },
    ]);
    setSteerDraft("");
    setIsSteering(false);
  };

  if (selectedNodes.length === 0) return null;

  // -------------------------------------------------------------
  // TIER 1: Collapsed Pill Badge (42px)
  // -------------------------------------------------------------
  if (isCollapsed) {
    const pillX = position ? position.x : 24;
    const pillY = position ? position.y : 60;

    return (
      <div
        ref={containerRef}
        className="fc-compact-pill"
        data-testid="fc-compact-pill"
        style={{
          position: "fixed",
          left: `${pillX}px`,
          top: `${pillY}px`,
          height: "42px",
          padding: "0 12px",
          borderRadius: "var(--radius-pill)",
          backgroundColor: "var(--bg-elevated)",
          border: "1px solid var(--border-strong)",
          boxShadow: "var(--shadow-lg)",
          display: "flex",
          alignItems: "center",
          gap: "8px",
          cursor: isDragging ? "grabbing" : "pointer",
          zIndex: 55,
          userSelect: "none",
        }}
        onMouseDown={handleHeaderMouseDown}
      >
        <div
          style={{ display: "flex", alignItems: "center", gap: "6px" }}
          onClick={() => setIsCollapsed(false)}
        >
          <span style={{ fontSize: "12px", fontWeight: 700, color: "var(--action-primary)" }}>
            🎯 {isMulti ? `${selectedNodes.length} Nodes` : primaryNode?.screenKey}
          </span>
          {isRunning && (
            <span
              style={{
                fontSize: "10px",
                fontWeight: 600,
                color: "var(--action-primary)",
                backgroundColor: "rgba(59, 130, 246, 0.15)",
                padding: "2px 6px",
                borderRadius: "var(--radius-xs)",
              }}
            >
              ⚡ Đang xử lý
            </span>
          )}
          {isQueued && (
            <span
              style={{
                fontSize: "10px",
                fontWeight: 600,
                color: "#f59e0b",
                backgroundColor: "rgba(245, 158, 11, 0.15)",
                padding: "2px 6px",
                borderRadius: "var(--radius-xs)",
              }}
            >
              ⏳ Hàng đợi #{primaryNodeStatus?.queuePosition}
            </span>
          )}
        </div>

        <button
          type="button"
          onClick={() => setShowReviewSummary((prev) => !prev)}
          style={{
            fontSize: "10.5px",
            padding: "2px 8px",
            borderRadius: "var(--radius-pill)",
            backgroundColor: "var(--bg-subtle)",
            border: "1px solid var(--border-default)",
            color: "var(--text-secondary)",
            cursor: "pointer",
          }}
        >
          Review Summary
        </button>

        <button
          type="button"
          onClick={() => setIsCollapsed(false)}
          title="Mở rộng khung Chat"
          style={{
            border: "none",
            background: "transparent",
            cursor: "pointer",
            fontSize: "12px",
            color: "var(--text-muted)",
            padding: "2px",
          }}
        >
          ▲
        </button>

        {showReviewSummary && (
          <div
            role="dialog"
            aria-label="Tóm tắt công việc Node"
            style={{
              position: "absolute",
              bottom: "48px",
              left: 0,
              width: "280px",
              backgroundColor: "var(--bg-surface)",
              border: "1px solid var(--border-default)",
              borderRadius: "var(--radius-md)",
              padding: "10px",
              boxShadow: "var(--shadow-lg)",
              zIndex: 60,
              fontSize: "11px",
              color: "var(--text-primary)",
            }}
          >
            <div style={{ fontWeight: 600, marginBottom: "4px" }}>
              Tóm tắt Node {primaryNode?.screenKey}
            </div>
            <div style={{ color: "var(--text-secondary)", marginBottom: "8px" }}>
              Tiêu đề: {primaryNode?.title}
              <br />
              Trạng thái: {primaryNode?.status}
              <br />
              Sub-State: {primaryNode?.activeSubState || "Default"}
            </div>
            <button
              type="button"
              onClick={() => setShowReviewSummary(false)}
              style={{
                fontSize: "10px",
                padding: "2px 8px",
                borderRadius: "var(--radius-xs)",
                border: "1px solid var(--border-default)",
                backgroundColor: "var(--bg-subtle)",
                cursor: "pointer",
              }}
            >
              Đóng
            </button>
          </div>
        )}
      </div>
    );
  }

  // -------------------------------------------------------------
  // TIER 2: Full Movable & Resizable Window
  // -------------------------------------------------------------
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
      {/* Zone A: Drag Header Bar */}
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
          <span style={{ color: "var(--text-muted)", fontSize: "12px", letterSpacing: "-1px" }}>
            ⋮⋮
          </span>
          <span
            style={{
              fontSize: "10px",
              fontFamily: "var(--font-mono)",
              color: "var(--text-muted)",
              backgroundColor: "var(--bg-canvas)",
              padding: "1px 4px",
              borderRadius: "var(--radius-xs)",
            }}
          >
            {projectId.slice(0, 8)}
          </span>
          <span
            style={{
              fontSize: "12px",
              fontWeight: 600,
              color: "var(--text-primary)",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {isMulti
              ? `🎯 ${selectedNodes.length} Màn hình đang chọn`
              : `🎯 ${primaryNode?.screenKey}: ${primaryNode?.title}`}
          </span>
          {isRunning && (
            <span
              style={{
                fontSize: "9px",
                fontWeight: 700,
                color: "var(--action-primary)",
                backgroundColor: "rgba(59, 130, 246, 0.15)",
                padding: "1px 5px",
                borderRadius: "var(--radius-xs)",
              }}
            >
              RUNNING
            </span>
          )}
          {isQueued && (
            <span
              style={{
                fontSize: "9px",
                fontWeight: 700,
                color: "#f59e0b",
                backgroundColor: "rgba(245, 158, 11, 0.15)",
                padding: "1px 5px",
                borderRadius: "var(--radius-xs)",
              }}
            >
              QUEUED #{primaryNodeStatus?.queuePosition}
            </span>
          )}
        </div>

        {/* Window controls */}
        <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
          <button
            type="button"
            onClick={handleRedockToBottom}
            title={t("flow.redockBottom", "Đưa về đáy chuẩn (⌖)")}
            style={{
              border: "none",
              background: "transparent",
              cursor: "pointer",
              fontSize: "12px",
              color: "var(--text-muted)",
              padding: "2px 4px",
            }}
          >
            ⌖
          </button>
          {onOpenLeftChat && (
            <button
              type="button"
              onClick={onOpenLeftChat}
              title={t("flow.openLeftChat", "Mở sang khung chat trái")}
              style={{
                border: "none",
                background: "transparent",
                cursor: "pointer",
                fontSize: "11px",
                color: "var(--text-muted)",
                padding: "2px 4px",
              }}
            >
              ◧
            </button>
          )}
          <button
            type="button"
            onClick={() => setIsCollapsed(true)}
            title="Thu gọn thành Pill (Esc)"
            style={{
              border: "none",
              background: "transparent",
              cursor: "pointer",
              fontSize: "11px",
              color: "var(--text-muted)",
              padding: "2px 4px",
            }}
          >
            ▾
          </button>
          <button
            type="button"
            onClick={onClearSelection}
            title="Đóng khung chat"
            style={{
              border: "none",
              background: "transparent",
              cursor: "pointer",
              fontSize: "12px",
              color: "var(--text-muted)",
              padding: "2px 4px",
            }}
          >
            ✕
          </button>
        </div>
      </div>

      {/* Zone B: Scope & Multi-Node Chips Strip */}
      <div
        className="fc-scope-strip"
        style={{
          padding: "4px 8px",
          backgroundColor: "var(--bg-workspace)",
          borderBottom: "1px solid var(--border-subtle)",
          display: "flex",
          alignItems: "center",
          gap: "4px",
          flexWrap: "wrap",
        }}
      >
        {selectedNodes.map((n) => {
          const compKey = `${projectId}::${sessionId}::${n.id}`;
          return (
            <span
              key={n.id}
              title={`Composite Scope: ${compKey}`}
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: "3px",
                fontSize: "10px",
                fontWeight: 600,
                backgroundColor: "var(--bg-surface)",
                border: "1px solid var(--border-default)",
                borderRadius: "var(--radius-xs)",
                padding: "1px 5px",
                color: "var(--text-primary)",
              }}
            >
              <span
                style={{ cursor: "pointer" }}
                onClick={() => onFocusNode?.(n.id)}
                title="Focus Node trên DAG"
              >
                🎯 {n.screenKey}
              </span>
              {selectedNodes.length > 1 && (
                <button
                  type="button"
                  onClick={() => onDeselectNode(n.id)}
                  style={{
                    border: "none",
                    background: "transparent",
                    color: "var(--text-muted)",
                    cursor: "pointer",
                    padding: 0,
                    fontSize: "9px",
                  }}
                >
                  ✕
                </button>
              )}
            </span>
          );
        })}
      </div>

      {/* Zone C: Scrollable Message Stream */}
      <div
        className="fc-history"
        style={{
          flex: 1,
          overflowY: "auto",
          padding: "8px 10px",
          display: "flex",
          flexDirection: "column",
          gap: "6px",
          backgroundColor: "var(--bg-canvas)",
          fontSize: "12px",
        }}
      >
        {messages.length === 0 ? (
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
              color: "var(--text-muted)",
              textAlign: "center",
              gap: "4px",
            }}
          >
            <BrandIcon name="sparkles" size={24} color="var(--action-primary)" />
            <div style={{ fontSize: "11.5px", fontWeight: 500 }}>
              Hội thoại ngữ cảnh {isMulti ? `${selectedNodes.length} Màn hình` : primaryNode?.title}
            </div>
            <div style={{ fontSize: "10px" }}>
              Nhập yêu cầu để Agent tự động chỉnh sửa hoặc kết nối luồng
            </div>
          </div>
        ) : (
          messages.map((m) => {
            const isUser = m.role === "user";
            return (
              <div
                key={m.id}
                style={{
                  alignSelf: isUser ? "flex-end" : "flex-start",
                  maxWidth: "85%",
                  padding: "6px 9px",
                  borderRadius: "var(--radius-sm)",
                  backgroundColor: isUser ? "var(--bg-selected)" : "var(--bg-surface)",
                  border: "1px solid var(--border-subtle)",
                  color: "var(--text-primary)",
                  lineHeight: 1.4,
                  fontSize: "11.5px",
                }}
              >
                {m.content}
              </div>
            );
          })
        )}
        <div ref={historyEndRef} />
      </div>

      {/* Mid-turn Action Controls (Pause / Steer / Cancel) */}
      {(isRunning || isQueued || isPaused) && activeJobId && (
        <div
          style={{
            padding: "4px 8px",
            backgroundColor: "var(--bg-subtle)",
            borderTop: "1px solid var(--border-subtle)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            fontSize: "10.5px",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
            <span style={{ fontWeight: 600, color: "var(--action-primary)" }}>
              {isPaused ? "Đã tạm dừng" : "Đang xử lý..."}
            </span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
            {isPaused ? (
              <button
                type="button"
                onClick={() => coordinator.resumeJob(activeJobId)}
                style={{
                  fontSize: "10px",
                  padding: "1px 6px",
                  borderRadius: "var(--radius-xs)",
                  border: "1px solid var(--border-default)",
                  backgroundColor: "var(--bg-surface)",
                  cursor: "pointer",
                }}
              >
                ▶ Tiếp tục
              </button>
            ) : (
              <button
                type="button"
                onClick={() => coordinator.pauseJob(activeJobId)}
                style={{
                  fontSize: "10px",
                  padding: "1px 6px",
                  borderRadius: "var(--radius-xs)",
                  border: "1px solid var(--border-default)",
                  backgroundColor: "var(--bg-surface)",
                  cursor: "pointer",
                }}
              >
                ⏸ Tạm dừng
              </button>
            )}

            <button
              type="button"
              onClick={() => setIsSteering((p) => !p)}
              style={{
                fontSize: "10px",
                padding: "1px 6px",
                borderRadius: "var(--radius-xs)",
                border: "1px solid var(--border-default)",
                backgroundColor: "var(--bg-surface)",
                cursor: "pointer",
              }}
            >
              🧭 Steer
            </button>

            <button
              type="button"
              onClick={() => coordinator.cancelJob(activeJobId)}
              style={{
                fontSize: "10px",
                padding: "1px 6px",
                borderRadius: "var(--radius-xs)",
                border: "1px solid var(--state-danger)",
                color: "var(--state-danger)",
                backgroundColor: "var(--bg-surface)",
                cursor: "pointer",
              }}
            >
              ✕ Hủy
            </button>
          </div>
        </div>
      )}

      {/* Steer Input Drawer */}
      {isSteering && (
        <div
          style={{
            padding: "6px 8px",
            backgroundColor: "var(--bg-workspace)",
            borderTop: "1px solid var(--border-default)",
            display: "flex",
            gap: "4px",
          }}
        >
          <input
            type="text"
            placeholder="Chỉ dẫn điều hướng giữa phiên (Steer)..."
            value={steerDraft}
            onChange={(e) => setSteerDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleSteerSubmit();
            }}
            style={{
              flex: 1,
              fontSize: "11px",
              padding: "4px 6px",
              borderRadius: "var(--radius-xs)",
              border: "1px solid var(--border-default)",
              backgroundColor: "var(--bg-surface)",
              color: "var(--text-primary)",
            }}
          />
          <button
            type="button"
            onClick={handleSteerSubmit}
            style={{
              fontSize: "10.5px",
              padding: "3px 8px",
              borderRadius: "var(--radius-xs)",
              backgroundColor: "var(--action-primary)",
              color: "var(--action-primary-fg)",
              border: "none",
              cursor: "pointer",
            }}
          >
            Gửi
          </button>
        </div>
      )}

      {/* Zone D: Composer & Execution Footer */}
      <div
        className="fc-footer"
        style={{
          padding: "6px 10px",
          backgroundColor: "var(--bg-elevated)",
          borderTop: "1px solid var(--border-subtle)",
          display: "flex",
          flexDirection: "column",
          gap: "6px",
        }}
      >
        <div
          className="fc-composer-box"
          style={{
            display: "flex",
            flexDirection: "column",
            gap: "4px",
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-default)",
            borderRadius: "var(--radius-sm)",
            padding: "6px 8px",
          }}
        >
          <textarea
            value={inputPrompt}
            onChange={(e) => setInputPrompt(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleSendMessage();
              }
            }}
            placeholder={
              isMulti
                ? "Yêu cầu Agent xử lý đồng loạt các màn hình đang chọn..."
                : `Yêu cầu chỉnh sửa ${primaryNode?.title}... (Enter gửi)`
            }
            rows={2}
            style={{
              width: "100%",
              resize: "none",
              border: "none",
              outline: "none",
              backgroundColor: "transparent",
              color: "var(--text-primary)",
              fontFamily: "inherit",
              fontSize: "12px",
              lineHeight: 1.4,
              boxSizing: "border-box",
            }}
          />
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              paddingTop: "2px",
            }}
          >
            <span
              style={{
                fontSize: "10px",
                color: "var(--text-muted)",
                display: "flex",
                alignItems: "center",
                gap: "3px",
              }}
            >
              🤖 Cowok Swarm · {isMulti ? "Song song" : "FIFO per-node"}
            </span>
            <button
              type="button"
              onClick={handleSendMessage}
              style={{
                padding: "3px 10px",
                borderRadius: "var(--radius-xs)",
                backgroundColor: "var(--action-primary)",
                color: "var(--action-primary-fg)",
                fontSize: "11px",
                fontWeight: 600,
                border: "none",
                cursor: "pointer",
              }}
            >
              {isQueued ? "⏳ Xếp hàng ↵" : "Gửi AI ↵"}
            </button>
          </div>
        </div>
      </div>

      {/* Resize Handles */}
      <div
        style={{
          position: "absolute",
          top: "6px",
          bottom: "6px",
          right: 0,
          width: "6px",
          cursor: "ew-resize",
        }}
        onMouseDown={(e) => handleResizeStart(e, "e")}
      />
      <div
        style={{
          position: "absolute",
          top: "6px",
          bottom: "6px",
          left: 0,
          width: "6px",
          cursor: "ew-resize",
        }}
        onMouseDown={(e) => handleResizeStart(e, "w")}
      />
      <div
        style={{
          position: "absolute",
          top: 0,
          left: "6px",
          right: "6px",
          height: "6px",
          cursor: "ns-resize",
        }}
        onMouseDown={(e) => handleResizeStart(e, "n")}
      />
      <div
        style={{
          position: "absolute",
          bottom: "2px",
          right: "2px",
          width: "14px",
          height: "14px",
          cursor: "nwse-resize",
          display: "grid",
          placeItems: "center",
          color: "var(--text-muted)",
          fontSize: "10px",
          userSelect: "none",
        }}
        onMouseDown={(e) => handleResizeStart(e, "se")}
      >
        ◢
      </div>
    </div>
  );
};
