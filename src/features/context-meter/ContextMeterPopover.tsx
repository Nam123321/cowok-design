// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import React from "react";
import { useTranslation } from "../../hooks/useTranslation";
import { CONTEXT_LIMITS } from "../../lib/validations/context-errors";
import type {
  ContextModelProfile,
  PortalId,
  TaskHorizon,
} from "../../lib/validations/context-types";
import type { ContextMeterSnapshot } from "./useContextMeter";

const PORTAL_OPTIONS: PortalId[] = [
  "DeveloperPortal",
  "SalesApp",
  "MarketingWeb",
  "Presentation",
  "TenantAdmin",
  "SuperAdmin",
];
const PROFILE_OPTIONS: ContextModelProfile[] = ["A", "B", "C"];
const HORIZON_OPTIONS: TaskHorizon[] = ["short", "mid", "long"];
const PERCENT_FULL = 100;
const SHA_PREVIEW_LEN = 10;
const POPOVER_Z_INDEX = 90;
const FONT_WEIGHT_SEMIBOLD = 600;

const SELECT_STYLE: React.CSSProperties = {
  fontSize: "11px",
  padding: "4px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-default)",
  backgroundColor: "var(--bg-surface)",
  color: "var(--text-primary)",
};

const POPOVER_CONTAINER_STYLE: React.CSSProperties = {
  position: "absolute",
  top: "32px",
  right: 0,
  width: "320px",
  padding: "12px",
  zIndex: POPOVER_Z_INDEX,
  backgroundColor: "var(--bg-elevated)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-md)",
  boxShadow: "var(--shadow-lg)",
  display: "flex",
  flexDirection: "column",
  gap: "8px",
  fontSize: "11px",
  color: "var(--text-primary)",
};

const HEADER_ROW_STYLE: React.CSSProperties = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
};

const ASSEMBLE_BUTTON_BASE_STYLE: React.CSSProperties = {
  padding: "6px 10px",
  borderRadius: "var(--radius-xs)",
  border: "none",
  backgroundColor: "var(--action-primary)",
  color: "var(--action-primary-fg)",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
};

export interface ContextMeterPopoverProps {
  popoverId: string;
  status: "empty" | "loading" | "ready" | "error";
  portalId: PortalId;
  horizon: TaskHorizon;
  modelProfile: ContextModelProfile;
  snapshot: ContextMeterSnapshot | null;
  error: { code: string; message: string } | null;
  hasValidProject: boolean;
  onPortalChange: (portal: PortalId) => void;
  onHorizonChange: (horizon: TaskHorizon) => void;
  onProfileChange: (profile: ContextModelProfile) => void;
  onAssemble: () => void;
}

const MeterSelectorsRow: React.FC<{
  portalId: PortalId;
  horizon: TaskHorizon;
  modelProfile: ContextModelProfile;
  onPortalChange: (p: PortalId) => void;
  onHorizonChange: (h: TaskHorizon) => void;
  onProfileChange: (m: ContextModelProfile) => void;
}> = ({ portalId, horizon, modelProfile, onPortalChange, onHorizonChange, onProfileChange }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: "6px" }}>
      <select
        aria-label={t("contextMeter.portalLabel", "Portal")}
        data-ff-id="context-meter-portal-select"
        data-testid="context-meter-portal-select"
        value={portalId}
        onChange={(e) => onPortalChange(e.target.value as PortalId)}
        style={SELECT_STYLE}
      >
        {PORTAL_OPTIONS.map((p) => (<option key={p} value={p}>{p}</option>))}
      </select>
      <select
        aria-label={t("contextMeter.horizonLabel", "Horizon")}
        data-ff-id="context-meter-horizon-select"
        data-testid="context-meter-horizon-select"
        value={horizon}
        onChange={(e) => onHorizonChange(e.target.value as TaskHorizon)}
        style={SELECT_STYLE}
      >
        {HORIZON_OPTIONS.map((h) => (<option key={h} value={h}>{h}</option>))}
      </select>
      <select
        aria-label={t("contextMeter.profileLabel", "Model Profile")}
        data-ff-id="context-meter-profile-select"
        data-testid="context-meter-profile-select"
        value={modelProfile}
        onChange={(e) => onProfileChange(e.target.value as ContextModelProfile)}
        style={SELECT_STYLE}
      >
        {PROFILE_OPTIONS.map((m) => (<option key={m} value={m}>{m}</option>))}
      </select>
    </div>
  );
};

const MeterStatsSection: React.FC<{
  snapshot: ContextMeterSnapshot | null;
}> = ({ snapshot }) => {
  const { t } = useTranslation();
  const textUsed = snapshot?.textTotalTokens ?? 0;
  const outlineUsed = snapshot?.outlineTokens ?? 0;
  const ratioLabel = snapshot
    ? `${Math.round(snapshot.ssotDomRatio * PERCENT_FULL)}%`
    : "—";
  const textPct = Math.min(
    PERCENT_FULL,
    Math.round((textUsed / CONTEXT_LIMITS.MAX_TEXT_BPE_TOKENS) * PERCENT_FULL)
  );

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
      <div style={{ display: "flex", justifyContent: "space-between", fontFamily: "var(--font-mono)" }}>
        <span>{t("contextMeter.textBudgetLabel", "Text BPE")}</span>
        <span>{textUsed} / {CONTEXT_LIMITS.MAX_TEXT_BPE_TOKENS}</span>
      </div>
      <div style={{ height: "6px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-subtle)", overflow: "hidden" }}>
        <div style={{ width: `${textPct}%`, height: "100%", backgroundColor: "var(--state-success)" }} />
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", fontFamily: "var(--font-mono)", color: "var(--text-secondary)" }}>
        <span>{t("contextMeter.outlineLabel", "DOM Outline")}: {outlineUsed}/{CONTEXT_LIMITS.MAX_OUTLINE_BPE_TOKENS}</span>
        <span>{t("contextMeter.ssotLabel", "SSOT")}: {ratioLabel}</span>
      </div>
      {snapshot && (
        <div style={{ display: "flex", justifyContent: "space-between", fontFamily: "var(--font-mono)", fontSize: "10px", color: "var(--text-secondary)", paddingTop: "4px", borderTop: "1px solid var(--border-subtle)" }}>
          <span>{t("contextMeter.shaLabel", "SHA")}: {snapshot.staticPrefixSha256.slice(0, SHA_PREVIEW_LEN)}</span>
          <span>{snapshot.reasonCode}</span>
        </div>
      )}
    </div>
  );
};

const PopoverStatusBanner: React.FC<{
  status: ContextMeterPopoverProps["status"];
  error: ContextMeterPopoverProps["error"];
}> = ({ status, error }) => {
  const { t } = useTranslation();
  if (status === "loading") {
    return <div role="status" style={{ color: "var(--text-muted)" }}>{t("contextMeter.loading", "Assembling portal context...")}</div>;
  }
  if (status === "empty") {
    return <div style={{ color: "var(--text-muted)" }}>{t("contextMeter.emptyState", "Select a project or click Assemble to inspect context budget.")}</div>;
  }
  if (status === "error" && error) {
    return (
      <div role="alert" style={{ padding: "6px 8px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-danger-soft)", color: "var(--state-danger)", border: "1px solid var(--state-danger)" }}>
        <strong>{error.code}</strong>: {t(error.message, error.message)}
      </div>
    );
  }
  return null;
};

export const ContextMeterPopover: React.FC<ContextMeterPopoverProps> = (props) => {
  const { t } = useTranslation();
  const disabled = props.status === "loading" || !props.hasValidProject;

  return (
    <div
      id={props.popoverId}
      data-ff-id="context-meter-popover"
      data-testid="context-meter-popover"
      role="dialog"
      aria-label={t("contextMeter.popoverTitle", "KV-Cache & Portal Context Telemetry")}
      style={POPOVER_CONTAINER_STYLE}
    >
      <div style={HEADER_ROW_STYLE}>
        <span>{t("contextMeter.popoverTitle", "KV-Cache & Portal Context Telemetry")}</span>
        <span style={{ fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
          {props.snapshot?.adapterMode || props.modelProfile}
        </span>
      </div>
      <MeterSelectorsRow
        portalId={props.portalId}
        horizon={props.horizon}
        modelProfile={props.modelProfile}
        onPortalChange={props.onPortalChange}
        onHorizonChange={props.onHorizonChange}
        onProfileChange={props.onProfileChange}
      />
      <MeterStatsSection snapshot={props.snapshot} />
      <PopoverStatusBanner status={props.status} error={props.error} />
      <button
        type="button"
        data-ff-id="context-meter-refresh-btn"
        data-testid="context-meter-refresh-btn"
        onClick={props.onAssemble}
        disabled={disabled}
        style={{ ...ASSEMBLE_BUTTON_BASE_STYLE, cursor: disabled ? "not-allowed" : "pointer" }}
      >
        {t("contextMeter.assembleButton", "Assemble Context")}
      </button>
    </div>
  );
};
