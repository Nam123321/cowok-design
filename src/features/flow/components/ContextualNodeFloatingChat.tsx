// @story 04.3, 04.4
// @implements AC-02, AC-01 (Contextual Multi-Node Floating Chat v2 — Zero useEffect, FSD Clean)
import React, { useState, useRef, useCallback, useEffect } from "react";
import { AgentChatComposer } from "@/components/workbench/ChatPrimitivesBridge";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import { composePromptWithAttachments, extractSourceCommentId, forkConversationFromMessage, selectSdiClarificationVariant, sendSdiCompile } from "../utils/agent-chat-actions";
import { WorkbenchNode } from "../store/flow-workbench-store";
import {
  dispatchNodeExecutionToDaemon,
  dispatchSteerToDaemon,
  dispatchTransitionToDaemon,
  FLOATING_CHAT_BOUNDS,
  startResizeDrag,
  useFloatingChatEffects,
  useFloatingDraftState,
  useFloatingSharedMessages,
  useFloatingWindowGeometry,
  type FloatingChatMessage,
  type ResizeDirection,
} from "../hooks/useFloatingChatSync";
import { useKeyedParallelCoordinator } from "../hooks/useKeyedParallelCoordinator";
import { sharedChatStore, toSessionChatKey } from "../store/shared-chat-sync";
import { FloatingChatCollapsedPill } from "./FloatingChatCollapsedPill";
import { FloatingChatStreamPanel } from "./FloatingChatStreamPanel";

export interface ContextualNodeFloatingChatProps {
  projectId: string;
  sessionId: string;
  selectedNodes: WorkbenchNode[];
  attachmentChips?: string[];
  attachmentSnippets?: Record<string, string>;
  onRemoveAttachmentChip?: (chip: string) => void;
  onAttachFiles?: (fileNames: string[], snippetsByName?: Record<string, string>) => void;
  onClearAttachments?: () => void;
  onDeselectNode: (nodeId: string) => void;
  onClearSelection: () => void;
  onOpenLeftChat?: () => void;
  onFocusNode?: (nodeId: string) => void;
  onAttachContext?: (content: string) => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
  defaultExpanded?: boolean;
}

function resolveFloatingModeBadge(
  selectedCount: number,
  isMulti: boolean,
  t: (k: string, defVal?: string) => string
): string {
  if (selectedCount === 0) return t("floatingChat.batchMode", "Đính kèm");
  if (isMulti) return t("floatingChat.parallelMode", "Song song");
  return t("floatingChat.fifoMode", "FIFO per-node");
}

function resolveFloatingTitleAndPlaceholder(opts: {
  selectedCount: number;
  isMulti: boolean;
  chipsCount: number;
  primaryTitle?: string;
  primaryScreenKey?: string;
  formatNumber: (n: number) => string;
  t: (k: string, defVal?: string) => string;
}): { titleText: string; composerPlaceholder: string } {
  const { selectedCount, isMulti, chipsCount, primaryTitle, primaryScreenKey, formatNumber, t } = opts;
  if (selectedCount === 0) {
    return {
      titleText: `${t("floatingChat.commentBatchTitle", "Nhận xét đã ghim")} (${formatNumber(chipsCount)})`,
      composerPlaceholder: t("floatingChat.commentBatchPlaceholder", "Gửi các nhận xét đang đính kèm hoặc nhập thêm chỉ thị cho Agent..."),
    };
  }
  if (isMulti) {
    return {
      titleText: `${formatNumber(selectedCount)} ${t("floatingChat.screensSelectedSuffix", "Màn hình đang chọn")}`,
      composerPlaceholder: t("floatingChat.multiNodePlaceholder", "Yêu cầu Agent xử lý đồng loạt các màn hình đang chọn..."),
    };
  }
  return {
    titleText: `${primaryScreenKey}: ${primaryTitle}`,
    composerPlaceholder: `${t("floatingChat.askAgentAboutPrefix", "Hỏi Agent về")} ${primaryTitle}... ${t("floatingChat.enterHintSuffix", "(Enter gửi, Shift+Enter xuống dòng)")}`,
  };
}

const FloatingChatHeader: React.FC<{
  titleText: string;
  selectedCount: number;
  isMulti: boolean;
  isDragging: boolean;
  isRunning: boolean;
  isQueued: boolean;
  isPaused: boolean;
  isInterrupted: boolean;
  queuePosition?: number;
  fencingToken?: number;
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
          {resolveFloatingModeBadge(p.selectedCount, p.isMulti, t)}
        </span>
        {p.fencingToken ? (
          <span data-testid="fc-fencing-badge" style={{ fontSize: "9.5px", fontFamily: "var(--font-mono)", color: "var(--text-muted)", backgroundColor: "var(--bg-subtle)", padding: "1px 5px", borderRadius: "var(--radius-xs)", flexShrink: 0 }}>
            #{p.fencingToken}
          </span>
        ) : null}
        {p.isRunning && <span style={{ fontSize: "9px", fontWeight: 700, color: "var(--action-primary)", backgroundColor: "var(--bg-selected)", padding: "1px 5px", borderRadius: "var(--radius-xs)" }}>{t("floatingChat.runningBadge", "RUNNING")}</span>}
        {p.isPaused && <span style={{ fontSize: "9px", fontWeight: 700, color: "var(--text-secondary)", backgroundColor: "var(--bg-subtle)", padding: "1px 5px", borderRadius: "var(--radius-xs)" }}>{t("flow.pausedBadge", "PAUSED")}</span>}
        {p.isInterrupted && <span style={{ fontSize: "9px", fontWeight: 700, color: "var(--state-danger)", backgroundColor: "var(--bg-subtle)", padding: "1px 5px", borderRadius: "var(--radius-xs)" }}>{t("flow.interruptedBadge", "INTERRUPTED")}</span>}
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

const RESIZE_SPECS: Array<{ dir: ResizeDirection; cursor: string; style: React.CSSProperties }> = [
  { dir: "n", cursor: "ns-resize", style: { top: 0, left: "8px", right: "8px", height: "6px" } },
  { dir: "s", cursor: "ns-resize", style: { bottom: 0, left: "8px", right: "8px", height: "6px" } },
  { dir: "e", cursor: "ew-resize", style: { top: "8px", bottom: "8px", right: 0, width: "6px" } },
  { dir: "w", cursor: "ew-resize", style: { top: "8px", bottom: "8px", left: 0, width: "6px" } },
  { dir: "ne", cursor: "nesw-resize", style: { top: 0, right: 0, width: "10px", height: "10px" } },
  { dir: "nw", cursor: "nwse-resize", style: { top: 0, left: 0, width: "10px", height: "10px" } },
  { dir: "se", cursor: "nwse-resize", style: { bottom: 0, right: 0, width: "10px", height: "10px" } },
  { dir: "sw", cursor: "nesw-resize", style: { bottom: 0, left: 0, width: "10px", height: "10px" } },
];

const FloatingChatResizeHandles: React.FC<{
  resizeCtx: { width: number; height: number; x: number; bottom: number };
  onApply: (next: { width?: number; height?: number; x?: number; bottom?: number }) => void;
}> = ({ resizeCtx, onApply }) => (
  <>
    {RESIZE_SPECS.map((h) => (
      <div key={h.dir} data-testid={`fc-resize-${h.dir}`} style={{ position: "absolute", cursor: h.cursor, zIndex: 4, ...h.style }} onMouseDown={(e) => startResizeDrag(e, h.dir, resizeCtx, onApply)} />
    ))}
  </>
);

function useFloatingCommentAutoExpand(setIsCollapsed: React.Dispatch<React.SetStateAction<boolean>>) {
  useEffect(() => {
    if (typeof window === "undefined") return;
    const onStructuredComment = () => setIsCollapsed(false);
    window.addEventListener("cowok:structured-comment", onStructuredComment);
    return () => window.removeEventListener("cowok:structured-comment", onStructuredComment);
  }, [setIsCollapsed]);
}

function executeFloatingSendPrompt(opts: {
  inputPrompt: string;
  chips: string[];
  props: ContextualNodeFloatingChatProps;
  primaryNode: WorkbenchNode | null;
  selectedIds: string[];
  coordinator: ReturnType<typeof useKeyedParallelCoordinator>;
  streamAbortRef: React.MutableRefObject<AbortController | null>;
  lastSendRef: React.MutableRefObject<{ at: number; displayMessage: string } | null>;
  setInputPrompt: (v: string) => void;
  appendMsg: (m: FloatingChatMessage) => void;
  formatNumber: ReturnType<typeof useFormatter>["formatNumber"];
  formatTime: ReturnType<typeof useFormatter>["formatTime"];
  t: ReturnType<typeof useTranslation>["t"];
}) {
  const { inputPrompt, chips, props, primaryNode, selectedIds, lastSendRef, appendMsg, t } = opts;
  if (!inputPrompt.trim() && chips.length === 0) return;
  const { displayMessage, compiledPrompt } = composePromptWithAttachments(
    inputPrompt,
    chips,
    props.attachmentSnippets,
    t("chat.attachedFallback", "Đính kèm")
  );
  const sourceCommentId = extractSourceCommentId(chips, props.attachmentSnippets);
  const now = Date.now();
  if (lastSendRef.current && now - lastSendRef.current.at < 350 && lastSendRef.current.displayMessage === displayMessage) {
    return;
  }
  lastSendRef.current = { at: now, displayMessage };
  opts.setInputPrompt("");
  props.onClearAttachments?.();
  const msgSuffix = Math.random().toString(36).slice(2, 6);
  appendMsg({ id: `msg-${now}-${msgSuffix}`, nodeId: primaryNode?.id || "batch", role: "user", content: displayMessage, timestamp: now });
  if (!props.projectId.trim()) {
    appendMsg({ id: `msg-standalone-${now}`, nodeId: primaryNode?.id || "batch", role: "assistant", content: `${t("workbench.standaloneChatAck", "Đã ghi nhận trao đổi trong phiên độc lập")}: ${displayMessage}`, timestamp: Date.now() });
    return;
  }
  if (props.selectedNodes.length > 0) {
    void dispatchNodeExecutionToDaemon({
      projectId: props.projectId, sessionId: props.sessionId, primaryNode, selectedNodeIds: selectedIds,
      prompt: compiledPrompt, sourceCommentId, coordinator: opts.coordinator, streamAbortRef: opts.streamAbortRef, onAppendMsg: appendMsg, t, formatNumber: opts.formatNumber,
    });
    return;
  }
  sendSdiCompile({
    projectId: props.projectId, targetKey: toSessionChatKey(props.projectId, props.sessionId),
    primaryNodeId: primaryNode?.id, sourceCommentId, sentPrompt: compiledPrompt, formatTime: opts.formatTime, formatNumber: opts.formatNumber, t,
  });
}

function useFloatingChatController(props: ContextualNodeFloatingChatProps) {
  const { t, locale } = useTranslation();
  const { formatNumber, formatTime } = useFormatter({ locale });
  const coordinator = useKeyedParallelCoordinator();
  const chips = props.attachmentChips ?? [];
  const [isCollapsed, setIsCollapsed] = useState(!props.defaultExpanded && !(props.selectedNodes.length === 0 && chips.length > 0));
  const [showReviewSummary, setShowReviewSummary] = useState(false);
  const [inputPrompt, setInputPrompt] = useFloatingDraftState(props.projectId, props.sessionId);
  const [steerDraft, setSteerDraft] = useState("");
  const [isSteering, setIsSteering] = useState(false);
  const geo = useFloatingWindowGeometry(isCollapsed);
  const containerRef = useRef<HTMLDivElement>(null);
  const historyEndRef = useRef<HTMLDivElement>(null);
  const streamAbortRef = useRef<AbortController | null>(null);
  const lastSendRef = useRef<{ at: number; displayMessage: string } | null>(null);
  const primaryNode = props.selectedNodes[0] || null;
  const primaryStatus = primaryNode ? coordinator.getNodeStatus(primaryNode.id, { projectId: props.projectId, sessionId: props.sessionId }) : null;
  const selectedIds = props.selectedNodes.map((n) => n.id);
  const messages = useFloatingSharedMessages(props.projectId, props.sessionId, selectedIds);

  useFloatingChatEffects({ projectId: props.projectId, sessionId: props.sessionId, messages, primaryStatus, historyEndRef, streamAbortRef });
  useFloatingCommentAutoExpand(setIsCollapsed);

  const appendMsg = useCallback((m: FloatingChatMessage) => {
    sharedChatStore.appendMessage(toSessionChatKey(props.projectId, props.sessionId), {
      id: m.id, sender: m.role === "user" ? "human" : "agent", text: m.content, timestamp: formatTime(m.timestamp),
      createdAtMs: m.timestamp, nodeId: m.nodeId, projectSeq: m.projectSeq, fencingToken: m.fencingToken, sdiClarification: m.sdiClarification,
    });
  }, [props.projectId, props.sessionId, formatTime]);

  const handleSendMessage = () => executeFloatingSendPrompt({
    inputPrompt, chips, props, primaryNode, selectedIds, coordinator, streamAbortRef, lastSendRef, setInputPrompt, appendMsg, formatNumber, formatTime, t,
  });
  const handleSteerSubmit = () => {
    if (!steerDraft.trim() || !primaryStatus?.jobId) return;
    const instruction = steerDraft.trim();
    const fence = primaryStatus.fencingToken ?? 1;
    appendMsg({ id: `steer-${Date.now()}`, nodeId: primaryNode?.id || "unknown", role: "user", content: `${t("floatingChat.steerPrefix", "Chỉ dẫn thêm (Steer)")}: ${instruction}`, timestamp: Date.now(), fencingToken: fence });
    dispatchSteerToDaemon({ projectId: props.projectId, jobId: primaryStatus.jobId, nodeId: primaryNode?.id || "unknown", fence, instruction, coordinator, appendMsg, errorPrefix: t("floatingChat.daemonSyncError", "Không thể đồng bộ với Rust daemon") });
    setSteerDraft("");
    setIsSteering(false);
  };
  const handleTransition = (kind: "pause" | "resume" | "cancel", jobId: string) =>
    dispatchTransitionToDaemon({
      projectId: props.projectId, kind, jobId, fence: primaryStatus?.fencingToken ?? 1, coordinator,
      onErrorMsg: (errText) => appendMsg({ id: `tx-err-${Date.now()}`, nodeId: primaryNode?.id || "unknown", role: "assistant", content: errText, timestamp: Date.now() }),
    });
  return { isCollapsed, setIsCollapsed, showReviewSummary, setShowReviewSummary, messages, inputPrompt, setInputPrompt, steerDraft, setSteerDraft, isSteering, setIsSteering, geo, containerRef, historyEndRef, primaryNode, primaryStatus, handleSendMessage, handleSteerSubmit, handleTransition, formatTime, t };
}

function buildExpandedChatStyle(c: ReturnType<typeof useFloatingChatController>, winX: number, winBottom: number): React.CSSProperties {
  return {
    position: "fixed", left: `${winX}px`, bottom: `${winBottom}px`,
    width: `${c.geo.width}px`, minWidth: `${FLOATING_CHAT_BOUNDS.MIN_WIDTH}px`, maxWidth: `${FLOATING_CHAT_BOUNDS.MAX_WIDTH}px`,
    minHeight: `${FLOATING_CHAT_BOUNDS.MIN_HEIGHT}px`, height: c.geo.customHeight ? `${c.geo.customHeight}px` : "auto",
    maxHeight: `min(${FLOATING_CHAT_BOUNDS.MAX_HEIGHT}px, calc(100vh - 96px))`,
    backgroundColor: "var(--bg-canvas)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-lg)", boxShadow: "var(--shadow-lg)",
    display: "flex", flexDirection: "column", overflow: "hidden", zIndex: 55, boxSizing: "border-box",
  };
}

const ExpandedFloatingChatWindow: React.FC<{
  props: ContextualNodeFloatingChatProps; c: ReturnType<typeof useFloatingChatController>;
  isMulti: boolean; winX: number; winBottom: number; titleText: string; composerPlaceholder: string;
}> = ({ props, c, isMulti, winX, winBottom, titleText, composerPlaceholder }) => {
  const resizeCtx = { width: c.geo.width, height: c.geo.customHeight ?? c.containerRef.current?.offsetHeight ?? FLOATING_CHAT_BOUNDS.MIN_HEIGHT, x: winX, bottom: winBottom };
  const applyResize = (next: { width?: number; height?: number; x?: number; bottom?: number }) => {
    if (next.width !== undefined) c.geo.setWidth(next.width);
    if (next.height !== undefined) c.geo.setCustomHeight(next.height);
    if (next.x !== undefined || next.bottom !== undefined) c.geo.setAnchor({ x: next.x ?? winX, bottom: next.bottom ?? winBottom });
  };
  return (
    <div
      ref={c.containerRef}
      role="dialog"
      aria-label={titleText}
      tabIndex={-1}
      onKeyDown={(ev) => {
        if (ev.key === "Escape") {
          ev.stopPropagation();
          c.setIsCollapsed(true);
        }
      }}
      className="contextual-node-floating-chat"
      data-testid="contextual-node-floating-chat"
      style={buildExpandedChatStyle(c, winX, winBottom)}
    >
      <FloatingChatHeader titleText={titleText} selectedCount={props.selectedNodes.length} isMulti={isMulti} isDragging={c.geo.isDragging} isRunning={c.primaryStatus?.status === "running"} isQueued={c.primaryStatus?.status === "queued"} isPaused={c.primaryStatus?.status === "paused"} isInterrupted={c.primaryStatus?.status === "interrupted"} queuePosition={c.primaryStatus?.queuePosition} fencingToken={c.primaryStatus?.fencingToken} onMouseDown={c.geo.handleHeaderMouseDown} onRedock={c.geo.handleRedockToBottom} onOpenLeftChat={props.onOpenLeftChat} onCollapse={() => c.setIsCollapsed(true)} onClose={props.onClearSelection} />
      <FloatingChatStreamPanel projectId={props.projectId} sessionId={props.sessionId} selectedNodes={props.selectedNodes} primaryNode={c.primaryNode} isMulti={isMulti} messages={c.messages} historyEndRef={c.historyEndRef} isRunning={c.primaryStatus?.status === "running"} isQueued={c.primaryStatus?.status === "queued"} isPaused={c.primaryStatus?.status === "paused"} isInterrupted={c.primaryStatus?.status === "interrupted"} activeJobId={c.primaryStatus?.jobId} fencingToken={c.primaryStatus?.fencingToken} projectSeq={c.primaryStatus?.projectSeq} isSteering={c.isSteering} steerDraft={c.steerDraft} onDeselectNode={props.onDeselectNode} onFocusNode={props.onFocusNode} onPauseJob={(id) => c.handleTransition("pause", id)} onResumeJob={(id) => c.handleTransition("resume", id)} onCancelJob={(id) => c.handleTransition("cancel", id)} onToggleSteer={() => c.setIsSteering((p) => !p)} onSteerDraftChange={c.setSteerDraft} onSteerSubmit={c.handleSteerSubmit} onForkFromMessage={(m) => { void forkConversationFromMessage({ projectId: props.projectId, sourceSessionId: props.sessionId, messageId: m.id }); }} onSelectSdiVariant={(intentId, pid, variantId) => { void selectSdiClarificationVariant({ sessionKey: toSessionChatKey(props.projectId, props.sessionId), intentId, pid, variantId, formatTime: c.formatTime, t: c.t }); }} />
      <AgentChatComposer inputValue={c.inputPrompt} onInputChange={c.setInputPrompt} onSend={c.handleSendMessage} attachmentChips={props.attachmentChips || []} onRemoveAttachmentChip={props.onRemoveAttachmentChip} onAttachFiles={props.onAttachFiles} onOpenSettings={props.onOpenSettings} testIdPrefix="floating-chat-composer" compact placeholder={composerPlaceholder} />
      <FloatingChatResizeHandles resizeCtx={resizeCtx} onApply={applyResize} />
    </div>
  );
};

export const ContextualNodeFloatingChat: React.FC<ContextualNodeFloatingChatProps> = (props) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const c = useFloatingChatController(props);
  const chips = props.attachmentChips ?? [];
  if (props.selectedNodes.length === 0 && chips.length === 0) return null;

  const isMulti = props.selectedNodes.length > 1;
  const winX = c.geo.anchor ? c.geo.anchor.x : FLOATING_CHAT_BOUNDS.DEFAULT_LEFT;
  const winBottom = c.geo.anchor ? c.geo.anchor.bottom : FLOATING_CHAT_BOUNDS.DEFAULT_BOTTOM;
  if (c.isCollapsed) {
    return (
      <FloatingChatCollapsedPill
        containerRef={c.containerRef} pillX={winX} pillBottom={winBottom} isDragging={c.geo.isDragging}
        isMulti={isMulti} selectedCount={props.selectedNodes.length} attachmentCount={chips.length} primaryNode={c.primaryNode}
        primaryNodeStatus={c.primaryStatus} showReviewSummary={c.showReviewSummary}
        onHeaderMouseDown={c.geo.handleHeaderMouseDown} onExpand={() => c.setIsCollapsed(false)}
        onToggleReviewSummary={() => c.setShowReviewSummary((prev) => !prev)}
        onCloseReviewSummary={() => c.setShowReviewSummary(false)} onClearSelection={props.onClearSelection}
      />
    );
  }
  const { titleText, composerPlaceholder } = resolveFloatingTitleAndPlaceholder({
    selectedCount: props.selectedNodes.length,
    isMulti,
    chipsCount: chips.length,
    primaryTitle: c.primaryNode?.title,
    primaryScreenKey: c.primaryNode?.screenKey,
    formatNumber,
    t,
  });
  return <ExpandedFloatingChatWindow props={props} c={c} isMulti={isMulti} winX={winX} winBottom={winBottom} titleText={titleText} composerPlaceholder={composerPlaceholder} />;
};
