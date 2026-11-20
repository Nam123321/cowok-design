// @story 04.2, 05.2, 06.1
// @implements AC-06 (DESIGN.md [5.12] Toast & Action Receipt — Unified cowok.ai Notification Pattern)
import React, { useEffect } from "react";
import { BrandIcon } from "./BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";

export type CowokToastTone = "success" | "danger" | "warning" | "info";

export interface CowokToastProps {
  tone?: CowokToastTone;
  message: string;
  actionLabel?: string;
  onAction?: () => void;
  actionTestId?: string;
  onDismiss?: () => void;
  testId?: string;
  /** Auto-dismiss delay in milliseconds (`0` disables auto-dismiss). Matches cowok.ai Toaster default (5000ms). */
  durationMs?: number;
  bottomOffset?: "default" | "raised";
  className?: string;
}

const DEFAULT_DURATION_MS = 5000;
const STATUS_ICON_SIZE = 14;
const CLOSE_ICON_SIZE = 12;

const TONE_ICON_MAP: Record<CowokToastTone, string> = {
  success: "check-circle",
  danger: "shield-alert",
  warning: "shield-alert",
  info: "info",
};

const TONE_COLOR_MAP: Record<CowokToastTone, { accent: string; softBg: string }> = {
  success: { accent: "var(--state-success)", softBg: "var(--state-success-soft)" },
  danger: { accent: "var(--state-danger)", softBg: "var(--state-danger-soft)" },
  warning: { accent: "var(--state-warning)", softBg: "var(--state-warning-soft)" },
  info: { accent: "var(--action-primary)", softBg: "var(--bg-selected)" },
};

function useToastAutoDismiss(durationMs: number, message: string, tone: CowokToastTone, onDismiss?: () => void) {
  useEffect(() => {
    if (!onDismiss || durationMs <= 0) return undefined;
    const timer = window.setTimeout(onDismiss, durationMs);
    return () => window.clearTimeout(timer);
  }, [durationMs, message, onDismiss, tone]);
}

export const CowokToast: React.FC<CowokToastProps> = ({
  tone = "info", message, actionLabel, onAction, actionTestId, onDismiss,
  testId = "cowok-toast", durationMs = DEFAULT_DURATION_MS, bottomOffset = "default", className = "",
}) => {
  const { t } = useTranslation();
  useToastAutoDismiss(durationMs, message, tone, onDismiss);
  const palette = TONE_COLOR_MAP[tone];
  const roleAttr = tone === "danger" || tone === "warning" ? "alert" : "status";

  return (
    <div role={roleAttr} aria-live="polite" data-testid={testId} className={`cowok-toast cowok-toast--${tone}${bottomOffset === "raised" ? " cowok-toast--raised" : ""}${className ? ` ${className}` : ""}`}>
      <span className="cowok-toast__badge" style={{ backgroundColor: palette.softBg, color: palette.accent }} aria-hidden="true">
        <BrandIcon name={TONE_ICON_MAP[tone]} size={STATUS_ICON_SIZE} color={palette.accent} />
      </span>
      <span className="cowok-toast__message">{message}</span>
      {actionLabel && onAction && (
        <button type="button" onClick={onAction} data-testid={actionTestId} className="cowok-toast__action">
          {actionLabel}
        </button>
      )}
      {onDismiss && (
        <button type="button" onClick={onDismiss} aria-label={t("common.close", "Đóng")} className="cowok-toast__close">
          <BrandIcon name="close" size={CLOSE_ICON_SIZE} color="currentColor" />
        </button>
      )}
    </div>
  );
};
