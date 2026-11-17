// @story 04.1, 04.5
// @implements AC-01 (Inline HTML Code Editor with dirty_buffer Lock Guard)
import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CanvasTabItem } from "./CompanionCanvasTab";

export interface InlineCodeEditorPanelProps {
  activeTab: CanvasTabItem;
  inlineCodeText: string;
  dirtyBuffer: boolean;
  showDirtyGuardPrompt: boolean;
  errorMsg?: string | null;
  onChangeInlineCode: (val: string) => void;
  onApplyHtml: () => void;
  onConfirmDiscardDirty?: () => void;
  onCancelCloseEditor?: () => void;
}

const DirtyGuardActions: React.FC<{
  onApplyHtml: () => void;
  onConfirmDiscardDirty?: () => void;
  onCancelCloseEditor?: () => void;
}> = ({ onApplyHtml, onConfirmDiscardDirty, onCancelCloseEditor }) => {
  const { t } = useTranslation();
  const btnStyle: React.CSSProperties = { padding: "2px 8px", fontSize: "10.5px", borderRadius: "var(--radius-xs)", cursor: "pointer" };
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
      <button type="button" data-testid="dirty-guard-apply-btn" onClick={onApplyHtml} style={{ ...btnStyle, fontWeight: 600, backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", border: "none" }}>
        {t("canvas.dirtyGuardApply", "Áp dụng ngay")}
      </button>
      {onConfirmDiscardDirty && (
        <button type="button" data-testid="dirty-guard-discard-btn" onClick={onConfirmDiscardDirty} style={{ ...btnStyle, backgroundColor: "var(--bg-surface)", color: "var(--state-danger)", border: "1px solid var(--state-danger)" }}>
          {t("canvas.dirtyGuardDiscard", "Bỏ thay đổi")}
        </button>
      )}
      {onCancelCloseEditor && (
        <button type="button" data-testid="dirty-guard-keep-btn" onClick={onCancelCloseEditor} style={{ ...btnStyle, backgroundColor: "var(--bg-surface)", color: "var(--text-secondary)", border: "1px solid var(--border-default)" }}>
          {t("canvas.dirtyGuardKeep", "Ở lại sửa")}
        </button>
      )}
    </div>
  );
};

const InlineEditorHeader: React.FC<{
  title: string;
  dirtyBuffer: boolean;
  onApplyHtml: () => void;
}> = ({ title, dirtyBuffer, onApplyHtml }) => {
  const { t } = useTranslation();
  return (
    <div style={{ height: "32px", padding: "0 12px", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", fontFamily: "var(--font-mono)", fontSize: "11px" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <span>&lt;/&gt; {title}</span>
        {dirtyBuffer && (
          <span data-testid="inline-html-dirty-badge" style={{ fontSize: "10px", padding: "1px 6px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-warning-soft)", border: "1px solid var(--state-warning)", color: "var(--text-primary)", fontWeight: 600 }}>
            {t("canvas.dirtyBufferBadge", "Chưa lưu (dirty_buffer)")}
          </span>
        )}
      </div>
      <button type="button" data-testid="apply-inline-html-btn" onClick={onApplyHtml} className="cowok-btn cowok-btn-primary" style={{ height: "22px", padding: "0 8px", fontSize: "10.5px", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)" }}>
        {t("canvas.applyToHtml", "✓ Áp dụng lên HTML")}
      </button>
    </div>
  );
};

export const InlineCodeEditorPanel: React.FC<InlineCodeEditorPanelProps> = (p) => {
  const { t } = useTranslation();
  return (
    <div data-testid="inline-code-editor-panel" data-inline-code-editor="true" style={{ width: "100%", height: "100%", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-md)", display: "flex", flexDirection: "column", overflow: "hidden" }}>
      <InlineEditorHeader title={p.activeTab.filePath || p.activeTab.title} dirtyBuffer={p.dirtyBuffer} onApplyHtml={p.onApplyHtml} />
      {p.showDirtyGuardPrompt && (
        <div role="alertdialog" data-testid="dirty-buffer-guard-banner" style={{ padding: "8px 12px", backgroundColor: "var(--state-warning-soft)", borderBottom: "1px solid var(--state-warning)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", fontSize: "11px", color: "var(--text-primary)" }}>
          <span>{t("canvas.dirtyBufferPrompt", "Bạn có thay đổi HTML chưa áp dụng (dirty_buffer). Lưu hoặc hủy trước khi rời trình sửa?")}</span>
          <DirtyGuardActions onApplyHtml={p.onApplyHtml} onConfirmDiscardDirty={p.onConfirmDiscardDirty} onCancelCloseEditor={p.onCancelCloseEditor} />
        </div>
      )}
      {p.errorMsg && (
        <div role="alert" data-testid="inline-html-error" style={{ padding: "6px 12px", backgroundColor: "var(--bg-subtle)", borderBottom: "1px solid var(--state-error)", color: "var(--state-error)", fontSize: "11px", fontFamily: "var(--font-mono)" }}>
          {p.errorMsg}
        </div>
      )}
      <textarea data-testid="inline-html-textarea" value={p.inlineCodeText} onChange={(e) => p.onChangeInlineCode(e.target.value)} spellCheck={false} style={{ flex: 1, width: "100%", padding: "12px", border: "none", backgroundColor: "var(--bg-canvas)", color: "var(--text-primary)", fontFamily: "var(--font-mono)", fontSize: "12px", lineHeight: "1.55", resize: "none", outline: "none", boxSizing: "border-box" }} />
    </div>
  );
};
