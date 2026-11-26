// @story 04.1, 07.1, 10.3, 13.1
// @implements PA-3 2-Tier Scope Separation — Tab-Local Contextual Stage Toolbar
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { SsotToolbarStrip } from "../../../features/ssot-registry";
import { DEFAULT_TABS } from "../../../features/flow/components/companion-canvas-data";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";

export type StageTabKind = "html" | "slide" | "md" | "code";

export interface ContextualStageToolbarProps {
  projectId?: string;
  activeTabId: string;
  activeFormatTab?: "dag" | "slide-cms" | "md" | "assets" | "code";
  onOpenVisualQc?: () => void;
  onOpenTournament?: () => void;
  onOpenCertification?: () => void;
  activeTournamentRunCount?: number;
}

const ICON_SIZE_SM = 11;
const ICON_STROKE = 1.85;
const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;

export function resolveStageTabKind(
  activeTabId: string,
  activeFormatTab?: string
): StageTabKind {
  const idLower = activeTabId.toLowerCase();
  if (
    activeFormatTab === "md" ||
    idLower === "tab-design-md" ||
    idLower === "tab-prd" ||
    idLower.includes("md")
  ) {
    return "md";
  }
  if (
    activeFormatTab === "code" ||
    idLower === "tab-checkout-tsx" ||
    idLower === "tab-code" ||
    idLower.includes("tsx")
  ) {
    return "code";
  }
  if (
    activeFormatTab === "slide-cms" ||
    idLower.includes("slide") ||
    idLower.includes("deck")
  ) {
    return "slide";
  }
  return "html";
}

export function resolveStageTabTitle(
  activeTabId: string,
  activeFormatTab?: string
): string {
  const matched = DEFAULT_TABS.find((tab) => tab.id === activeTabId);
  if (matched) return matched.title;
  const kind = resolveStageTabKind(activeTabId, activeFormatTab);
  if (kind === "md") return "DESIGN.md";
  if (kind === "code") return "CheckoutForm.tsx";
  if (kind === "slide") return "q4-architecture-deck.html";
  return "checkout-app.html";
}

const TOOLBAR_ROOT_STYLE: React.CSSProperties = {
  height: "34px",
  minHeight: "34px",
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  gap: "8px",
  padding: "0 10px",
  backgroundColor: "var(--bg-surface)",
  borderBottom: "1px solid var(--border-subtle)",
  boxSizing: "border-box",
  flexShrink: 0,
  overflowX: "auto",
  overflowY: "hidden",
  whiteSpace: "nowrap",
  userSelect: "none",
};

const ACTION_GROUP_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "6px",
  flexShrink: 0,
};

const ACTION_BTN_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  height: "24px",
  padding: "0 8px",
  fontSize: "11px",
  fontWeight: FONT_WEIGHT_MEDIUM,
  color: "var(--text-secondary)",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-workspace)",
  cursor: "pointer",
  whiteSpace: "nowrap",
  flexShrink: 0,
};

const CONTEXT_BADGE_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  height: "22px",
  padding: "0 8px",
  fontSize: "11px",
  fontWeight: FONT_WEIGHT_MEDIUM,
  color: "var(--text-secondary)",
  borderRadius: "var(--radius-pill)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-workspace)",
  whiteSpace: "nowrap",
  flexShrink: 0,
};

const RUN_COUNT_PILL_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  justifyContent: "center",
  minWidth: "16px",
  height: "15px",
  padding: "0 4px",
  fontSize: "10px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  borderRadius: "var(--radius-pill)",
  backgroundColor: "var(--action-primary)",
  color: "var(--action-primary-fg)",
};

const StageContextBadge: React.FC<{ iconName: string; label: string }> = ({
  iconName,
  label,
}) => (
  <span data-testid="contextual-stage-badge" style={CONTEXT_BADGE_STYLE}>
    <BrandIcon name={iconName} size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
    <span>{label}</span>
  </span>
);

const VisualQcTriggerButton: React.FC<{ label: string; onClick: () => void }> = ({
  label,
  onClick,
}) => (
  <button
    type="button"
    data-testid="nav-visual-qc-btn"
    aria-label={label}
    title={label}
    onClick={onClick}
    style={ACTION_BTN_STYLE}
  >
    <BrandIcon name="check" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
    <span>{label}</span>
  </button>
);

const CertificationLauncherButton: React.FC<{ label: string; onClick: () => void }> = ({
  label,
  onClick,
}) => (
  <button
    type="button"
    data-testid="nav-certification-btn"
    aria-label={label}
    title={label}
    onClick={onClick}
    style={ACTION_BTN_STYLE}
  >
    <BrandIcon
      name="shield-check"
      size={ICON_SIZE_SM}
      strokeWidth={ICON_STROKE}
      color="currentColor"
    />
    <span>{label}</span>
  </button>
);

const TournamentLauncherButton: React.FC<{
  label: string;
  activeRunCount: number;
  onClick: () => void;
}> = ({ label, activeRunCount, onClick }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const hasActiveRuns = activeRunCount > 0;
  const formattedRunCount = formatNumber(activeRunCount);
  const accessibleLabel = hasActiveRuns
    ? t("workbench.tournamentActiveRunsAria", "{{label}} ({{count}} phiên đang chạy)", {
        label,
        count: formattedRunCount,
      })
    : label;
  return (
    <button
      type="button"
      data-testid="nav-tournament-btn"
      data-active-runs={activeRunCount}
      aria-label={accessibleLabel}
      title={accessibleLabel}
      onClick={onClick}
      style={{
        ...ACTION_BTN_STYLE,
        color: hasActiveRuns ? "var(--action-primary)" : "var(--text-secondary)",
        backgroundColor: hasActiveRuns ? "var(--bg-selected)" : "var(--bg-workspace)",
        border: hasActiveRuns
          ? "1px solid var(--action-primary)"
          : "1px solid var(--border-subtle)",
      }}
    >
      <BrandIcon name="layers" size={ICON_SIZE_SM} strokeWidth={ICON_STROKE} color="currentColor" />
      <span>{label}</span>
      {hasActiveRuns && (
        <span data-testid="tournament-active-run-badge" style={RUN_COUNT_PILL_STYLE}>
          {formattedRunCount}
        </span>
      )}
    </button>
  );
};

const StageToolbarFrame: React.FC<{
  tabKind: StageTabKind;
  leftSlot: React.ReactNode;
  rightSlot: React.ReactNode;
}> = ({ tabKind, leftSlot, rightSlot }) => {
  const { t } = useTranslation();
  return (
    <div
      role="toolbar"
      aria-label={t("workbench.contextualStageToolbar", "Thanh công cụ theo tab")}
      data-testid="contextual-stage-toolbar"
      data-tab-kind={tabKind}
      style={TOOLBAR_ROOT_STYLE}
    >
      <div style={ACTION_GROUP_STYLE}>{leftSlot}</div>
      <div style={ACTION_GROUP_STYLE}>{rightSlot}</div>
    </div>
  );
};

export const ContextualStageToolbar: React.FC<ContextualStageToolbarProps> = ({
  projectId,
  activeTabId,
  activeFormatTab,
  onOpenVisualQc,
  onOpenTournament,
  onOpenCertification,
  activeTournamentRunCount = 0,
}) => {
  const { t } = useTranslation();
  const tabKind = resolveStageTabKind(activeTabId, activeFormatTab);
  const certBtn = onOpenCertification ? (
    <CertificationLauncherButton
      label={t("evalCertification.navButton", "Chứng nhận Phát hành")}
      onClick={onOpenCertification}
    />
  ) : null;

  if (tabKind === "md") {
    return (
      <StageToolbarFrame
        tabKind={tabKind}
        leftSlot={<StageContextBadge iconName="wiki" label={t("workbench.stageContextMd", "Ngữ cảnh Đặc tả · Markdown")} />}
        rightSlot={
          <>
            {onOpenTournament && <TournamentLauncherButton label={t("tournament.launchFromSpecButton", "Tạo Đấu trường từ Spec")} activeRunCount={activeTournamentRunCount} onClick={onOpenTournament} />}
            {certBtn}
          </>
        }
      />
    );
  }

  if (tabKind === "code") {
    return (
      <StageToolbarFrame
        tabKind={tabKind}
        leftSlot={<StageContextBadge iconName="code" label={t("workbench.stageContextCode", "Ngữ cảnh Mã nguồn · TSX")} />}
        rightSlot={
          <>
            {onOpenVisualQc && <VisualQcTriggerButton label={t("visualQc.checkTokenButton", "Kiểm tra Token (QC)")} onClick={onOpenVisualQc} />}
            {certBtn}
          </>
        }
      />
    );
  }

  return (
    <StageToolbarFrame
      tabKind={tabKind}
      leftSlot={<SsotToolbarStrip projectId={projectId} />}
      rightSlot={
        <>
          {onOpenVisualQc && <VisualQcTriggerButton label={t("visualQc.navReadinessPill", "Kiểm định QC")} onClick={onOpenVisualQc} />}
          {onOpenTournament && <TournamentLauncherButton label={t("tournament.navButton", "Đấu trường Thiết kế")} activeRunCount={activeTournamentRunCount} onClick={onOpenTournament} />}
          {certBtn}
        </>
      }
    />
  );
};
