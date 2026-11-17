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

export const NARROW_TOOLBAR_BREAKPOINT_PX = 505;
const DROPDOWN_OFFSET_PX = 4;
const PORTAL_Z_INDEX = 9999;

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
      if (ev.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", handleOutside);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleOutside);
      document.removeEventListener("keydown", handleKey);
    };
  }, [anchorRef, menuRef, open, onClose]);
}

function measureAnchorCoords(el: HTMLElement | null): DropdownCoords {
  if (!el) return { top: 48, left: 200 };
  const rect = el.getBoundingClientRect();
  return { top: rect.bottom + DROPDOWN_OFFSET_PX, left: Math.max(8, Math.min(rect.left, window.innerWidth - 220)) };
}

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
  const pick = (sec: ExportInitialSection) => {
    setOpen(false);
    onOpenExportSection(sec);
  };

  return (
    <>
      <button
        ref={anchorRef}
        type="button"
        data-testid="toolbar-export-btn"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => {
          setCoords(measureAnchorCoords(anchorRef.current));
          setOpen((v) => !v);
        }}
        title={t("snapshots.exportMenuTooltip", "Xuất bản: Tải ZIP, Tạo GitHub PR hoặc Mở IDE")}
        style={{
          ...TOOLBAR_BTN_BASE,
          backgroundColor: open ? "var(--bg-selected)" : "var(--bg-workspace)",
          border: "1px solid var(--border-default)",
          color: "var(--text-primary)",
          fontWeight: 500,
        }}
      >
        <BrandIcon name="deploy" size={12} strokeWidth={1.85} color="currentColor" />
        <span>{t("snapshots.exportShortLabel", "Export")}</span>
        <BrandIcon name="chevron-down" size={10} color="currentColor" />
      </button>
      {open &&
        typeof document !== "undefined" &&
        createPortal(
          <div ref={menuRef} role="menu" data-testid="toolbar-export-menu" style={{ ...MENU_PANEL_STYLE, top: `${coords.top}px`, left: `${coords.left}px` }}>
            <button type="button" role="menuitem" data-testid="toolbar-export-zip" style={MENU_ITEM_STYLE} onClick={() => pick("zip")}>
              <span>{t("snapshots.downloadZipBtn", "Tải ZIP Full Repo")}</span>
            </button>
            <button type="button" role="menuitem" data-testid="toolbar-export-github" style={MENU_ITEM_STYLE} onClick={() => pick("github-pr")}>
              <span>{t("snapshots.pushGithubPrMenu", "Đẩy lên GitHub PR")}</span>
            </button>
            <button type="button" role="menuitem" data-testid="toolbar-export-ide" style={MENU_ITEM_STYLE} onClick={() => pick("ide-handoff")}>
              <span>{t("snapshots.ideHandoffTitle", "Mở trong IDE")}</span>
            </button>
          </div>,
          document.body
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
}> = ({
  isInlineCodeOpen,
  dirtyBuffer,
  isSampleDataOpen,
  isReadOnly,
  onToggleInlineCode,
  onToggleSampleData,
  onOpenExportSection,
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<DropdownCoords>({ top: 0, left: 0 });
  const anchorRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  usePortalMenuDismiss(open, anchorRef, menuRef, () => setOpen(false));

  const openMenu = () => {
    setCoords(measureAnchorCoords(anchorRef.current));
    setOpen(true);
  };

  return (
    <>
      <button
        ref={anchorRef}
        type="button"
        data-testid="toolbar-more-actions-btn"
        aria-haspopup="menu"
        aria-expanded={open}
        onMouseEnter={openMenu}
        onClick={() => (open ? setOpen(false) : openMenu())}
        title={t("canvas.moreActionsTooltip", "Thêm tác vụ (Sửa HTML, Dữ liệu mẫu, Export)")}
        aria-label={t("canvas.moreActionsTooltip", "Thêm tác vụ (Sửa HTML, Dữ liệu mẫu, Export)")}
        style={{
          ...TOOLBAR_ICON_BTN_BASE,
          width: "26px",
          backgroundColor: open ? "var(--bg-selected)" : "var(--bg-workspace)",
          border: "1px solid var(--border-default)",
          color: "var(--text-primary)",
        }}
      >
        <BrandIcon name="more-horizontal" size={13} color="currentColor" />
      </button>
      {open &&
        typeof document !== "undefined" &&
        createPortal(
          <div
            ref={menuRef}
            role="menu"
            data-testid="toolbar-more-actions-menu"
            onMouseLeave={() => setOpen(false)}
            style={{ ...MENU_PANEL_STYLE, top: `${coords.top}px`, left: `${coords.left}px` }}
          >
            {onToggleInlineCode && (
              <button
                type="button"
                role="menuitem"
                disabled={isReadOnly}
                data-testid="overflow-edit-html-btn"
                onClick={() => { setOpen(false); onToggleInlineCode(); }}
                style={{ ...MENU_ITEM_STYLE, opacity: isReadOnly ? 0.45 : 1, backgroundColor: isInlineCodeOpen ? "var(--bg-selected)" : "transparent" }}
              >
                <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
                  <BrandIcon name="code" size={12} color="currentColor" />
                  <span>{t("canvas.editHtmlLabel", "Sửa HTML")}</span>
                </span>
                {dirtyBuffer && <span style={{ width: "6px", height: "6px", borderRadius: "50%", backgroundColor: "var(--agent-accent)" }} />}
              </button>
            )}
            {onToggleSampleData && (
              <button
                type="button"
                role="menuitem"
                disabled={isReadOnly}
                data-testid="overflow-sample-data-btn"
                onClick={() => { setOpen(false); onToggleSampleData(); }}
                style={{ ...MENU_ITEM_STYLE, opacity: isReadOnly ? 0.45 : 1, backgroundColor: isSampleDataOpen ? "var(--bg-selected)" : "transparent" }}
              >
                <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
                  <BrandIcon name="database" size={12} color="currentColor" />
                  <span>{t("canvas.sampleDataLabel", "Dữ liệu mẫu")}</span>
                </span>
              </button>
            )}
            {onOpenExportSection && (
              <>
                <div style={{ height: "1px", backgroundColor: "var(--border-subtle)", margin: "2px 4px" }} />
                <button type="button" role="menuitem" data-testid="overflow-export-zip" style={MENU_ITEM_STYLE} onClick={() => { setOpen(false); onOpenExportSection("zip"); }}>
                  <span>{t("snapshots.downloadZipBtn", "Export · Tải ZIP Full Repo")}</span>
                </button>
                <button type="button" role="menuitem" data-testid="overflow-export-github" style={MENU_ITEM_STYLE} onClick={() => { setOpen(false); onOpenExportSection("github-pr"); }}>
                  <span>{t("snapshots.pushGithubPrMenu", "Export · Đẩy GitHub PR")}</span>
                </button>
                <button type="button" role="menuitem" data-testid="overflow-export-ide" style={MENU_ITEM_STYLE} onClick={() => { setOpen(false); onOpenExportSection("ide-handoff"); }}>
                  <span>{t("snapshots.ideHandoffTitle", "Export · Mở trong IDE")}</span>
                </button>
              </>
            )}
          </div>,
          document.body
        )}
    </>
  );
};

export const SubStateSwitcher: React.FC<{
  activeSubState?: FixtureVariant;
  isReadOnly?: boolean;
  onSelectSubState?: (variant: FixtureVariant) => void;
}> = ({ activeSubState, isReadOnly = false, onSelectSubState }) => {
  const current = normalizeFixtureRef(activeSubState);
  return (
    <div data-testid="fixture-substate-switcher" style={GROUP_WRAP_STYLE}>
      {FIXTURE_VARIANTS.map(({ variant }) => {
        const isActive = current === variant;
        return (
          <button
            key={variant}
            type="button"
            disabled={isReadOnly}
            data-testid={`fixture-variant-${variant}`}
            aria-pressed={isActive}
            onClick={() => onSelectSubState?.(variant)}
            style={{
              height: "20px",
              padding: "0 6px",
              border: "none",
              borderRadius: "4px",
              fontSize: "9.5px",
              fontFamily: "var(--font-mono)",
              fontWeight: isActive ? 700 : 500,
              backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
              color: isActive ? "var(--action-primary)" : "var(--text-muted)",
              opacity: isReadOnly ? 0.45 : 1,
              cursor: isReadOnly ? "not-allowed" : "pointer",
              whiteSpace: "nowrap",
            }}
          >
            {variant}
          </button>
        );
      })}
    </div>
  );
};
