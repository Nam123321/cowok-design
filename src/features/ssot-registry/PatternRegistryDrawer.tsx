// @story 02.4
// @implements AC-01, AC-02, AC-05
import React, { useEffect } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import {
  type ComponentPatternDto,
  type SsotPortalId,
  SsotPortalIdSchema,
} from "../../lib/validations/ssot-registry";

const DRAWER_Z_INDEX = 60;
const HEADER_ICON_SIZE = 14;
const SMALL_ICON_SIZE = 11;
const ICON_STROKE = 1.85;
const SEMIBOLD_WEIGHT = 600;

const DRAWER_STYLE: React.CSSProperties = {
  position: "fixed",
  top: "44px",
  right: "12px",
  bottom: "12px",
  width: "min(380px, calc(100vw - 24px))",
  zIndex: DRAWER_Z_INDEX,
  display: "flex",
  flexDirection: "column",
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-subtle)",
  borderRadius: "var(--radius-md)",
  boxShadow: "var(--shadow-lg, var(--shadow-md))",
  color: "var(--text-primary)",
};

const ROW_HEADER_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: "12px 14px",
  borderBottom: "1px solid var(--border-subtle)",
};

const TOOLBAR_STYLE: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "8px",
  padding: "10px 14px",
  borderBottom: "1px solid var(--border-subtle)",
};

const SELECT_STYLE: React.CSSProperties = {
  flex: 1,
  height: "28px",
  fontSize: "12px",
  padding: "0 8px",
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
};

const refreshBtnStyle = (isLoading: boolean): React.CSSProperties => ({
  display: "inline-flex",
  alignItems: "center",
  gap: "4px",
  height: "28px",
  padding: "0 10px",
  fontSize: "11px",
  fontWeight: 500,
  borderRadius: "var(--radius-xs)",
  border: "1px solid var(--border-subtle)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  cursor: isLoading ? "wait" : "pointer",
});

const PatternDrawerHeader: React.FC<{ quarantinedCount: number; onClose: () => void }> = ({
  quarantinedCount,
  onClose,
}) => {
  const { t } = useTranslation();
  return (
    <header style={ROW_HEADER_STYLE}>
      <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
        <BrandIcon name="layers" size={HEADER_ICON_SIZE} strokeWidth={ICON_STROKE} color="currentColor" />
        <h2 style={{ margin: 0, fontSize: "13px", fontWeight: SEMIBOLD_WEIGHT }}>
          {t("ssot.patterns.title", "Design patterns")}
        </h2>
        {quarantinedCount > 0 && (
          <span data-testid="pattern-quarantine-badge" style={{ fontSize: "11px", fontWeight: SEMIBOLD_WEIGHT, padding: "1px 6px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)" }}>
            {quarantinedCount} {t("ssot.patterns.pendingSuffix", "pending review")}
          </span>
        )}
      </div>
      <button type="button" data-testid="pattern-registry-close" aria-label={t("ssot.patterns.close", "Close patterns")} onClick={onClose} style={{ border: "none", background: "transparent", color: "var(--text-secondary)", cursor: "pointer", padding: "4px" }}>
        <BrandIcon name="x" size={HEADER_ICON_SIZE} strokeWidth={ICON_STROKE} color="currentColor" />
      </button>
    </header>
  );
};

const PatternPortalToolbar: React.FC<{
  portalId: SsotPortalId;
  isLoading: boolean;
  onPortalChange: (portal: SsotPortalId) => void;
  onRefresh: () => void;
}> = ({ portalId, isLoading, onPortalChange, onRefresh }) => {
  const { t } = useTranslation();
  return (
    <div style={TOOLBAR_STYLE}>
      <select aria-label={t("ssot.patterns.portalSelect", "Active workspace portal")} data-testid="pattern-portal-select" value={portalId} onChange={(e) => onPortalChange(e.target.value as SsotPortalId)} style={SELECT_STYLE}>
        {SsotPortalIdSchema.options.map((pid) => (
          <option key={pid} value={pid}>{pid}</option>
        ))}
      </select>
      <button type="button" data-testid="pattern-refresh-btn" disabled={isLoading} onClick={onRefresh} style={refreshBtnStyle(isLoading)}>
        <BrandIcon name="refresh-cw" size={SMALL_ICON_SIZE} strokeWidth={ICON_STROKE} color="currentColor" />
        <span>{t("ssot.patterns.refresh", "Refresh")}</span>
      </button>
    </div>
  );
};

const PatternListItem: React.FC<{ pattern: ComponentPatternDto }> = ({ pattern }) => {
  const { t } = useTranslation();
  const isQuarantined = pattern.driftStatus === "quarantined_drift";
  const badgeBg = isQuarantined ? "var(--state-warning-soft)" : "var(--state-success-soft)";
  const badgeColor = isQuarantined ? "var(--state-warning)" : "var(--state-success)";

  return (
    <li data-testid={`pattern-item-${pattern.patternSlug}`} style={{ padding: "10px", borderRadius: "var(--radius-sm)", border: "1px solid var(--border-subtle)", backgroundColor: "var(--bg-workspace)" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px" }}>
        <strong style={{ fontSize: "12px", fontWeight: SEMIBOLD_WEIGHT }}>{pattern.componentName}</strong>
        <span style={{ fontSize: "11px", fontWeight: 500, padding: "1px 6px", borderRadius: "var(--radius-xs)", backgroundColor: badgeBg, color: badgeColor }}>
          {isQuarantined ? t("ssot.patterns.pending", "Pending review") : t("ssot.patterns.synced", "Synced")}
        </span>
      </div>
      {pattern.sourcePath && (
        <div style={{ marginTop: "4px", fontSize: "11px", fontFamily: "var(--font-mono)", color: "var(--text-secondary)", wordBreak: "break-all" }}>
          {pattern.sourcePath}
        </div>
      )}
    </li>
  );
};

export interface PatternRegistryDrawerProps {
  isOpen: boolean;
  portalId: SsotPortalId;
  patterns: ComponentPatternDto[];
  quarantinedCount: number;
  isLoading: boolean;
  onPortalChange: (portal: SsotPortalId) => void;
  onRefresh: () => void;
  onClose: () => void;
}

export const PatternRegistryDrawer: React.FC<PatternRegistryDrawerProps> = ({
  isOpen,
  portalId,
  patterns,
  quarantinedCount,
  isLoading,
  onPortalChange,
  onRefresh,
  onClose,
}) => {
  const { t } = useTranslation();

  useEffect(() => {
    if (!isOpen) return undefined;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <aside role="dialog" aria-label={t("ssot.patterns.title", "Design patterns")} data-ff-id="pattern-registry-drawer" data-testid="pattern-registry-drawer" style={DRAWER_STYLE}>
      <PatternDrawerHeader quarantinedCount={quarantinedCount} onClose={onClose} />
      <PatternPortalToolbar portalId={portalId} isLoading={isLoading} onPortalChange={onPortalChange} onRefresh={onRefresh} />
      <div style={{ flex: 1, overflowY: "auto", padding: "12px 14px" }}>
        {patterns.length === 0 ? (
          <p style={{ margin: 0, fontSize: "12px", color: "var(--text-secondary)" }}>
            {t("ssot.patterns.empty", "No saved components in this portal yet.")}
          </p>
        ) : (
          <ul style={{ listStyle: "none", margin: 0, padding: 0, display: "grid", gap: "8px" }}>
            {patterns.map((pattern) => (
              <PatternListItem key={pattern.id} pattern={pattern} />
            ))}
          </ul>
        )}
      </div>
    </aside>
  );
};
