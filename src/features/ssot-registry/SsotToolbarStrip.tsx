// @story 02.4
// @implements AC-01, AC-02, AC-04, AC-05
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { CowokToast } from "../../components/ui/CowokToast";
import { useTranslation } from "../../hooks/useTranslation";
import type { SsotPortalId } from "../../lib/validations/ssot-registry";
import { AppShellLockPanel } from "./AppShellLockPanel";
import { PatternRegistryDrawer } from "./PatternRegistryDrawer";
import { SsotExportButton } from "./SsotExportButton";
import { useSsotRegistryState } from "./useSsotRegistryState";

const TRIGGER_ICON_SIZE = 11;
const TRIGGER_ICON_STROKE = 1.85;
const SEMIBOLD_WEIGHT = 600;

const TRIGGER_BTN_STYLE: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  height: "26px",
  padding: "0 8px",
  fontSize: "11px",
  fontWeight: 500,
  color: "var(--text-secondary)",
  backgroundColor: "var(--bg-workspace)",
  border: "1px solid var(--border-subtle)",
  borderRadius: "var(--radius-xs)",
  cursor: "pointer",
};

const PatternsTriggerButton: React.FC<{
  drawerOpen: boolean;
  quarantinedCount: number;
  onToggle: () => void;
}> = ({ drawerOpen, quarantinedCount, onToggle }) => {
  const { t } = useTranslation();
  return (
    <button type="button" data-testid="ssot-patterns-trigger" aria-expanded={drawerOpen} onClick={onToggle} style={TRIGGER_BTN_STYLE}>
      <BrandIcon name="layers" size={TRIGGER_ICON_SIZE} strokeWidth={TRIGGER_ICON_STROKE} color="currentColor" />
      <span>{t("ssot.patterns.button", "Patterns")}</span>
      {quarantinedCount > 0 && (
        <span style={{ fontSize: "10px", fontWeight: SEMIBOLD_WEIGHT, padding: "0 4px", borderRadius: "999px", backgroundColor: "var(--state-warning-soft)", color: "var(--state-warning)" }}>
          {quarantinedCount}
        </span>
      )}
    </button>
  );
};

export interface SsotToolbarStripProps {
  projectId?: string;
  initialPortal?: SsotPortalId;
}

export const SsotToolbarStrip: React.FC<SsotToolbarStripProps> = ({
  projectId,
  initialPortal = "DeveloperPortal",
}) => {
  const [drawerOpen, setDrawerOpen] = useState(false);
  const ssot = useSsotRegistryState(projectId, initialPortal);

  return (
    <>
      <div style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
        <AppShellLockPanel isLocked={ssot.isLocked} isBusy={ssot.isMutatingLock || ssot.isLoading} hasConflict={ssot.hasConflict} onToggleLock={() => void ssot.toggleLock()} onRefresh={() => void ssot.refresh()} />
        <SsotExportButton isExporting={ssot.isExporting} disabled={!projectId} onExport={() => void ssot.triggerExport()} />
        <PatternsTriggerButton drawerOpen={drawerOpen} quarantinedCount={ssot.quarantinedCount} onToggle={() => setDrawerOpen((prev) => !prev)} />
      </div>

      <PatternRegistryDrawer isOpen={drawerOpen} portalId={ssot.portalId} patterns={ssot.patterns} quarantinedCount={ssot.quarantinedCount} isLoading={ssot.isLoading} onPortalChange={ssot.setPortalId} onRefresh={() => void ssot.refresh()} onClose={() => setDrawerOpen(false)} />

      {ssot.toast && <CowokToast tone={ssot.toast.tone} message={ssot.toast.message} testId="ssot-export-toast" onDismiss={ssot.dismissToast} />}
    </>
  );
};
