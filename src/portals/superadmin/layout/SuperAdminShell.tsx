// @story 14.2
// @implements AC-02, ADR-008 (Enterprise SuperAdmin Portal App Shell — Responsive Desktop Sidebar & Mobile Drawer)
import React, { useEffect, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { AdminOperator, CloudAdminRole } from "../../../lib/validations/cloud-admin";
import {
  canRoleEditSection,
  findNavSection,
  SUPERADMIN_NAV_GROUPS,
  SUPERADMIN_NAV_ITEMS,
  type SuperAdminNavItem,
  type SuperAdminNavStatus,
  type SuperAdminSectionId,
} from "../navigation";

export interface SuperAdminShellProps {
  operator: AdminOperator;
  activeSection: SuperAdminSectionId;
  loading: boolean;
  openTicketCount: number;
  activeCrashCount: number;
  onSelectSection: (section: SuperAdminSectionId) => void;
  onRefresh: () => void;
  onPreviewRole: (role: CloudAdminRole) => void;
  onSignOut: () => void;
  children: React.ReactNode;
}

const DESKTOP_BREAKPOINT_PX = 960;
const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_BOLD = 700;
const ICON_SIZE_XS = 13;
const ICON_SIZE_SM = 15;
const ICON_SIZE_MD = 18;
const ICON_SIZE_LG = 20;

const CONTROL_STYLE: React.CSSProperties = {
  height: "34px",
  padding: "0 10px",
  borderRadius: "8px",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  fontSize: "12px",
  fontWeight: FONT_WEIGHT_MEDIUM,
  boxSizing: "border-box",
};

const OPERATOR_BADGE_STYLE: React.CSSProperties = {
  padding: "5px 10px",
  borderRadius: "999px",
  backgroundColor: "var(--state-success-soft)",
  color: "var(--state-success)",
  fontSize: "12px",
  fontWeight: FONT_WEIGHT_BOLD,
  display: "inline-flex",
  alignItems: "center",
  gap: "6px",
  maxWidth: "100%",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

const SIDEBAR_STYLE: React.CSSProperties = {
  width: "256px",
  backgroundColor: "var(--bg-surface)",
  borderRight: "1px solid var(--border-default)",
  padding: "18px 12px",
  display: "flex",
  flexDirection: "column",
  justifyContent: "space-between",
  gap: "20px",
  position: "sticky",
  top: 0,
  height: "100vh",
  overflowY: "auto",
  boxSizing: "border-box",
};

const MOBILE_PILL_BAR_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "6px",
  overflowX: "auto",
  padding: "8px 12px",
  backgroundColor: "var(--bg-surface)",
  borderBottom: "1px solid var(--border-default)",
  maxWidth: "100%",
  boxSizing: "border-box",
};

const STATUS_PILL_META: Record<
  SuperAdminNavStatus,
  { key: string; fallback: string; bg: string; fg: string }
> = {
  live: {
    key: "superadmin.shell.statusLive",
    fallback: "Đang hoạt động",
    bg: "var(--state-success-soft)",
    fg: "var(--state-success)",
  },
  roadmap: {
    key: "superadmin.shell.statusRoadmap",
    fallback: "Sắp ra mắt",
    bg: "var(--state-warning-soft)",
    fg: "var(--state-warning)",
  },
  policy: {
    key: "superadmin.shell.statusPolicy",
    fallback: "Chính sách bảo mật",
    bg: "var(--state-info-soft)",
    fg: "var(--state-info)",
  },
};

function useIsMobileViewport(): boolean {
  const [isMobile, setIsMobile] = useState<boolean>(() =>
    typeof window !== "undefined" ? window.innerWidth < DESKTOP_BREAKPOINT_PX : false
  );

  useEffect(() => {
    if (typeof window === "undefined") return;
    const handleResize = () => setIsMobile(window.innerWidth < DESKTOP_BREAKPOINT_PX);
    handleResize();
    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  return isMobile;
}

function resolveNavBadge(
  item: SuperAdminNavItem,
  openTicketCount: number,
  activeCrashCount: number,
  t: (key: string, fallback: string) => string
): string | null {
  if (item.id === "helpdesk" && openTicketCount > 0) return String(openTicketCount);
  if (item.id === "crashes" && activeCrashCount > 0) return String(activeCrashCount);
  if (item.badgeKey) return t(item.badgeKey, item.badge ?? "");
  return item.badge ?? null;
}

function resolveBadgePalette(item: SuperAdminNavItem): { bg: string; fg: string } {
  if (item.id === "crashes") return { bg: "var(--state-danger-soft)", fg: "var(--state-danger)" };
  if (item.status === "live") return { bg: "var(--state-info-soft)", fg: "var(--state-info)" };
  return { bg: "var(--bg-subtle)", fg: "var(--text-muted)" };
}

function NavItemBadge({ item, badgeText }: { item: SuperAdminNavItem; badgeText: string }) {
  const palette = resolveBadgePalette(item);
  return (
    <span style={{ padding: "1px 6px", borderRadius: "999px", backgroundColor: palette.bg, color: palette.fg, fontSize: "10px", fontWeight: FONT_WEIGHT_BOLD, flexShrink: 0 }}>
      {badgeText}
    </span>
  );
}

function SidebarNavButton({
  item,
  active,
  badgeText,
  testIdPrefix,
  onSelect,
}: {
  item: SuperAdminNavItem;
  active: boolean;
  badgeText: string | null;
  testIdPrefix: string;
  onSelect: () => void;
}) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      data-testid={`${testIdPrefix}-${item.id}`}
      onClick={onSelect}
      style={{
        width: "100%",
        minHeight: "36px",
        padding: "7px 10px",
        borderRadius: "8px",
        border: `1px solid ${active ? "var(--action-primary)" : "transparent"}`,
        backgroundColor: active ? "var(--bg-selected)" : "transparent",
        color: active ? "var(--action-primary)" : "var(--text-secondary)",
        fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
        fontSize: "13px",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: "8px",
        textAlign: "left",
      }}
    >
      <span style={{ display: "inline-flex", alignItems: "center", gap: "8px", minWidth: 0 }}>
        <BrandIcon name={item.icon} size={ICON_SIZE_SM} color="currentColor" />
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {t(item.labelKey, item.labelFallback)}
        </span>
      </span>
      {badgeText && <NavItemBadge item={item} badgeText={badgeText} />}
    </button>
  );
}

function NavGroupList({
  activeSection,
  openTicketCount,
  activeCrashCount,
  testIdPrefix,
  onSelectSection,
}: {
  activeSection: SuperAdminSectionId;
  openTicketCount: number;
  activeCrashCount: number;
  testIdPrefix: string;
  onSelectSection: (section: SuperAdminSectionId) => void;
}) {
  const { t } = useTranslation();
  return (
    <nav role="tablist" aria-label={t("superadmin.tabs.ariaLabel", "Các khu vực quản trị")} style={{ display: "flex", flexDirection: "column", gap: "16px" }}>
      {SUPERADMIN_NAV_GROUPS.map((group) => (
        <div key={group.id} data-testid={`superadmin-nav-group-${group.id}`} style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
          <div style={{ padding: "0 10px", fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.04em" }}>
            {t(group.titleKey, group.titleFallback)}
          </div>
          {group.items.map((item) => (
            <SidebarNavButton
              key={item.id}
              item={item}
              active={activeSection === item.id}
              badgeText={resolveNavBadge(item, openTicketCount, activeCrashCount, t)}
              testIdPrefix={testIdPrefix}
              onSelect={() => onSelectSection(item.id)}
            />
          ))}
        </div>
      ))}
    </nav>
  );
}

function SidebarBrandHeader() {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "10px", padding: "0 8px" }}>
      <span style={{ width: "34px", height: "34px", borderRadius: "9px", backgroundColor: "var(--bg-selected)", display: "inline-flex", alignItems: "center", justifyContent: "center", flexShrink: 0 }}>
        <BrandIcon name="shield" size={ICON_SIZE_LG} color="var(--action-primary)" />
      </span>
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: "14px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {t("superadmin.shell.brandTitle", "Cowok Cloud Admin")}
        </div>
        <div style={{ fontSize: "11px", color: "var(--text-muted)" }}>admin.cowok.design</div>
      </div>
    </div>
  );
}

function SidebarAccessFooter({ canEdit }: { canEdit: boolean }) {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "10px", borderRadius: "8px", backgroundColor: "var(--bg-subtle)", border: "1px solid var(--border-subtle)", fontSize: "11px", color: "var(--text-secondary)", display: "flex", flexDirection: "column", gap: "4px" }}>
      <div style={{ fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>
        {t("superadmin.shell.planeIsolationBadge", "Cổng quản trị độc lập")}
      </div>
      <div>
        {canEdit
          ? t("superadmin.shell.accessReadWrite", "Quyền mục hiện tại: Đọc & Ghi (có Audit Log)")
          : t("superadmin.shell.accessReadOnly", "Quyền mục hiện tại: Chỉ đọc (RBAC)")}
      </div>
    </div>
  );
}

function DesktopSidebar({
  operator,
  activeSection,
  openTicketCount,
  activeCrashCount,
  onSelectSection,
}: {
  operator: AdminOperator;
  activeSection: SuperAdminSectionId;
  openTicketCount: number;
  activeCrashCount: number;
  onSelectSection: (section: SuperAdminSectionId) => void;
}) {
  const canEdit = canRoleEditSection(findNavSection(activeSection), operator.role);
  return (
    <aside data-testid="superadmin-desktop-sidebar" style={SIDEBAR_STYLE}>
      <div style={{ display: "flex", flexDirection: "column", gap: "18px" }}>
        <SidebarBrandHeader />
        <NavGroupList
          activeSection={activeSection}
          openTicketCount={openTicketCount}
          activeCrashCount={activeCrashCount}
          testIdPrefix="superadmin-tab"
          onSelectSection={onSelectSection}
        />
      </div>
      <SidebarAccessFooter canEdit={canEdit} />
    </aside>
  );
}

function MobilePillButton({ item, active, onSelect }: { item: SuperAdminNavItem; active: boolean; onSelect: () => void }) {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      data-testid={`superadmin-tab-${item.id}`}
      onClick={onSelect}
      style={{
        height: "32px",
        padding: "0 10px",
        borderRadius: "999px",
        border: `1px solid ${active ? "var(--action-primary)" : "var(--border-subtle)"}`,
        backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)",
        color: active ? "var(--action-primary)" : "var(--text-secondary)",
        fontSize: "12px",
        fontWeight: FONT_WEIGHT_SEMIBOLD,
        whiteSpace: "nowrap",
        display: "inline-flex",
        alignItems: "center",
        gap: "5px",
        flexShrink: 0,
        cursor: "pointer",
      }}
    >
      <BrandIcon name={item.icon} size={ICON_SIZE_XS} color="currentColor" />
      <span>{t(item.labelKey, item.labelFallback)}</span>
    </button>
  );
}

function MobileQuickPillBar({ activeSection, onSelectSection }: { activeSection: SuperAdminSectionId; onSelectSection: (section: SuperAdminSectionId) => void }) {
  const { t } = useTranslation();
  return (
    <nav role="tablist" aria-label={t("superadmin.tabs.ariaLabel", "Các khu vực quản trị")} data-testid="superadmin-mobile-pill-nav" style={MOBILE_PILL_BAR_STYLE}>
      {SUPERADMIN_NAV_ITEMS.map((item) => (
        <MobilePillButton key={item.id} item={item} active={activeSection === item.id} onSelect={() => onSelectSection(item.id)} />
      ))}
    </nav>
  );
}

function OperatorActionsBar({
  operator,
  loading,
  onRefresh,
  onPreviewRole,
  onSignOut,
}: {
  operator: AdminOperator;
  loading: boolean;
  onRefresh: () => void;
  onPreviewRole: (role: CloudAdminRole) => void;
  onSignOut: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap", maxWidth: "100%" }}>
      <span data-testid="superadmin-operator-badge" style={OPERATOR_BADGE_STYLE}>
        <BrandIcon name="shield" size={ICON_SIZE_XS} color="var(--state-success)" />
        <span>{operator.email} · {operator.role} · MFA</span>
      </span>
      {operator.role === "SUPER_ADMIN" && (
        <select aria-label={t("superadmin.auth.roleSwitcher", "Vai trò vận hành")} data-testid="superadmin-role-switcher" value={operator.role} onChange={(e) => onPreviewRole(e.target.value as CloudAdminRole)} style={CONTROL_STYLE}>
          <option value="SUPER_ADMIN">SUPER_ADMIN</option>
          <option value="SUPPORT_AGENT">SUPPORT_AGENT</option>
          <option value="SKILL_CURATOR">SKILL_CURATOR</option>
          <option value="ANALYST_READONLY">ANALYST_READONLY</option>
        </select>
      )}
      <button type="button" onClick={onRefresh} disabled={loading} data-testid="superadmin-refresh-btn" style={{ ...CONTROL_STYLE, cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "6px" }}>
        <BrandIcon name="refresh" size={ICON_SIZE_XS} />
        <span>{t("superadmin.common.refresh", "Làm mới")}</span>
      </button>
      <button type="button" onClick={onSignOut} data-testid="superadmin-signout-btn" style={{ ...CONTROL_STYLE, cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "6px" }}>
        <BrandIcon name="lock" size={ICON_SIZE_XS} />
        <span>{t("superadmin.auth.signOut", "Đăng xuất")}</span>
      </button>
    </div>
  );
}

function SectionStatusPill({ status }: { status: SuperAdminNavStatus }) {
  const { t } = useTranslation();
  const meta = STATUS_PILL_META[status];
  return (
    <span style={{ padding: "2px 8px", borderRadius: "999px", backgroundColor: meta.bg, color: meta.fg, fontSize: "11px", fontWeight: FONT_WEIGHT_BOLD, whiteSpace: "nowrap" }}>
      {t(meta.key, meta.fallback)}
    </span>
  );
}

function TopbarSectionHeader({
  activeItem,
  isMobile,
  mobileMenuOpen,
  onToggleMobileMenu,
}: {
  activeItem: SuperAdminNavItem;
  isMobile: boolean;
  mobileMenuOpen: boolean;
  onToggleMobileMenu: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "10px", minWidth: 0 }}>
      {isMobile && (
        <button type="button" aria-label={t("superadmin.shell.toggleMenu", "Mở danh mục điều hướng")} data-testid="superadmin-mobile-menu-btn" onClick={onToggleMobileMenu} style={{ ...CONTROL_STYLE, width: "34px", padding: 0, display: "inline-flex", alignItems: "center", justifyContent: "center", cursor: "pointer", flexShrink: 0 }}>
          <BrandIcon name={mobileMenuOpen ? "close" : "panel-left-open"} size={ICON_SIZE_SM} />
        </button>
      )}
      <BrandIcon name={activeItem.icon} size={ICON_SIZE_MD} color="var(--action-primary)" />
      <div style={{ minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <h1 style={{ margin: 0, fontSize: isMobile ? "15px" : "17px", fontWeight: FONT_WEIGHT_BOLD, color: "var(--text-primary)" }}>
            {t(activeItem.labelKey, activeItem.labelFallback)}
          </h1>
          <SectionStatusPill status={activeItem.status} />
        </div>
        <div style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>
          {t(activeItem.subtitleKey, activeItem.subtitleFallback)}
        </div>
      </div>
    </div>
  );
}

function PortalTopbar(props: {
  activeItem: SuperAdminNavItem;
  operator: AdminOperator;
  loading: boolean;
  isMobile: boolean;
  mobileMenuOpen: boolean;
  onToggleMobileMenu: () => void;
  onRefresh: () => void;
  onPreviewRole: (role: CloudAdminRole) => void;
  onSignOut: () => void;
}) {
  return (
    <header style={{ display: "flex", flexWrap: "wrap", justifyContent: "space-between", alignItems: "center", gap: "12px", padding: props.isMobile ? "12px" : "14px 24px", backgroundColor: "var(--bg-surface)", borderBottom: "1px solid var(--border-default)", boxSizing: "border-box" }}>
      <TopbarSectionHeader activeItem={props.activeItem} isMobile={props.isMobile} mobileMenuOpen={props.mobileMenuOpen} onToggleMobileMenu={props.onToggleMobileMenu} />
      <OperatorActionsBar operator={props.operator} loading={props.loading} onRefresh={props.onRefresh} onPreviewRole={props.onPreviewRole} onSignOut={props.onSignOut} />
    </header>
  );
}

function buildShellRootStyle(isMobile: boolean): React.CSSProperties {
  return {
    display: isMobile ? "flex" : "grid",
    flexDirection: isMobile ? "column" : undefined,
    gridTemplateColumns: isMobile ? undefined : "256px minmax(0, 1fr)",
    minHeight: "100vh",
    width: "100%",
    maxWidth: "100vw",
    overflowX: "hidden",
    backgroundColor: "var(--bg-canvas)",
    color: "var(--text-primary)",
    boxSizing: "border-box",
  };
}

export const SuperAdminShell: React.FC<SuperAdminShellProps> = (props) => {
  const isMobile = useIsMobileViewport();
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const activeItem = findNavSection(props.activeSection);
  const handleSelectMobile = (section: SuperAdminSectionId) => {
    props.onSelectSection(section);
    setMobileMenuOpen(false);
  };

  return (
    <div style={buildShellRootStyle(isMobile)}>
      {!isMobile && (
        <DesktopSidebar operator={props.operator} activeSection={props.activeSection} openTicketCount={props.openTicketCount} activeCrashCount={props.activeCrashCount} onSelectSection={props.onSelectSection} />
      )}
      <div style={{ display: "flex", flexDirection: "column", minWidth: 0, flex: 1 }}>
        <PortalTopbar activeItem={activeItem} operator={props.operator} loading={props.loading} isMobile={isMobile} mobileMenuOpen={mobileMenuOpen} onToggleMobileMenu={() => setMobileMenuOpen((prev) => !prev)} onRefresh={props.onRefresh} onPreviewRole={props.onPreviewRole} onSignOut={props.onSignOut} />
        {isMobile && <MobileQuickPillBar activeSection={props.activeSection} onSelectSection={handleSelectMobile} />}
        {isMobile && mobileMenuOpen && (
          <div data-testid="superadmin-mobile-drawer" style={{ padding: "12px", backgroundColor: "var(--bg-surface)", borderBottom: "1px solid var(--border-default)" }}>
            <NavGroupList activeSection={props.activeSection} openTicketCount={props.openTicketCount} activeCrashCount={props.activeCrashCount} testIdPrefix="superadmin-drawer-tab" onSelectSection={handleSelectMobile} />
          </div>
        )}
        <div style={{ padding: isMobile ? "12px" : "20px 24px", display: "flex", flexDirection: "column", gap: "16px", minWidth: 0, boxSizing: "border-box" }}>
          {props.children}
        </div>
      </div>
    </div>
  );
};
