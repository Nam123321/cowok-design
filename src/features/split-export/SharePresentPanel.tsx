// @story 07.2
// @implements AC-01, AC-02, AC-03, AC-04
import React from "react";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import {
  type HtmlBundleExportData,
  type PptxExportData,
  SAFE_RELATIVE_INSTALL_ACTION_REGEX,
  type VectorPdfExportData,
} from "../../lib/validations/split-export";
import type { SplitExportActiveFormat } from "./useSplitExport";

const BYTES_PER_KB = 1024;
const BYTES_PER_MB = 1024 * 1024;
const KB_DECIMAL_THRESHOLD = 10;
const MIN_DISPLAY_KB = 0.1;

type TranslateFn = (
  key: string,
  defaultVal?: string,
  vars?: Record<string, string | number>
) => string;

function formatHumanFileSize(
  byteSize: number,
  formatNumber: (val: number, decimals?: number) => string,
  t: TranslateFn
): string {
  if (byteSize >= BYTES_PER_MB) {
    return t("splitExport.fileSizeMb", "{{size}} MB", {
      size: formatNumber(byteSize / BYTES_PER_MB, 1),
    });
  }
  const kb = Math.max(MIN_DISPLAY_KB, byteSize / BYTES_PER_KB);
  return t("splitExport.fileSizeKb", "{{size}} KB", {
    size: formatNumber(kb, kb >= KB_DECIMAL_THRESHOLD ? 0 : 1),
  });
}

export interface SharePresentPanelProps {
  activeFormat: SplitExportActiveFormat;
  disabled?: boolean;
  htmlResult: HtmlBundleExportData | null;
  pdfResult: VectorPdfExportData | null;
  pptxResult: PptxExportData | null;
  pptxPluginInstalled: boolean;
  onExportOfflineHtml: () => void;
  onExportVectorPdf: () => void;
  onExportPptx: () => void;
  onNavigatePluginInstall?: (installAction: string) => void;
}

const cardStyle: React.CSSProperties = {
  backgroundColor: "var(--bg-surface)",
  border: "1px solid var(--border-subtle)",
  borderRadius: "var(--radius-md)",
  padding: "16px",
  display: "flex",
  flexDirection: "column",
  justifyContent: "space-between",
  gap: "12px",
};

const primaryBtnStyle: React.CSSProperties = {
  backgroundColor: "var(--action-primary)",
  color: "var(--action-primary-fg)",
  border: "1px solid var(--action-primary)",
  borderRadius: "var(--radius-sm)",
  padding: "8px 14px",
  fontSize: "13px",
  fontWeight: 600,
  cursor: "pointer",
};

const secondaryBtnStyle: React.CSSProperties = {
  backgroundColor: "var(--bg-subtle)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-strong)",
  borderRadius: "var(--radius-sm)",
  padding: "8px 14px",
  fontSize: "13px",
  fontWeight: 600,
  cursor: "pointer",
};

const pillBadgeStyle: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "6px",
  fontSize: "11.5px",
  fontWeight: 600,
  padding: "3px 9px",
  borderRadius: "var(--radius-pill)",
  backgroundColor: "var(--state-success-soft)",
  color: "var(--state-success)",
};

export const SharePresentPanel: React.FC<SharePresentPanelProps> = ({
  activeFormat,
  disabled = false,
  htmlResult,
  pdfResult,
  pptxResult,
  pptxPluginInstalled,
  onExportOfflineHtml,
  onExportVectorPdf,
  onExportPptx,
  onNavigatePluginInstall,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });

  const isBusy = activeFormat !== null || disabled;
  const showPptxInstallGuide =
    pptxResult !== null && !pptxResult.pluginInstalled;

  return (
    <section
      style={{ display: "flex", flexDirection: "column", gap: "14px" }}
      aria-label={t("splitExport.sharePresentTab", "Chia sẻ & Trình chiếu")}
      data-ff-id="share-present-panel"
      data-testid="share-present-panel"
    >
      <p
        style={{
          margin: 0,
          fontSize: "13px",
          color: "var(--text-secondary)",
          lineHeight: 1.5,
        }}
      >
        {t(
          "splitExport.sharePresentSubtitle",
          "Xuất bản tức thì để trình chiếu, gửi đối tác hoặc in ấn sắc nét — không cần kiến thức lập trình."
        )}
      </p>

      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
          gap: "12px",
        }}
      >
        {/* 1. Self-Contained 1-File Offline HTML */}
        <article style={cardStyle} data-testid="export-card-offline-html">
          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            <span style={pillBadgeStyle}>
              {t("splitExport.offlineHtmlBadge", "100% Ngoại tuyến • 16:9")}
            </span>
            <h4
              style={{
                margin: "4px 0 0",
                fontSize: "14px",
                color: "var(--text-primary)",
              }}
            >
              {t("splitExport.offlineHtmlTitle", "Trình chiếu HTML 1 tệp")}
            </h4>
            <p
              style={{
                margin: 0,
                fontSize: "12.5px",
                color: "var(--text-secondary)",
                lineHeight: 1.45,
              }}
            >
              {t(
                "splitExport.offlineHtmlDesc",
                "Mở trực tiếp bằng trình duyệt kể cả khi mất mạng. Hỗ trợ phím mũi tên chuyển trang và toàn màn hình."
              )}
            </p>
          </div>

          {htmlResult && (
            <div
              style={{
                backgroundColor: "var(--bg-subtle)",
                borderRadius: "var(--radius-sm)",
                padding: "8px 10px",
                fontSize: "11.5px",
                color: "var(--text-secondary)",
                display: "flex",
                flexDirection: "column",
                gap: "3px",
              }}
              data-testid="offline-html-receipt"
            >
              <strong style={{ color: "var(--state-success)" }}>
                {t("splitExport.offlineHtmlReady", "Đã xuất thành công:")}{" "}
                {htmlResult.fileName}
              </strong>
              <span>
                {formatHumanFileSize(htmlResult.byteSize, formatNumber, t)} •{" "}
                {formatNumber(htmlResult.slideCount)}{" "}
                {t("splitExport.slidesUnit", "trang")} •{" "}
                <span
                  style={{
                    color:
                      htmlResult.externalCdnRefsCount === 0
                        ? "var(--state-success)"
                        : "var(--state-warning)",
                    fontWeight: 600,
                  }}
                >
                  {htmlResult.externalCdnRefsCount === 0
                    ? t("splitExport.offlineReadyStatus", "Sẵn sàng chạy ngoại tuyến")
                    : t(
                        "splitExport.externalLinksWarning",
                        "Còn {{count}} liên kết ngoài",
                        { count: formatNumber(htmlResult.externalCdnRefsCount) }
                      )}
                </span>
              </span>
            </div>
          )}

          <button
            type="button"
            style={primaryBtnStyle}
            disabled={isBusy}
            onClick={onExportOfflineHtml}
            data-ff-id="export-offline-html-btn"
            data-testid="export-offline-html-btn"
          >
            {activeFormat === "html"
              ? t("splitExport.packagingBtn", "Đang đóng gói...")
              : t("splitExport.downloadOfflineHtmlBtn", "Tải tệp trình chiếu (.html)")}
          </button>
        </article>

        {/* 2. Vector PDF 16:9 */}
        <article style={cardStyle} data-testid="export-card-vector-pdf">
          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            <span
              style={{
                ...pillBadgeStyle,
                backgroundColor: "var(--human-soft)",
                color: "var(--human-accent)",
              }}
            >
              {t("splitExport.vectorPdfBadge", "Vector 16:9 Sắc nét")}
            </span>
            <h4
              style={{
                margin: "4px 0 0",
                fontSize: "14px",
                color: "var(--text-primary)",
              }}
            >
              {t("splitExport.vectorPdfTitle", "Tài liệu PDF chuẩn in ấn")}
            </h4>
            <p
              style={{
                margin: 0,
                fontSize: "12.5px",
                color: "var(--text-secondary)",
                lineHeight: 1.45,
              }}
            >
              {t(
                "splitExport.vectorPdfDesc",
                "Giữ nguyên độ sắc nét của chữ và hình khối vector ở tỉ lệ màn ảnh rộng 16:9 khi gửi email hoặc in."
              )}
            </p>
          </div>

          {pdfResult && (
            <div
              style={{
                backgroundColor: "var(--bg-subtle)",
                borderRadius: "var(--radius-sm)",
                padding: "8px 10px",
                fontSize: "11.5px",
                color: "var(--text-secondary)",
              }}
              data-testid="vector-pdf-receipt"
            >
              <strong style={{ color: "var(--state-success)" }}>
                {pdfResult.fileName}
              </strong>{" "}
              ({formatNumber(pdfResult.pageCount)}{" "}
              {t("splitExport.pagesUnit", "trang")})
            </div>
          )}

          <button
            type="button"
            style={secondaryBtnStyle}
            disabled={isBusy}
            onClick={onExportVectorPdf}
            data-ff-id="export-vector-pdf-btn"
            data-testid="export-vector-pdf-btn"
          >
            {activeFormat === "pdf"
              ? t("splitExport.packagingBtn", "Đang đóng gói...")
              : t("splitExport.downloadVectorPdfBtn", "Xuất PDF 16:9 (.pdf)")}
          </button>
        </article>

        {/* 3. Plugin-Isolated PowerPoint (.pptx) */}
        <article style={cardStyle} data-testid="export-card-pptx">
          <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
            <span
              style={{
                ...pillBadgeStyle,
                backgroundColor: pptxPluginInstalled
                  ? "var(--state-success-soft)"
                  : "var(--state-warning-soft)",
                color: pptxPluginInstalled
                  ? "var(--state-success)"
                  : "var(--state-warning)",
              }}
            >
              {pptxPluginInstalled
                ? t("splitExport.pptxInstalledBadge", "Tiện ích đã sẵn sàng")
                : t("splitExport.pptxOptionalBadge", "Tiện ích mở rộng tùy chọn")}
            </span>
            <h4
              style={{
                margin: "4px 0 0",
                fontSize: "14px",
                color: "var(--text-primary)",
              }}
            >
              {t("splitExport.pptxTitle", "Tệp PowerPoint (.pptx)")}
            </h4>
            <p
              style={{
                margin: 0,
                fontSize: "12.5px",
                color: "var(--text-secondary)",
                lineHeight: 1.45,
              }}
            >
              {t(
                "splitExport.pptxDesc",
                "Tạo bản trình chiếu chỉnh sửa được trong PowerPoint thông qua tiện ích mở rộng tách biệt, không làm nặng ứng dụng."
              )}
            </p>
          </div>

          {showPptxInstallGuide && (
            <div
              style={{
                backgroundColor: "var(--state-warning-soft)",
                border: "1px solid var(--state-warning)",
                borderRadius: "var(--radius-sm)",
                padding: "10px",
                fontSize: "12px",
                color: "var(--text-primary)",
                display: "flex",
                flexDirection: "column",
                gap: "8px",
              }}
              data-testid="pptx-plugin-install-guide"
            >
              <span>
                {t(
                  "splitExport.pptxNotInstalledDesc",
                  "Tiện ích xuất PowerPoint chưa được cài đặt. Cài nhanh trong Kho Tiện Ích để mở khóa tính năng này."
                )}
              </span>
              {(() => {
                const safeInstallAction = SAFE_RELATIVE_INSTALL_ACTION_REGEX.test(
                  pptxResult.installAction
                )
                  ? pptxResult.installAction
                  : "/plugins";
                return (
                  <a
                    href={safeInstallAction}
                    onClick={(ev) => {
                      if (onNavigatePluginInstall) {
                        ev.preventDefault();
                        onNavigatePluginInstall(safeInstallAction);
                      }
                    }}
                    style={{
                      ...primaryBtnStyle,
                      textDecoration: "none",
                      textAlign: "center",
                      display: "inline-block",
                    }}
                    data-ff-id="export-pptx-plugin-cta"
                    data-testid="export-pptx-plugin-cta"
                  >
                    {t("splitExport.installPptxPluginBtn", "Cài tiện ích PowerPoint")}
                  </a>
                );
              })()}
            </div>
          )}

          {pptxResult && pptxResult.pluginInstalled && (
            <div
              style={{
                backgroundColor: "var(--state-success-soft)",
                borderRadius: "var(--radius-sm)",
                padding: "8px 10px",
                fontSize: "12px",
                color: "var(--state-success)",
              }}
              data-testid="pptx-dispatched-receipt"
            >
              {t("splitExport.pptxDispatched", "Đã chuyển bộ slide sang tiện ích PowerPoint:")}{" "}
              <strong>{pptxResult.fileName}</strong>
            </div>
          )}

          <button
            type="button"
            style={secondaryBtnStyle}
            disabled={isBusy}
            onClick={onExportPptx}
            data-ff-id="export-pptx-btn"
            data-testid="export-pptx-btn"
          >
            {activeFormat === "pptx"
              ? t("splitExport.checkingPluginBtn", "Đang kiểm tra tiện ích...")
              : t("splitExport.exportPptxBtn", "Xuất PowerPoint (.pptx)")}
          </button>
        </article>
      </div>
    </section>
  );
};
