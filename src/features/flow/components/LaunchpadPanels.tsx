// @story 05.1, 06.1
// @implements AC-06 (4-Path Launchpad Panel Subcomponents — Full 9-Locale Formatting Guardian)
import React from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";

const TEMPLATE_COUNT_FINTECH = 5;
const TEMPLATE_COUNT_ECOMMERCE = 4;
const TEMPLATE_COUNT_SAAS = 6;
const TEMPLATE_COUNT_DECK = 4;
const PRD_ROWS = 5;
const CLIPBOARD_ROWS = 3;
const ICON_SIZE_SM = 12;
const ICON_SIZE_LG = 20;
const STROKE_WIDTH_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;
const MAX_CLIPBOARD_SNIPPET_LEN = 8000;
const TEXT_DESIGN_EXT_RE = /\.(svg|json|html|htm|css|txt|md)$/i;

export const TEMPLATE_PRESETS = [
  {
    id: "tpl-fintech",
    titleKey: "launchpad.tplFintechTitle",
    titleFallback: "FinTech Multi-Sig Treasury",
    categoryKey: "launchpad.tplFintechCat",
    categoryFallback: "Security & Payments",
    badgeKey: "launchpad.tplFintechBadge",
    badgeFallback: "Option 5 Flow",
    screens: TEMPLATE_COUNT_FINTECH,
    descKey: "launchpad.tplFintechDesc",
    descFallback: "Multi-signature escrow approval with WebAuthn Passkey biometrics and audit trail.",
  },
  {
    id: "tpl-ecommerce",
    titleKey: "launchpad.tplEcommerceTitle",
    titleFallback: "E-Commerce Checkout & Subscriptions",
    categoryKey: "launchpad.tplEcommerceCat",
    categoryFallback: "Commerce",
    badgeKey: "launchpad.tplEcommerceBadge",
    badgeFallback: "4-SubState",
    screens: TEMPLATE_COUNT_ECOMMERCE,
    descKey: "launchpad.tplEcommerceDesc",
    descFallback: "Cart reconciliation, address selector, payment intent confirmation, and receipt flow.",
  },
  {
    id: "tpl-saas-rbac",
    titleKey: "launchpad.tplSaasTitle",
    titleFallback: "SaaS Multi-Tenant Workspace & RBAC",
    categoryKey: "launchpad.tplSaasCat",
    categoryFallback: "Enterprise",
    badgeKey: "launchpad.tplSaasBadge",
    badgeFallback: "PBAC Guard",
    screens: TEMPLATE_COUNT_SAAS,
    descKey: "launchpad.tplSaasDesc",
    descFallback: "Workspace onboarding, team invitation, role assignment, and organization settings.",
  },
  {
    id: "tpl-slide-deck",
    titleKey: "launchpad.tplDeckTitle",
    titleFallback: "Interactive Slide Deck & Pitch Studio",
    categoryKey: "launchpad.tplDeckCat",
    categoryFallback: "Presentation",
    badgeKey: "launchpad.tplDeckBadge",
    badgeFallback: "16:9 Studio",
    screens: TEMPLATE_COUNT_DECK,
    descKey: "launchpad.tplDeckDesc",
    descFallback: "Phase walkthrough slides with interactive live data charts and presenter notes.",
  },
] as const;

const FIELD_STYLE: React.CSSProperties = {
  width: "100%",
  padding: "8px 10px",
  fontSize: "11px",
  fontFamily: "var(--font-mono)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-sm)",
  boxSizing: "border-box",
};

export const LaunchpadPrdPanel: React.FC<{
  prdText: string;
  prdFileName: string | null;
  onChangeText: (v: string) => void;
  onSelectFile?: (fileName: string, content: string) => void;
  onLoadSamplePrd: () => void;
}> = ({ prdText, prdFileName, onChangeText, onSelectFile, onLoadSamplePrd }) => {
  const { t } = useTranslation();
  const handleFileInput = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file || typeof FileReader === "undefined") return;
    if (!/\.(md|txt|json)$/i.test(file.name)) {
      const marker = `[File: ${file.name}]`;
      if (onSelectFile) onSelectFile(file.name, marker);
      else onChangeText(marker);
      return;
    }
    const reader = new FileReader();
    reader.onload = () => {
      const content = typeof reader.result === "string" ? reader.result : "";
      if (onSelectFile) onSelectFile(file.name, content);
      else onChangeText(content);
    };
    reader.readAsText(file);
  };
  return (
    <div data-testid="launchpad-panel-prd" style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <div style={{ fontSize: "12px", color: "var(--text-secondary)" }}>{t("launchpad.prdDesc", "Nhập mô tả sản phẩm, user story, hoặc tải lên file Markdown / PDF:")}</div>
      <label data-ff-id="cas-file-dropzone" data-testid="launchpad-prd-dropzone" style={{ border: "1px dashed var(--border-strong)", borderRadius: "var(--radius-md)", padding: "16px", textAlign: "center", backgroundColor: "var(--bg-canvas)", cursor: "pointer", display: "block" }}>
        <input data-testid="launchpad-prd-file-input" type="file" accept=".csv,.tsv,.xlsx,.fig,.md,.txt,.pdf,.json,.png,.svg" onChange={handleFileInput} style={{ position: "absolute", width: "1px", height: "1px", opacity: 0, overflow: "hidden" }} />
        <BrandIcon name="upload" size={ICON_SIZE_LG} strokeWidth={STROKE_WIDTH_MONO} color="var(--action-primary)" />
        <div style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", marginTop: "6px" }}>
          {prdFileName ? `${t("launchpad.selectedFilePrefix", "Selected file:")} ${prdFileName}` : t("launchpad.dropPrdPrompt", "Drop or browse PRD file (.md / .txt / .pdf)")}
        </div>
      </label>
      <button type="button" data-testid="launchpad-load-sample-prd-btn" onClick={onLoadSamplePrd} style={{ alignSelf: "flex-start", padding: "4px 10px", fontSize: "10.5px", borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-secondary)", cursor: "pointer" }}>
        {t("launchpad.loadSamplePrdBtn", "Load Sample PRD")}
      </button>
      <textarea data-testid="launchpad-prd-textarea" rows={PRD_ROWS} placeholder={t("launchpad.prdPlaceholder", "Dán nội dung Product Requirements Document hoặc prompt khởi tạo luồng vào đây...")} value={prdText} onChange={(e) => onChangeText(e.target.value)} style={{ ...FIELD_STYLE, resize: "vertical" }} />
    </div>
  );
};

export const LaunchpadTemplatesPanel: React.FC<{
  selectedTemplateId: string;
  onSelectTemplate: (id: string) => void;
}> = ({ selectedTemplateId, onSelectTemplate }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="launchpad-panel-templates" style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      <div style={{ fontSize: "12px", color: "var(--text-secondary)" }}>{t("launchpad.templatesDesc", "Chọn từ thư viện starter templates chuẩn kiến trúc Flow-First:")}</div>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "10px" }}>
        {TEMPLATE_PRESETS.map((tpl) => {
          const isSelected = selectedTemplateId === tpl.id;
          return (
            <div key={tpl.id} data-testid={`launchpad-tpl-${tpl.id}`} onClick={() => onSelectTemplate(tpl.id)} style={{ padding: "12px", borderRadius: "var(--radius-md)", border: isSelected ? "2px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: isSelected ? "var(--bg-selected)" : "var(--bg-canvas)", cursor: "pointer" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: "10px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--action-primary)" }}>{t(tpl.categoryKey, tpl.categoryFallback)}</span>
                <span style={{ fontSize: "8.5px", fontFamily: "var(--font-mono)", padding: "1px 5px", borderRadius: "var(--radius-xs)", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-subtle)", color: "var(--text-muted)" }}>{t(tpl.badgeKey, tpl.badgeFallback)}</span>
              </div>
              <div style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", margin: "4px 0" }}>{t(tpl.titleKey, tpl.titleFallback)}</div>
              <div style={{ fontSize: "10.5px", color: "var(--text-muted)", lineHeight: "1.4" }}>{t(tpl.descKey, tpl.descFallback)}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
};

export const LaunchpadDesignPanel: React.FC<{
  designUrl: string;
  clipboardSnippet: string;
  extractedTokens: string | null;
  onChangeUrl: (v: string) => void;
  onChangeClipboard: (v: string) => void;
  onExtractTokens: () => void;
}> = ({ designUrl, clipboardSnippet, extractedTokens, onChangeUrl, onChangeClipboard, onExtractTokens }) => {
  const { t } = useTranslation();
  const handleDesignFile = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (!TEXT_DESIGN_EXT_RE.test(file.name) || typeof FileReader === "undefined") {
      onChangeClipboard(`[File: ${file.name}]`);
      return;
    }
    const reader = new FileReader();
    reader.onload = () => {
      const content = typeof reader.result === "string" ? reader.result : `[File: ${file.name}]`;
      onChangeClipboard(content.slice(0, MAX_CLIPBOARD_SNIPPET_LEN));
    };
    reader.readAsText(file);
  };
  return (
    <div data-testid="launchpad-panel-design" style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <div style={{ fontSize: "12px", color: "var(--text-secondary)" }}>{t("launchpad.designDesc", "Nhập URL file Figma, nạp file .fig hoặc dán vector clipboard SVG/HTML:")}</div>
      <input data-testid="launchpad-design-file-input" type="file" accept=".fig,.svg,.json,.png,.csv,.tsv,.xlsx,.pdf,.txt" onChange={handleDesignFile} style={{ fontSize: "11px", color: "var(--text-secondary)" }} />
      <input data-testid="launchpad-figma-url-input" type="text" placeholder={t("launchpad.figmaUrlPlaceholder", "https://www.figma.com/design/... hoặc dán token")} value={designUrl} onChange={(e) => onChangeUrl(e.target.value)} style={FIELD_STYLE} />
      <textarea data-testid="launchpad-clipboard-textarea" rows={CLIPBOARD_ROWS} placeholder={t("launchpad.clipboardPlaceholder", "Dán clipboard mã nguồn SVG / HTML khung wireframe...")} value={clipboardSnippet} onChange={(e) => onChangeClipboard(e.target.value)} style={{ ...FIELD_STYLE, resize: "vertical" }} />
      <button type="button" data-testid="launchpad-extract-tokens-btn" onClick={onExtractTokens} style={{ alignSelf: "flex-start", padding: "6px 12px", fontSize: "11px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-workspace)", color: "var(--text-primary)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "6px" }}>
        <BrandIcon name="cpu" size={ICON_SIZE_SM} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
        <span>{t("launchpad.extractTokensBtn", "Deterministic Offline Token Extraction")}</span>
      </button>
      {extractedTokens && <pre data-testid="launchpad-extracted-tokens-preview" style={{ margin: 0, padding: "8px", backgroundColor: "var(--bg-canvas)", border: "1px solid var(--border-subtle)", borderRadius: "var(--radius-xs)", fontSize: "10px", fontFamily: "var(--font-mono)", color: "var(--state-success)", maxHeight: "100px", overflow: "auto" }}>{extractedTokens}</pre>}
    </div>
  );
};

export const LaunchpadGitPanel: React.FC<{
  gitRepoUrl: string;
  gitBranch: string;
  onChangeUrl: (v: string) => void;
  onChangeBranch: (v: string) => void;
}> = ({ gitRepoUrl, gitBranch, onChangeUrl, onChangeBranch }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="launchpad-panel-git" style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <div style={{ fontSize: "12px", color: "var(--text-secondary)" }}>{t("launchpad.gitDesc", "Quét mã nguồn repository hiện có để sinh luồng màn hình tự động:")}</div>
      <input data-testid="launchpad-git-url-input" type="text" placeholder={t("launchpad.gitUrlPlaceholder", "https://github.com/org/repo.git hoặc đường dẫn cục bộ")} value={gitRepoUrl} onChange={(e) => onChangeUrl(e.target.value)} style={FIELD_STYLE} />
      <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
        <span style={{ fontSize: "11px", fontWeight: FONT_WEIGHT_MEDIUM, color: "var(--text-muted)" }}>{t("launchpad.gitBranchLabel", "Branch:")}</span>
        <input data-testid="launchpad-git-branch-input" type="text" value={gitBranch} onChange={(e) => onChangeBranch(e.target.value)} style={{ ...FIELD_STYLE, width: "140px", padding: "4px 8px" }} />
      </div>
    </div>
  );
};
