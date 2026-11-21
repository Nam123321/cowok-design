// @story 02.4
// @implements AC-02, AC-04, AC-05
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";

const LOCK_ICON_SIZE = 11;
const LOCK_ICON_STROKE = 1.85;
const TOGGLE_FONT_WEIGHT = 600;

interface LockVisualState {
  bg: string;
  color: string;
  iconName: string;
  badgeText: string;
  actionText: string;
}

function resolveLockVisualState(
  isLocked: boolean,
  hasConflict: boolean,
  t: (k: string, fallback: string) => string
): LockVisualState {
  if (hasConflict) {
    return {
      bg: "var(--state-warning-soft)",
      color: "var(--state-warning)",
      iconName: "pin-off",
      badgeText: t("ssot.lock.conflict", "Locked elsewhere"),
      actionText: t("ssot.lock.refresh", "Refresh"),
    };
  }
  if (isLocked) {
    return {
      bg: "var(--bg-selected)",
      color: "var(--action-primary)",
      iconName: "shield-check",
      badgeText: t("ssot.lock.locked", "Shell locked"),
      actionText: t("ssot.lock.unlock", "Unlock"),
    };
  }
  return {
    bg: "var(--bg-workspace)",
    color: "var(--text-secondary)",
    iconName: "pin-off",
    badgeText: t("ssot.lock.unlocked", "Unlocked"),
    actionText: t("ssot.lock.lock", "Lock"),
  };
}

const badgeStyle = (color: string): React.CSSProperties => ({
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  fontSize: "11px",
  fontWeight: 500,
  color,
});

const toggleStyle = (isBusy: boolean): React.CSSProperties => ({
  border: "none",
  background: "transparent",
  color: "var(--text-primary)",
  fontSize: "11px",
  fontWeight: TOGGLE_FONT_WEIGHT,
  padding: "1px 4px",
  borderRadius: "var(--radius-xs)",
  cursor: isBusy ? "wait" : "pointer",
});

export interface AppShellLockPanelProps {
  isLocked: boolean;
  isBusy: boolean;
  hasConflict: boolean;
  onToggleLock: () => void;
  onRefresh: () => void;
}

export const AppShellLockPanel: React.FC<AppShellLockPanelProps> = ({
  isLocked,
  isBusy,
  hasConflict,
  onToggleLock,
  onRefresh,
}) => {
  const { t } = useTranslation();
  const visual = resolveLockVisualState(isLocked, hasConflict, t);

  return (
    <div
      data-ff-id="appshell-lock-panel"
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "4px",
        height: "26px",
        padding: "0 6px",
        borderRadius: "var(--radius-xs)",
        border: "1px solid var(--border-subtle)",
        backgroundColor: visual.bg,
      }}
    >
      <span data-testid="appshell-lock-badge" style={badgeStyle(visual.color)}>
        <BrandIcon name={visual.iconName} size={LOCK_ICON_SIZE} strokeWidth={LOCK_ICON_STROKE} color="currentColor" />
        <span>{visual.badgeText}</span>
      </span>
      <button
        type="button"
        data-testid="appshell-lock-toggle"
        aria-pressed={isLocked}
        aria-busy={isBusy}
        disabled={isBusy}
        onClick={hasConflict ? onRefresh : onToggleLock}
        style={toggleStyle(isBusy)}
      >
        {visual.actionText}
      </button>
    </div>
  );
};
