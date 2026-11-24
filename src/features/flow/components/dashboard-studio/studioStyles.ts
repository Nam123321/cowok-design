// @story 06.5
// @implements AC-02, AC-03, AC-04 (Semantic CSS Variable Styles for Option D Dashboard Studio)
import type React from "react";
import { DRAWER_Z_INDEX } from "./types";

export const ICON_SM = 13;
export const ICON_MD = 14;
export const ICON_LG = 16;
export const FONT_WEIGHT_MEDIUM = 500;
export const FONT_WEIGHT_SEMIBOLD = 600;
export const FONT_WEIGHT_BOLD = 700;

export const DRAWER_BACKDROP_STYLE: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: DRAWER_Z_INDEX,
  backgroundColor: "var(--bg-overlay)",
  backdropFilter: "blur(3px)",
  display: "flex",
  justifyContent: "flex-end",
};

export const DRAWER_PANEL_BASE: React.CSSProperties = {
  position: "relative",
  height: "100vh",
  maxWidth: "85vw",
  backgroundColor: "var(--bg-surface)",
  color: "var(--text-primary)",
  borderLeft: "1px solid var(--border-default)",
  boxShadow: "var(--shadow-lg)",
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
};

export const STUDIO_INPUT_STYLE: React.CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-sm)",
  padding: "6px 8px",
  fontSize: "12px",
};

export const STUDIO_COMPACT_INPUT: React.CSSProperties = {
  width: "100%",
  boxSizing: "border-box",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-subtle)",
  borderRadius: "var(--radius-xs)",
  padding: "4px 6px",
  fontSize: "11.5px",
};

export const STUDIO_PRIMARY_BTN: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  padding: "6px 12px",
  borderRadius: "var(--radius-sm)",
  border: "none",
  backgroundColor: "var(--action-primary)",
  color: "var(--action-primary-fg)",
  fontSize: "12px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  cursor: "pointer",
  whiteSpace: "nowrap",
};

export const STUDIO_SECONDARY_BTN: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  padding: "5px 10px",
  borderRadius: "var(--radius-sm)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "11.5px",
  fontWeight: FONT_WEIGHT_MEDIUM,
  cursor: "pointer",
  whiteSpace: "nowrap",
};

export const STUDIO_ICON_BTN: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  gap: "4px",
  padding: "5px 7px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-secondary)",
  fontSize: "11.5px",
  cursor: "pointer",
};

export const SECTION_TITLE_STYLE: React.CSSProperties = {
  fontSize: "11px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  color: "var(--text-muted)",
  textTransform: "uppercase",
  letterSpacing: "0.04em",
};

export const SEGMENT_GROUP_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "2px",
  padding: "2px",
  borderRadius: "var(--radius-sm)",
  backgroundColor: "var(--bg-subtle)",
  border: "1px solid var(--border-subtle)",
};

export function segmentPillStyle(active: boolean): React.CSSProperties {
  return {
    padding: "4px 8px",
    borderRadius: "var(--radius-xs)",
    border: active ? "1px solid var(--action-primary)" : "1px solid transparent",
    backgroundColor: active ? "var(--bg-selected)" : "transparent",
    color: active ? "var(--action-primary)" : "var(--text-secondary)",
    fontSize: "11px",
    fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
    cursor: "pointer",
    whiteSpace: "nowrap",
  };
}

export function chipStyle(active: boolean): React.CSSProperties {
  return {
    display: "inline-flex",
    alignItems: "center",
    gap: "4px",
    padding: "4px 8px",
    borderRadius: "999px",
    border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
    backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
    color: active ? "var(--action-primary)" : "var(--text-secondary)",
    fontSize: "11px",
    fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
    cursor: "pointer",
  };
}
