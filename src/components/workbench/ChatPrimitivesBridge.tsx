// @story 04.4
// @implements AC-02 (FSD Layer Bridge: Shared Chat Stream & Composer Primitives in components/)
import React, { useRef, useState } from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useTranslation } from "@/hooks/useTranslation";
import { useSyncedDefaultModel } from "@/hooks/useDoctorProvidersUsage";
import { loadPersistedSettings, savePersistedSettings } from "@/pages/workbench/hooks/useWorkbenchSettingsStorage";
import { AVAILABLE_CHAT_MODELS, ChatStreamItem, type ChatMessage } from "./ChatStreamPrimitives";
import { ModelPickerQuotaPopover } from "./ModelPickerQuotaPopover";

export { AVAILABLE_CHAT_MODELS, ChatStreamItem, type ChatMessage };

export interface AgentChatComposerProps {
  inputValue: string;
  onInputChange: (val: string) => void;
  onSend: () => void;
  attachmentChips: string[];
  onRemoveAttachmentChip?: (chip: string) => void;
  onAttachClick?: () => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
  placeholder?: string;
  testIdPrefix?: string;
  compact?: boolean;
}

const TEXTAREA_STYLE: React.CSSProperties = {
  width: "100%",
  minHeight: "38px",
  maxHeight: "144px",
  border: "none",
  background: "transparent",
  outline: "none",
  resize: "none",
  overflowY: "auto",
  fontSize: "12.5px",
  fontFamily: "var(--font-sans)",
  color: "var(--text-primary)",
  lineHeight: "24px",
  boxSizing: "border-box",
};

function syncTextareaHeight(el: HTMLTextAreaElement | null): void {
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(Math.max(el.scrollHeight, 38), 144)}px`;
}

const AttachmentChipList: React.FC<{
  chips: string[];
  onRemove?: (chip: string) => void;
}> = ({ chips, onRemove }) => {
  const { t } = useTranslation();
  if (chips.length === 0) return null;
  return (
    <div id="chatAttachmentTray" style={{ display: "flex", gap: "5px", flexWrap: "wrap", paddingBottom: "4px" }}>
      {chips.map((chip, idx) => (
        <span key={idx} className="cowok-badge cowok-badge-mono" style={{ fontSize: "10px", padding: "2px 7px", borderRadius: "var(--radius-pill)", display: "inline-flex", alignItems: "center", gap: "5px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", color: "var(--text-primary)" }}>
          <span>{chip}</span>
          {onRemove && (
            <button type="button" onClick={() => onRemove(chip)} aria-label={t("workbench.deleteItem", "Xóa")} style={{ border: "none", background: "transparent", cursor: "pointer", padding: 0, fontSize: "11px", lineHeight: 1, color: "var(--text-muted)" }}>×</button>
          )}
        </span>
      ))}
    </div>
  );
};

const ComposerFooterBar: React.FC<{
  selectedModel: { id: string; label: string };
  onSelectModel: (model: { id: string; label: string }) => void;
  isSendDisabled: boolean;
  onSend: () => void;
  onAttachClick?: () => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
}> = ({ selectedModel, onSelectModel, isSendDisabled, onSend, onAttachClick, onOpenSettings }) => {
  const { t } = useTranslation();
  const attachLabel = t("chat.attachContext", "Đính kèm tệp hoặc vùng chọn");
  const sendLabel = t("chat.sendPrompt", "Gửi chỉ thị (Enter)");

  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", paddingTop: "2px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "6px", position: "relative" }}>
        <button type="button" onClick={onAttachClick} title={attachLabel} aria-label={attachLabel} className="cowok-btn-ghost" style={{ width: "24px", height: "24px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)", cursor: "pointer" }}>
          <BrandIcon name="plus" size={13} color="currentColor" />
        </button>
        <ModelPickerQuotaPopover
          selectedModel={selectedModel}
          onSelectModel={(next) => {
            onSelectModel(next);
            savePersistedSettings({ defaultModel: next.id });
          }}
          onOpenSettings={onOpenSettings}
        />
      </div>
      <button type="button" onMouseDown={(e) => e.preventDefault()} onClick={onSend} disabled={isSendDisabled} title={sendLabel} aria-label={sendLabel} data-testid="chat-composer-send-btn" className="cowok-btn-primary" style={{ width: "28px", height: "28px", minWidth: "28px", padding: 0, flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center", borderRadius: "50%", backgroundColor: isSendDisabled ? "var(--bg-subtle)" : "var(--action-primary)", color: isSendDisabled ? "var(--text-muted)" : "var(--action-primary-fg)", border: "none", cursor: isSendDisabled ? "not-allowed" : "pointer", transition: "all var(--duration-fast)" }}>
        <BrandIcon name="send" size={12} color="currentColor" />
      </button>
    </div>
  );
};

export const AgentChatComposer: React.FC<AgentChatComposerProps> = (props) => {
  const { t } = useTranslation();
  const [selectedModel, setSelectedModel] = useSyncedDefaultModel(
    AVAILABLE_CHAT_MODELS[0],
    loadPersistedSettings,
    AVAILABLE_CHAT_MODELS
  );
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const isSendDisabled = !props.inputValue.trim() && props.attachmentChips.length === 0;
  const placeholder =
    props.placeholder ||
    t("chat.composerPlaceholder", "Hỏi cowok Agent hoặc gõ lệnh... (Shift+Enter để xuống dòng, bôi đen chữ để Quote)");
  const prefix = props.testIdPrefix || "chat-composer";
  const wrapperPadding = props.compact ? "6px 10px 10px 10px" : "8px 12px 12px 12px";

  const handleSend = () => {
    props.onSend();
    if (textareaRef.current) textareaRef.current.style.height = "38px";
  };

  return (
    <div data-testid={`${prefix}-wrapper`} style={{ padding: wrapperPadding, backgroundColor: "var(--bg-canvas)", borderTop: "none" }}>
      <div className="chat-composer-capsule" style={{ backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-md)", padding: "8px 10px", boxShadow: "var(--shadow-sm)", display: "flex", flexDirection: "column", gap: "6px" }}>
        <AttachmentChipList chips={props.attachmentChips} onRemove={props.onRemoveAttachmentChip} />
        <textarea
          ref={textareaRef}
          data-testid={`${prefix}-textarea`}
          value={props.inputValue}
          onChange={(e) => { props.onInputChange(e.target.value); syncTextareaHeight(e.currentTarget); }}
          onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); } }}
          placeholder={placeholder}
          rows={1}
          style={TEXTAREA_STYLE}
        />
        <ComposerFooterBar selectedModel={selectedModel} onSelectModel={setSelectedModel} isSendDisabled={isSendDisabled} onSend={handleSend} onAttachClick={props.onAttachClick} onOpenSettings={props.onOpenSettings} />
      </div>
    </div>
  );
};
