// @story 04.1
// @implements AC-06
import React, { useEffect, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ConflictState } from "../store/flow-workbench-types";

export interface DraftDeployBannerProps {
  isDraftDirty: boolean;
  orphanCount?: number;
  conflictState?: ConflictState | null;
  floatingOnCanvas?: boolean;
  onDeployDraft?: () => void;
  onDiscardDraft?: () => void;
  onResolveConflict?: (choice: "keep_mine" | "take_agent" | "inspect_collisions") => void;
}

const DRAFT_BAR_STYLE: React.CSSProperties = {
  padding: "6px 12px",
  backgroundColor: "var(--bg-elevated)",
  borderBottom: "1px solid var(--border-default)",
  borderLeft: "3px solid var(--action-primary)",
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  flexWrap: "wrap",
  gap: "8px",
};

const FLOATING_DRAFT_Z_INDEX = 25;

const FLOATING_CANVAS_WRAP_STYLE: React.CSSProperties = {
  position: "absolute",
  bottom: "12px",
  left: "12px",
  zIndex: FLOATING_DRAFT_Z_INDEX,
  maxWidth: "calc(100% - 24px)",
  display: "flex",
  flexDirection: "column",
  gap: "6px",
  pointerEvents: "auto",
};

const FLOATING_DRAFT_PILL_STYLE: React.CSSProperties = {
  padding: "4px 8px",
  backgroundColor: "var(--bg-elevated, var(--bg-surface))",
  border: "1px solid var(--action-primary)",
  borderLeft: "3px solid var(--action-primary)",
  borderRadius: "var(--radius-sm)",
  boxShadow: "var(--shadow-md)",
  display: "inline-flex",
  alignItems: "center",
  gap: "8px",
  flexWrap: "wrap",
};

const DRAFT_BADGE_STYLE: React.CSSProperties = {
  fontFamily: "var(--font-mono)",
  fontSize: "10px",
  fontWeight: 600,
  padding: "2px 6px",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--bg-subtle)",
  color: "var(--text-secondary)",
  border: "1px solid var(--border-default)",
  letterSpacing: "0.03em",
};

const DEPLOY_BTN_STYLE: React.CSSProperties = {
  height: "24px",
  padding: "0 10px",
  fontSize: "11px",
  fontWeight: 600,
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--action-primary)",
  color: "var(--action-primary-fg)",
  display: "inline-flex",
  alignItems: "center",
  gap: "5px",
  cursor: "pointer",
};

const AntiFrankenConflictBanner: React.FC<{
  conflictState: ConflictState;
  onResolveConflict?: (choice: "keep_mine" | "take_agent" | "inspect_collisions") => void;
}> = ({ conflictState, onResolveConflict }) => {
  const { t } = useTranslation();
  const [showList, setShowList] = useState(false);
  const total = conflictState.collidingNodeIds.length + conflictState.collidingEdgeIds.length;
  const takeAgent = () => {
    if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
      void navigator.clipboard.writeText(conflictState.backupJson);
    }
    onResolveConflict?.("take_agent");
  };

  return (
    <div data-testid="dag-conflict-banner" style={{ padding: "8px 12px", backgroundColor: "var(--bg-elevated)", borderBottom: "1px solid var(--border-default)", borderLeft: "3px solid var(--state-danger)", display: "flex", flexDirection: "column", gap: "6px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "8px", flexWrap: "wrap" }}>
        <span style={{ fontSize: "11.5px", fontWeight: 600, color: "var(--text-primary)" }}>
          {t("canvas.conflictDetected", "Xung đột phiên bản đồ thị với Agent — chọn phương án đồng bộ:")}
        </span>
        <div style={{ display: "flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
          <button type="button" data-testid="conflict-keep-mine-btn" onClick={() => onResolveConflict?.("keep_mine")} className="cowok-btn cowok-btn-primary" style={{ height: "24px", padding: "0 10px", fontSize: "11px", fontWeight: 600, borderRadius: "var(--radius-xs)", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer" }}>
            {t("canvas.conflictKeepMine", "Giữ bản của tôi")}
          </button>
          <button type="button" data-testid="conflict-take-agent-btn" onClick={takeAgent} className="cowok-btn cowok-btn-secondary" style={{ height: "24px", padding: "0 10px", fontSize: "11px", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
            {t("canvas.conflictTakeAgent", "Lấy bản của Agent (Kèm Copy JSON nháp)")}
          </button>
          <button type="button" data-testid="conflict-inspect-btn" onClick={() => { setShowList((prev) => !prev); onResolveConflict?.("inspect_collisions"); }} className="cowok-btn cowok-btn-ghost" style={{ height: "24px", padding: "0 8px", fontSize: "11px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", cursor: "pointer" }}>
            {`${t("canvas.conflictInspectPrefix", "Xem")} ${useFormatter().formatNumber(total)} ${t("canvas.conflictInspectSuffix", "điểm trùng")}`}
          </button>
        </div>
      </div>
      {showList && total > 0 && (
        <div style={{ fontSize: "10.5px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
          {`${t("canvas.conflictMatchingIds", "Trùng khớp ID")}: ${[...conflictState.collidingNodeIds, ...conflictState.collidingEdgeIds].join(", ")}`}
        </div>
      )}
    </div>
  );
};

const ORPHAN_BADGE_STYLE: React.CSSProperties = {
  fontSize: "10.5px",
  fontWeight: 600,
  padding: "1px 6px",
  borderRadius: "var(--radius-xs)",
  backgroundColor: "var(--state-warning-soft)",
  color: "var(--state-warning)",
  border: "1px solid var(--state-warning)",
};

const DraftBannerStatus: React.FC<{ isDraftDirty: boolean; orphanCount: number; draftNoticeLabel: string }> = ({
  isDraftDirty,
  orphanCount,
  draftNoticeLabel,
}) => {
  const { t } = useTranslation();
  const { formatNumber } = useFormatter();
  return (
    <div style={{ display: "inline-flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
      <span aria-hidden="true" style={{ width: "6px", height: "6px", borderRadius: "50%", backgroundColor: isDraftDirty ? "var(--action-primary)" : "var(--state-warning)", flexShrink: 0 }} />
      {isDraftDirty && (
        <>
          <span className="dag-draft-badge" style={DRAFT_BADGE_STYLE}>DRAFT</span>
          <span style={{ fontSize: "11.5px", fontWeight: 500, color: "var(--text-primary)" }}>{draftNoticeLabel}</span>
        </>
      )}
      {orphanCount > 0 && (
        <span data-testid="dag-draft-orphan-warning" style={ORPHAN_BADGE_STYLE}>
          <BrandIcon name="warning" size={11} color="currentColor" />
          <span>{`${formatNumber(orphanCount)} ${t("canvas.orphanNodesSuffix", "ScreenState cô lập (chưa nối luồng)")}`}</span>
        </span>
      )}
    </div>
  );
};

const OrphanDeployConfirmBar: React.FC<{
  onConfirm?: () => void;
  onCancel: () => void;
}> = ({ onConfirm, onCancel }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="orphan-deploy-confirm-bar" role="alert" style={{ display: "flex", alignItems: "center", flexWrap: "wrap", gap: "6px" }}>
      <span style={{ fontSize: "11px", color: "var(--state-warning)", fontWeight: 600 }}>
        {t("canvas.orphanDeployConfirmPrompt", "Still deploy with orphan nodes?")}
      </span>
      {onConfirm && (
        <button type="button" data-testid="confirm-orphan-deploy-btn" onClick={onConfirm} className="cowok-btn cowok-btn-primary" style={DEPLOY_BTN_STYLE}>
          {t("canvas.confirmDeployAnyway", "Xác nhận triển khai")}
        </button>
      )}
      <button type="button" data-testid="cancel-orphan-deploy-btn" onClick={onCancel} className="cowok-btn cowok-btn-secondary" style={{ height: "24px", padding: "0 10px", fontSize: "11px", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
        {t("common.cancel", "Hủy")}
      </button>
    </div>
  );
};

const DraftBannerActions: React.FC<{
  showConfirm: boolean;
  onDeployClick: () => void;
  onConfirmOrphanDeploy: () => void;
  onCancelOrphanDeploy: () => void;
  onDeployDraft?: () => void;
  onDiscardDraft?: () => void;
}> = ({ showConfirm, onDeployClick, onConfirmOrphanDeploy, onCancelOrphanDeploy, onDeployDraft, onDiscardDraft }) => {
  const { t } = useTranslation();
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "6px", flexWrap: "wrap" }}>
      {showConfirm ? (
        <OrphanDeployConfirmBar onConfirm={onDeployDraft ? onConfirmOrphanDeploy : undefined} onCancel={onCancelOrphanDeploy} />
      ) : (
        <>
          {onDiscardDraft && (
            <button type="button" data-testid="discard-draft-btn" onClick={onDiscardDraft} className="cowok-btn cowok-btn-secondary" style={{ height: "24px", padding: "0 10px", fontSize: "11px", borderRadius: "var(--radius-xs)", cursor: "pointer" }}>
              {t("canvas.discardDraft", "Hủy nháp")}
            </button>
          )}
          {onDeployDraft && (
            <button type="button" data-testid="deploy-draft-btn" onClick={onDeployClick} className="cowok-btn cowok-btn-primary" style={DEPLOY_BTN_STYLE}>
              <BrandIcon name="deploy" size={12} color="currentColor" />
              <span>{t("canvas.deployDraft", "Triển khai bản nháp")}</span>
            </button>
          )}
        </>
      )}
    </div>
  );
};

function wrapFloating(floating: boolean, node: React.ReactNode): React.ReactNode {
  return floating ? <div style={FLOATING_CANVAS_WRAP_STYLE}>{node}</div> : node;
}

export const DraftDeployBanner: React.FC<DraftDeployBannerProps> = (props) => {
  const { isDraftDirty, orphanCount = 0, conflictState, floatingOnCanvas = false } = props;
  const { t } = useTranslation();
  const [confirmOrphanDeploy, setConfirmOrphanDeploy] = useState(false);

  useEffect(() => {
    if (!isDraftDirty || orphanCount <= 0) setConfirmOrphanDeploy(false);
  }, [isDraftDirty, orphanCount]);

  const handleDeployClick = () => {
    if (!props.onDeployDraft) return;
    if (orphanCount > 0 && !confirmOrphanDeploy) { setConfirmOrphanDeploy(true); return; }
    setConfirmOrphanDeploy(false);
    props.onDeployDraft();
  };

  if (!isDraftDirty && orphanCount <= 0) {
    return conflictState ? wrapFloating(floatingOnCanvas, <AntiFrankenConflictBanner conflictState={conflictState} onResolveConflict={props.onResolveConflict} />) : null;
  }

  return wrapFloating(floatingOnCanvas, (
    <>
      {conflictState && <AntiFrankenConflictBanner conflictState={conflictState} onResolveConflict={props.onResolveConflict} />}
      <div id="dagDraftBar" data-testid="dag-draft-bar" onMouseDown={floatingOnCanvas ? (e) => e.stopPropagation() : undefined} style={floatingOnCanvas ? FLOATING_DRAFT_PILL_STYLE : DRAFT_BAR_STYLE}>
        <DraftBannerStatus isDraftDirty={isDraftDirty} orphanCount={orphanCount} draftNoticeLabel={floatingOnCanvas ? "" : t("canvas.draftNotice", "Thay đổi cấu trúc đồ thị chưa triển khai")} />
        {isDraftDirty && (
          <DraftBannerActions showConfirm={confirmOrphanDeploy && orphanCount > 0} onDeployClick={handleDeployClick} onConfirmOrphanDeploy={() => { setConfirmOrphanDeploy(false); props.onDeployDraft?.(); }} onCancelOrphanDeploy={() => setConfirmOrphanDeploy(false)} onDeployDraft={props.onDeployDraft} onDiscardDraft={props.onDiscardDraft} />
        )}
      </div>
    </>
  ));
};
