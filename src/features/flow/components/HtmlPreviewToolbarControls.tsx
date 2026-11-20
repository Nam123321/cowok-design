// @story 04.1, 04.5, 05.1, 05.3
// @implements AC-01, AC-03 (Unified Pointer Toggle, Export/Overflow Portal Menus & Sub-State Switcher)
import React, { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  FIXTURE_VARIANTS,
  normalizeFixtureRef,
  type FixtureVariant,
} from "../../../lib/validations/screen-fixture";
import type { ExportInitialSection } from "../../snapshots/components/ExportBundleModal";

export const NARROW_TOOLBAR_BREAKPOINT_PX = 460;
export const NARROW_UTILITY_FLOW_BREAKPOINT_PX = 480;
export const NARROW_UTILITY_SLIDE_BREAKPOINT_PX = 540;
const DROPDOWN_OFFSET_PX = 4;
const COMPACT_CHEVRON_PX = 10;
const PORTAL_Z_INDEX = 9999;
const DISABLED_CONTROL_OPACITY = 0.45;
const COMPACT_ICON_PX = 12;
const COMPACT_CHECK_PX = 11;
const WEIGHT_MEDIUM = 500;
const WEIGHT_SEMIBOLD = 600;

const GROUP_WRAP_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-xs)",
  padding: "2px",
  gap: "2px",
  flexWrap: "nowrap",
};

export const TOOLBAR_BTN_BASE: React.CSSProperties = {
  height: "24px",
  padding: "0 6px",
  border: "none",
  borderRadius: "4px",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  fontSize: "10.5px",
  whiteSpace: "nowrap",
  flexShrink: 0,
  transition: "all var(--duration-fast) var(--ease-standard)",
};

export const TOOLBAR_ICON_BTN_BASE: React.CSSProperties = {
  width: "24px",
  height: "24px",
  padding: 0,
  border: "none",
  borderRadius: "4px",
  cursor: "pointer",
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  position: "relative",
  flexShrink: 0,
  transition: "all var(--duration-fast) var(--ease-standard)",
};

const MENU_PANEL_STYLE: React.CSSProperties = {
  position: "fixed",
  minWidth: "200px",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-sm)",
  boxShadow: "var(--shadow-lg)",
  padding: "4px",
  display: "flex",
  flexDirection: "column",
  gap: "2px",
  zIndex: PORTAL_Z_INDEX,
};

const MENU_ITEM_STYLE: React.CSSProperties = {
  width: "100%",
  padding: "6px 9px",
  border: "none",
  borderRadius: "4px",
  backgroundColor: "transparent",
  color: "var(--text-primary)",
  fontSize: "11.5px",
  fontWeight: 500,
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "8px",
  cursor: "pointer",
  textAlign: "left",
};

interface DropdownCoords {
  top: number;
  left: number;
}

function usePortalMenuDismiss(
  open: boolean,
  anchorRef: React.RefObject<HTMLElement | null>,
  menuRef: React.RefObject<HTMLDivElement | null>,
  onClose: () => void
): void {
  useEffect(() => {
    if (!open) return;
    const handleOutside = (ev: MouseEvent) => {
      const target = ev.target;
      if (!(target instanceof Node)) return;
      if (!anchorRef.current?.contains(target) && !menuRef.current?.contains(target)) onClose();
    };
    const handleKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") {
        ev.stopPropagation();
        onClose();
      }
    };
    document.addEventListener("mousedown", handleOutside);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleOutside);
      document.removeEventListener("keydown", handleKey);
    };
  }, [anchorRef, menuRef, open, onClose]);
}

const FALLBACK_TOP_PX = 48;
const FALLBACK_LEFT_PX = 200;
const FALLBACK_COORDS: DropdownCoords = { top: FALLBACK_TOP_PX, left: FALLBACK_LEFT_PX };
const VIEWPORT_EDGE_PAD_PX = 8;
const MENU_MAX_RIGHT_OFFSET_PX = 220;
const ICON_STROKE_UPLOAD = 1.85;
const MORE_ICON_PX = 13;

function measureAnchorCoords(el: HTMLElement | null): DropdownCoords {
  if (!el) return FALLBACK_COORDS;
  const rect = el.getBoundingClientRect();
  return {
    top: rect.bottom + DROPDOWN_OFFSET_PX,
    left: Math.max(VIEWPORT_EDGE_PAD_PX, Math.min(rect.left, window.innerWidth - MENU_MAX_RIGHT_OFFSET_PX)),
  };
}

const ExportMenuItemsList: React.FC<{
  testIdPrefix: string;
  onSelect: (sec: ExportInitialSection) => void;
}> = ({ testIdPrefix, onSelect }) => {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" role="menuitem" data-testid={`${testIdPrefix}-zip`} title={t("snapshots.downloadZipTooltip", "Đóng gói toàn bộ dự án sạch không chứa secret thành tệp ZIP")} style={MENU_ITEM_STYLE} onClick={() => onSelect("zip")}>
        <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="zip" size={COMPACT_ICON_PX} color="currentColor" />
          <span>{t("snapshots.downloadZipBtn", "Tải ZIP Full Repo")}</span>
        </span>
      </button>
      <button type="button" role="menuitem" data-testid={`${testIdPrefix}-github`} title={t("snapshots.pushGithubPrTooltip", "Tạo nhánh và đẩy Pull Request lên GitHub repository")} style={MENU_ITEM_STYLE} onClick={() => onSelect("github-pr")}>
        <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="handoff" size={COMPACT_ICON_PX} color="currentColor" />
          <span>{t("snapshots.pushGithubPrMenu", "Đẩy GitHub PR")}</span>
        </span>
      </button>
      <button type="button" role="menuitem" data-testid={`${testIdPrefix}-ide`} title={t("snapshots.openIdeTooltip", "Tạo manifest bàn giao 1-Click và mở trong Cursor / VS Code / Antigravity")} style={MENU_ITEM_STYLE} onClick={() => onSelect("ide-handoff")}>
        <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="terminal" size={COMPACT_ICON_PX} color="currentColor" />
          <span>{t("snapshots.ideHandoffTitle", "Mở trong IDE")}</span>
        </span>
      </button>
    </>
  );
};

export const ExportActionDropdown: React.FC<{
  onOpenExportSection?: (section: ExportInitialSection) => void;
}> = ({ onOpenExportSection }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<DropdownCoords>({ top: 0, left: 0 });
  const anchorRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  usePortalMenuDismiss(open, anchorRef, menuRef, () => setOpen(false));
  if (!onOpenExportSection) return null;
  const pick = (sec: ExportInitialSection) => { setOpen(false); onOpenExportSection(sec); };
  return (
    <>
      <button
        ref={anchorRef} type="button" data-testid="toolbar-export-btn" aria-haspopup="menu" aria-expanded={open}
        onClick={() => { setCoords(measureAnchorCoords(anchorRef.current)); setOpen((v) => !v); }}
        title={t("snapshots.exportMenuTooltip", "Xuất bản: Tải ZIP, Tạo GitHub PR hoặc Mở IDE")}
        style={{ ...TOOLBAR_BTN_BASE, backgroundColor: open ? "var(--bg-selected)" : "var(--bg-workspace)", border: "1px solid var(--border-default)", color: "var(--text-primary)", fontWeight: WEIGHT_MEDIUM }}
      >
        <BrandIcon name="upload" size={COMPACT_ICON_PX} strokeWidth={ICON_STROKE_UPLOAD} color="currentColor" />
        <span>{t("snapshots.exportShortLabel", "Export")}</span>
        <BrandIcon name="chevron-down" size={COMPACT_CHEVRON_PX} color="currentColor" />
      </button>
      {open && typeof document !== "undefined" && createPortal(
        <div ref={menuRef} role="menu" data-testid="toolbar-export-menu" style={{ ...MENU_PANEL_STYLE, top: `${coords.top}px`, left: `${coords.left}px` }}>
          <ExportMenuItemsList testIdPrefix="toolbar-export" onSelect={pick} />
        </div>,
        document.body
      )}
    </>
  );
};

const OverflowEditItems: React.FC<{
  isInlineCodeOpen?: boolean;
  dirtyBuffer?: boolean;
  isSampleDataOpen?: boolean;
  isReadOnly?: boolean;
  onToggleInlineCode?: () => void;
  onToggleSampleData?: () => void;
  onClose: () => void;
}> = (p) => {
  const { t } = useTranslation();
  return (
    <>
      {p.onToggleInlineCode && (
        <button type="button" role="menuitem" disabled={p.isReadOnly} data-testid="overflow-edit-html-btn" onClick={() => { p.onClose(); p.onToggleInlineCode?.(); }} style={{ ...MENU_ITEM_STYLE, opacity: p.isReadOnly ? DISABLED_CONTROL_OPACITY : 1, backgroundColor: p.isInlineCodeOpen ? "var(--bg-selected)" : "transparent" }}>
          <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
            <BrandIcon name="code" size={COMPACT_ICON_PX} color="currentColor" />
            <span>{t("canvas.editHtmlLabel", "Sửa HTML")}</span>
          </span>
          {p.dirtyBuffer && <span style={{ width: "6px", height: "6px", borderRadius: "50%", backgroundColor: "var(--agent-accent)" }} />}
        </button>
      )}
      {p.onToggleSampleData && (
        <button type="button" role="menuitem" disabled={p.isReadOnly} data-testid="overflow-sample-data-btn" onClick={() => { p.onClose(); p.onToggleSampleData?.(); }} style={{ ...MENU_ITEM_STYLE, opacity: p.isReadOnly ? DISABLED_CONTROL_OPACITY : 1, backgroundColor: p.isSampleDataOpen ? "var(--bg-selected)" : "transparent" }}>
          <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
            <BrandIcon name="database" size={COMPACT_ICON_PX} color="currentColor" />
            <span>{t("canvas.sampleDataLabel", "Dữ liệu mẫu")}</span>
          </span>
        </button>
      )}
    </>
  );
};

export const OverflowActionsDropdown: React.FC<{
  isInlineCodeOpen?: boolean;
  dirtyBuffer?: boolean;
  isSampleDataOpen?: boolean;
  isReadOnly?: boolean;
  onToggleInlineCode?: () => void;
  onToggleSampleData?: () => void;
  onOpenExportSection?: (section: ExportInitialSection) => void;
}> = (p) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<DropdownCoords>({ top: 0, left: 0 });
  const anchorRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  usePortalMenuDismiss(open, anchorRef, menuRef, () => setOpen(false));
  const openMenu = () => { setCoords(measureAnchorCoords(anchorRef.current)); setOpen(true); };
  const label = t("canvas.moreActionsTooltip", "Thêm tác vụ (Sửa HTML, Dữ liệu mẫu, Export)");
  return (
    <>
      <button
        ref={anchorRef} type="button" data-testid="toolbar-more-actions-btn" aria-haspopup="menu" aria-expanded={open}
        onClick={() => (open ? setOpen(false) : openMenu())} title={label} aria-label={label}
        style={{ ...TOOLBAR_ICON_BTN_BASE, width: "26px", backgroundColor: open ? "var(--bg-selected)" : "var(--bg-workspace)", border: "1px solid var(--border-default)", color: "var(--text-primary)" }}
      >
        <BrandIcon name="more-horizontal" size={MORE_ICON_PX} color="currentColor" />
      </button>
      {open && typeof document !== "undefined" && createPortal(
        <div ref={menuRef} role="menu" data-testid="toolbar-more-actions-menu" style={{ ...MENU_PANEL_STYLE, top: `${coords.top}px`, left: `${coords.left}px` }}>
          <OverflowEditItems {...p} onClose={() => setOpen(false)} />
          {p.onOpenExportSection && (
            <>
              <div style={{ height: "1px", backgroundColor: "var(--border-subtle)", margin: "2px 4px" }} />
              <ExportMenuItemsList testIdPrefix="overflow-export" onSelect={(sec) => { setOpen(false); p.onOpenExportSection?.(sec); }} />
            </>
          )}
        </div>,
        document.body
      )}
    </>
  );
};

export interface CompactOptionItem<T extends string> {
  value: T;
  label: string;
  icon?: React.ComponentProps<typeof BrandIcon>["name"];
  testId?: string;
}

function CompactDropdownPortalMenu<T extends string>(p: {
  menuRef: React.RefObject<HTMLDivElement | null>;
  testId: string;
  coords: DropdownCoords;
  isMono?: boolean;
  value: T;
  options: ReadonlyArray<CompactOptionItem<T>>;
  onSelect: (value: T) => void;
}) {
  if (typeof document === "undefined") return null;
  const font = p.isMono ? "var(--font-mono)" : "var(--font-sans)";
  return createPortal(
    <div ref={p.menuRef} role="menu" data-testid={`${p.testId}-menu`} style={{ ...MENU_PANEL_STYLE, minWidth: "150px", top: `${p.coords.top}px`, left: `${p.coords.left}px` }}>
      {p.options.map((opt) => {
        const selected = opt.value === p.value;
        return (
          <button
            key={opt.value}
            type="button"
            role="menuitemradio"
            aria-checked={selected}
            data-testid={opt.testId}
            onClick={() => p.onSelect(opt.value)}
            style={{
              ...MENU_ITEM_STYLE,
              fontFamily: font,
              backgroundColor: selected ? "var(--bg-selected)" : "transparent",
              color: selected ? "var(--action-primary)" : "var(--text-primary)",
              fontWeight: selected ? WEIGHT_SEMIBOLD : WEIGHT_MEDIUM,
            }}
          >
            <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
              {opt.icon && <BrandIcon name={opt.icon} size={COMPACT_ICON_PX} color="currentColor" />}
              <span>{opt.label}</span>
            </span>
            {selected && <BrandIcon name="check" size={COMPACT_CHECK_PX} color="currentColor" />}
          </button>
        );
      })}
    </div>,
    document.body
  );
}

export function CompactOptionDropdown<T extends string>(p: {
  testId: string;
  ariaLabel: string;
  title?: string;
  activeLabel: string;
  activeIcon?: React.ComponentProps<typeof BrandIcon>["name"];
  disabled?: boolean;
  isMono?: boolean;
  value: T;
  options: ReadonlyArray<CompactOptionItem<T>>;
  onChange: (value: T) => void;
}) {
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<DropdownCoords>({ top: 0, left: 0 });
  const anchorRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  usePortalMenuDismiss(open, anchorRef, menuRef, () => setOpen(false));
  return (
    <>
      <button
        ref={anchorRef}
        type="button"
        disabled={p.disabled}
        data-testid={p.testId}
        className="toolbar-compact-select-btn"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={p.ariaLabel}
        title={p.title ?? p.ariaLabel}
        onClick={() => {
          setCoords(measureAnchorCoords(anchorRef.current));
          setOpen((v) => !v);
        }}
        style={{
          fontFamily: p.isMono ? "var(--font-mono)" : "var(--font-sans)",
          opacity: p.disabled ? DISABLED_CONTROL_OPACITY : 1,
          cursor: p.disabled ? "not-allowed" : "pointer",
        }}
      >
        {p.activeIcon && <BrandIcon name={p.activeIcon} size={COMPACT_ICON_PX} color="currentColor" />}
        <span>{p.activeLabel}</span>
        <BrandIcon name="chevron-down" size={COMPACT_CHEVRON_PX} color="currentColor" />
      </button>
      {open && <CompactDropdownPortalMenu menuRef={menuRef} testId={p.testId} coords={coords} isMono={p.isMono} value={p.value} options={p.options} onSelect={(val) => { setOpen(false); p.onChange(val); }} />}
    </>
  );
}

const SUBSTATE_BOLD_WEIGHT = 700;

function subStateBtnStyle(isActive: boolean, isReadOnly: boolean): React.CSSProperties {
  return {
    height: "20px",
    padding: "0 6px",
    border: "none",
    borderRadius: "var(--radius-xs)",
    fontSize: "9.5px",
    fontFamily: "var(--font-mono)",
    fontWeight: isActive ? SUBSTATE_BOLD_WEIGHT : WEIGHT_MEDIUM,
    backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
    color: isActive ? "var(--action-primary)" : "var(--text-muted)",
    opacity: isReadOnly ? DISABLED_CONTROL_OPACITY : 1,
    cursor: isReadOnly ? "not-allowed" : "pointer",
    whiteSpace: "nowrap",
  };
}

export const SubStateSwitcher: React.FC<{
  activeSubState?: FixtureVariant;
  isReadOnly?: boolean;
  isCompact?: boolean;
  onSelectSubState?: (variant: FixtureVariant) => void;
}> = ({ activeSubState, isReadOnly = false, isCompact = false, onSelectSubState }) => {
  const { t } = useTranslation();
  const current = normalizeFixtureRef(activeSubState);
  if (isCompact) {
    return (
      <div data-testid="fixture-substate-switcher" style={{ display: "inline-flex" }}>
        <CompactOptionDropdown
          testId="fixture-substate-dropdown"
          ariaLabel={t("canvas.subStateAria", "Trạng thái dữ liệu màn hình")}
          activeLabel={current}
          disabled={isReadOnly}
          isMono
          value={current}
          options={FIXTURE_VARIANTS.map(({ variant }) => ({ value: variant, label: variant, testId: `fixture-variant-${variant}` }))}
          onChange={(variant) => onSelectSubState?.(variant)}
        />
      </div>
    );
  }
  return (
    <div data-testid="fixture-substate-switcher" style={GROUP_WRAP_STYLE}>
      {FIXTURE_VARIANTS.map(({ variant }) => (
        <button
          key={variant}
          type="button"
          disabled={isReadOnly}
          data-testid={`fixture-variant-${variant}`}
          aria-pressed={current === variant}
          onClick={() => onSelectSubState?.(variant)}
          style={subStateBtnStyle(current === variant, isReadOnly)}
        >
          {variant}
        </button>
      ))}
    </div>
  );
};
