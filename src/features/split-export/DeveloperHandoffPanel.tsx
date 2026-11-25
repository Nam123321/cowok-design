// @story 07.2
// @implements AC-02
import React from "react";
import { useFormatter } from "../../hooks/useFormatter";
import { useTranslation } from "../../hooks/useTranslation";
import type { DeveloperHandoffData } from "../../lib/validations/split-export";
import type { SplitExportActiveFormat } from "./useSplitExport";

export interface DeveloperHandoffPanelProps {
  activeFormat: SplitExportActiveFormat;
  disabled?: boolean;
  hasProjectId: boolean;
  devHandoffResult: DeveloperHandoffData | null;
  onExportDeveloperHandoff: () => void;
  children?: React.ReactNode;
}

const SHORT_SHA_LEN = 12;

export const DeveloperHandoffPanel: React.FC<DeveloperHandoffPanelProps> = ({
  activeFormat,
  disabled = false,
  hasProjectId,
  devHandoffResult,
  onExportDeveloperHandoff,
  children,
}) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });

  return (
    <section
      style={{ display: "flex", flexDirection: "column", gap: "14px" }}
      aria-label={t("splitExport.developerHandoffTab", "Bàn giao Lập trình viên")}
      data-ff-id="developer-handoff-panel"
      data-testid="developer-handoff-panel"
    >
      {hasProjectId && (
        <div
          style={{
            backgroundColor: "var(--bg-surface)",
            border: "1px solid var(--border-subtle)",
            borderRadius: "var(--radius-md)",
            padding: "14px 16px",
            display: "flex",
            flexDirection: "column",
            gap: "10px",
          }}
          data-testid="dev-handoff-bundle-card"
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              gap: "12px",
              flexWrap: "wrap",
            }}
          >
            <div>
              <h4
                style={{
                  margin: 0,
                  fontSize: "14px",
                  color: "var(--text-primary)",
                }}
              >
                {t(
                  "splitExport.devHandoffTitle",
                  "Gói bàn giao kỹ thuật (Spec, Tokens, React TSX & E2E)"
                )}
              </h4>
              <p
                style={{
                  margin: "4px 0 0",
                  fontSize: "12.5px",
                  color: "var(--text-secondary)",
                }}
              >
                {t(
                  "splitExport.devHandoffDesc",
                  "Đóng gói đầy đủ SPEC.md, DESIGN_TOKENS.css/.json, Screen.tsx và kịch bản kiểm thử Playwright kèm mã kiểm chứng SHA-256."
                )}
              </p>
            </div>
            <button
              type="button"
              disabled={activeFormat !== null || disabled}
              onClick={onExportDeveloperHandoff}
              style={{
                backgroundColor: "var(--action-primary)",
                color: "var(--action-primary-fg)",
                border: "1px solid var(--action-primary)",
                borderRadius: "var(--radius-sm)",
                padding: "8px 14px",
                fontSize: "13px",
                fontWeight: 600,
                cursor: "pointer",
              }}
              data-ff-id="export-dev-handoff-btn"
              data-testid="export-dev-handoff-btn"
            >
              {activeFormat === "developer-handoff"
                ? t("splitExport.packagingBtn", "Đang đóng gói...")
                : t("splitExport.downloadDevHandoffBtn", "Tải gói bàn giao Dev")}
            </button>
          </div>

          {devHandoffResult && (
            <div
              style={{
                backgroundColor: "var(--bg-subtle)",
                borderRadius: "var(--radius-sm)",
                padding: "10px 12px",
                fontSize: "12px",
                color: "var(--text-secondary)",
                display: "flex",
                flexDirection: "column",
                gap: "6px",
              }}
              data-testid="dev-handoff-receipt"
            >
              <div
                style={{
                  display: "flex",
                  justifyContent: "space-between",
                  flexWrap: "wrap",
                  gap: "8px",
                }}
              >
                <strong style={{ color: "var(--state-success)" }}>
                  {devHandoffResult.fileName}
                </strong>
                <span>
                  {t("splitExport.byteSizeLabel", "{{size}} bytes", {
                    size: formatNumber(devHandoffResult.byteSize),
                  })}{" "}
                  • {t("splitExport.shaChecksumLabel", "SHA-256")}{" "}
                  <code>{devHandoffResult.sha256.slice(0, SHORT_SHA_LEN)}</code>
                </span>
              </div>
              <ul
                style={{
                  margin: 0,
                  paddingLeft: "18px",
                  display: "flex",
                  flexDirection: "column",
                  gap: "2px",
                }}
              >
                {devHandoffResult.files.map((file) => (
                  <li key={file.path}>
                    <code>{file.path}</code> (
                    {t("splitExport.byteSizeLabel", "{{size}} bytes", {
                      size: formatNumber(file.byteSize),
                    })}
                    )
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {children}
    </section>
  );
};
