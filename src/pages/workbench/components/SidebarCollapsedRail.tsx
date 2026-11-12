// @story 04.1
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

interface SidebarCollapsedRailProps {
  onToggleCollapse?: () => void;
  onOpenNewConversationModal: () => void;
  onOpenSettings: (tab?: "general" | "models" | "mcp" | "automations") => void;
}

const RAIL_STYLE: React.CSSProperties = {
  width: "44px", height: "100%", display: "flex", flexDirection: "column", alignItems: "center",
  justifyContent: "space-between", padding: "8px 0", backgroundColor: "var(--bg-workspace)",
  borderRight: "1px solid var(--border-subtle)", boxSizing: "border-box", flexShrink: 0, zIndex: 5,
};

const RAIL_ICON_BTN_STYLE: React.CSSProperties = {
  width: "28px", height: "28px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center",
  borderRadius: "var(--radius-xs)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-surface)",
};

export const SidebarCollapsedRail: React.FC<SidebarCollapsedRailProps> = ({
  onToggleCollapse,
  onOpenNewConversationModal,
  onOpenSettings,
}) => {
  const { t } = useTranslation();
  const expandLabel = t("workbench.expandSidebar", "Mở rộng thanh bên");
  const newLabel = t("workbench.newConversation", "New Conversation");
  const settingsLabel = t("workbench.settings", "Settings");

  return (
    <aside className="workbench-main-sidebar-rail" aria-label={t("workbench.collapseSidebar", "Thanh bên thu gọn")} style={RAIL_STYLE}>
      <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: "8px" }}>
        {onToggleCollapse && (
          <button type="button" onClick={onToggleCollapse} title={expandLabel} aria-label={expandLabel} className="cowok-btn cowok-btn-ghost" style={{ ...RAIL_ICON_BTN_STYLE, color: "var(--text-secondary)" }}>
            <BrandIcon name="panel-left-open" size={14} color="currentColor" />
          </button>
        )}
        <button type="button" onClick={onOpenNewConversationModal} title={newLabel} aria-label={newLabel} className="cowok-btn cowok-btn-ghost" style={{ ...RAIL_ICON_BTN_STYLE, color: "var(--action-primary)" }}>
          <BrandIcon name="plus" size={14} color="currentColor" />
        </button>
      </div>
      <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: "8px" }}>
        <button type="button" onClick={() => onOpenSettings()} title={settingsLabel} aria-label={settingsLabel} className="cowok-btn cowok-btn-ghost" style={{ width: "28px", height: "28px", padding: 0, display: "flex", alignItems: "center", justifyContent: "center", color: "var(--text-secondary)", borderRadius: "var(--radius-xs)" }}>
          <BrandIcon name="settings" size={14} color="currentColor" />
        </button>
      </div>
    </aside>
  );
};
