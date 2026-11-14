// @story 04.3
// @implements AC-02, AC-01 (Scope Strip, Unified Sidebar Stream & Mid-turn Execution Controls)
import React from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { ChatStreamItem, type ChatMessage } from "@/pages/workbench/components/AgentChatStream";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
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
}> = ({ projectId, sessionId, selectedNodes, onDeselectNode, onFocusNode }) => {
  const { t } = useTranslation();
  return (
    <div
      className="fc-scope-strip"
      style={{ padding: "5px 10px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", gap: "5px", flexWrap: "wrap", flexShrink: 0 }}
    >
      {selectedNodes.map((n) => (
        <span
          key={n.id}
          title={`${t("floatingChat.compositeScopePrefix", "Composite Scope")}: ${projectId}::${sessionId}::${n.id}`}
          style={{ display: "inline-flex", alignItems: "center", gap: "4px", fontSize: "10.5px", fontWeight: 500, backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", borderRadius: "var(--radius-pill)", padding: "2px 8px", color: "var(--text-primary)" }}
        >
          <BrandIcon name="crosshair" size={10} color="var(--action-primary)" />
          <span style={{ cursor: "pointer" }} onClick={() => onFocusNode?.(n.id)} title={t("floatingChat.focusNodeOnDag", "Focus Node trên DAG")}>
            {n.screenKey}
          </span>
          {selectedNodes.length > 1 && (
            <button
              type="button"
              onClick={() => onDeselectNode(n.id)}
              aria-label={`${t("floatingChat.deselectNode", "Bỏ chọn")} ${n.screenKey}`}
              style={{ border: "none", background: "transparent", color: "var(--text-muted)", cursor: "pointer", padding: 0, display: "inline-flex", alignItems: "center" }}
            >
              <BrandIcon name="close" size={10} color="currentColor" />
            </button>
          )}
        </span>
      ))}
    </div>
  );
};

const MessageHistoryList: React.FC<{
  messages: FloatingChatMessage[];
  isMulti: boolean;
  selectedCount: number;
  primaryTitle?: string;
  historyEndRef: React.RefObject<HTMLDivElement | null>;
}> = ({ messages, isMulti, selectedCount, primaryTitle, historyEndRef }) => {
  const { t, locale } = useTranslation();
  const { formatTime, formatNumber } = useFormatter({ locale });
  if (messages.length === 0) {
    return (
      <div
        className="fc-history"
        style={{ padding: "8px 12px", display: "flex", alignItems: "center", gap: "6px", backgroundColor: "var(--bg-canvas)", fontSize: "11.5px", color: "var(--text-muted)", flexShrink: 0 }}
      >
        <BrandIcon name="sparkles" size={12} color="var(--agent-accent)" />
        <span>
          {t("floatingChat.emptyHint", "Nhập yêu cầu cho")} {isMulti ? `${formatNumber(selectedCount)} ${t("floatingChat.screensUnit", "màn hình")}` : primaryTitle || t("floatingChat.singleScreenFallback", "màn hình")}
        </span>
      </div>
    );
  }
  return (
    <div
      className="fc-history"
      style={{ flex: "0 1 auto", minHeight: 0, overflowY: "auto", padding: "10px 12px", display: "flex", flexDirection: "column", gap: "10px", backgroundColor: "var(--bg-canvas)" }}
    >
      {messages.map((m) => {
        const mapped: ChatMessage = {
          id: m.id,
          sender: m.role === "user" ? "human" : "agent",
          text: m.content,
          timestamp: formatTime(m.timestamp),
        };
        return <ChatStreamItem key={m.id} msg={mapped} />;
      })}
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
}> = ({ activeJobId, isPaused, onPauseJob, onResumeJob, onToggleSteer, onCancelJob }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "5px 10px", backgroundColor: "var(--bg-subtle)", borderTop: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", justifyContent: "space-between", fontSize: "10.5px", flexShrink: 0 }}>
      <span style={{ fontWeight: 600, color: "var(--action-primary)", display: "inline-flex", alignItems: "center", gap: "4px" }}>
        <BrandIcon name={isPaused ? "clock" : "execution-pipeline"} size={11} color="currentColor" />
        <span>{isPaused ? t("floatingChat.jobPaused", "Đã tạm dừng") : t("floatingChat.jobRunning", "Đang xử lý...")}</span>
      </span>
      <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
        <button type="button" onClick={() => (isPaused ? onResumeJob(activeJobId) : onPauseJob(activeJobId))} style={{ fontSize: "10px", padding: "2px 6px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "3px" }}>
          <BrandIcon name={isPaused ? "play" : "clock"} size={10} color="currentColor" />
          <span>{isPaused ? t("floatingChat.resumeBtn", "Tiếp tục") : t("floatingChat.pauseBtn", "Tạm dừng")}</span>
        </button>
        <button type="button" onClick={onToggleSteer} style={{ fontSize: "10px", padding: "2px 6px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "3px" }}>
          <BrandIcon name="switch" size={10} color="currentColor" />
          <span>{t("floatingChat.steerBtn", "Steer")}</span>
        </button>
        <button type="button" onClick={() => onCancelJob(activeJobId)} style={{ fontSize: "10px", padding: "2px 6px", borderRadius: "var(--radius-xs)", border: "1px solid var(--state-danger)", color: "var(--state-danger)", backgroundColor: "var(--bg-surface)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "3px" }}>
          <BrandIcon name="close" size={10} color="currentColor" />
          <span>{t("workbench.cancelBtn", "Hủy")}</span>
        </button>
      </div>
    </div>
  );
};

const SteerInputBar: React.FC<{
  steerDraft: string;
  onSteerDraftChange: (value: string) => void;
  onSteerSubmit: () => void;
}> = ({ steerDraft, onSteerDraftChange, onSteerSubmit }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "6px 10px", backgroundColor: "var(--bg-workspace)", borderTop: "1px solid var(--border-default)", display: "flex", gap: "4px", flexShrink: 0 }}>
      <input
        type="text"
        placeholder={t("floatingChat.steerPlaceholder", "Chỉ dẫn điều hướng giữa phiên (Steer)...")}
        value={steerDraft}
        onChange={(e) => onSteerDraftChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") onSteerSubmit();
        }}
        style={{ flex: 1, fontSize: "11px", padding: "4px 8px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)" }}
      />
      <button type="button" onClick={onSteerSubmit} style={{ fontSize: "10.5px", padding: "3px 8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", border: "none", cursor: "pointer" }}>
        {t("floatingChat.sendSteerBtn", "Gửi")}
      </button>
    </div>
  );
};

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
