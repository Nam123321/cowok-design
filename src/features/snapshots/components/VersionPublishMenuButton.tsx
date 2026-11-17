// @story 05.3
import React, { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ExportInitialSection } from "./ExportBundleModal";

export interface VersionPublishMenuButtonProps {
  snapshotsCount: number;
  onOpenHistory: () => void;
  onOpenExportSection: (section: ExportInitialSection) => void;
}

const MENU_ICON_SIZE = 14;
const SUBMENU_ICON_SIZE = 12;
const MENU_OFFSET_Y = 6;
const PORTAL_Z_INDEX = 9999;

interface MenuCoords {
  top: number;
  right: number;
}

function useDropdownDismiss(
  anchorRef: React.RefObject<HTMLDivElement | null>,
  menuRef: React.RefObject<HTMLDivElement | null>,
  open: boolean,
  onClose: () => void
): void {
  useEffect(() => {
    if (!open) return;
    const handleOutside = (ev: MouseEvent) => {
      const target = ev.target as Node;
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

const PublishSubmenuPanel: React.FC<{
  submenuOpen: boolean;
  onPickSection: (sec: ExportInitialSection) => void;
}> = ({ submenuOpen, onPickSection }) => {
  const { t } = useTranslation();
  return (
    <div
      className="snap-submenu-panel"
      role="menu"
      style={submenuOpen ? { display: "flex", zIndex: PORTAL_Z_INDEX } : { zIndex: PORTAL_Z_INDEX }}
      data-testid="export-publish-submenu"
    >
      <button type="button" role="menuitem" className="snap-dropdown-item" data-testid="submenu-export-zip" onClick={() => onPickSection("zip")}>
        <span>{t("snapshots.downloadZipBtn", "Download Full Repo ZIP")}</span>
      </button>
      <button type="button" role="menuitem" className="snap-dropdown-item" data-testid="submenu-export-github" onClick={() => onPickSection("github-pr")}>
        <span>{t("snapshots.pushGithubPrMenu", "Push GitHub PR")}</span>
      </button>
      <button type="button" role="menuitem" className="snap-dropdown-item" data-testid="submenu-open-ide" onClick={() => onPickSection("ide-handoff")}>
        <span>{t("snapshots.ideHandoffTitle", "Open in IDE")}</span>
      </button>
    </div>
  );
};

const VersionPublishDropdown: React.FC<{
  menuRef: React.RefObject<HTMLDivElement | null>;
  coords: MenuCoords | null;
  snapshotsCount: number;
  submenuOpen: boolean;
  setSubmenuOpen: React.Dispatch<React.SetStateAction<boolean>>;
  onOpenHistory: () => void;
  onPickSection: (sec: ExportInitialSection) => void;
}> = ({ menuRef, coords, snapshotsCount, submenuOpen, setSubmenuOpen, onOpenHistory, onPickSection }) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  const fixedStyle: React.CSSProperties | undefined = coords
    ? { position: "fixed", top: `${coords.top}px`, right: `${coords.right}px`, zIndex: PORTAL_Z_INDEX }
    : undefined;

  const content = (
    <div ref={menuRef} className="snap-dropdown-menu" style={fixedStyle} role="menu" data-testid="version-publish-dropdown">
      <button type="button" role="menuitem" className="snap-dropdown-item" data-testid="toggle-version-history-btn" onClick={onOpenHistory}>
        <span>{t("snapshots.historyTitle", "Version History")}</span>
        <span className="snap-badge snap-badge-manual">{formatNumber(snapshotsCount)}</span>
      </button>
      <div className="snap-submenu-trigger" onMouseEnter={() => setSubmenuOpen(true)} onMouseLeave={() => setSubmenuOpen(false)}>
        <button type="button" role="menuitem" aria-haspopup="menu" aria-expanded={submenuOpen} className="snap-dropdown-item" data-testid="open-export-handoff-btn" onClick={() => setSubmenuOpen((prev) => !prev)}>
          <span>{t("snapshots.exportMenuLabel", "Export / Sync")}</span>
          <BrandIcon name="chevron-right" size={SUBMENU_ICON_SIZE} />
        </button>
        <PublishSubmenuPanel submenuOpen={submenuOpen} onPickSection={onPickSection} />
      </div>
    </div>
  );

  return coords && typeof document !== "undefined" ? createPortal(content, document.body) : content;
};

export const VersionPublishMenuButton: React.FC<VersionPublishMenuButtonProps> = ({
  snapshotsCount,
  onOpenHistory,
  onOpenExportSection,
}) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [submenuOpen, setSubmenuOpen] = useState(false);
  const [coords, setCoords] = useState<MenuCoords | null>(null);
  const anchorRef = useRef<HTMLDivElement | null>(null);
  const menuRef = useRef<HTMLDivElement | null>(null);
  useDropdownDismiss(anchorRef, menuRef, open, () => { setOpen(false); setSubmenuOpen(false); });
  const titleLabel = t("snapshots.versionPublishTooltip", "Version History & Export");

  const handleToggle = () => {
    if (!open && anchorRef.current && typeof window !== "undefined") {
      const rect = anchorRef.current.getBoundingClientRect();
      setCoords(rect.width > 0 ? { top: rect.bottom + MENU_OFFSET_Y, right: Math.max(MENU_OFFSET_Y, window.innerWidth - rect.right) } : null);
    }
    setOpen((prev) => !prev);
    setSubmenuOpen(false);
  };

  return (
    <div ref={anchorRef} className="snap-menu-anchor">
      <button type="button" className="snap-icon-trigger-btn" title={titleLabel} aria-label={titleLabel} aria-haspopup="menu" aria-expanded={open} data-testid="version-publish-menu-btn" onClick={handleToggle}>
        <BrandIcon name="deploy" size={MENU_ICON_SIZE} />
      </button>
      {open && (
        <VersionPublishDropdown
          menuRef={menuRef} coords={coords} snapshotsCount={snapshotsCount} submenuOpen={submenuOpen}
          setSubmenuOpen={setSubmenuOpen}
          onOpenHistory={() => { setOpen(false); onOpenHistory(); }}
          onPickSection={(sec) => { setOpen(false); onOpenExportSection(sec); }}
        />
      )}
    </div>
  );
};
