// @story 05.6, 06.5
// @implements [5.26] Collapsible Scoped Sub-Chat Embedding Main AgentChatComposer + Live Bidirectional Main-Chat Sync
import React from "react";
import { BrandIcon } from "../BrandIcon";
import { AgentChatComposer } from "../../workbench/ChatPrimitivesBridge";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  STUDIO_FONT_SEMIBOLD,
  STUDIO_ICON_SM,
  STUDIO_ICON_XS,
  STUDIO_STROKE_MONO,
  STUDIO_SUBCHAT_COLLAPSED_PX,
  type StudioSubChatMessage,
} from "./types";

const BUBBLE_LINE_HEIGHT = 1.4;

export interface StudioSubChatPaneProps {
  heightPx: number;
  collapsed: boolean;
  selectedFfId: string | null;
  pinCount: number;
  inputValue: string;
  messages: readonly StudioSubChatMessage[];
  attachmentChips?: string[];
  onRemoveAttachmentChip?: (chip: string) => void;
  onAttachFiles?: (fileNames: string[], snippetsByName?: Record<string, string>) => void;
  onPasteBinaryFiles?: (files: File[]) => void;
  placeholder?: string;
  syncToMainChatEnabled?: boolean;
  onToggleSyncToMainChat?: () => void;
  onToggleCollapsed: () => void;
  onInputChange: (val: string) => void;
  onSend: () => void;
}

function resolveBubbleBackground(msg: StudioSubChatMessage): string {
  if (msg.kind === "comment") return "var(--state-warning-soft)";
  if (msg.role === "user") return "var(--bg-selected)";
  return "var(--bg-subtle)";
}

const SyncChatToggleBtn: React.FC<{ enabled: boolean; onToggle: () => void }> = ({ enabled, onToggle }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      data-testid="studio-subchat-sync-toggle"
      aria-pressed={enabled}
      onClick={onToggle}
      title={t("brandStudio.subChatSyncTooltip", "Tự động đồng bộ tóm tắt Sub-Chat về khung Chat chính đang mở")}
      style={{
        height: "20px", padding: "0 6px", borderRadius: "999px",
        border: enabled ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
        backgroundColor: enabled ? "var(--bg-selected)" : "transparent",
        color: enabled ? "var(--action-primary)" : "var(--text-muted)",
        fontSize: "10px", fontWeight: STUDIO_FONT_SEMIBOLD, display: "inline-flex",
        alignItems: "center", gap: "3px", cursor: "pointer", whiteSpace: "nowrap",
      }}
    >
      <BrandIcon name="refresh-cw" size={STUDIO_ICON_XS} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
      <span>{t("brandStudio.subChatSyncLabel", "Đồng bộ Chat")}</span>
    </button>
  );
};

const SubChatHeader: React.FC<{
  collapsed: boolean;
  selectedFfId: string | null;
  pinCount: number;
  syncToMainChatEnabled?: boolean;
  onToggleSyncToMainChat?: () => void;
  onToggleCollapsed: () => void;
}> = ({ collapsed, selectedFfId, pinCount, syncToMainChatEnabled, onToggleSyncToMainChat, onToggleCollapsed }) => (
  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", height: `${STUDIO_SUBCHAT_COLLAPSED_PX}px`, padding: "0 10px", backgroundColor: "var(--bg-subtle)", borderBottom: collapsed ? "none" : "1px solid var(--border-subtle)", flexShrink: 0 }}>
    <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", minWidth: 0, fontSize: "11px" }}>
      <BrandIcon name="message-square" size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="var(--action-primary)" />
      <span style={{ fontWeight: STUDIO_FONT_SEMIBOLD, whiteSpace: "nowrap" }}>Sub-Chat</span>
      {selectedFfId && (
        <span data-testid="studio-subchat-scope-pill" style={{ padding: "1px 6px", borderRadius: "999px", backgroundColor: "var(--bg-selected)", color: "var(--action-primary)", fontSize: "10.5px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: "140px" }}>
          ⌖ {selectedFfId}
        </span>
      )}
      {pinCount > 0 && (
        <span data-testid="studio-subchat-pin-badge" style={{ padding: "1px 6px", borderRadius: "999px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", fontSize: "10.5px", whiteSpace: "nowrap" }}>
          {pinCount}
        </span>
      )}
    </div>
    <div style={{ display: "inline-flex", alignItems: "center", gap: "4px", flexShrink: 0 }}>
      {onToggleSyncToMainChat && <SyncChatToggleBtn enabled={Boolean(syncToMainChatEnabled)} onToggle={onToggleSyncToMainChat} />}
      <button type="button" data-testid="studio-toggle-subchat-btn" onClick={onToggleCollapsed} aria-expanded={!collapsed} style={{ border: "none", background: "transparent", color: "var(--text-secondary)", cursor: "pointer", display: "inline-flex", alignItems: "center", padding: "2px" }}>
        <BrandIcon name={collapsed ? "chevron-right" : "chevron-down"} size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
      </button>
    </div>
  </div>
);

const SubChatMessageBubble: React.FC<{ msg: StudioSubChatMessage }> = ({ msg }) => {
  const isComment = msg.kind === "comment";
  return (
    <div
      data-testid={isComment ? "studio-subchat-comment-item" : undefined}
      style={{
        alignSelf: msg.role === "user" ? "flex-end" : "flex-start",
        backgroundColor: resolveBubbleBackground(msg),
        border: isComment ? "1px solid var(--state-warning)" : "1px solid transparent",
        color: "var(--text-primary)", padding: "4px 8px", borderRadius: "var(--radius-sm)", maxWidth: "94%", lineHeight: BUBBLE_LINE_HEIGHT,
      }}
    >
      {isComment && (
        <span style={{ fontSize: "9.5px", fontWeight: STUDIO_FONT_SEMIBOLD, color: "var(--state-warning)", marginRight: "5px" }}>
          {`#${msg.pinNumber ?? 1}`}
        </span>
      )}
      <span>{msg.text}</span>
    </div>
  );
};

function useSubChatAttachments(props: StudioSubChatPaneProps) {
  const [localChips, setLocalChips] = React.useState<string[]>([]);
  const chips = props.attachmentChips ?? localChips;
  const onAttachFiles = (fileNames: string[], snippetsByName?: Record<string, string>) => {
    if (props.onAttachFiles) props.onAttachFiles(fileNames, snippetsByName);
    else setLocalChips((prev) => Array.from(new Set([...prev, ...fileNames])));
  };
  const onRemoveChip = (chip: string) => {
    if (props.onRemoveAttachmentChip) props.onRemoveAttachmentChip(chip);
    else setLocalChips((prev) => prev.filter((c) => c !== chip));
  };
  return { chips, onAttachFiles, onRemoveChip };
}

export const StudioSubChatPane: React.FC<StudioSubChatPaneProps> = (props) => {
  const { t } = useTranslation();
  const { chips, onAttachFiles, onRemoveChip } = useSubChatAttachments(props);
  return (
    <div data-testid="studio-subchat-panel" style={{ height: props.collapsed ? `${STUDIO_SUBCHAT_COLLAPSED_PX}px` : `${props.heightPx}px`, flexShrink: 0, display: "flex", flexDirection: "column", borderTop: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)" }}>
      <SubChatHeader collapsed={props.collapsed} selectedFfId={props.selectedFfId} pinCount={props.pinCount} syncToMainChatEnabled={props.syncToMainChatEnabled} onToggleSyncToMainChat={props.onToggleSyncToMainChat} onToggleCollapsed={props.onToggleCollapsed} />
      {!props.collapsed && (
        <div style={{ flex: 1, display: "flex", flexDirection: "column", minHeight: 0, backgroundColor: "var(--bg-canvas)" }}>
          <div data-testid="studio-subchat-messages" style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: "4px", padding: "6px 10px 2px 10px", fontSize: "11px" }}>
            {props.messages.map((msg) => <SubChatMessageBubble key={msg.id} msg={msg} />)}
          </div>
          <AgentChatComposer
            compact testIdPrefix="studio-subchat" textareaTestId="studio-subchat-input" sendButtonTestId="studio-subchat-send-btn"
            inputValue={props.inputValue} onInputChange={props.onInputChange} onSend={props.onSend}
            attachmentChips={chips} onRemoveAttachmentChip={onRemoveChip} onAttachFiles={onAttachFiles} onPasteBinaryFiles={props.onPasteBinaryFiles}
            placeholder={props.placeholder ?? t("brandStudio.subChatPlaceholder", "Nhắn lệnh sửa trực tiếp hoặc đính kèm tệp...")}
          />
        </div>
      )}
    </div>
  );
};
