// @story 04.3
// @implements AC-02, AC-01 (Scope Strip, Message Stream & Mid-turn Execution Controls)
import React from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import { WorkbenchNode } from "../store/flow-workbench-store";

export interface FloatingChatMessage {
  id: string;
  nodeId: string;
  role: "user" | "assistant";
  content: string;
  timestamp: number;
}

export interface FloatingChatStreamPanelProps {
  projectId: string;
  sessionId: string;
  selectedNodes: WorkbenchNode[];
  primaryNode: WorkbenchNode | null;
  isMulti: boolean;
  messages: FloatingChatMessage[];
  historyEndRef: React.RefObject<HTMLDivElement | null>;
  isRunning: boolean;
  isQueued: boolean;
  isPaused: boolean;
  activeJobId?: string;
  isSteering: boolean;
  steerDraft: string;
  onDeselectNode: (nodeId: string) => void;
  onFocusNode?: (nodeId: string) => void;
  onPauseJob: (jobId: string) => void;
  onResumeJob: (jobId: string) => void;
  onCancelJob: (jobId: string) => void;
  onToggleSteer: () => void;
  onSteerDraftChange: (value: string) => void;
  onSteerSubmit: () => void;
}

const ScopeChipStrip: React.FC<{
  projectId: string;
  sessionId: string;
  selectedNodes: WorkbenchNode[];
  onDeselectNode: (nodeId: string) => void;
  onFocusNode?: (nodeId: string) => void;
}> = ({ projectId, sessionId, selectedNodes, onDeselectNode, onFocusNode }) => (
  <div
    className="fc-scope-strip"
    style={{ padding: "4px 8px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", gap: "4px", flexWrap: "wrap" }}
  >
    {selectedNodes.map((n) => (
      <span
        key={n.id}
        title={`Composite Scope: ${projectId}::${sessionId}::${n.id}`}
        style={{ display: "inline-flex", alignItems: "center", gap: "3px", fontSize: "10px", fontWeight: 600, backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-xs)", padding: "1px 5px", color: "var(--text-primary)" }}
      >
        <span style={{ cursor: "pointer" }} onClick={() => onFocusNode?.(n.id)} title="Focus Node trên DAG">
          🎯 {n.screenKey}
        </span>
        {selectedNodes.length > 1 && (
          <button
            type="button"
            onClick={() => onDeselectNode(n.id)}
            style={{ border: "none", background: "transparent", color: "var(--text-muted)", cursor: "pointer", padding: 0, fontSize: "9px" }}
          >
            ✕
          </button>
        )}
      </span>
    ))}
  </div>
);

const MessageHistoryList: React.FC<{
  messages: FloatingChatMessage[];
  isMulti: boolean;
  selectedCount: number;
  primaryTitle?: string;
  historyEndRef: React.RefObject<HTMLDivElement | null>;
}> = ({ messages, isMulti, selectedCount, primaryTitle, historyEndRef }) => {
  const { t } = useTranslation();
  return (
    <div
      className="fc-history"
      style={{ flex: 1, overflowY: "auto", padding: "8px 10px", display: "flex", flexDirection: "column", gap: "6px", backgroundColor: "var(--bg-canvas)", fontSize: "12px" }}
    >
      {messages.length === 0 ? (
        <div style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--text-muted)", textAlign: "center", gap: "4px" }}>
          <BrandIcon name="sparkles" size={24} color="var(--action-primary)" />
          <div style={{ fontSize: "11.5px", fontWeight: 500 }}>
            {t("floatingChat.contextHeader", "Hội thoại ngữ cảnh")} {isMulti ? `${selectedCount} Màn hình` : primaryTitle}
          </div>
          <div style={{ fontSize: "10px" }}>
            {t("floatingChat.emptyHint", "Nhập yêu cầu để Agent tự động chỉnh sửa hoặc kết nối luồng")}
          </div>
        </div>
      ) : (
        messages.map((m) => (
          <div
            key={m.id}
            style={{
              alignSelf: m.role === "user" ? "flex-end" : "flex-start",
              maxWidth: "85%",
              padding: "6px 9px",
              borderRadius: "var(--radius-sm)",
              backgroundColor: m.role === "user" ? "var(--bg-selected)" : "var(--bg-surface)",
              border: "1px solid var(--border-subtle)",
              color: "var(--text-primary)",
              lineHeight: 1.4,
              fontSize: "11.5px",
            }}
          >
            {m.content}
          </div>
        ))
      )}
      <div ref={historyEndRef} />
    </div>
  );
};

const JobControlBar: React.FC<{
  activeJobId: string;
  isPaused: boolean;
  onPauseJob: (jobId: string) => void;
  onResumeJob: (jobId: string) => void;
  onToggleSteer: () => void;
  onCancelJob: (jobId: string) => void;
}> = ({ activeJobId, isPaused, onPauseJob, onResumeJob, onToggleSteer, onCancelJob }) => (
  <div style={{ padding: "4px 8px", backgroundColor: "var(--bg-subtle)", borderTop: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", justifyContent: "space-between", fontSize: "10.5px" }}>
    <span style={{ fontWeight: 600, color: "var(--action-primary)" }}>
      {isPaused ? "Đã tạm dừng" : "Đang xử lý..."}
    </span>
    <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
      <button
        type="button"
        onClick={() => (isPaused ? onResumeJob(activeJobId) : onPauseJob(activeJobId))}
        style={{ fontSize: "10px", padding: "1px 6px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", cursor: "pointer" }}
      >
        {isPaused ? "▶ Tiếp tục" : "⏸ Tạm dừng"}
      </button>
      <button
        type="button"
        onClick={onToggleSteer}
        style={{ fontSize: "10px", padding: "1px 6px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", cursor: "pointer" }}
      >
        🧭 Steer
      </button>
      <button
        type="button"
        onClick={() => onCancelJob(activeJobId)}
        style={{ fontSize: "10px", padding: "1px 6px", borderRadius: "var(--radius-xs)", border: "1px solid var(--state-danger)", color: "var(--state-danger)", backgroundColor: "var(--bg-surface)", cursor: "pointer" }}
      >
        ✕ Hủy
      </button>
    </div>
  </div>
);

const SteerInputBar: React.FC<{
  steerDraft: string;
  onSteerDraftChange: (value: string) => void;
  onSteerSubmit: () => void;
}> = ({ steerDraft, onSteerDraftChange, onSteerSubmit }) => (
  <div style={{ padding: "6px 8px", backgroundColor: "var(--bg-workspace)", borderTop: "1px solid var(--border-default)", display: "flex", gap: "4px" }}>
    <input
      type="text"
      placeholder="Chỉ dẫn điều hướng giữa phiên (Steer)..."
      value={steerDraft}
      onChange={(e) => onSteerDraftChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter") onSteerSubmit();
      }}
      style={{ flex: 1, fontSize: "11px", padding: "4px 6px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)" }}
    />
    <button
      type="button"
      onClick={onSteerSubmit}
      style={{ fontSize: "10.5px", padding: "3px 8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", border: "none", cursor: "pointer" }}
    >
      Gửi
    </button>
  </div>
);

export const FloatingChatStreamPanel: React.FC<FloatingChatStreamPanelProps> = (props) => {
  const showJobBar = (props.isRunning || props.isQueued || props.isPaused) && Boolean(props.activeJobId);
  return (
    <>
      <ScopeChipStrip
        projectId={props.projectId}
        sessionId={props.sessionId}
        selectedNodes={props.selectedNodes}
        onDeselectNode={props.onDeselectNode}
        onFocusNode={props.onFocusNode}
      />
      <MessageHistoryList
        messages={props.messages}
        isMulti={props.isMulti}
        selectedCount={props.selectedNodes.length}
        primaryTitle={props.primaryNode?.title}
        historyEndRef={props.historyEndRef}
      />
      {showJobBar && props.activeJobId && (
        <JobControlBar
          activeJobId={props.activeJobId}
          isPaused={props.isPaused}
          onPauseJob={props.onPauseJob}
          onResumeJob={props.onResumeJob}
          onToggleSteer={props.onToggleSteer}
          onCancelJob={props.onCancelJob}
        />
      )}
      {props.isSteering && (
        <SteerInputBar
          steerDraft={props.steerDraft}
          onSteerDraftChange={props.onSteerDraftChange}
          onSteerSubmit={props.onSteerSubmit}
        />
      )}
    </>
  );
};
