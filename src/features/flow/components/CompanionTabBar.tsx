// @story 04.1
// @implements AC-03
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CanvasTabItem } from "./CompanionCanvasTab";

const ICON_SIZE_SM = 12;
const ICON_SIZE_MD = 13;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;
const POPOVER_Z_INDEX = 120;

const NEW_TAB_MENU_ITEMS: Array<{
  kind: "app-flow" | "slide-deck" | "md" | "assets" | "code";
  defaultName: string;
  icon: string;
  labelKey: string;
  defaultLabel: string;
  badge: string;
}> = [
  { kind: "app-flow", defaultName: "multi-sig-wallet.html", icon: "workflow", labelKey: "canvas.newTabAppFlow", defaultLabel: "Tạo App HTML (Userflow + Live HTML)", badge: ".html" },
  { kind: "slide-deck", defaultName: "series-a-pitch.html", icon: "layers", labelKey: "canvas.newTabSlideDeck", defaultLabel: "Tạo Slide Deck HTML (Slide CMS + HTML)", badge: "16:9" },
  { kind: "md", defaultName: "product-spec.md", icon: "wiki", labelKey: "canvas.newTabMarkdown", defaultLabel: "Mở Tài liệu Markdown (Hỗ trợ Quote)", badge: ".md" },
  { kind: "assets", defaultName: "brand-symbol.svg", icon: "image", labelKey: "canvas.newTabAssets", defaultLabel: "Mở Tệp Đồ họa / Biểu tượng (.svg/.png)", badge: ".svg/.png" },
  { kind: "code", defaultName: "tokens.css", icon: "code", labelKey: "canvas.newTabCode", defaultLabel: "Mở Tệp Mã nguồn / Cấu hình (.css/.tsx/.json)", badge: ".tsx/.json" },
];

const TAB_BAR_STYLE: React.CSSProperties = {
  height: "36px",
  minHeight: "36px",
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  borderBottom: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  padding: "0 0 0 8px",
  boxSizing: "border-box",
};

const FROZEN_ACTIONS_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "4px",
  padding: "0 8px",
  height: "100%",
  borderLeft: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-workspace)",
  flexShrink: 0,
  position: "relative",
};

const buildTabPillStyle = (isActive: boolean): React.CSSProperties => ({
  display: "inline-flex",
  alignItems: "center",
  gap: "6px",
  height: "28px",
  padding: "0 10px",
  borderRadius: "6px 6px 0 0",
  fontSize: "12px",
  fontWeight: isActive ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM,
  backgroundColor: isActive ? "var(--bg-selected)" : "transparent",
  color: isActive ? "var(--text-primary)" : "var(--text-muted)",
  border: "none",
  borderBottom: isActive ? "2px solid var(--action-primary)" : "2px solid transparent",
  cursor: "pointer",
  userSelect: "none",
  whiteSpace: "nowrap",
  alignSelf: "flex-end",
  transition: "all var(--duration-fast) var(--ease-standard)",
});

const CanvasTabPill: React.FC<{
  tab: CanvasTabItem;
  isActive: boolean;
  onSelect: (tabId: string, format: CanvasTabItem["format"]) => void;
  onClose: (e: React.MouseEvent, tabId: string) => void;
}> = ({ tab, isActive, onSelect, onClose }) => {
  const { t } = useTranslation();
  return (
    <div
      role="tab"
      aria-selected={isActive}
      data-testid={`canvas-tab-${tab.id}`}
      onClick={() => onSelect(tab.id, tab.format)}
      style={buildTabPillStyle(isActive)}
    >
      <BrandIcon name={tab.iconName} size={ICON_SIZE_SM} color="currentColor" />
      <span>{tab.title}</span>
      {tab.isClosable && (
        <button
          type="button"
          onClick={(e) => onClose(e, tab.id)}
          aria-label={`${t("canvas.closeTab", "Đóng tab")} ${tab.title}`}
          style={{ border: "none", background: "transparent", cursor: "pointer", fontSize: "13px", lineHeight: 1, color: "var(--text-muted)", padding: "0 2px", marginLeft: "2px" }}
        >
          ×
        </button>
      )}
    </div>
  );
};

const NewTabPopoverMenu: React.FC<{
  onCreateTab: (kind: "app-flow" | "slide-deck" | "md" | "assets" | "code", customTitle?: string) => void;
}> = ({ onCreateTab }) => {
  const { t } = useTranslation();
  return (
    <div
      style={{
        position: "absolute",
        top: "34px",
        right: "8px",
        width: "290px",
        backgroundColor: "var(--bg-surface)",
        border: "1px solid var(--border-default)",
        borderRadius: "var(--radius-md)",
        boxShadow: "var(--shadow-lg)",
        padding: "6px",
        zIndex: POPOVER_Z_INDEX,
        display: "flex",
        flexDirection: "column",
        gap: "2px",
      }}
    >
      {NEW_TAB_MENU_ITEMS.map((item) => (
        <button
          key={item.kind}
          type="button"
          onClick={() => onCreateTab(item.kind, item.defaultName)}
          style={{ padding: "7px 10px", border: "none", background: "transparent", borderRadius: "var(--radius-xs)", cursor: "pointer", display: "flex", alignItems: "center", justifyContent: "space-between", fontSize: "12px", color: "var(--text-primary)", textAlign: "left" }}
        >
          <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
            <BrandIcon name={item.icon} size={ICON_SIZE_SM} color="var(--human-accent)" />
            <span>{t(item.labelKey, item.defaultLabel)}</span>
          </span>
          <span style={{ fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>{item.badge}</span>
        </button>
      ))}
    </div>
  );
};

export const CompanionTabBar: React.FC<{
  tabs: CanvasTabItem[];
  activeTabId: string;
  isNewTabMenuOpen: boolean;
  onToggleNewTabMenu: () => void;
  onTabSelect: (tabId: string, format: CanvasTabItem["format"]) => void;
  onCloseTab: (e: React.MouseEvent, tabId: string) => void;
  onCreateTab: (kind: "app-flow" | "slide-deck" | "md" | "assets" | "code", customTitle?: string) => void;
}> = (props) => {
  const { t } = useTranslation();
  const openLabel = t("canvas.openNewTab", "Mở tệp dự án hoặc tạo thiết kế mới");

  return (
    <div className="canvas-tab-bar" role="tablist" style={TAB_BAR_STYLE}>
      <div style={{ display: "flex", alignItems: "center", gap: "3px", overflowX: "auto", flex: 1, height: "100%", paddingRight: "8px" }}>
        {props.tabs.map((tab) => (
          <CanvasTabPill key={tab.id} tab={tab} isActive={tab.id === props.activeTabId} onSelect={props.onTabSelect} onClose={props.onCloseTab} />
        ))}
      </div>
      <div className="canvas-tabbar-frozen-actions" style={FROZEN_ACTIONS_STYLE}>
        <button
          type="button"
          onClick={props.onToggleNewTabMenu}
          title={openLabel}
          aria-label={openLabel}
          className="cowok-btn cowok-btn-ghost"
          style={{ width: "28px", height: "28px", padding: 0, borderRadius: "var(--radius-xs)", color: "var(--text-secondary)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)" }}
        >
          <BrandIcon name="plus" size={ICON_SIZE_MD} color="currentColor" />
        </button>
        {props.isNewTabMenuOpen && <NewTabPopoverMenu onCreateTab={props.onCreateTab} />}
      </div>
    </div>
  );
};
