// @story 06.5
// @implements AC-04, AC-05 (Collapsible Scoped Sub-Chat 3-4 Lines with Canvas Element Pill & Pin Count)
import React from "react";
import { BrandIcon } from "../../../../components/ui/BrandIcon";
import { useTranslation } from "../../../../hooks/useTranslation";
import {
  FONT_WEIGHT_SEMIBOLD,
  ICON_SM,
  STUDIO_INPUT_STYLE,
  STUDIO_PRIMARY_BTN,
} from "./studioStyles";
import { SUBCHAT_COLLAPSED_PX, type SubChatMessage } from "./types";

export interface StudioSubChatProps {
  heightPx: number;
  collapsed: boolean;
  selectedFfId: string | null;
  pinCount: number;
  inputValue: string;
  messages: readonly SubChatMessage[];
  onToggleCollapsed: () => void;
  onInputChange: (val: string) => void;
  onSend: () => void;
}

const SUBCHAT_TEXTAREA_ROWS = 3;
const SUBCHAT_LINE_HEIGHT = 1.35;

const SubChatHeaderBar: React.FC<{
  collapsed: boolean;
  selectedFfId: string | null;
  pinCount: number;
  onToggleCollapsed: () => void;
}> = ({ collapsed, selectedFfId, pinCount, onToggleCollapsed }) => (
  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", height: `${SUBCHAT_COLLAPSED_PX}px`, padding: "0 10px", backgroundColor: "var(--bg-subtle)", borderBottom: collapsed ? "none" : "1px solid var(--border-subtle)", flexShrink: 0 }}>
    <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", minWidth: 0, fontSize: "11px" }}>
      <BrandIcon name="message-square" size={ICON_SM} color="var(--action-primary)" />
      <span style={{ fontWeight: FONT_WEIGHT_SEMIBOLD, whiteSpace: "nowrap" }}>Sub-Chat</span>
      {selectedFfId && (
        <span data-testid="studio-subchat-scope-pill" style={{ padding: "1px 6px", borderRadius: "999px", backgroundColor: "var(--bg-selected)", color: "var(--action-primary)", fontSize: "10.5px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: "155px" }}>
          ⌖ Đang chọn: {selectedFfId}
        </span>
      )}
      {pinCount > 0 && (
        <span data-testid="studio-subchat-pin-badge" style={{ padding: "1px 6px", borderRadius: "999px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)", fontSize: "10.5px", whiteSpace: "nowrap" }}>
          💬 {pinCount} ghim
        </span>
      )}
    </div>
    <button type="button" data-testid="studio-toggle-subchat-btn" onClick={onToggleCollapsed} aria-expanded={!collapsed} style={{ border: "none", background: "transparent", color: "var(--text-secondary)", cursor: "pointer", display: "inline-flex", alignItems: "center", padding: "2px" }}>
      <BrandIcon name={collapsed ? "chevron-right" : "chevron-down"} size={ICON_SM} color="currentColor" />
    </button>
  </div>
);

const SubChatBody: React.FC<{
  messages: readonly SubChatMessage[];
  inputValue: string;
  onInputChange: (val: string) => void;
  onSend: () => void;
}> = ({ messages, inputValue, onInputChange, onSend }) => {
  const { t } = useTranslation();
  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); onSend(); }
  };
  return (
    <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: "6px", padding: "6px 10px", minHeight: 0 }}>
      <div data-testid="studio-subchat-messages" style={{ flex: 1, overflowY: "auto", display: "flex", flexDirection: "column", gap: "4px", fontSize: "11px" }}>
        {messages.map((msg) => (
          <div key={msg.id} style={{ alignSelf: msg.role === "user" ? "flex-end" : "flex-start", backgroundColor: msg.role === "user" ? "var(--bg-selected)" : "var(--bg-subtle)", color: "var(--text-primary)", padding: "4px 8px", borderRadius: "var(--radius-sm)", maxWidth: "92%" }}>
            {msg.text}
          </div>
        ))}
      </div>
      <div style={{ display: "flex", gap: "6px", alignItems: "flex-end" }}>
        <textarea rows={SUBCHAT_TEXTAREA_ROWS} value={inputValue} onChange={(e) => onInputChange(e.target.value)} onKeyDown={handleKeyDown} placeholder={t("deckGen.subChatPlaceholder", "Tinh chỉnh nhanh vùng đang chọn (Enter để gửi)...")} data-testid="studio-subchat-input" style={{ ...STUDIO_INPUT_STYLE, flex: 1, resize: "none", fontSize: "11.5px", lineHeight: SUBCHAT_LINE_HEIGHT }} />
        <button type="button" data-testid="studio-subchat-send-btn" onClick={onSend} style={{ ...STUDIO_PRIMARY_BTN, padding: "7px 10px" }}>
          <BrandIcon name="send" size={ICON_SM} color="currentColor" />
          <span>{t("deckGen.subChatSend", "Gửi lệnh")}</span>
        </button>
      </div>
    </div>
  );
};

export const StudioSubChat: React.FC<StudioSubChatProps> = (props) => (
  <div data-testid="studio-subchat-panel" style={{ height: props.collapsed ? `${SUBCHAT_COLLAPSED_PX}px` : `${props.heightPx}px`, display: "flex", flexDirection: "column", backgroundColor: "var(--bg-surface)", borderTop: "1px solid var(--border-default)", flexShrink: 0, overflow: "hidden" }}>
    <SubChatHeaderBar collapsed={props.collapsed} selectedFfId={props.selectedFfId} pinCount={props.pinCount} onToggleCollapsed={props.onToggleCollapsed} />
    {!props.collapsed && <SubChatBody messages={props.messages} inputValue={props.inputValue} onInputChange={props.onInputChange} onSend={props.onSend} />}
  </div>
);
