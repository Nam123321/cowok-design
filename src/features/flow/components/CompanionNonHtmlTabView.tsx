// @story 04.1, 05.1
import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type { CanvasTabItem } from "./CompanionCanvasTab";

const FONT_WEIGHT_SEMIBOLD = 600;

export const NonHtmlTabView: React.FC<{ activeTab: CanvasTabItem; inlineCodeText: string }> = ({
  activeTab,
  inlineCodeText,
}) => {
  const { t } = useTranslation();
  if (activeTab.format === "md") {
    return (
      <div
        data-quotable="true"
        style={{
          flex: 1,
          padding: "24px",
          boxSizing: "border-box",
          overflowY: "auto",
          backgroundColor: "var(--bg-surface)",
          color: "var(--text-primary)",
        }}
      >
        <h3 style={{ margin: "0 0 12px 0", fontSize: "16px", fontWeight: FONT_WEIGHT_SEMIBOLD }}>
          # {activeTab.title} — {t("canvas.mdDocTitleSuffix", "Tài liệu Đặc tả Thiết kế Hệ thống (SSOT)")}
        </h3>
        <p style={{ color: "var(--text-secondary)", fontSize: "13px", lineHeight: "1.6" }}>
          {t(
            "canvas.mdDocQuoteHintPrefix",
            "Bạn có thể bôi đen bất kỳ đoạn văn bản nào trong tài liệu Markdown này để trích dẫn"
          )}{" "}
          <code>@[Quote]</code>{" "}
          {t("canvas.mdDocQuoteHintSuffix", "trực tiếp vào khung trò chuyện cowok Agent.")}
        </p>
      </div>
    );
  }
  if (activeTab.format === "assets") {
    return (
      <div
        style={{
          flex: 1,
          padding: "24px",
          boxSizing: "border-box",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          backgroundColor: "var(--bg-canvas)",
          color: "var(--text-primary)",
          gap: "12px",
        }}
      >
        <div style={{ fontSize: "12px", fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>
          {activeTab.filePath || activeTab.title}
        </div>
        <div
          style={{
            padding: "24px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--border-default)",
            backgroundColor: "var(--bg-surface)",
          }}
        >
          <code>
            [{t("canvas.assetPreviewPrefix", "Asset Preview")}: {activeTab.title}]
          </code>
        </div>
      </div>
    );
  }
  return (
    <div
      style={{
        flex: 1,
        padding: "20px",
        boxSizing: "border-box",
        backgroundColor: "var(--bg-canvas)",
        color: "var(--text-primary)",
        fontFamily: "var(--font-mono)",
        fontSize: "12px",
        overflowY: "auto",
      }}
    >
      <div style={{ color: "var(--text-muted)", marginBottom: "8px" }}>// {activeTab.filePath || activeTab.title}</div>
      <pre
        data-testid="non-html-code-pre"
        style={{
          margin: 0,
          lineHeight: "1.55",
          overflowX: "auto",
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
        }}
      >
        {inlineCodeText}
      </pre>
    </div>
  );
};
