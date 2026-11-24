// @story 05.6, 06.5
// @implements [5.26] Canonical Spatial Action Bar for Element Select, Drag-Region & Right-Click Context Menu
import React, { useEffect, useState } from "react";
import { BrandIcon } from "../BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  STUDIO_FONT_SEMIBOLD,
  STUDIO_ICON_SM,
  STUDIO_STROKE_MONO,
  type StudioSpatialCoreAction,
  type StudioSpatialDomainAction,
  type StudioSpatialTarget,
} from "./types";

const ACTION_BAR_Z = 35;
const MIN_CLAMP_PX = 8;
const VERTICAL_OFFSET_PX = 38;

export interface StudioSpatialActionBarProps {
  target: StudioSpatialTarget | null;
  domainActions?: readonly StudioSpatialDomainAction[];
  onCoreAction: (action: StudioSpatialCoreAction, target: StudioSpatialTarget) => void;
  onSubmitComment?: (target: StudioSpatialTarget, commentText: string) => void;
  onClose: () => void;
}

function isEditableEvent(ev: KeyboardEvent): boolean {
  if (ev.isComposing) return true;
  const el = ev.target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName?.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || el.isContentEditable;
}

const CORE_ACTION_ITEMS: ReadonlyArray<{ id: StudioSpatialCoreAction; i18nKey: string; defaultLabel: string; hotkey: string; icon: string }> = [
  { id: "ask-subchat", i18nKey: "studioDrawer.askAi", defaultLabel: "Hỏi AI", hotkey: "S", icon: "sparkles" },
  { id: "comment", i18nKey: "studioDrawer.pinComment", defaultLabel: "Ghim", hotkey: "C", icon: "message-square" },
  { id: "edit", i18nKey: "studioDrawer.quickEdit", defaultLabel: "Sửa nhanh", hotkey: "E", icon: "edit-3" },
  { id: "inspect", i18nKey: "studioDrawer.inspect", defaultLabel: "Soi", hotkey: "I", icon: "sliders" },
];

const PILL_BTN_STYLE: React.CSSProperties = {
  height: "24px",
  padding: "0 7px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid transparent",
  backgroundColor: "transparent",
  color: "var(--text-primary)",
  fontSize: "10.5px",
  fontWeight: STUDIO_FONT_SEMIBOLD,
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  cursor: "pointer",
  whiteSpace: "nowrap",
};

const HOTKEY_TO_ACTION: Readonly<Record<string, StudioSpatialCoreAction>> = {
  s: "ask-subchat",
  c: "comment",
  e: "edit",
  i: "inspect",
};

function useSpatialActionHotkeys(
  target: StudioSpatialTarget | null,
  onTriggerAction: (action: StudioSpatialCoreAction, target: StudioSpatialTarget) => void,
  onClose: () => void
): void {
  useEffect(() => {
    if (!target) return;
    const onKeyDown = (ev: KeyboardEvent) => {
      if (isEditableEvent(ev) || ev.metaKey || ev.ctrlKey || ev.altKey) return;
      const k = ev.key.toLowerCase();
      if (k === "escape") {
        ev.preventDefault();
        ev.stopImmediatePropagation();
        onClose();
        return;
      }
      const matched = HOTKEY_TO_ACTION[k];
      if (matched) {
        ev.preventDefault();
        ev.stopImmediatePropagation();
        onTriggerAction(matched, target);
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [target, onTriggerAction, onClose]);
}

const InlineCommentForm: React.FC<{
  target: StudioSpatialTarget;
  draft: string;
  onChange: (v: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}> = ({ target, draft, onChange, onSubmit, onCancel }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="studio-spatial-comment-composer" style={{ display: "inline-flex", alignItems: "center", gap: "4px", paddingLeft: "4px" }}>
      <input
        type="text"
        autoFocus
        data-testid="studio-spatial-comment-input"
        value={draft}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.nativeEvent.isComposing) { e.preventDefault(); onSubmit(); }
          if (e.key === "Escape") { e.preventDefault(); onCancel(); }
        }}
        placeholder={t("comments.inlinePlaceholder", `Ghim bình luận cho ${target.label ?? target.ffId}...`)}
        style={{ width: "180px", height: "24px", padding: "0 7px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", fontSize: "11px" }}
      />
      <button type="button" data-testid="studio-spatial-comment-submit" onClick={onSubmit} disabled={!draft.trim()} style={{ height: "24px", padding: "0 8px", borderRadius: "var(--radius-xs)", border: "none", backgroundColor: draft.trim() ? "var(--action-primary)" : "var(--bg-subtle)", color: draft.trim() ? "var(--action-primary-fg)" : "var(--text-muted)", fontSize: "10.5px", fontWeight: STUDIO_FONT_SEMIBOLD, cursor: draft.trim() ? "pointer" : "not-allowed" }}>
        {t("common.send", "Gửi")}
      </button>
    </div>
  );
};

const SpatialBarButtons: React.FC<{
  target: StudioSpatialTarget;
  domainActions: readonly StudioSpatialDomainAction[];
  onAction: (action: StudioSpatialCoreAction, tgt: StudioSpatialTarget) => void;
}> = ({ target, domainActions, onAction }) => {
  const { t } = useTranslation();
  return (
    <>
      {CORE_ACTION_ITEMS.map((item) => {
        const label = t(item.i18nKey, item.defaultLabel);
        return (
          <button key={item.id} type="button" data-testid={`studio-spatial-action-${item.id}`} title={`${label} (${item.hotkey})`} onClick={() => onAction(item.id, target)} style={PILL_BTN_STYLE}>
            <BrandIcon name={item.icon} size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="var(--action-primary)" />
            <span>{label}</span>
          </button>
        );
      })}
      {domainActions.map((act) => (
        <button key={act.id} type="button" data-testid={`studio-spatial-domain-${act.id}`} onClick={act.onSelect} style={{ ...PILL_BTN_STYLE, borderLeft: "1px solid var(--border-subtle)" }}>
          <BrandIcon name={act.icon} size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="var(--text-secondary)" />
          <span>{act.label}</span>
        </button>
      ))}
    </>
  );
};

export const StudioSpatialActionBar: React.FC<StudioSpatialActionBarProps> = ({
  target,
  domainActions = [],
  onCoreAction,
  onSubmitComment,
  onClose,
}) => {
  const { t } = useTranslation();
  const [commenting, setCommenting] = useState(false);
  const [commentDraft, setCommentDraft] = useState("");
  useEffect(() => { setCommenting(false); setCommentDraft(""); }, [target?.ffId, target?.x, target?.y]);

  const handleAction = (action: StudioSpatialCoreAction, tgt: StudioSpatialTarget) => {
    if (action === "comment" && onSubmitComment) { setCommenting(true); return; }
    onCoreAction(action, tgt);
  };
  useSpatialActionHotkeys(target, handleAction, onClose);
  if (!target) return null;

  const handleCommitComment = () => {
    const trimmed = commentDraft.trim();
    if (!trimmed || !onSubmitComment) return;
    onSubmitComment(target, trimmed);
    setCommenting(false);
    setCommentDraft("");
  };

  const barStyle: React.CSSProperties = {
    position: "absolute", left: `${Math.max(MIN_CLAMP_PX, target.x)}px`, top: `${Math.max(MIN_CLAMP_PX, target.y - VERTICAL_OFFSET_PX)}px`,
    zIndex: ACTION_BAR_Z, display: "inline-flex", alignItems: "center", gap: "2px", padding: "3px 5px",
    borderRadius: "var(--radius-sm)", border: "1px solid var(--action-primary)", backgroundColor: "var(--bg-elevated)", boxShadow: "var(--shadow-lg)",
  };
  const barLabel = t("studioDrawer.quickActionsAria", "Thao tác nhanh");
  return (
    <div role="toolbar" aria-label={`${barLabel}: ${target.label ?? target.ffId}`} data-testid="studio-spatial-action-bar" style={barStyle}>
      <span style={{ fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--action-primary)", padding: "0 5px", borderRight: "1px solid var(--border-subtle)" }}>
        {target.label ?? target.ffId}
      </span>
      {commenting
        ? <InlineCommentForm target={target} draft={commentDraft} onChange={setCommentDraft} onSubmit={handleCommitComment} onCancel={() => setCommenting(false)} />
        : <SpatialBarButtons target={target} domainActions={domainActions} onAction={handleAction} />}
      <button type="button" aria-label={t("common.close", "Đóng")} data-testid="studio-spatial-action-close" onClick={onClose} style={{ ...PILL_BTN_STYLE, padding: "0 4px", color: "var(--text-muted)" }}>
        <BrandIcon name="x" size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
      </button>
    </div>
  );
};
