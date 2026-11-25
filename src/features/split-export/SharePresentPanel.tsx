// @story 07.2, 07.3
// @implements AC-01, AC-02, AC-03, AC-04
import React, { useState } from "react";
import { BrandIcon } from "../../components/ui/BrandIcon";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import {
  type HtmlBundleExportData,
  type PptxExportData,
  type PptxExportMode,
  type PptxFidelityWarning,
  type PptxFidelityWarningCode,
  SAFE_RELATIVE_INSTALL_ACTION_REGEX,
  type VectorPdfExportData,
} from "../../lib/validations/split-export";
import type { SplitExportActiveFormat } from "./useSplitExport";

const BYTES_PER_KB = 1024;
const BYTES_PER_MB = BYTES_PER_KB * BYTES_PER_KB;
const KB_DECIMAL_THRESHOLD = 10;
const MIN_DISPLAY_KB = 0.1;
const ICON_XS = 12;
const ICON_SM = 14;
const FONT_WEIGHT_MEDIUM = 500;
const FONT_WEIGHT_SEMIBOLD = 600;
const LINE_HEIGHT_TIGHT = 1.35;
const LINE_HEIGHT_SNUG = 1.4;
const LINE_HEIGHT_NORMAL = 1.45;
const LINE_HEIGHT_RELAXED = 1.5;

type TranslateFn = (
  key: string,
  defaultVal?: string,
  vars?: Record<string, string | number>
) => string;

type FormatNumberFn = (val: number, decimals?: number) => string;

const WARNING_I18N_MAP: Record<PptxFidelityWarningCode, { key: string; fallback: string }> = {
  BACKDROP_FILTER_RASTERIZED: {
    key: "splitExport.pptxWarnBackdropFilter",
    fallback: "Trang {{slide}}: Hiệu ứng kính mờ đã được chuyển thành ảnh nét cao để hiển thị đúng.",
  },
  CSS_ANIMATION_OMITTED: {
    key: "splitExport.pptxWarnCssAnimation",
    fallback: "Trang {{slide}}: Chuyển động CSS tùy biến không áp dụng trên PowerPoint và đã được bỏ qua.",
  },
  HOVER_STATE_OMITTED: {
    key: "splitExport.pptxWarnHoverState",
    fallback: "Trang {{slide}}: Hiệu ứng di chuột (hover) không áp dụng trên PowerPoint.",
  },
  CANVAS_OR_3D_RASTERIZED: {
    key: "splitExport.pptxWarnCanvas3d",
    fallback: "Trang {{slide}}: Biểu đồ/3D tương tác đã được chuyển thành ảnh nét cao.",
  },
  COMPLEX_SVG_RASTERIZED: {
    key: "splitExport.pptxWarnComplexSvg",
    fallback: "Trang {{slide}}: Hình vector phức tạp đã được tối ưu sang ảnh nét cao kèm dự phòng.",
  },
  SHAPE_BUDGET_RASTERIZED: {
    key: "splitExport.pptxWarnShapeBudget",
    fallback: "Trang {{slide}}: Cụm nhiều chi tiết đã được gộp ảnh nét cao để mở mượt trên PowerPoint.",
  },
  CUSTOM_FONT_FALLBACK: {
    key: "splitExport.pptxWarnCustomFont",
    fallback: "Trang {{slide}}: Đã bật tự động co chữ để giữ bố cục nếu máy người nhận chưa có phông tùy chỉnh.",
  },
  EXTERNAL_MEDIA_BLOCKED: {
    key: "splitExport.pptxWarnExternalMedia",
    fallback: "Trang {{slide}}: Đã chặn ảnh từ liên kết mạng ngoài để bảo đảm an toàn ngoại tuyến.",
  },
  STATIC_LAYOUT_FALLBACK: {
    key: "splitExport.pptxWarnStaticFallback",
    fallback: "Trang {{slide}}: Đã áp dụng bố cục dự phòng ngoại tuyến.",
  },
  VISUAL_EXACT_RASTER_UNAVAILABLE: {
    key: "splitExport.pptxWarnVisualExactFallback",
    fallback: "Trang {{slide}}: Ảnh chụp toàn trang không khả dụng; đã giữ nguyên các thành phần có thể chỉnh sửa thay vì trang trống.",
  },
  INVALID_MEDIA_REJECTED: {
    key: "splitExport.pptxWarnInvalidMedia",
    fallback: "Trang {{slide}}: Đã bỏ qua hình ảnh không hợp lệ hoặc sai định dạng để bảo vệ tệp trình chiếu.",
  },
};

function formatHumanFileSize(byteSize: number, formatNumber: FormatNumberFn, t: TranslateFn): string {
  if (byteSize >= BYTES_PER_MB) {
    return t("splitExport.fileSizeMb", "{{size}} MB", { size: formatNumber(byteSize / BYTES_PER_MB, 1) });
  }
  const kb = Math.max(MIN_DISPLAY_KB, byteSize / BYTES_PER_KB);
  return t("splitExport.fileSizeKb", "{{size}} KB", { size: formatNumber(kb, kb >= KB_DECIMAL_THRESHOLD ? 0 : 1) });
}

function formatFidelityWarning(
  warning: PptxFidelityWarning,
  formatNumber: FormatNumberFn,
  t: TranslateFn
): string {
  const entry = WARNING_I18N_MAP[warning.code];
  return t(entry.key, entry.fallback, { slide: formatNumber(warning.slideIndex) });
}

export interface SharePresentPanelProps {
  activeFormat: SplitExportActiveFormat;
  disabled?: boolean;
  htmlResult: HtmlBundleExportData | null;
  pdfResult: VectorPdfExportData | null;
  pptxResult: PptxExportData | null;
  pptxPluginInstalled: boolean;
  pptxExportMode?: PptxExportMode;
  onPptxExportModeChange?: (mode: PptxExportMode) => void;
  onExportOfflineHtml: () => void;
  onExportVectorPdf: () => void;
  onExportPptx: (mode?: PptxExportMode) => void;
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
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  cursor: "pointer",
};

const secondaryBtnStyle: React.CSSProperties = {
  backgroundColor: "var(--bg-subtle)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-strong)",
  borderRadius: "var(--radius-sm)",
  padding: "8px 14px",
  fontSize: "13px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  cursor: "pointer",
};

const pillBadgeStyle: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: "6px",
  fontSize: "11.5px",
  fontWeight: FONT_WEIGHT_SEMIBOLD,
  padding: "3px 9px",
  borderRadius: "var(--radius-pill)",
  backgroundColor: "var(--state-success-soft)",
  color: "var(--state-success)",
};

const titleStyle: React.CSSProperties = { margin: "4px 0 0", fontSize: "14px", color: "var(--text-primary)" };
const descStyle: React.CSSProperties = { margin: 0, fontSize: "12.5px", color: "var(--text-secondary)", lineHeight: LINE_HEIGHT_NORMAL };
const receiptBoxStyle: React.CSSProperties = {
  backgroundColor: "var(--bg-subtle)",
  borderRadius: "var(--radius-sm)",
  padding: "8px 10px",
  fontSize: "11.5px",
  color: "var(--text-secondary)",
  display: "flex",
  flexDirection: "column",
  gap: "4px",
};

const OfflineHtmlCard: React.FC<{
  htmlResult: HtmlBundleExportData | null;
  activeFormat: SplitExportActiveFormat;
  isBusy: boolean;
  onExport: () => void;
  formatNumber: FormatNumberFn;
  t: TranslateFn;
}> = ({ htmlResult, activeFormat, isBusy, onExport, formatNumber, t }) => (
  <article style={cardStyle} data-testid="export-card-offline-html">
    <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
      <span style={pillBadgeStyle}>{t("splitExport.offlineHtmlBadge", "100% Ngoại tuyến • 16:9")}</span>
      <h4 style={titleStyle}>{t("splitExport.offlineHtmlTitle", "Trình chiếu HTML 1 tệp")}</h4>
      <p style={descStyle}>
        {t("splitExport.offlineHtmlDesc", "Mở trực tiếp bằng trình duyệt kể cả khi mất mạng. Hỗ trợ phím mũi tên chuyển trang và toàn màn hình.")}
      </p>
    </div>
    {htmlResult && (
      <div style={receiptBoxStyle} data-testid="offline-html-receipt">
        <strong style={{ color: "var(--state-success)" }}>
          {t("splitExport.offlineHtmlReady", "Đã xuất thành công:")} {htmlResult.fileName}
        </strong>
        <span>
          {formatHumanFileSize(htmlResult.byteSize, formatNumber, t)} • {formatNumber(htmlResult.slideCount)}{" "}
          {t("splitExport.slidesUnit", "trang")} •{" "}
          <span style={{ color: htmlResult.externalCdnRefsCount === 0 ? "var(--state-success)" : "var(--state-warning)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
            {htmlResult.externalCdnRefsCount === 0
              ? t("splitExport.offlineReadyStatus", "Sẵn sàng chạy ngoại tuyến")
              : t("splitExport.externalLinksWarning", "Còn {{count}} liên kết ngoài", { count: formatNumber(htmlResult.externalCdnRefsCount) })}
          </span>
        </span>
      </div>
    )}
    <button type="button" style={primaryBtnStyle} disabled={isBusy} onClick={onExport} data-ff-id="export-offline-html-btn" data-testid="export-offline-html-btn">
      {activeFormat === "html" ? t("splitExport.packagingBtn", "Đang đóng gói...") : t("splitExport.downloadOfflineHtmlBtn", "Tải tệp trình chiếu (.html)")}
    </button>
  </article>
);

const VectorPdfCard: React.FC<{
  pdfResult: VectorPdfExportData | null;
  activeFormat: SplitExportActiveFormat;
  isBusy: boolean;
  onExport: () => void;
  formatNumber: FormatNumberFn;
  t: TranslateFn;
}> = ({ pdfResult, activeFormat, isBusy, onExport, formatNumber, t }) => (
  <article style={cardStyle} data-testid="export-card-vector-pdf">
    <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
      <span style={{ ...pillBadgeStyle, backgroundColor: "var(--human-soft)", color: "var(--human-accent)" }}>
        {t("splitExport.vectorPdfBadge", "Vector 16:9 Sắc nét")}
      </span>
      <h4 style={titleStyle}>{t("splitExport.vectorPdfTitle", "Tài liệu PDF chuẩn in ấn")}</h4>
      <p style={descStyle}>
        {t("splitExport.vectorPdfDesc", "Giữ nguyên độ sắc nét của chữ và hình khối vector ở tỉ lệ màn ảnh rộng 16:9 khi gửi email hoặc in.")}
      </p>
    </div>
    {pdfResult && (
      <div style={receiptBoxStyle} data-testid="vector-pdf-receipt">
        <strong style={{ color: "var(--state-success)" }}>{pdfResult.fileName}</strong>{" "}
        ({formatNumber(pdfResult.pageCount)} {t("splitExport.pagesUnit", "trang")})
      </div>
    )}
    <button type="button" style={secondaryBtnStyle} disabled={isBusy} onClick={onExport} data-ff-id="export-vector-pdf-btn" data-testid="export-vector-pdf-btn">
      {activeFormat === "pdf" ? t("splitExport.packagingBtn", "Đang đóng gói...") : t("splitExport.downloadVectorPdfBtn", "Xuất PDF 16:9 (.pdf)")}
    </button>
  </article>
);

const PptxModeSelector: React.FC<{
  mode: PptxExportMode;
  disabled: boolean;
  onSelect: (mode: PptxExportMode) => void;
  t: TranslateFn;
}> = ({ mode, disabled, onSelect, t }) => {
  const optionStyle = (selected: boolean): React.CSSProperties => ({
    display: "flex", alignItems: "flex-start", gap: "8px", textAlign: "left", padding: "8px 10px",
    borderRadius: "var(--radius-sm)",
    border: selected ? "1px solid var(--action-primary)" : "1px solid var(--border-subtle)",
    backgroundColor: selected ? "var(--bg-subtle)" : "var(--bg-surface)",
    color: "var(--text-primary)", cursor: disabled ? "not-allowed" : "pointer", width: "100%",
  });
  const onKeyToggle = (ev: React.KeyboardEvent) => {
    if (ev.key === "ArrowDown" || ev.key === "ArrowRight" || ev.key === "ArrowUp" || ev.key === "ArrowLeft") {
      ev.preventDefault();
      onSelect(mode === "editable" ? "visual_exact" : "editable");
    }
  };
  return (
    <div role="radiogroup" aria-label={t("splitExport.pptxModeGroupAria", "Chọn chế độ xuất PowerPoint")} data-ff-id="export-pptx-mode-group" style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
      <button type="button" role="radio" aria-checked={mode === "editable"} disabled={disabled} onClick={() => onSelect("editable")} onKeyDown={onKeyToggle} style={optionStyle(mode === "editable")} data-ff-id="export-pptx-mode-editable" data-testid="pptx-mode-editable">
        <BrandIcon name="edit" size={ICON_SM} style={{ marginTop: "2px", flexShrink: 0 }} />
        <span style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
          <strong style={{ fontSize: "12px" }}>{t("splitExport.pptxModeEditableTitle", "Chỉnh sửa được (Khuyên dùng)")}</strong>
          <span style={{ fontSize: "11px", color: "var(--text-secondary)", lineHeight: LINE_HEIGHT_TIGHT }}>
            {t("splitExport.pptxModeEditableDesc", "Giữ chữ, bảng, bố cục 16:9 và biểu tượng sắc nét để sửa trực tiếp trong PowerPoint.")}
          </span>
        </span>
      </button>
      <button type="button" role="radio" aria-checked={mode === "visual_exact"} disabled={disabled} onClick={() => onSelect("visual_exact")} onKeyDown={onKeyToggle} style={optionStyle(mode === "visual_exact")} data-ff-id="export-pptx-mode-visual-exact" data-testid="pptx-mode-visual-exact">
        <BrandIcon name="eye" size={ICON_SM} style={{ marginTop: "2px", flexShrink: 0 }} />
        <span style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
          <strong style={{ fontSize: "12px" }}>{t("splitExport.pptxModeVisualExactTitle", "Giống hệt màn hình (Ảnh nét cao)")}</strong>
          <span style={{ fontSize: "11px", color: "var(--text-secondary)", lineHeight: LINE_HEIGHT_TIGHT }}>
            {t("splitExport.pptxModeVisualExactDesc", "Giữ 100% kính mờ, đổ bóng và phông chữ nguyên bản (không sửa được chữ bên trong).")}
          </span>
        </span>
      </button>
    </div>
  );
};

const PptxLimitationsNotice: React.FC<{ t: TranslateFn }> = ({ t }) => (
  <div
    style={{
      backgroundColor: "var(--bg-subtle)",
      border: "1px solid var(--border-subtle)",
      borderRadius: "var(--radius-sm)",
      padding: "10px",
      fontSize: "11.5px",
      color: "var(--text-secondary)",
      display: "flex",
      flexDirection: "column",
      gap: "6px",
      lineHeight: LINE_HEIGHT_SNUG,
    }}
    data-ff-id="export-pptx-limitations-notice"
    data-testid="pptx-limitations-notice"
  >
    <div style={{ display: "flex", alignItems: "center", gap: "6px", color: "var(--text-primary)", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
      <BrandIcon name="info" size={ICON_SM} />
      <span>{t("splitExport.pptxLimitationsTitle", "Lưu ý khi xuất sang PowerPoint")}</span>
    </div>
    <div style={{ display: "flex", alignItems: "flex-start", gap: "6px" }}>
      <BrandIcon name="check-circle" size={ICON_SM} color="var(--state-success)" style={{ marginTop: "2px", flexShrink: 0 }} />
      <span>
        <strong style={{ color: "var(--text-primary)" }}>{t("splitExport.pptxLimitationsKeptLabel", "Được giữ nguyên:")}</strong>{" "}
        {t("splitExport.pptxLimitationsKeptDesc", "Văn bản, bố cục 16:9, bảng biểu, màu sắc, biểu tượng sắc nét và hiệu ứng chuyển trang cơ bản (Mờ dần, Trượt, Morph).")}
      </span>
    </div>
    <div style={{ display: "flex", alignItems: "flex-start", gap: "6px" }}>
      <BrandIcon name="warning" size={ICON_SM} color="var(--state-warning)" style={{ marginTop: "2px", flexShrink: 0 }} />
      <span>
        <strong style={{ color: "var(--text-primary)" }}>{t("splitExport.pptxLimitationsWebOnlyLabel", "Hiệu ứng web nâng cao:")}</strong>{" "}
        {t("splitExport.pptxLimitationsWebOnlyDesc", "Nền kính mờ, chuyển động CSS tùy biến, hiệu ứng di chuột và biểu đồ/3D tương tác không có trong PowerPoint nên sẽ tự động tối ưu thành hình phẳng hoặc ảnh nét cao.")}
      </span>
    </div>
    <p style={{ margin: 0, color: "var(--text-primary)", fontWeight: FONT_WEIGHT_MEDIUM }}>
      {t("splitExport.pptxLimitationsHtmlTip", "Mẹo nhanh: Nếu cần trình chiếu giữ 100% hiệu ứng động và tương tác mượt mà, hãy dùng Trình chiếu HTML 1 tệp bên cạnh.")}
    </p>
  </div>
);

const PptxFidelitySummary: React.FC<{
  pptxResult: PptxExportData;
  formatNumber: FormatNumberFn;
  t: TranslateFn;
}> = ({ pptxResult, formatNumber, t }) => {
  const warnings = (pptxResult.fidelityWarnings ?? []).filter((w) => w.code !== "STATIC_LAYOUT_FALLBACK");
  const slidesCount = pptxResult.slideCount ?? 1;
  const modeLabel = pptxResult.exportMode === "visual_exact"
    ? t("splitExport.pptxModeVisualExactTitle", "Giống hệt màn hình (Ảnh nét cao)")
    : t("splitExport.pptxModeEditableTitle", "Chỉnh sửa được (Khuyên dùng)");

  return (
    <div style={receiptBoxStyle} data-ff-id="export-pptx-fidelity-summary" data-testid="pptx-fidelity-warnings">
      <strong style={{ color: "var(--state-success)" }}>
        {pptxResult.fileName} • {modeLabel}
      </strong>
      <span>
        {t("splitExport.pptxReadySummary", "Đã xuất {{slides}} trang (.pptx) • {{size}}", {
          slides: formatNumber(slidesCount),
          size: formatHumanFileSize(pptxResult.byteSize, formatNumber, t),
        })}
      </span>
      {pptxResult.usedStaticFallback && (
        <span style={{ color: "var(--text-secondary)" }}>
          {t("splitExport.pptxStaticFallbackNotice", "Đã dùng chế độ bố cục dự phòng nhanh vì trình duyệt dựng hình cục bộ chưa sẵn sàng.")}
        </span>
      )}
      {warnings.length === 0 ? (
        <span style={{ color: "var(--state-success)" }}>
          {t("splitExport.pptxCleanFidelityNote", "Tất cả trang đã được chuyển đổi trọn vẹn không có cảnh báo tương thích.")}
        </span>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "3px", marginTop: "2px" }}>
          <strong style={{ color: "var(--state-warning)" }}>
            {t("splitExport.pptxFidelityWarningsHeading", "Lưu ý tối ưu tương thích ({{count}} mục):", {
              count: formatNumber(warnings.length),
            })}
          </strong>
          <ul style={{ margin: 0, paddingLeft: "16px", display: "flex", flexDirection: "column", gap: "2px" }}>
            {warnings.map((w, idx) => (
              <li key={`${w.slideIndex}-${w.code}-${idx}`}>{formatFidelityWarning(w, formatNumber, t)}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
};

const PptxLegacyInstallGuide: React.FC<{
  installAction: string;
  onNavigatePluginInstall?: (installAction: string) => void;
  t: TranslateFn;
}> = ({ installAction, onNavigatePluginInstall, t }) => {
  const safeInstallAction = SAFE_RELATIVE_INSTALL_ACTION_REGEX.test(installAction) ? installAction : "/plugins";
  return (
    <div style={{ backgroundColor: "var(--state-warning-soft)", border: "1px solid var(--state-warning)", borderRadius: "var(--radius-sm)", padding: "10px", fontSize: "12px", color: "var(--text-primary)", display: "flex", flexDirection: "column", gap: "8px" }} data-testid="pptx-plugin-install-guide">
      <span>{t("splitExport.pptxNotInstalledDesc", "Tiện ích xuất PowerPoint chưa được cài đặt. Cài nhanh trong Kho Tiện Ích để mở khóa tính năng này.")}</span>
      <a
        href={safeInstallAction}
        onClick={(ev) => {
          if (onNavigatePluginInstall) {
            ev.preventDefault();
            onNavigatePluginInstall(safeInstallAction);
          }
        }}
        style={{ ...primaryBtnStyle, textDecoration: "none", textAlign: "center", display: "inline-block" }}
        data-ff-id="export-pptx-plugin-cta"
        data-testid="export-pptx-plugin-cta"
      >
        {t("splitExport.installPptxPluginBtn", "Cài tiện ích PowerPoint")}
      </a>
    </div>
  );
};

const PptxExportCard: React.FC<{
  pptxResult: PptxExportData | null;
  pptxPluginInstalled: boolean;
  controlledMode?: PptxExportMode;
  onModeChange?: (mode: PptxExportMode) => void;
  activeFormat: SplitExportActiveFormat;
  isBusy: boolean;
  onExportPptx: (mode?: PptxExportMode) => void;
  onNavigatePluginInstall?: (installAction: string) => void;
  formatNumber: FormatNumberFn;
  t: TranslateFn;
}> = ({ pptxResult, pptxPluginInstalled, controlledMode, onModeChange, activeFormat, isBusy, onExportPptx, onNavigatePluginInstall, formatNumber, t }) => {
  const [localMode, setLocalMode] = useState<PptxExportMode>("editable");
  const effectiveMode = controlledMode ?? localMode;
  const handleModeChange = (next: PptxExportMode) => {
    setLocalMode(next);
    onModeChange?.(next);
  };
  const showInstallGuide = pptxResult !== null && !pptxResult.pluginInstalled;

  return (
    <article style={cardStyle} data-testid="export-card-pptx">
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: "14px", alignItems: "start" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <span style={pillBadgeStyle} data-plugin-active={pptxPluginInstalled ? "true" : "false"}>
            <BrandIcon name="layers" size={ICON_XS} />
            {t("splitExport.pptxBuiltInBadge", "Tích hợp sẵn • 16:9")}
          </span>
          <h4 style={titleStyle}>{t("splitExport.pptxTitle", "Tệp PowerPoint (.pptx)")}</h4>
          <p style={descStyle}>
            {t("splitExport.pptxBuiltInDesc", "Xuất trực tiếp tệp trình chiếu tương thích PowerPoint, Keynote và Google Slides mà không cần cài thêm phần mềm.")}
          </p>
          <PptxModeSelector mode={effectiveMode} disabled={isBusy} onSelect={handleModeChange} t={t} />
          <button type="button" style={primaryBtnStyle} disabled={isBusy} onClick={() => onExportPptx(effectiveMode)} data-ff-id="export-pptx-btn" data-testid="export-pptx-btn">
            {activeFormat === "pptx" ? t("splitExport.pptxExportingBtn", "Đang tạo tệp PowerPoint...") : t("splitExport.exportPptxBtn", "Xuất PowerPoint (.pptx)")}
          </button>
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <PptxLimitationsNotice t={t} />
          {showInstallGuide && <PptxLegacyInstallGuide installAction={pptxResult.installAction} onNavigatePluginInstall={onNavigatePluginInstall} t={t} />}
          {pptxResult && pptxResult.pluginInstalled && <PptxFidelitySummary pptxResult={pptxResult} formatNumber={formatNumber} t={t} />}
        </div>
      </div>
    </article>
  );
};

export const SharePresentPanel: React.FC<SharePresentPanelProps> = ({
  activeFormat,
  disabled = false,
  htmlResult,
  pdfResult,
  pptxResult,
  pptxPluginInstalled,
  pptxExportMode,
  onPptxExportModeChange,
  onExportOfflineHtml,
  onExportVectorPdf,
  onExportPptx,
  onNavigatePluginInstall,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const isBusy = activeFormat !== null || disabled;

  return (
    <section style={{ display: "flex", flexDirection: "column", gap: "12px" }} aria-label={t("splitExport.sharePresentTab", "Chia sẻ & Trình chiếu")} data-ff-id="share-present-panel" data-testid="share-present-panel">
      <p style={{ margin: 0, fontSize: "13px", color: "var(--text-secondary)", lineHeight: LINE_HEIGHT_RELAXED }}>
        {t("splitExport.sharePresentSubtitle", "Xuất bản tức thì để trình chiếu, gửi đối tác hoặc in ấn sắc nét — không cần kiến thức lập trình.")}
      </p>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: "12px" }}>
        <OfflineHtmlCard htmlResult={htmlResult} activeFormat={activeFormat} isBusy={isBusy} onExport={onExportOfflineHtml} formatNumber={formatNumber} t={t} />
        <VectorPdfCard pdfResult={pdfResult} activeFormat={activeFormat} isBusy={isBusy} onExport={onExportVectorPdf} formatNumber={formatNumber} t={t} />
      </div>
      <PptxExportCard
        pptxResult={pptxResult}
        pptxPluginInstalled={pptxPluginInstalled}
        controlledMode={pptxExportMode}
        onModeChange={onPptxExportModeChange}
        activeFormat={activeFormat}
        isBusy={isBusy}
        onExportPptx={onExportPptx}
        onNavigatePluginInstall={onNavigatePluginInstall}
        formatNumber={formatNumber}
        t={t}
      />
    </section>
  );
};
