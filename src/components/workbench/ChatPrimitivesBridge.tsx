// @story 04.4
// @implements AC-02 (FSD Layer Bridge: Shared Chat Stream & Composer Primitives in components/)
import React, { useRef } from "react";
import { BrandIcon } from "@/components/ui/BrandIcon";
import { useTranslation } from "@/hooks/useTranslation";
import { useSyncedDefaultModel } from "@/hooks/useDoctorProvidersUsage";
import { loadPersistedSettings, savePersistedSettings } from "@/utils/workbench-settings-storage";
import {
  ALLOWED_CHAT_ATTACHMENT_ACCEPT,
  readSelectedChatFiles,
} from "@/features/chat-vault/read-selected-chat-files";
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
  onAttachFiles?: (fileNames: string[], snippetsByName?: Record<string, string>) => void;
  onPasteBinaryFiles?: (files: File[]) => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
  placeholder?: string;
  testIdPrefix?: string;
  textareaTestId?: string;
  sendButtonTestId?: string;
  compact?: boolean;
}

const COMPOSER_MIN_HEIGHT_PX = 38;
const COMPOSER_MAX_HEIGHT_PX = 144;
const ICON_PLUS_SIZE = 13;
const ICON_SEND_SIZE = 12;

const TEXTAREA_STYLE: React.CSSProperties = {
  width: "100%",
  minHeight: `${COMPOSER_MIN_HEIGHT_PX}px`,
  maxHeight: `${COMPOSER_MAX_HEIGHT_PX}px`,
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
  el.style.height = `${Math.min(Math.max(el.scrollHeight, COMPOSER_MIN_HEIGHT_PX), COMPOSER_MAX_HEIGHT_PX)}px`;
}

const AttachmentChipList: React.FC<{
  prefix: string;
  chips: string[];
  onRemove?: (chip: string) => void;
}> = ({ prefix, chips, onRemove }) => {
  const { t } = useTranslation();
  if (chips.length === 0) return null;
  const trayId = prefix === "chat-composer" ? "chatAttachmentTray" : `${prefix}-attachment-tray`;
  return (
    <div id={trayId} style={{ display: "flex", gap: "5px", flexWrap: "wrap", paddingBottom: "4px" }}>
      {chips.map((chip, idx) => (
        <span key={idx} data-testid={`attachment-chip-${idx}`} className="cowok-badge cowok-badge-mono" style={{ fontSize: "10px", padding: "2px 7px", borderRadius: "var(--radius-pill)", display: "inline-flex", alignItems: "center", gap: "5px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", color: "var(--text-primary)" }}>
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
  prefix: string;
  selectedModel: { id: string; label: string };
  onSelectModel: (model: { id: string; label: string }) => void;
  isSendDisabled: boolean;
  onSend: () => void;
  onAttachClick?: () => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
  sendButtonTestId?: string;
}> = ({ prefix, selectedModel, onSelectModel, isSendDisabled, onSend, onAttachClick, onOpenSettings, sendButtonTestId = "chat-composer-send-btn" }) => {
  const { t } = useTranslation();
  const attachLabel = t("chat.attachContext", "Đính kèm tệp hoặc vùng chọn");
  const sendLabel = t("chat.sendPrompt", "Gửi chỉ thị (Enter)");
  const attachTestId = prefix === "chat-composer" ? "chat-composer-attach-btn" : `${prefix}-attach-btn`;

  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", paddingTop: "2px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "6px", position: "relative" }}>
        <button type="button" data-testid={attachTestId} onClick={onAttachClick} title={attachLabel} aria-label={attachLabel} className="cowok-btn-ghost" style={{ width: "24px", height: "24px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)", cursor: "pointer" }}>
          <BrandIcon name="plus" size={ICON_PLUS_SIZE} color="currentColor" />
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
      <button type="button" onClick={onSend} disabled={isSendDisabled} title={sendLabel} aria-label={sendLabel} data-testid={sendButtonTestId} className="cowok-btn-primary" style={{ width: "28px", height: "28px", minWidth: "28px", padding: 0, flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center", borderRadius: "50%", backgroundColor: isSendDisabled ? "var(--bg-subtle)" : "var(--action-primary)", color: isSendDisabled ? "var(--text-muted)" : "var(--action-primary-fg)", border: "none", cursor: isSendDisabled ? "not-allowed" : "pointer", transition: "all var(--duration-fast)" }}>
        <BrandIcon name="send" size={ICON_SEND_SIZE} color="currentColor" />
      </button>
    </div>
  );
};

const COMPOSER_SEND_DEDUP_MS = 250;
const IME_COMPOSITION_KEYCODE = 229;

function ComposerTextareaField(opts: {
  textareaRef: React.RefObject<HTMLTextAreaElement | null>;
  prefix: string;
  textareaTestId?: string;
  inputValue: string;
  placeholder: string;
  isComposingRef: React.MutableRefObject<boolean>;
  onInputChange: (val: string) => void;
  onPasteFiles?: (files: File[]) => void;
  onSend: () => void;
}) {
  const { textareaRef, prefix, textareaTestId, inputValue, placeholder, isComposingRef, onInputChange, onPasteFiles, onSend } = opts;
  const inputFfId = prefix === "chat-composer" ? "chat-composer-input" : `${prefix}-input`;
  return (
    <textarea
      ref={textareaRef}
      data-ff-id={inputFfId}
      data-testid={textareaTestId ?? `${prefix}-textarea`}
      value={inputValue}
      onCompositionStart={() => { isComposingRef.current = true; }}
      onCompositionEnd={() => { isComposingRef.current = false; }}
      onChange={(e) => { onInputChange(e.target.value); syncTextareaHeight(e.currentTarget); }}
      onPaste={(e) => {
        const files = Array.from(e.clipboardData?.files || []);
        if (files.length === 0) return;
        e.preventDefault();
        onPasteFiles?.(files);
      }}
      onKeyDown={(e) => {
        if (e.key !== "Enter" || e.shiftKey) return;
        if (e.nativeEvent.isComposing || e.keyCode === IME_COMPOSITION_KEYCODE || isComposingRef.current || e.repeat) return;
        e.preventDefault();
        onSend();
      }}
      placeholder={placeholder}
      rows={1}
      style={TEXTAREA_STYLE}
    />
  );
}

export const AgentChatComposer: React.FC<AgentChatComposerProps> = (props) => {
  const { t } = useTranslation();
  const [selectedModel, setSelectedModel] = useSyncedDefaultModel(AVAILABLE_CHAT_MODELS[0], loadPersistedSettings, AVAILABLE_CHAT_MODELS);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const isComposingRef = useRef(false);
  const lastSendAtRef = useRef(0);
  const isSendDisabled = !props.inputValue.trim() && props.attachmentChips.length === 0;
  const placeholder = props.placeholder || t("chat.composerPlaceholder", "Hỏi cowok Agent hoặc gõ lệnh... (Shift+Enter để xuống dòng, bôi đen chữ để Quote)");
  const prefix = props.testIdPrefix || "chat-composer";

  const handleSend = () => {
    const now = Date.now();
    if (now - lastSendAtRef.current < COMPOSER_SEND_DEDUP_MS) return;
    lastSendAtRef.current = now;
    props.onSend();
    if (textareaRef.current) textareaRef.current.style.height = `${COMPOSER_MIN_HEIGHT_PX}px`;
  };
  const handleAttachButtonClick = () => {
    if (props.onAttachFiles && fileInputRef.current) fileInputRef.current.click();
    else props.onAttachClick?.();
  };
  const handleFileInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const selected = e.target.files ? Array.from(e.target.files) : [];
    e.target.value = "";
    if (selected.length > 0 && (props.onAttachFiles || props.onPasteBinaryFiles)) {
      void readSelectedChatFiles(selected).then(({ names, snippets, binaryFiles }) => {
        if (binaryFiles.length > 0) props.onPasteBinaryFiles?.(binaryFiles);
        if (names.length > 0) props.onAttachFiles?.(names, snippets);
      });
    }
  };

  return (
    <div data-testid={`${prefix}-wrapper`} style={{ padding: props.compact ? "6px 10px 10px 10px" : "8px 12px 12px 12px", backgroundColor: "var(--bg-canvas)", borderTop: "none" }}>
      <input ref={fileInputRef} type="file" multiple data-testid={`${prefix}-file-input`} accept={ALLOWED_CHAT_ATTACHMENT_ACCEPT} onChange={handleFileInputChange} style={{ display: "none" }} />
      <div className="chat-composer-capsule" style={{ backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-md)", padding: "8px 10px", boxShadow: "var(--shadow-sm)", display: "flex", flexDirection: "column", gap: "6px" }}>
        <AttachmentChipList prefix={prefix} chips={props.attachmentChips} onRemove={props.onRemoveAttachmentChip} />
        <ComposerTextareaField
          textareaRef={textareaRef}
          prefix={prefix}
          textareaTestId={props.textareaTestId}
          inputValue={props.inputValue}
          placeholder={placeholder}
          isComposingRef={isComposingRef}
          onInputChange={props.onInputChange}
          onPasteFiles={(files) => {
            props.onPasteBinaryFiles?.(files);
            void readSelectedChatFiles(files).then(({ names, snippets }) => props.onAttachFiles?.(names, snippets));
          }}
          onSend={handleSend}
        />
        <ComposerFooterBar prefix={prefix} selectedModel={selectedModel} onSelectModel={setSelectedModel} isSendDisabled={isSendDisabled} onSend={handleSend} onAttachClick={handleAttachButtonClick} onOpenSettings={props.onOpenSettings} sendButtonTestId={props.sendButtonTestId} />
      </div>
    </div>
  );
};
