// @story 04.1, 04.2
// @story 06.1
// @implements AC-01, AC-06 (Inline Display/Rename Alias Field for Project & Session Names)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

const ICON_EDIT_SIZE = 12;
const ICON_CHECK_SIZE = 12;
const SEMIBOLD_WEIGHT = 600;
const MEDIUM_WEIGHT = 500;

export interface EditableAliasFieldProps {
  label: string;
  value: string;
  placeholder?: string;
  inputId?: string;
  inputTestId: string;
  editBtnTestId: string;
  displayTestId?: string;
  helperText?: string;
  onChange: (next: string) => void;
}

const DISPLAY_ROW_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "8px",
  padding: "6px 10px",
  minHeight: "34px",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  boxSizing: "border-box",
};

const EDIT_INPUT_STYLE: React.CSSProperties = {
  flex: 1,
  height: "32px",
  padding: "0 10px",
  fontSize: "12px",
  fontFamily: "var(--font-sans)",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-surface)",
  color: "var(--text-primary)",
  border: "1px solid var(--action-primary)",
  boxSizing: "border-box",
};

const AliasEditRow: React.FC<{
  inputId?: string;
  inputTestId: string;
  value: string;
  placeholder?: string;
  doneTitle: string;
  onChange: (val: string) => void;
  onFinish: () => void;
}> = ({ inputId, inputTestId, value, placeholder, doneTitle, onChange, onFinish }) => (
  <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
    <input
      id={inputId}
      data-testid={inputTestId}
      type="text"
      autoFocus
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      onBlur={onFinish}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          onFinish();
        }
      }}
      className="cowok-input"
      style={EDIT_INPUT_STYLE}
    />
    <button
      type="button"
      onMouseDown={(e) => e.preventDefault()}
      onClick={onFinish}
      title={doneTitle}
      aria-label={doneTitle}
      style={{
        height: "32px",
        padding: "0 10px",
        borderRadius: "var(--radius-xs)",
        border: "1px solid var(--border-default)",
        backgroundColor: "var(--bg-selected)",
        color: "var(--action-primary)",
        fontSize: "11px",
        fontWeight: SEMIBOLD_WEIGHT,
        cursor: "pointer",
        display: "inline-flex",
        alignItems: "center",
        gap: "4px",
      }}
    >
      <BrandIcon name="check" size={ICON_CHECK_SIZE} color="currentColor" />
      <span>{doneTitle}</span>
    </button>
  </div>
);

const AliasDisplayRow: React.FC<{
  displayTestId?: string;
  editBtnTestId: string;
  value: string;
  placeholder?: string;
  renameTitle: string;
  onStartEdit: () => void;
}> = ({ displayTestId, editBtnTestId, value, placeholder, renameTitle, onStartEdit }) => (
  <div style={DISPLAY_ROW_STYLE}>
    <span
      data-testid={displayTestId}
      style={{
        fontSize: "12px",
        fontWeight: MEDIUM_WEIGHT,
        color: value.trim() ? "var(--text-primary)" : "var(--text-muted)",
        overflow: "hidden",
        textOverflow: "ellipsis",
        whiteSpace: "nowrap",
      }}
    >
      {value.trim() || placeholder}
    </span>
    <button
      type="button"
      data-testid={editBtnTestId}
      onClick={onStartEdit}
      title={renameTitle}
      aria-label={renameTitle}
      style={{
        border: "1px solid var(--border-default)",
        backgroundColor: "var(--bg-workspace)",
        color: "var(--text-secondary)",
        borderRadius: "var(--radius-xs)",
        padding: "3px 8px",
        fontSize: "11px",
        fontWeight: MEDIUM_WEIGHT,
        cursor: "pointer",
        display: "inline-flex",
        alignItems: "center",
        gap: "4px",
        flexShrink: 0,
      }}
    >
      <BrandIcon name="edit" size={ICON_EDIT_SIZE} color="currentColor" />
      <span>{renameTitle}</span>
    </button>
  </div>
);

export const EditableAliasField: React.FC<EditableAliasFieldProps> = ({
  label, value, placeholder, inputId, inputTestId, editBtnTestId, displayTestId, helperText, onChange,
}) => {
  const { t } = useTranslation();
  const [isEditing, setIsEditing] = useState(false);
  return (
    <div>
      <label htmlFor={inputId} style={{ display: "block", fontSize: "11px", fontWeight: SEMIBOLD_WEIGHT, color: "var(--text-secondary)", marginBottom: "4px" }}>
        {label}
      </label>
      {isEditing ? (
        <AliasEditRow inputId={inputId} inputTestId={inputTestId} value={value} placeholder={placeholder} doneTitle={t("workbench.confirmRenameBtn", "Xong")} onChange={onChange} onFinish={() => setIsEditing(false)} />
      ) : (
        <AliasDisplayRow displayTestId={displayTestId} editBtnTestId={editBtnTestId} value={value} placeholder={placeholder} renameTitle={t("workbench.renameAliasBtn", "Đổi tên")} onStartEdit={() => setIsEditing(true)} />
      )}
      {helperText && <div style={{ fontSize: "10px", color: "var(--text-muted)", marginTop: "3px" }}>{helperText}</div>}
    </div>
  );
};
