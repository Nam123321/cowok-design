// @story 05.4
// @implements AC-05 (Tier-2 Plain-Language LockedRuleChangeReceiptCard when editing a locked AppShell region)
import React from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useTranslation } from "../../hooks/useTranslation";
import type { LockedRuleChangeReceipt } from "../../lib/validations/sketch-polish";

const ICON_SIZE_MD = 15;
const STROKE_WIDTH_MONO = 1.85;

export interface LockedRuleChangeReceiptCardProps {
  receipt: LockedRuleChangeReceipt | null;
  onConfirmOverride: () => void;
  onDismiss: () => void;
}

export const LockedRuleChangeReceiptCard: React.FC<LockedRuleChangeReceiptCardProps> = ({
  receipt,
  onConfirmOverride,
  onDismiss,
}) => {
  const { t } = useTranslation();
  if (!receipt) return null;

  return (
    <aside
      role="status"
      aria-live="polite"
      className="sp-receipt-card"
      data-ff-id="locked-rule-receipt-card"
      data-testid="locked-rule-receipt-card"
    >
      <div className="sp-receipt-header">
        <span style={{ display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="security" size={ICON_SIZE_MD} strokeWidth={STROKE_WIDTH_MONO} />
          <span>{t("sketchPolish.lockedRuleTitle", "Vùng khung chung đang được khóa")}</span>
        </span>
      </div>
      <p className="sp-receipt-body">
        {t(
          "sketchPolish.lockedRulePlainDesc",
          "Thay đổi này ảnh hưởng tới phần khung dùng chung của toàn ứng dụng. Bạn có muốn mở khóa tạm thời để áp dụng cho mọi màn hình không?"
        )}
      </p>
      <div className="sp-receipt-actions">
        <button type="button" className="sp-chip-btn" data-testid="receipt-dismiss-btn" onClick={onDismiss}>
          {t("sketchPolish.keepLockedBtn", "Giữ nguyên")}
        </button>
        {receipt.requiresReview && (
          <button
            type="button"
            className="sp-phase-cta"
            data-testid="receipt-confirm-override-btn"
            onClick={onConfirmOverride}
          >
            <BrandIcon name="check-circle" size={ICON_SIZE_MD} strokeWidth={STROKE_WIDTH_MONO} />
            <span>{t("sketchPolish.confirmRuleOverride", "Mở khóa & áp dụng")}</span>
          </button>
        )}
      </div>
    </aside>
  );
};
