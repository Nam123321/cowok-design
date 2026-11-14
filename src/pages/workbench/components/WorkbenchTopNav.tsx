// cowok-design — Canonical Workbench Top Window Bar (DESIGN.md v4.3-OS & Section 9.3 / Hình 2 & Hình 5)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

export type ThemeMode = "warm" | "dim" | "night" | "night-low" | "monochrome";

export interface WorkbenchTopNavProps {
  projectName: string;
  activeSessionTitle?: string;
  isStandaloneConversation?: boolean;
  themeMode?: ThemeMode;
  isDraftDirty?: boolean;
  onDeployDraft?: () => void;
  onDiscardDraft?: () => void;
  // Backward-compatible optional props
  isSidebarCollapsed?: boolean;
  onToggleSidebar?: () => void;
  isChatCollapsed?: boolean;
  onToggleChat?: () => void;
  metrics?: any;
  onThemeChange?: (theme: ThemeMode) => void;
}

export const WorkbenchTopNav: React.FC<WorkbenchTopNavProps> = ({
  projectName,
  activeSessionTitle,
  isStandaloneConversation = false,
  isDraftDirty = false,
  onDeployDraft,
  onDiscardDraft,
}) => {
  const { t } = useTranslation();

  return (
    <header
      className="workbench-top-nav"
      style={{
        height: "36px",
        minHeight: "36px",
        borderBottom: "1px solid var(--border-default)",
        padding: "0 14px",
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        backgroundColor: "var(--bg-surface)",
        zIndex: 10,
        flexShrink: 0,
        boxSizing: "border-box",
        userSelect: "none",
      }}
    >
      {/* Left Zone: Global Window Controls (Red / Yellow / Green) + Canonical Path Breadcrumb */}
      <div style={{ display: "flex", alignItems: "center", gap: "14px" }}>
        {/* macOS Global Window Traffic Lights (Hình 2 / Item 4) */}
        <div
          className="window-traffic-lights"
          aria-label={t("workbench.windowControls", "Điều khiển cửa sổ")}
          style={{
            display: "flex",
            alignItems: "center",
            gap: "7px",
            paddingRight: "4px",
          }}
        >
          <span
            title={t("workbench.closeWindow", "Đóng")}
            style={{
              width: "11px",
              height: "11px",
              borderRadius: "50%",
              backgroundColor: "var(--window-dot-close)",
              border: "1px solid var(--window-dot-border)",
              display: "inline-block",
            }}
          />
          <span
            title={t("workbench.minimizeWindow", "Thu nhỏ")}
            style={{
              width: "11px",
              height: "11px",
              borderRadius: "50%",
              backgroundColor: "var(--window-dot-minimize)",
              border: "1px solid var(--window-dot-border)",
              display: "inline-block",
            }}
          />
          <span
            title={t("workbench.maximizeWindow", "Phóng to")}
            style={{
              width: "11px",
              height: "11px",
              borderRadius: "50%",
              backgroundColor: "var(--window-dot-maximize)",
              border: "1px solid var(--window-dot-border)",
              display: "inline-block",
            }}
          />
        </div>

        {/* Canonical Context Path: cowok-design / projects / <project_name> OR cowok-design / conversation */}
        <nav
          aria-label={t("workbench.breadcrumbNav", "Đường dẫn ngữ cảnh")}
          style={{
            display: "flex",
            alignItems: "center",
            gap: "6px",
            fontSize: "12px",
            fontFamily: "var(--font-sans)",
          }}
        >
          <span
            style={{
              color: "var(--text-muted)",
              fontWeight: 500,
              fontFamily: "var(--font-mono)",
              fontSize: "11.5px",
            }}
          >
            {t("workbench.appName", "cowok-design")}
          </span>
          <span style={{ color: "var(--border-strong)", fontSize: "11px" }}>/</span>

          {isStandaloneConversation ? (
            <>
              <span
                style={{
                  color: "var(--text-secondary)",
                  fontWeight: 500,
                  fontFamily: "var(--font-mono)",
                  fontSize: "11.5px",
                }}
              >
                {t("workbench.conversationSegment", "conversation")}
              </span>
              {activeSessionTitle && (
                <>
                  <span style={{ color: "var(--border-strong)", fontSize: "11px" }}>/</span>
                  <span
                    style={{
                      fontWeight: 600,
                      color: "var(--text-primary)",
                      fontSize: "12px",
                    }}
                  >
                    {activeSessionTitle}
                  </span>
                </>
              )}
            </>
          ) : (
            <>
              <span
                style={{
                  color: "var(--text-muted)",
                  fontWeight: 500,
                  fontFamily: "var(--font-mono)",
                  fontSize: "11.5px",
                }}
              >
                {t("workbench.projectsSegment", "projects")}
              </span>
              <span style={{ color: "var(--border-strong)", fontSize: "11px" }}>/</span>
              <span
                style={{
                  fontWeight: 600,
                  color: "var(--text-primary)",
                  fontSize: "12px",
                }}
              >
                {projectName}
              </span>
              {activeSessionTitle && (
                <>
                  <span style={{ color: "var(--border-strong)", fontSize: "11px" }}>/</span>
                  <span
                    style={{
                      fontWeight: 500,
                      color: "var(--text-secondary)",
                      fontSize: "12px",
                    }}
                  >
                    {activeSessionTitle}
                  </span>
                </>
              )}
            </>
          )}
        </nav>
      </div>

      {/* Right Zone: Clean Global Window Context (Tab-specific DAG/Slide controls are strictly scoped inside the active tab) */}
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }} />
    </header>
  );
};
