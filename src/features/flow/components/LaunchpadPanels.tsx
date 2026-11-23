// @story 05.1, 06.1, 09.1
// @implements AC-06 (Ultra-Clean Chat-at-Center Launchpad Panels: Local Disk, Online Design Tools, GitHub, Unified awesome-re Library)
import React, { useState } from "react";
import { apiHarnessDaemon } from "../../../api/harness-daemon";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ReaCatalogItemDto, ReaIngestOutcomeDto, ReaSearchHitDto, ReaSourceFormat } from "../../../lib/validations/rea";
import {
  LaunchpadReaIngestSection,
  LaunchpadReaSearchSection,
} from "./LaunchpadReaPanel";

const ICON_SIZE_MD = 16;
const STROKE_WIDTH_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;
const MAX_UPLOAD_BYTES = 1_400_000;

const DESIGN_TOOL_CHIPS: ReadonlyArray<{
  id: string;
  label: string;
  placeholder: string;
  format: ReaSourceFormat;
}> = [
  { id: "figma", label: "Figma", placeholder: "https://www.figma.com/design/...", format: "url" },
  { id: "framer", label: "Framer", placeholder: "https://framer.com/projects/...", format: "url" },
  { id: "sketch", label: "Sketch", placeholder: "https://www.sketch.com/s/...", format: "url" },
  { id: "penpot", label: "Penpot", placeholder: "https://design.penpot.app/...", format: "pen" },
  { id: "web", label: "Web URL", placeholder: "https://example.com/landing", format: "url" },
];

const FIELD_STYLE: React.CSSProperties = {
  width: "100%",
  padding: "8px 10px",
  fontSize: "12px",
  fontFamily: "var(--font-sans)",
  backgroundColor: "var(--bg-workspace)",
  color: "var(--text-primary)",
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-sm)",
  boxSizing: "border-box",
};

const QUICK_CARD_STYLE: React.CSSProperties = {
  border: "1px solid var(--border-default)",
  borderRadius: "var(--radius-md)",
  padding: "12px",
  backgroundColor: "var(--bg-canvas)",
  display: "flex",
  flexDirection: "column",
  gap: "8px",
};

function readLocalSelectedFile(
  file: File,
  onSelectFile?: (fileName: string, content: string) => void
) {
  if (typeof FileReader === "undefined") return;
  if (!/\.(md|txt|json|svg|html|css)$/i.test(file.name)) {
    onSelectFile?.(file.name, `[File: ${file.name}]`);
    return;
  }
  const reader = new FileReader();
  reader.onload = () => {
    const content = typeof reader.result === "string" ? reader.result : "";
    onSelectFile?.(file.name, content);
  };
  reader.readAsText(file);
}

const LocalFolderCard: React.FC<{
  localFolderPath: string;
  onChangeFolderPath?: (v: string) => void;
}> = ({ localFolderPath, onChangeFolderPath }) => {
  const { t } = useTranslation();
  const [pickingFolder, setPickingFolder] = useState(false);

  const handlePickNativeFolder = async () => {
    setPickingFolder(true);
    try {
      const res = await apiHarnessDaemon.pickNativeFolder({ initial_path: localFolderPath || undefined });
      if (res.path && onChangeFolderPath) onChangeFolderPath(res.path);
    } catch {
      // User can type or create a folder path directly when native OS picker is unavailable
    } finally {
      setPickingFolder(false);
    }
  };

  return (
    <div style={QUICK_CARD_STYLE}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "6px" }}>
        <span style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="folder" size={ICON_SIZE_MD} strokeWidth={STROKE_WIDTH_MONO} color="var(--action-primary)" />
          <span>{t("launchpad.localFolderTitle", "Thư mục trên máy")}</span>
        </span>
        <button type="button" disabled={pickingFolder} data-testid="launchpad-pick-folder-btn" onClick={() => void handlePickNativeFolder()} style={{ padding: "4px 9px", fontSize: "11px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-primary)", cursor: "pointer" }}>
          {t("launchpad.pickFolderBtn", "Chọn thư mục")}
        </button>
      </div>
      <input data-testid="launchpad-local-folder-input" type="text" placeholder={t("launchpad.localFolderPlaceholder", "~/Projects/du-an-moi (chọn hoặc tạo mới)")} value={localFolderPath} onChange={(e) => onChangeFolderPath?.(e.target.value)} style={{ ...FIELD_STYLE, fontSize: "11px", fontFamily: "var(--font-mono)" }} />
    </div>
  );
};

export const LaunchpadPrdPanel: React.FC<{
  localFolderPath?: string;
  onChangeFolderPath?: (v: string) => void;
  prdFileName: string | null;
  onSelectFile?: (fileName: string, content: string) => void;
}> = ({ localFolderPath = "", onChangeFolderPath, prdFileName, onSelectFile }) => {
  const { t } = useTranslation();
  const [uploadError, setUploadError] = useState<string | null>(null);

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (file.size > MAX_UPLOAD_BYTES) {
      setUploadError(t("rea.uploadTooLarge", "Tệp vượt quá giới hạn tải lên 1,4 MB."));
      return;
    }
    setUploadError(null);
    readLocalSelectedFile(file, onSelectFile);
  };

  return (
    <div data-testid="launchpad-panel-prd" style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: "10px" }}>
        <LocalFolderCard localFolderPath={localFolderPath} onChangeFolderPath={onChangeFolderPath} />
        <label data-ff-id="cas-file-dropzone" data-testid="launchpad-prd-dropzone" style={{ ...QUICK_CARD_STYLE, borderStyle: "dashed", cursor: "pointer", justifyContent: "center" }}>
          <input data-testid="launchpad-prd-file-input" type="file" accept=".csv,.tsv,.xlsx,.fig,.zip,.pen,.wire,.md,.txt,.pdf,.json,.png,.svg" onChange={handleFileChange} style={{ position: "absolute", width: "1px", height: "1px", opacity: 0, overflow: "hidden" }} />
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <BrandIcon name="upload" size={ICON_SIZE_MD} strokeWidth={STROKE_WIDTH_MONO} color="var(--action-primary)" />
            <div style={{ display: "flex", flexDirection: "column", gap: "2px" }}>
              <span style={{ fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)" }}>
                {prdFileName ? `${t("launchpad.selectedFilePrefix", "Đã chọn:")} ${prdFileName}` : t("launchpad.localFileTitle", "Chọn tệp từ máy")}
              </span>
              <span style={{ fontSize: "10.5px", color: "var(--text-muted)" }}>
                {t("launchpad.dropPrdPrompt", ".md, .pdf, .fig, .zip, ảnh hoặc tài liệu")}
              </span>
            </div>
          </div>
        </label>
      </div>
      {uploadError && (
        <div role="alert" data-testid="launchpad-local-upload-error" style={{ fontSize: "11px", color: "var(--state-danger)", padding: "6px 10px", borderRadius: "var(--radius-sm)", backgroundColor: "var(--state-danger-soft)", border: "1px solid var(--state-danger)" }}>
          {uploadError}
        </div>
      )}
    </div>
  );
};

export const LaunchpadTemplatesPanel: React.FC<{
  selectedTemplateId: string;
  onSelectTemplate: (id: string) => void;
  onSelectReaReference?: (hit: ReaSearchHitDto) => void;
  onSelectCatalogApp?: (app: ReaCatalogItemDto) => void;
}> = ({ selectedTemplateId, onSelectTemplate, onSelectReaReference, onSelectCatalogApp }) => (
  <div data-testid="launchpad-panel-templates" style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
    <LaunchpadReaSearchSection
      selectedTemplateId={selectedTemplateId}
      onSelectReference={(hit) => {
        onSelectTemplate(`rea:${hit.slug}`);
        onSelectReaReference?.(hit);
      }}
      onSelectCatalogApp={(app) => {
        onSelectTemplate(`awesome-re:${app.slug}`);
        onSelectCatalogApp?.(app);
      }}
    />
  </div>
);

export const LaunchpadDesignPanel: React.FC<{
  designUrl: string;
  clipboardSnippet: string;
  externalRejectError?: { code: string; message: string } | null;
  onChangeUrl: (v: string) => void;
  onChangeClipboard: (v: string) => void;
  onReaIngested?: (outcome: ReaIngestOutcomeDto) => void;
  onFormatChange?: (fmt: ReaSourceFormat) => void;
  onLicenseChange?: (spdx: string) => void;
}> = ({ designUrl, clipboardSnippet, externalRejectError, onChangeUrl, onChangeClipboard, onReaIngested, onFormatChange, onLicenseChange }) => {
  const [selectedTool, setSelectedTool] = useState(DESIGN_TOOL_CHIPS[0]);
  const [detectedFormat, setDetectedFormat] = useState<ReaSourceFormat>("url");

  return (
    <div data-testid="launchpad-panel-design" style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
      <div style={{ display: "flex", gap: "6px", flexWrap: "wrap", alignItems: "center" }}>
        {DESIGN_TOOL_CHIPS.map((chip) => {
          const active = selectedTool.id === chip.id;
          return (
            <button key={chip.id} type="button" data-testid={`launchpad-design-tool-${chip.id}`} onClick={() => { setSelectedTool(chip); setDetectedFormat(chip.format); onFormatChange?.(chip.format); }} style={{ padding: "5px 11px", fontSize: "11px", fontWeight: active ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-pill)", border: active ? "1px solid var(--action-primary)" : "1px solid var(--border-default)", backgroundColor: active ? "var(--bg-selected)" : "var(--bg-workspace)", color: active ? "var(--action-primary)" : "var(--text-secondary)", cursor: "pointer" }}>
              {chip.label}
            </button>
          );
        })}
      </div>
      <input data-testid="launchpad-figma-url-input" type="text" placeholder={selectedTool.placeholder} value={designUrl} onChange={(e) => onChangeUrl(e.target.value)} style={FIELD_STYLE} />
      <LaunchpadReaIngestSection designUrl={designUrl} clipboardSnippet={clipboardSnippet} detectedFormat={detectedFormat} externalRejectError={externalRejectError} onChangeClipboard={onChangeClipboard} onIngested={onReaIngested} onLicenseChange={onLicenseChange} />
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
      <div style={{ fontSize: "12px", color: "var(--text-secondary)" }}>
        {t("launchpad.gitDesc", "Dán link GitHub để mở nhanh mã nguồn vào Chat — trợ lý sẽ tự phân tích cấu trúc giao diện.")}
      </div>
      <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
        <input data-testid="launchpad-git-url-input" type="text" placeholder={t("launchpad.gitUrlPlaceholder", "https://github.com/org/repo")} value={gitRepoUrl} onChange={(e) => onChangeUrl(e.target.value)} style={{ ...FIELD_STYLE, flex: "1 1 280px" }} />
        <input data-testid="launchpad-git-branch-input" type="text" aria-label={t("launchpad.gitBranchLabel", "Nhánh")} value={gitBranch} onChange={(e) => onChangeBranch(e.target.value)} placeholder="main" style={{ ...FIELD_STYLE, width: "120px", fontFamily: "var(--font-mono)" }} />
      </div>
    </div>
  );
};
