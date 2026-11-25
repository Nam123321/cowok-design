// @story 07.2
// @implements AC-03
import React from "react";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { QcGateSummary } from "../../lib/validations/split-export";
import type { SplitExportErrorNotice } from "./useSplitExport";

const MAX_EXPORT_SIZE_MB = 25;

export interface QcGateBannerCardProps {
  qcGate: QcGateSummary;
  overrideQcWarning: boolean;
  onToggleOverride: (checked: boolean) => void;
  errorNotice?: SplitExportErrorNotice | null;
}

export const QcGateBannerCard: React.FC<QcGateBannerCardProps> = ({
  qcGate,
  overrideQcWarning,
  onToggleOverride,
  errorNotice = null,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const hasCriticalBlockers = qcGate.p0Count > 0;
  const isCleanPass = !hasCriticalBlockers && qcGate.status === "PASS";

  const bannerStyle: React.CSSProperties = hasCriticalBlockers
    ? {
        backgroundColor: "var(--state-danger-soft)",
        border: "1px solid var(--state-danger)",
        color: "var(--text-primary)",
        borderRadius: "var(--radius-md)",
        padding: "12px 14px",
        display: "flex",
        flexDirection: "column",
        gap: "8px",
      }
    : isCleanPass
      ? {
          backgroundColor: "var(--state-success-soft)",
          border: "1px solid var(--state-success)",
          color: "var(--text-primary)",
          borderRadius: "var(--radius-md)",
          padding: "10px 14px",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "12px",
        }
      : {
          backgroundColor: "var(--bg-subtle)",
          border: "1px solid var(--border-subtle)",
          color: "var(--text-secondary)",
          borderRadius: "var(--radius-md)",
          padding: "10px 14px",
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "12px",
        };

  return (
    <div
      style={bannerStyle}
      role={hasCriticalBlockers || errorNotice ? "alert" : "status"}
      data-ff-id="qc-gate-banner-card"
      data-testid="qc-gate-banner-card"
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "10px",
          flexWrap: "wrap",
        }}
      >
        <strong
          style={{
            fontSize: "13px",
            color: hasCriticalBlockers
              ? "var(--state-danger)"
              : isCleanPass
                ? "var(--state-success)"
                : "var(--text-primary)",
          }}
        >
          {hasCriticalBlockers
            ? t(
                "splitExport.qcBlockedTitle",
                "Phát hiện {{count}} lỗi hiển thị nghiêm trọng trước khi xuất",
                { count: formatNumber(qcGate.p0Count) }
              )
            : isCleanPass
              ? t(
                  "splitExport.qcPassedTitle",
                  "Kiểm tra hiển thị đã đạt chuẩn — sẵn sàng xuất bản"
                )
              : t(
                  "splitExport.qcUnscannedTitle",
                  "Chưa chạy kiểm tra hiển thị gần đây — bạn vẫn có thể xuất bản"
                )}
        </strong>

        {qcGate.p1Count > 0 && (
          <span
            style={{
              fontSize: "11.5px",
              padding: "2px 8px",
              borderRadius: "var(--radius-pill)",
              backgroundColor: "var(--state-warning-soft)",
              color: "var(--state-warning)",
              border: "1px solid var(--state-warning)",
              fontWeight: 600,
            }}
          >
            {t("splitExport.qcWarningsCount", "{{count}} lưu ý nhỏ", {
              count: formatNumber(qcGate.p1Count),
            })}
          </span>
        )}
      </div>

      {hasCriticalBlockers && (
        <>
          <p
            style={{
              margin: 0,
              fontSize: "12.5px",
              color: "var(--text-secondary)",
              lineHeight: 1.45,
            }}
          >
            {t(
              "splitExport.qcBlockedHint",
              "Bạn nên khắc phục lỗi bố cục trước khi gửi cho người xem, hoặc xác nhận bên dưới nếu vẫn muốn xuất bản ngay."
            )}
          </p>
          <label
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "8px",
              fontSize: "12.5px",
              fontWeight: 600,
              color: "var(--text-primary)",
              cursor: "pointer",
            }}
          >
            <input
              type="checkbox"
              checked={overrideQcWarning}
              onChange={(e) => onToggleOverride(e.target.checked)}
              data-ff-id="qc-override-checkbox"
              data-testid="qc-override-checkbox"
            />
            <span>
              {t(
                "splitExport.qcOverrideLabel",
                "Tôi đã hiểu rủi ro hiển thị và xác nhận vẫn muốn xuất file"
              )}
            </span>
          </label>
        </>
      )}

      {errorNotice && errorNotice.kind === "size_exceeded" && (
        <p
          style={{
            margin: 0,
            fontSize: "12.5px",
            color: "var(--state-danger)",
            fontWeight: 500,
          }}
          data-testid="split-export-size-error"
        >
          {t(
            "splitExport.sizeLimitExceeded",
            "Gói xuất bản vượt quá giới hạn an toàn {{limit}}MB. Hãy nén bớt ảnh dung lượng lớn rồi thử lại.",
            { limit: formatNumber(MAX_EXPORT_SIZE_MB) }
          )}
        </p>
      )}

      {errorNotice && errorNotice.kind === "generic" && (
        <p
          style={{
            margin: 0,
            fontSize: "12.5px",
            color: "var(--state-danger)",
          }}
          data-testid="split-export-generic-error"
        >
          {t(
            "splitExport.genericExportError",
            "Không thể đóng gói tệp lúc này. Vui lòng thử lại sau giây lát."
          )}
        </p>
      )}
    </div>
  );
};
