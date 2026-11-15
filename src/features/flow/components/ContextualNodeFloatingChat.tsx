// @story 04.4
// @implements AC-01, AC-02 (Contextual Multi-Node Floating Chat v2 — Floating & Draggable Position + Resize Persistence)
import React from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useFormatter } from "@/hooks/useFormatter";
import { useTranslation } from "@/hooks/useTranslation";
import { AgentChatComposer } from "@/pages/workbench/components/AgentChatComposer";
import { FloatingChatCollapsedPill } from "./FloatingChatCollapsedPill";
import { FloatingChatStreamPanel } from "./FloatingChatStreamPanel";
import {
  FLOATING_CHAT_LAYOUT_STORAGE_KEY,
  MIN_WIDTH,
  MAX_WIDTH,
  DEFAULT_WIDTH,
  MIN_HEIGHT,
  MAX_HEIGHT,
  DEFAULT_HEIGHT,
  getEffectiveMaxHeight,
  loadSavedLayout,
  saveLayout,
  type FloatingChatLayout,
} from "../hooks/useFloatingWindowGeometry";
import {
  useFloatingChatController,
  type ContextualNodeFloatingChatProps,
} from "../hooks/useFloatingChatController";

export {
  FLOATING_CHAT_LAYOUT_STORAGE_KEY,
  MIN_WIDTH,
  MAX_WIDTH,
  DEFAULT_WIDTH,
  MIN_HEIGHT,
  MAX_HEIGHT,
  DEFAULT_HEIGHT,
  getEffectiveMaxHeight,
  loadSavedLayout,
  saveLayout,
  type FloatingChatLayout,
  type ContextualNodeFloatingChatProps,
};

const RESIZE_HANDLE_Z_INDEX = 10;
const CORNER_HANDLE_Z_INDEX = 11;
const FLOATING_CHAT_Z_INDEX = 55;
const ICON_SIZE_SM = 12;
const ICON_SIZE_MD = 13;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;

const FloatingChatResizeHandles: React.FC<{
  onStartResize: (e: React.MouseEvent, direction: "e" | "s" | "se") => void;
}> = ({ onStartResize }) => (
  <>
    <div
      data-testid="floating-chat-resize-e"
      role="separator"
      aria-orientation="vertical"
      style={{
        position: "absolute",
        top: "6px",
        bottom: "10px",
        right: 0,
        width: "6px",
        cursor: "ew-resize",
        zIndex: RESIZE_HANDLE_Z_INDEX,
      }}
      onMouseDown={(e) => onStartResize(e, "e")}
    />
    <div
      data-testid="floating-chat-resize-s"
      role="separator"
      aria-orientation="horizontal"
      style={{
        position: "absolute",
        bottom: 0,
        left: "6px",
        right: "10px",
        height: "6px",
        cursor: "ns-resize",
        zIndex: RESIZE_HANDLE_Z_INDEX,
      }}
      onMouseDown={(e) => onStartResize(e, "s")}
    />
    <div
      data-testid="floating-chat-resize-se"
      role="separator"
      style={{
        position: "absolute",
        bottom: 0,
        right: 0,
        width: "12px",
        height: "12px",
        cursor: "nwse-resize",
        zIndex: CORNER_HANDLE_Z_INDEX,
      }}
      onMouseDown={(e) => onStartResize(e, "se")}
    />
  </>
);

const HeaderBadges: React.FC<{
  isMulti: boolean;
  isRunning: boolean;
  isQueued: boolean;
  queuePosition?: number;
}> = (p) => {
  const { t } = useTranslation();
  return (
    <>
      <span
        style={{
          fontSize: "10px",
          fontFamily: "var(--font-mono)",
          color: "var(--text-muted)",
          backgroundColor: "var(--bg-subtle)",
          padding: "1px 5px",
          borderRadius: "var(--radius-xs)",
          flexShrink: 0,
        }}
      >
        {p.isMulti ? t("floatingChat.parallelMode", "Song song") : t("floatingChat.fifoMode", "FIFO per-node")}
      </span>
      {p.isRunning && (
        <span
          style={{
            fontSize: "9px",
            fontWeight: FONT_WEIGHT_BOLD,
            color: "var(--action-primary)",
            backgroundColor: "var(--bg-selected)",
            padding: "1px 5px",
            borderRadius: "var(--radius-xs)",
          }}
        >
          {t("floatingChat.runningBadge", "RUNNING")}
        </span>
      )}
      {p.isQueued && (
        <span
          style={{
            fontSize: "9px",
            fontWeight: FONT_WEIGHT_BOLD,
            color: "var(--state-warning)",
            backgroundColor: "var(--bg-subtle)",
            padding: "1px 5px",
            borderRadius: "var(--radius-xs)",
          }}
        >
          {t("floatingChat.queuedBadge", "QUEUED")} #{p.queuePosition}
        </span>
      )}
    </>
  );
};

interface HeaderTitleProps {
  titleText: string;
  isMulti: boolean;
  isRunning: boolean;
  isQueued: boolean;
  queuePosition?: number;
}

const FloatingChatHeaderTitle: React.FC<HeaderTitleProps> = (p) => (
  <div style={{ display: "flex", alignItems: "center", gap: "6px", minWidth: 0, flex: 1 }}>
    <BrandIcon name="ai-agent" size={ICON_SIZE_MD} color="var(--action-primary)" style={{ flexShrink: 0 }} />
    <span
      style={{
        fontSize: "12px",
        fontWeight: FONT_WEIGHT_SEMIBOLD,
        color: "var(--text-primary)",
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
      }}
    >
      {p.titleText}
    </span>
    <HeaderBadges
      isMulti={p.isMulti}
      isRunning={p.isRunning}
      isQueued={p.isQueued}
      queuePosition={p.queuePosition}
    />
  </div>
);

const HeaderActionButton: React.FC<{
  onClick?: () => void;
  title: string;
  icon: string;
}> = ({ onClick, title, icon }) => (
  <button
    type="button"
    onClick={onClick}
    title={title}
    aria-label={title}
    style={{
      width: "24px",
      height: "24px",
      border: "none",
      background: "transparent",
      cursor: "pointer",
      color: "var(--text-muted)",
      display: "inline-flex",
      alignItems: "center",
      justifyContent: "center",
      borderRadius: "var(--radius-xs)",
    }}
  >
    <BrandIcon name={icon} size={ICON_SIZE_SM} color="currentColor" />
  </button>
);

interface HeaderActionsProps {
  onRedock: () => void;
  onOpenLeftChat?: () => void;
  onCollapse: () => void;
  onClose: () => void;
}

const FloatingChatHeaderActions: React.FC<HeaderActionsProps> = (p) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "3px" }}>
      <HeaderActionButton
        onClick={p.onRedock}
        title={t("flow.redockBottom", "Khôi phục vị trí mặc định")}
        icon="fit-view"
      />
      {p.onOpenLeftChat && (
        <HeaderActionButton
          onClick={p.onOpenLeftChat}
          title={t("flow.openLeftChat", "Mở sang khung chat trái")}
          icon="panel-left-open"
        />
      )}
      <HeaderActionButton
        onClick={p.onCollapse}
        title={t("floatingChat.collapsePill", "Thu gọn thành Pill (Esc)")}
        icon="chevron-down"
      />
      <HeaderActionButton
        onClick={p.onClose}
        title={t("floatingChat.closeChat", "Đóng khung chat")}
        icon="close"
      />
    </div>
  );
};

interface FloatingChatHeaderProps extends HeaderTitleProps, HeaderActionsProps {
  isDragging: boolean;
  onMouseDown: (e: React.MouseEvent) => void;
}

const FloatingChatHeader: React.FC<FloatingChatHeaderProps> = (p) => (
  <div
    className="fc-drag-header"
    onMouseDown={p.onMouseDown}
    style={{
      height: "36px",
      minHeight: "36px",
      padding: "0 10px 0 12px",
      backgroundColor: "var(--bg-workspace)",
      borderBottom: "1px solid var(--border-subtle)",
      display: "flex",
      alignItems: "center",
      justifyContent: "space-between",
      cursor: p.isDragging ? "grabbing" : "grab",
      userSelect: "none",
      flexShrink: 0,
    }}
  >
    <FloatingChatHeaderTitle
      titleText={p.titleText}
      isMulti={p.isMulti}
      isRunning={p.isRunning}
      isQueued={p.isQueued}
      queuePosition={p.queuePosition}
    />
    <FloatingChatHeaderActions
      onRedock={p.onRedock}
      onOpenLeftChat={p.onOpenLeftChat}
      onCollapse={p.onCollapse}
      onClose={p.onClose}
    />
  </div>
);

const FloatingChatBody: React.FC<{
  props: ContextualNodeFloatingChatProps;
  c: ReturnType<typeof useFloatingChatController>;
}> = ({ props, c }) => {
  const isMulti = props.selectedNodes.length > 1;
  return (
    <FloatingChatStreamPanel
      projectId={props.projectId}
      sessionId={props.sessionId}
      selectedNodes={props.selectedNodes}
      primaryNode={c.primaryNode}
      isMulti={isMulti}
      messages={c.messages}
      historyEndRef={c.historyEndRef}
      isRunning={c.primaryStatus?.status === "running"}
      isQueued={c.primaryStatus?.status === "queued"}
      isPaused={c.primaryStatus?.status === "paused"}
      activeJobId={c.primaryStatus?.jobId}
      isSteering={c.isSteering}
      steerDraft={c.steerDraft}
      onDeselectNode={props.onDeselectNode}
      onFocusNode={props.onFocusNode}
      onPauseJob={(id) => c.coordinator.pauseJob(id)}
      onResumeJob={(id) => c.coordinator.resumeJob(id)}
      onCancelJob={(id) => c.coordinator.cancelJob(id)}
      onToggleSteer={() => c.setIsSteering((p) => !p)}
      onSteerDraftChange={c.setSteerDraft}
      onSteerSubmit={c.handleSteerSubmit}
    />
  );
};

function useFloatingChatDerivedTexts(
  props: ContextualNodeFloatingChatProps,
  c: ReturnType<typeof useFloatingChatController>
) {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const isMulti = props.selectedNodes.length > 1;
  const titleText = isMulti
    ? `${formatNumber(props.selectedNodes.length)} ${t("floatingChat.screensSelectedSuffix", "Màn hình đang chọn")}`
    : `${c.primaryNode?.screenKey}: ${c.primaryNode?.title}`;

  const composerPlaceholder = isMulti
    ? t("floatingChat.multiNodePlaceholder", "Yêu cầu Agent xử lý đồng loạt các màn hình đang chọn...")
    : `${t("floatingChat.askAgentAboutPrefix", "Hỏi Agent về")} ${c.primaryNode?.title}... ${t("floatingChat.enterHintSuffix", "(Enter gửi, Shift+Enter xuống dòng)")}`;

  return { isMulti, titleText, composerPlaceholder };
}

function getExpandedWindowStyle(
  geo: ReturnType<typeof useFloatingChatController>["geo"]
): React.CSSProperties {
  return {
    position: "fixed",
    left: `${geo.position.x}px`,
    top: `${geo.position.y}px`,
    width: `${geo.width}px`,
    minWidth: `${MIN_WIDTH}px`,
    maxWidth: `${MAX_WIDTH}px`,
    height: geo.customHeight ? `${geo.customHeight}px` : "auto",
    minHeight: geo.customHeight ? `${MIN_HEIGHT}px` : undefined,
    maxHeight: `${getEffectiveMaxHeight()}px`,
    backgroundColor: "var(--bg-canvas)",
    border: "1px solid var(--border-default)",
    borderRadius: "var(--radius-lg)",
    boxShadow: "var(--shadow-lg)",
    display: "flex",
    flexDirection: "column",
    overflow: "hidden",
    zIndex: FLOATING_CHAT_Z_INDEX,
    boxSizing: "border-box",
  };
}

const FloatingChatExpandedWindow: React.FC<{
  props: ContextualNodeFloatingChatProps;
  c: ReturnType<typeof useFloatingChatController>;
  derived: ReturnType<typeof useFloatingChatDerivedTexts>;
}> = ({ props, c, derived }) => (
  <div
    ref={c.containerRef}
    className="contextual-node-floating-chat"
    data-testid="contextual-node-floating-chat"
    style={getExpandedWindowStyle(c.geo)}
  >
    <FloatingChatHeader
      titleText={derived.titleText}
      isMulti={derived.isMulti}
      isDragging={c.geo.isDragging}
      isRunning={c.primaryStatus?.status === "running"}
      isQueued={c.primaryStatus?.status === "queued"}
      queuePosition={c.primaryStatus?.queuePosition}
      onMouseDown={c.geo.handleHeaderMouseDown}
      onRedock={c.geo.handleRedockToBottom}
      onOpenLeftChat={props.onOpenLeftChat}
      onCollapse={() => c.setIsCollapsed(true)}
      onClose={props.onClearSelection}
    />
    <FloatingChatBody props={props} c={c} />
    <AgentChatComposer
      inputValue={c.inputPrompt}
      onInputChange={c.setInputPrompt}
      onSend={c.handleSendMessage}
      attachmentChips={[]}
      onOpenSettings={props.onOpenSettings}
      testIdPrefix="floating-chat-composer"
      compact
      placeholder={derived.composerPlaceholder}
    />
    <FloatingChatResizeHandles onStartResize={c.geo.handleStartResize} />
  </div>
);

export const ContextualNodeFloatingChat: React.FC<ContextualNodeFloatingChatProps> = (props) => {
  const c = useFloatingChatController(props);
  const derived = useFloatingChatDerivedTexts(props, c);
  if (props.selectedNodes.length === 0) return null;

  if (c.isCollapsed) {
    return (
      <FloatingChatCollapsedPill
        containerRef={c.containerRef}
        pillX={c.geo.position.x}
        pillY={c.geo.position.y}
        isDragging={c.geo.isDragging}
        isMulti={derived.isMulti}
        selectedCount={props.selectedNodes.length}
        primaryNode={c.primaryNode}
        primaryNodeStatus={c.primaryStatus}
        showReviewSummary={c.showReviewSummary}
        onHeaderMouseDown={c.geo.handleHeaderMouseDown}
        onExpand={() => c.setIsCollapsed(false)}
        onToggleReviewSummary={() => c.setShowReviewSummary((prev) => !prev)}
        onCloseReviewSummary={() => c.setShowReviewSummary(false)}
        onClearSelection={props.onClearSelection}
      />
    );
  }

  return <FloatingChatExpandedWindow props={props} c={c} derived={derived} />;
};
