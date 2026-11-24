// @story 05.6, 06.5
// @implements [5.26] Unified 3-Tier Detail Canvas Toolbar matching Main Canvas Grammar (Merged Pointer Toggle + Pin Badge + 5 Responsive Viewports)
import React from "react";
import { BrandIcon } from "../BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  STUDIO_FONT_BOLD,
  STUDIO_FONT_MEDIUM,
  STUDIO_FONT_SEMIBOLD,
  STUDIO_ICON_SM,
  STUDIO_STROKE_MONO,
  type StudioPointerMode,
  type StudioSubState,
  type StudioViewportPreset,
} from "./types";

const VP_DESKTOP_PX = 1440;
const VP_TABLET_PX = 768;
const VP_FOLD_INNER_PX = 712;
const VP_MOBILE_PX = 390;
const VP_FOLD_OUTER_PX = 344;
const SUB_STATES: readonly StudioSubState[] = ["DEFAULT", "EMPTY", "LOADING", "ERROR"];

const VIEWPORT_OPTIONS: ReadonlyArray<{
  id: StudioViewportPreset;
  icon: "monitor" | "tablet" | "fold-inner" | "smartphone" | "fold-outer";
  i18nKey: string;
  defaultLabel: string;
  pxWidth: number;
}> = [
  { id: "desktop-1440", icon: "monitor", i18nKey: "canvas.viewportDesktop", defaultLabel: "Desktop", pxWidth: VP_DESKTOP_PX },
  { id: "tablet-768", icon: "tablet", i18nKey: "canvas.viewportTablet", defaultLabel: "Tablet", pxWidth: VP_TABLET_PX },
  { id: "fold-inner-712", icon: "fold-inner", i18nKey: "canvas.viewportFoldInner", defaultLabel: "Màn hình mở gập", pxWidth: VP_FOLD_INNER_PX },
  { id: "mobile-390", icon: "smartphone", i18nKey: "canvas.viewportMobile", defaultLabel: "Mobile", pxWidth: VP_MOBILE_PX },
  { id: "fold-outer-344", icon: "fold-outer", i18nKey: "canvas.viewportFoldOuter", defaultLabel: "Màn hình ngoài gập", pxWidth: VP_FOLD_OUTER_PX },
];

const ROW_BASE_STYLE: React.CSSProperties = {
  minHeight: "30px", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px", flexWrap: "wrap",
};

const PILL_GROUP_STYLE: React.CSSProperties = {
  display: "inline-flex", alignItems: "center", backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-default)", borderRadius: "var(--radius-xs)", padding: "2px", gap: "2px", flexWrap: "nowrap",
};

function subStateStyle(active: boolean): React.CSSProperties {
  return {
    height: "20px", padding: "0 6px", border: "none", borderRadius: "var(--radius-xs)",
    fontSize: "9.5px", fontFamily: "var(--font-mono)", fontWeight: active ? STUDIO_FONT_BOLD : STUDIO_FONT_MEDIUM,
    backgroundColor: active ? "var(--bg-selected)" : "transparent", color: active ? "var(--action-primary)" : "var(--text-muted)",
    cursor: "pointer", whiteSpace: "nowrap",
  };
}

export interface StudioThreeTierToolbarProps {
  pointerMode: StudioPointerMode;
  pinCount?: number;
  isCommentsHighlighted?: boolean;
  onPointerModeChange: (mode: StudioPointerMode) => void;
  onToggleComments?: () => void;
  row1LeftExtra?: React.ReactNode;
  row1Right?: React.ReactNode;
  subState?: StudioSubState;
  onSubStateChange?: (state: StudioSubState) => void;
  row2LeftExtra?: React.ReactNode;
  viewportPreset?: StudioViewportPreset;
  onViewportPresetChange?: (vp: StudioViewportPreset) => void;
  row2RightExtra?: React.ReactNode;
  row3Strip?: React.ReactNode;
}

const CommentsBadgeButton: React.FC<{
  pinCount: number;
  highlighted?: boolean;
  onClick: () => void;
}> = ({ pinCount, highlighted, onClick }) => {
  const { t } = useTranslation();
  const title = `${t("comments.drawerTitle", "Bình luận ghim trên phần tử")} (${pinCount})`;
  return (
    <button
      type="button" data-testid="studio-comments-toggle-btn" aria-pressed={Boolean(highlighted)} aria-label={title} title={title} onClick={onClick}
      style={{
        position: "relative", width: "26px", height: "24px", padding: 0, borderRadius: "var(--radius-xs)",
        border: highlighted ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
        backgroundColor: highlighted ? "var(--bg-selected)" : "var(--bg-workspace)",
        color: highlighted ? "var(--action-primary)" : "var(--text-secondary)",
        display: "inline-flex", alignItems: "center", justifyContent: "center", cursor: "pointer",
      }}
    >
      <BrandIcon name="message-square" size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
      {pinCount > 0 && (
        <span data-testid="studio-mode-pin-count" style={{ position: "absolute", top: "-4px", right: "-4px", minWidth: "13px", height: "13px", padding: "0 3px", borderRadius: "999px", fontSize: "8.5px", fontWeight: STUDIO_FONT_BOLD, backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", display: "inline-flex", alignItems: "center", justifyContent: "center" }}>
          {pinCount}
        </span>
      )}
    </button>
  );
};

const MergedPointerControls: React.FC<{
  mode: StudioPointerMode;
  pinCount: number;
  isCommentsHighlighted?: boolean;
  onChange: (m: StudioPointerMode) => void;
  onToggleComments?: () => void;
  extra?: React.ReactNode;
}> = ({ mode, pinCount, isCommentsHighlighted, onChange, onToggleComments, extra }) => {
  const { t } = useTranslation();
  const isSelecting = mode === "select-region";
  const label = isSelecting ? t("canvas.modeSelectRegion", "Chọn vùng") : t("canvas.modeInteractShort", "Tương tác");
  const tooltip = t("canvas.modeToggleTooltip", "Tương tác ↔ Chọn vùng: nhấn để click phần tử hoặc kéo vùng (>=16x16px) vào bình luận/chat (Phím tắt C)");
  return (
    <div style={{ display: "inline-flex", alignItems: "center", gap: "5px", flexWrap: "nowrap" }}>
      <button
        type="button" data-testid="studio-pointer-mode-toggle" aria-pressed={isSelecting} aria-label={label} title={tooltip}
        onClick={() => onChange(isSelecting ? "interact" : "select-region")}
        style={{
          height: "24px", padding: "0 8px", borderRadius: "var(--radius-xs)",
          border: isSelecting ? "1px solid var(--action-primary)" : "1px solid var(--border-default)",
          backgroundColor: isSelecting ? "var(--action-primary)" : "var(--bg-workspace)",
          color: isSelecting ? "var(--action-primary-fg)" : "var(--text-primary)",
          fontSize: "11px", fontWeight: STUDIO_FONT_SEMIBOLD, display: "inline-flex", alignItems: "center", gap: "5px", cursor: "pointer", whiteSpace: "nowrap",
        }}
      >
        <BrandIcon name={isSelecting ? "crosshair" : "cursor-click"} size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
        <span>{label}</span>
      </button>
      <CommentsBadgeButton pinCount={pinCount} highlighted={isCommentsHighlighted} onClick={() => (onToggleComments ? onToggleComments() : onChange("select-region"))} />
      {extra}
    </div>
  );
};

const ViewportSelectorGroup: React.FC<{ viewportPreset: StudioViewportPreset; onChange: (vp: StudioViewportPreset) => void }> = ({ viewportPreset, onChange }) => {
  const { t } = useTranslation();
  return (
    <div role="radiogroup" aria-label={t("canvas.viewportSwitcherAria", "Kích thước màn hình")} data-testid="studio-viewport-selector" style={PILL_GROUP_STYLE}>
      {VIEWPORT_OPTIONS.map((item) => {
        const active = viewportPreset === item.id;
        const label = `${t(item.i18nKey, item.defaultLabel)} (${item.pxWidth}px)`;
        return (
          <button
            key={item.id} type="button" role="radio" aria-checked={active} data-testid={`studio-viewport-${item.id}`} title={label} aria-label={label} onClick={() => onChange(item.id)}
            style={{ width: "24px", height: "20px", padding: 0, border: "none", borderRadius: "var(--radius-xs)", backgroundColor: active ? "var(--bg-selected)" : "transparent", color: active ? "var(--action-primary)" : "var(--text-muted)", display: "inline-flex", alignItems: "center", justifyContent: "center", cursor: "pointer" }}
          >
            <BrandIcon name={item.icon} size={STUDIO_ICON_SM} strokeWidth={STUDIO_STROKE_MONO} color="currentColor" />
          </button>
        );
      })}
    </div>
  );
};

const ToolbarTier2Row: React.FC<Pick<StudioThreeTierToolbarProps, "subState" | "onSubStateChange" | "row2LeftExtra" | "viewportPreset" | "onViewportPresetChange" | "row2RightExtra">> = ({
  subState, onSubStateChange, row2LeftExtra, viewportPreset, onViewportPresetChange, row2RightExtra,
}) => {
  const { t } = useTranslation();
  return (
    <div data-testid="studio-toolbar-tier2" style={{ ...ROW_BASE_STYLE, borderTop: "1px solid var(--border-subtle)", paddingTop: "3px" }}>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
        {subState && onSubStateChange && (
          <div style={PILL_GROUP_STYLE} role="group" aria-label={t("canvas.subStateAria", "Trạng thái dữ liệu màn hình")}>
            {SUB_STATES.map((st) => (
              <button key={st} type="button" data-testid={`studio-substate-${st}`} aria-pressed={subState === st} onClick={() => onSubStateChange(st)} style={subStateStyle(subState === st)}>
                {st}
              </button>
            ))}
          </div>
        )}
        {row2LeftExtra}
      </div>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
        {viewportPreset && onViewportPresetChange && <ViewportSelectorGroup viewportPreset={viewportPreset} onChange={onViewportPresetChange} />}
        {row2RightExtra}
      </div>
    </div>
  );
};

export const StudioThreeTierToolbar: React.FC<StudioThreeTierToolbarProps> = (props) => {
  const hasRow2 = Boolean(props.onSubStateChange || props.row2LeftExtra || props.onViewportPresetChange || props.row2RightExtra);
  return (
    <div data-testid="studio-canvas-toolbar" style={{ padding: "4px 10px", backgroundColor: "var(--bg-surface)", borderBottom: "1px solid var(--border-default)", display: "flex", flexDirection: "column", gap: "3px", flexShrink: 0 }}>
      <div data-testid="studio-toolbar-tier1" style={ROW_BASE_STYLE}>
        <MergedPointerControls mode={props.pointerMode} pinCount={props.pinCount ?? 0} isCommentsHighlighted={props.isCommentsHighlighted} onChange={props.onPointerModeChange} onToggleComments={props.onToggleComments} extra={props.row1LeftExtra} />
        {props.row1Right && <div style={{ display: "inline-flex", alignItems: "center", gap: "4px", flexWrap: "wrap" }}>{props.row1Right}</div>}
      </div>
      {hasRow2 && <ToolbarTier2Row subState={props.subState} onSubStateChange={props.onSubStateChange} row2LeftExtra={props.row2LeftExtra} viewportPreset={props.viewportPreset} onViewportPresetChange={props.onViewportPresetChange} row2RightExtra={props.row2RightExtra} />}
      {props.row3Strip && <div data-testid="studio-toolbar-tier3" style={{ ...ROW_BASE_STYLE, borderTop: "1px solid var(--border-subtle)", paddingTop: "3px" }}>{props.row3Strip}</div>}
    </div>
  );
};
