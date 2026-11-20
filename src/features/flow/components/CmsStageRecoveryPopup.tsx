// @story 06.2
// @implements AC-05, AC-06, AC-07 (Floating HTML Stage Recovery Popup — Aligned with DESIGN.md [5.12] CowokToast)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CmsSyncState } from "../hooks/useCmsSlideEditorController";
import type { CmsRecoveryDraft } from "../../../lib/validations/presentation";

const ICON_STATUS_PX = 14;
const ICON_CLOSE_PX = 12;

export interface CmsStageRecoveryProps {
  recoveredDraft: CmsRecoveryDraft | null;
  syncState: CmsSyncState;
  lastFlushError: string | null;
  customHtmlOverride: boolean;
  onRestoreDraft: () => void;
  onDismissDraft: () => void;
  onRetryFlush: () => void;
  onReloadServerRevision?: () => void;
  onResyncCustomHtml: () => void;
}

function resolveFlushErrorCopy(t: (k: string, d: string) => string, errCode: string | null): string {
  const code = (errCode || "").toUpperCase();
  if (code.includes("E_MXSS_BLOCKED")) {
    return t("presentation.error.mxssBlocked", "HTML bị chặn bởi bộ lọc bảo mật mXSS / CSS.");
  }
  if (code.includes("E_SQLITE_BUSY_RETRY") || code.includes("503")) {
    return t("presentation.error.sqliteBusy", "Hệ thống đang bận đồng bộ SQLite (503). Nhấn Thử lưu lại.");
  }
  if (code.includes("E_PAYLOAD_TOO_LARGE") || code.includes("413")) {
    return t("presentation.error.payloadTooLarge", "Nội dung vượt quá giới hạn dung lượng 512 KB (413).");
  }
  if (code.includes("404") || code.includes("E_CMS_SLOTS_HYDRATION_FAILED") || code.includes("NOT FOUND")) {
    return t(
      "presentation.error.slotsHydrationFailed",
      "Không tải được đầy đủ các khối CMS của slide (404). Nhấn Tải bản mới nhất để đồng bộ lại."
    );
  }
  return t("presentation.cms.flushFailedBanner", "Lưu thay đổi thất bại hoặc lệch phiên bản (409). Bản nháp vẫn được giữ an toàn.");
}

function resolvePopupCopy(
  t: (k: string, d: string) => string,
  showFlushErr: boolean,
  showOverride: boolean,
  lastFlushError: string | null
): string {
  if (showFlushErr) return resolveFlushErrorCopy(t, lastFlushError);
  if (showOverride) {
    return t("presentation.cms.customOverrideBanner", "HTML đã được sửa thủ công — một số trường CMS tạm khóa cho tới khi đồng bộ lại.");
  }
  return t("presentation.cms.draftRecoveredBanner", "Phát hiện bản nháp CMS chưa lưu từ phiên trước.");
}

const RecoveryBtn: React.FC<{
  testId: string;
  variant: "primary" | "secondary";
  label: string;
  onClick: () => void;
}> = ({ testId, variant, label, onClick }) => (
  <button type="button" className={`cowok-btn cowok-btn-${variant} ff-cms-mini-btn`} data-testid={testId} onClick={onClick}>
    {label}
  </button>
);

const RecoveryActionButtons: React.FC<{
  showDraft: boolean;
  showFlushErr: boolean;
  showOverride: boolean;
  onRestoreDraft: () => void;
  onDismissDraft: () => void;
  onRetryFlush: () => void;
  onReloadServerRevision?: () => void;
  onResyncCustomHtml: () => void;
}> = (p) => {
  const { t } = useTranslation();
  return (
    <div className="ff-cms-stage-popup-actions">
      {p.showDraft && (
        <>
          <RecoveryBtn testId="cms-restore-draft-btn" variant="primary" label={t("presentation.cms.restoreDraft", "Khôi phục")} onClick={p.onRestoreDraft} />
          <RecoveryBtn testId="cms-dismiss-draft-btn" variant="secondary" label={t("presentation.cms.dismissDraft", "Bỏ qua")} onClick={p.onDismissDraft} />
        </>
      )}
      {p.showFlushErr && (
        <>
          <RecoveryBtn testId="cms-retry-flush-btn" variant="primary" label={t("presentation.cms.retryFlush", "Thử lưu lại")} onClick={p.onRetryFlush} />
          {p.onReloadServerRevision && <RecoveryBtn testId="cms-reload-revision-btn" variant="secondary" label={t("presentation.cms.reloadServerRevision", "Tải bản mới nhất")} onClick={p.onReloadServerRevision} />}
        </>
      )}
      {p.showOverride && <RecoveryBtn testId="cms-reset-overrides-btn" variant="secondary" label={t("presentation.cms.resyncSlots", "Đồng bộ lại CMS")} onClick={p.onResyncCustomHtml} />}
    </div>
  );
};

export const CmsStageRecoveryPopup: React.FC<CmsStageRecoveryProps> = (p) => {
  const { t } = useTranslation();
  const showDraft = Boolean(p.recoveredDraft);
  const showFlushErr = p.syncState === "FLUSH_FAILED_RETRY" || Boolean(p.lastFlushError);
  const showOverride = p.customHtmlOverride;
  if (!showDraft && !showFlushErr && !showOverride) return null;

  const toneClass = showFlushErr ? "cowok-toast--danger" : "cowok-toast--warning";
  const accentColor = showFlushErr ? "var(--state-danger)" : "var(--state-warning)";
  const softBg = showFlushErr ? "var(--state-danger-soft)" : "var(--state-warning-soft)";

  return (
    <div className="ff-cms-stage-popup-layer">
      <div id="cms-recovery-banner" data-testid="cms-recovery-banner" role={showFlushErr ? "alert" : "status"} aria-live="polite" data-severity={showFlushErr ? "danger" : "warning"} className={`cowok-toast ${toneClass} ff-cms-stage-popup`}>
        <span className="cowok-toast__badge" style={{ backgroundColor: softBg, color: accentColor }} aria-hidden="true">
          <BrandIcon name="shield-alert" size={ICON_STATUS_PX} color={accentColor} />
        </span>
        <div className="ff-cms-stage-popup-content">
          <span className="cowok-toast__message ff-cms-stage-popup-msg">{resolvePopupCopy(t, showFlushErr, showOverride, p.lastFlushError)}</span>
          <RecoveryActionButtons showDraft={showDraft} showFlushErr={showFlushErr} showOverride={showOverride} onRestoreDraft={p.onRestoreDraft} onDismissDraft={p.onDismissDraft} onRetryFlush={p.onRetryFlush} onReloadServerRevision={p.onReloadServerRevision} onResyncCustomHtml={p.onResyncCustomHtml} />
        </div>
        {showDraft && !showFlushErr && (
          <button type="button" onClick={p.onDismissDraft} aria-label={t("common.close", "Đóng")} className="cowok-toast__close">
            <BrandIcon name="close" size={ICON_CLOSE_PX} color="currentColor" />
          </button>
        )}
      </div>
    </div>
  );
};
