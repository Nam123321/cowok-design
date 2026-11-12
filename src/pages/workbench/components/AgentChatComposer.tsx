// @story 04.1
import React, { useEffect, useRef, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

export const AVAILABLE_CHAT_MODELS = [
  { id: "cowok-swarm", label: "Cowok Swarm · High" },
  { id: "claude-37", label: "Claude 3.7 Sonnet" },
  { id: "gemini-25", label: "Gemini 2.5 Pro" },
];

interface AgentChatComposerProps {
  inputValue: string;
  onInputChange: (val: string) => void;
  onSend: () => void;
  attachmentChips: string[];
  onRemoveAttachmentChip?: (chip: string) => void;
  onAttachClick?: () => void;
  onOpenSettings?: (tab?: "general" | "models" | "mcp" | "automations") => void;
}

const TEXTAREA_STYLE: React.CSSProperties = {
  width: "100%", minHeight: "38px", maxHeight: "144px", border: "none", background: "transparent",
  outline: "none", resize: "none", overflowY: "auto", fontSize: "12.5px",
  fontFamily: "var(--font-sans)", color: "var(--text-primary)", lineHeight: "24px", boxSizing: "border-box",
};

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

const ComposerModelMenu: React.FC<{
  selectedId: string;
  onSelect: (model: { id: string; label: string }) => void;
  onConfigure?: () => void;
}> = ({ selectedId, onSelect, onConfigure }) => {
  const { t } = useTranslation();
  return (
    <div style={{ position: "absolute", bottom: "30px", left: "28px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-sm)", boxShadow: "var(--shadow-md)", padding: "4px", zIndex: 20, minWidth: "160px" }}>
      {AVAILABLE_CHAT_MODELS.map((m) => {
        const active = selectedId === m.id;
        return (
          <div key={m.id} role="button" onClick={() => onSelect(m)} style={{ padding: "6px 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", cursor: "pointer", backgroundColor: active ? "var(--bg-selected)" : "transparent", color: "var(--text-primary)", borderLeft: active ? "2px solid var(--action-primary)" : "2px solid transparent", fontWeight: active ? 600 : 400 }}>
            {m.label}
          </div>
        );
      })}
      <div style={{ height: "1px", backgroundColor: "var(--border-subtle)", margin: "4px 0" }} />
      <div role="button" onClick={onConfigure} style={{ padding: "6px 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", cursor: "pointer", color: "var(--action-primary)", fontWeight: 500, display: "flex", alignItems: "center", gap: "5px" }}>
        <BrandIcon name="settings" size={11} color="currentColor" />
        <span>{t("chat.configureModels", "Cấu hình API & BYOS...")}</span>
      </div>
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
  const [open, setOpen] = useState(false);
  const attachLabel = t("chat.attachContext", "Đính kèm tệp hoặc vùng chọn");
  const modelLabel = t("chat.selectModel", "Chọn mô hình AI");
  const sendLabel = t("chat.sendPrompt", "Gửi chỉ thị (Enter)");

  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", paddingTop: "2px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "6px", position: "relative" }}>
        <button type="button" onClick={onAttachClick} title={attachLabel} aria-label={attachLabel} className="cowok-btn-ghost" style={{ width: "24px", height: "24px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-subtle)", color: "var(--text-secondary)", cursor: "pointer" }}>
          <BrandIcon name="plus" size={13} color="currentColor" />
        </button>
        <button type="button" onClick={() => setOpen((v) => !v)} title={modelLabel} aria-label={modelLabel} style={{ height: "24px", padding: "0 8px", display: "flex", alignItems: "center", gap: "5px", fontSize: "11px", fontWeight: 500, borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", color: "var(--text-primary)", cursor: "pointer" }}>
          <span>{selectedModel.label}</span>
          <BrandIcon name="chevron-down" size={10} color="var(--text-muted)" />
        </button>
        {open && (
          <ComposerModelMenu selectedId={selectedModel.id} onSelect={(m) => { onSelectModel(m); setOpen(false); }} onConfigure={() => { setOpen(false); onOpenSettings?.("models"); }} />
        )}
      </div>
      <button type="button" onClick={onSend} disabled={isSendDisabled} title={sendLabel} aria-label={sendLabel} className="cowok-btn-primary" style={{ width: "26px", height: "26px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", borderRadius: "50%", backgroundColor: isSendDisabled ? "var(--bg-subtle)" : "var(--action-primary)", color: isSendDisabled ? "var(--text-muted)" : "var(--action-primary-fg)", border: "none", cursor: isSendDisabled ? "not-allowed" : "pointer", transition: "all var(--duration-fast)" }}>
        <BrandIcon name="send" size={12} color="currentColor" />
      </button>
    </div>
  );
};

export const AgentChatComposer: React.FC<AgentChatComposerProps> = (props) => {
  const { t } = useTranslation();
  const [selectedModel, setSelectedModel] = useState(AVAILABLE_CHAT_MODELS[0]);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${Math.min(Math.max(textareaRef.current.scrollHeight, 38), 144)}px`;
    }
  }, [props.inputValue]);

  const isSendDisabled = !props.inputValue.trim() && props.attachmentChips.length === 0;
  const placeholder = t("chat.composerPlaceholder", "Hỏi cowok Agent hoặc gõ lệnh... (Shift+Enter để xuống dòng, bôi đen chữ để Quote)");

  return (
    <div data-testid="chat-composer-wrapper" style={{ padding: "8px 12px 12px 12px", backgroundColor: "var(--bg-canvas)", borderTop: "none" }}>
      <div className="chat-composer-capsule" style={{ backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-md)", padding: "8px 10px", boxShadow: "var(--shadow-sm)", display: "flex", flexDirection: "column", gap: "6px" }}>
        <AttachmentChipList chips={props.attachmentChips} onRemove={props.onRemoveAttachmentChip} />
        <textarea ref={textareaRef} data-testid="chat-composer-textarea" value={props.inputValue} onChange={(e) => props.onInputChange(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); props.onSend(); } }} placeholder={placeholder} rows={1} style={TEXTAREA_STYLE} />
        <ComposerFooterBar selectedModel={selectedModel} onSelectModel={setSelectedModel} isSendDisabled={isSendDisabled} onSend={props.onSend} onAttachClick={props.onAttachClick} onOpenSettings={props.onOpenSettings} />
      </div>
    </div>
  );
};
