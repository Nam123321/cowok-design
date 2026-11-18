// @story 04.3, 04.4
// @implements AC-02, AC-01 (Scope Strip, Bridge Stream, Live Fencing Token & Project Seq Controls)
import React from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { ChatStreamItem, type ChatMessage } from "@/components/workbench/ChatPrimitivesBridge";
import { useTranslation } from "@/hooks/useTranslation";
import { useFormatter } from "@/hooks/useFormatter";
import { WorkbenchNode } from "../store/flow-workbench-store";
import type { FloatingChatMessage } from "../hooks/useFloatingChatSync";

export type { FloatingChatMessage };

function extractPayloadSummaryParts(
  parsed: Record<string, unknown>,
  t: (k: string, f: string) => string
): string[] {
  if (parsed.summary) {
    return [String(parsed.summary)];
  }
  const parts: string[] = [];
  if (parsed.prompt) {
    parts.push(`"${parsed.prompt}"`);
  }
  if (parsed.status) {
    parts.push(`${t("floatingChat.nodeStatusLabel", "Status")}: ${parsed.status}`);
  }
  if (parsed.fencingToken !== undefined) {
    parts.push(`#${parsed.fencingToken}`);
  }
  if (parsed.instruction) {
    parts.push(`${t("floatingChat.steerPrefix", "Steer instruction")}: "${parsed.instruction}"`);
  }
  return parts;
}

export function formatStructuredPayload(payloadText: string, t: (k: string, f: string) => string): string {
  if (!payloadText || typeof payloadText !== "string") return payloadText;
  const trimmed = payloadText.trim();
  const match = trimmed.match(/^(\[[a-zA-Z0-9_-]+\]\s*)?(\{[\s\S]*\})$/);
  if (!match) return payloadText;

  const prefix = match[1] ? match[1].trim() : "";
  const jsonStr = match[2];

  try {
    const parsed = JSON.parse(jsonStr);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return payloadText;

    const parts = extractPayloadSummaryParts(parsed as Record<string, unknown>, t);
    if (parts.length === 0) return payloadText;
    const body = parts.join(" · ");
    return prefix ? `${prefix} ${body}` : body;
  } catch {
    return payloadText;
  }
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
  isInterrupted?: boolean;
  activeJobId?: string;
  fencingToken?: number;
  projectSeq?: number;
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
    <div className="fc-scope-strip" style={{ padding: "5px 10px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", gap: "5px", flexWrap: "wrap", flexShrink: 0 }}>
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
            <button type="button" onClick={(e) => { e.stopPropagation(); onDeselectNode(n.id); }} aria-label={`${t("floatingChat.deselectNode", "Bỏ chọn")} ${n.screenKey}`} style={{ border: "none", background: "transparent", color: "var(--text-muted)", cursor: "pointer", width: "16px", height: "16px", minWidth: "16px", padding: "2px", display: "inline-flex", alignItems: "center", justifyContent: "center", flexShrink: 0 }}>
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
      <div className="fc-history" style={{ flex: 1, padding: "12px", display: "flex", alignItems: "flex-start", gap: "6px", backgroundColor: "var(--bg-canvas)", fontSize: "11.5px", color: "var(--text-muted)" }}>
        <BrandIcon name="ai-agent" size={14} color="var(--agent-accent)" />
        <span>
          {t("floatingChat.emptyHint", "Nhập yêu cầu cho")} {isMulti ? `${formatNumber(selectedCount)} ${t("floatingChat.screensUnit", "màn hình")}` : primaryTitle || t("floatingChat.singleScreenFallback", "màn hình")}
        </span>
      </div>
    );
  }
  return (
    <div className="fc-history" style={{ flex: 1, minHeight: 0, overflowY: "auto", padding: "10px 12px", display: "flex", flexDirection: "column", gap: "10px", backgroundColor: "var(--bg-canvas)" }}>
      {messages.map((m) => {
        const text = m.role === "assistant" ? formatStructuredPayload(m.content, t) : m.content;
        const mapped: ChatMessage = { id: m.id, sender: m.role === "user" ? "human" : "agent", text, timestamp: formatTime(m.timestamp) };
        return <ChatStreamItem key={m.id} msg={mapped} />;
      })}
      <div ref={historyEndRef} />
    </div>
  );
};

const JobControlBar: React.FC<{
  activeJobId: string;
  isPaused: boolean;
  isInterrupted?: boolean;
  fencingToken?: number;
  projectSeq?: number;
  onPauseJob: (jobId: string) => void;
  onResumeJob: (jobId: string) => void;
  onToggleSteer: () => void;
  onCancelJob: (jobId: string) => void;
}> = ({ activeJobId, isPaused, isInterrupted, fencingToken, projectSeq, onPauseJob, onResumeJob, onToggleSteer, onCancelJob }) => {
  const { t } = useTranslation();
  const canResume = isPaused || isInterrupted;
  const label = isInterrupted
    ? t("floatingChat.jobInterrupted", "Bị gián đoạn")
    : isPaused
    ? t("floatingChat.jobPaused", "Đã tạm dừng")
    : t("floatingChat.jobRunning", "Đang xử lý...");
  return (
    <div style={{ padding: "5px 10px", backgroundColor: "var(--bg-subtle)", borderTop: "1px solid var(--border-subtle)", display: "flex", alignItems: "center", justifyContent: "space-between", fontSize: "10.5px", flexShrink: 0, gap: "6px" }}>
      <span style={{ fontWeight: 600, color: isInterrupted ? "var(--state-danger)" : "var(--action-primary)", display: "inline-flex", alignItems: "center", gap: "5px" }}>
        <BrandIcon name={canResume ? "clock" : "execution-pipeline"} size={11} color="currentColor" />
        <span>{label}</span>
        {fencingToken ? <span style={{ fontFamily: "var(--font-mono)", fontSize: "9.5px", color: "var(--text-muted)" }}>{`${t("floatingChat.fenceLabel", "fence:")} ${fencingToken}`}</span> : null}
        {projectSeq ? <span style={{ fontFamily: "var(--font-mono)", fontSize: "9.5px", color: "var(--text-muted)" }}>{`${t("floatingChat.seqLabel", "seq:")} ${projectSeq}`}</span> : null}
      </span>
      <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
        <button type="button" onClick={() => (canResume ? onResumeJob(activeJobId) : onPauseJob(activeJobId))} style={{ fontSize: "10px", padding: "2px 6px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "3px" }}>
          <BrandIcon name={canResume ? "play" : "clock"} size={10} color="currentColor" />
          <span>{canResume ? t("floatingChat.resumeBtn", "Tiếp tục") : t("floatingChat.pauseBtn", "Tạm dừng")}</span>
        </button>
        {!isInterrupted && (
          <button type="button" onClick={onToggleSteer} style={{ fontSize: "10px", padding: "2px 6px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "3px" }}>
            <BrandIcon name="switch" size={10} color="currentColor" />
            <span>{t("floatingChat.steerBtn", "Steer")}</span>
          </button>
        )}
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
        onKeyDown={(e) => { if (e.key === "Enter") onSteerSubmit(); }}
        style={{ flex: 1, fontSize: "11px", padding: "4px 8px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)" }}
      />
      <button type="button" onClick={onSteerSubmit} style={{ fontSize: "10.5px", padding: "3px 8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", border: "none", cursor: "pointer" }}>
        {t("floatingChat.sendSteerBtn", "Gửi")}
      </button>
    </div>
  );
};

export const FloatingChatStreamPanel: React.FC<FloatingChatStreamPanelProps> = (props) => {
  const showJobBar = (props.isRunning || props.isQueued || props.isPaused || props.isInterrupted) && Boolean(props.activeJobId);
  return (
    <>
      <ScopeChipStrip projectId={props.projectId} sessionId={props.sessionId} selectedNodes={props.selectedNodes} onDeselectNode={props.onDeselectNode} onFocusNode={props.onFocusNode} />
      <MessageHistoryList messages={props.messages} isMulti={props.isMulti} selectedCount={props.selectedNodes.length} primaryTitle={props.primaryNode?.title} historyEndRef={props.historyEndRef} />
      {showJobBar && props.activeJobId && (
        <JobControlBar
          activeJobId={props.activeJobId} isPaused={props.isPaused} isInterrupted={props.isInterrupted}
          fencingToken={props.fencingToken} projectSeq={props.projectSeq}
          onPauseJob={props.onPauseJob} onResumeJob={props.onResumeJob} onToggleSteer={props.onToggleSteer} onCancelJob={props.onCancelJob}
        />
      )}
      {props.isSteering && <SteerInputBar steerDraft={props.steerDraft} onSteerDraftChange={props.onSteerDraftChange} onSteerSubmit={props.onSteerSubmit} />}
    </>
  );
};
