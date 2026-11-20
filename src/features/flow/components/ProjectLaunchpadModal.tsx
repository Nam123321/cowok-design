// @story 05.1
// @story 06.1
// @implements AC-06 (4-Path Launchpad Modal — Localized Tabs, Post-Create Skip & Sample PRD)
import React, { useEffect, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  LaunchpadDesignPanel,
  LaunchpadGitPanel,
  LaunchpadPrdPanel,
  LaunchpadTemplatesPanel,
} from "./LaunchpadPanels";

export type LaunchpadEntryPath = "prd" | "templates" | "design" | "git";

export interface LaunchpadMaterialContext {
  projectId: string;
  projectName: string;
  entryKind: "fe-dag" | "presentation";
}

export interface ProjectLaunchpadModalProps {
  isOpen: boolean;
  onClose: () => void;
  materialContext?: LaunchpadMaterialContext | null;
  onLaunchPath?: (pathType: LaunchpadEntryPath, payload: Record<string, unknown>) => void;
}

const MODAL_Z_INDEX = 50;
const ICON_HEADER_SIZE = 16;
const ICON_TAB_SIZE = 13;
const ICON_BTN_SIZE = 13;
const STROKE_WIDTH_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;

const PATH_TABS: Array<{ id: LaunchpadEntryPath; icon: "file-text" | "layout" | "figma" | "git-branch"; labelKey: string; fallbackLabel: string }> = [
  { id: "prd", icon: "file-text", labelKey: "launchpad.tabPrd", fallbackLabel: "1. Prompt / PRD" },
  { id: "templates", icon: "layout", labelKey: "launchpad.tabTemplates", fallbackLabel: "2. Starter Templates" },
  { id: "design", icon: "figma", labelKey: "launchpad.tabDesign", fallbackLabel: "3. Design Import" },
  { id: "git", icon: "git-branch", labelKey: "launchpad.tabGit", fallbackLabel: "4. Git Codebase" },
];

const OVERLAY_STYLE: React.CSSProperties = {
  position: "fixed", inset: 0, zIndex: MODAL_Z_INDEX, backgroundColor: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", padding: "16px",
};

const DIALOG_STYLE: React.CSSProperties = {
  width: "100%", maxWidth: "680px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-lg)", boxShadow: "var(--shadow-lg)", display: "flex", flexDirection: "column", overflow: "hidden",
};

const tabButtonStyle = (isActive: boolean): React.CSSProperties => ({
  padding: "8px 14px", fontSize: "12px", fontWeight: isActive ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM, backgroundColor: isActive ? "var(--bg-selected)" : "transparent", color: isActive ? "var(--action-primary)" : "var(--text-secondary)", border: "none", borderBottom: isActive ? "2px solid var(--action-primary)" : "2px solid transparent", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "6px",
});

const LaunchpadHeader: React.FC<{
  materialContext?: LaunchpadMaterialContext | null;
  onClose: () => void;
}> = ({ materialContext, onClose }) => {
  const { t } = useTranslation();
  const titleText = materialContext ? `${t("launchpad.loadMaterialTitle", "Nạp tài liệu khởi tạo")}: ${materialContext.projectName}` : t("launchpad.title", "Project Launchpad (4 Entry Paths)");
  const subtitleText = materialContext
    ? t("launchpad.loadMaterialSubtitle", "Chọn 1 trong 4 nguồn khởi tạo bên dưới, hoặc bấm Bỏ qua nếu thư mục đã có sẵn tệp hay muốn đính kèm trực tiếp trong khung chat.")
    : t("launchpad.subtitle", "Khởi tạo nhanh luồng màn hình UI/UX qua 4 phương thức chuẩn hóa.");
  const kindBadge = materialContext ? (materialContext.entryKind === "presentation" ? t("workbench.entryKindPresentation", "Tạo Presentation") : t("workbench.entryKindFeDag", "Tạo Ứng dụng")) : null;

  return (
    <div style={{ padding: "16px 20px", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px" }}>
      <div>
        <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", display: "flex", alignItems: "center", gap: "8px" }}>
            <BrandIcon name="play" size={ICON_HEADER_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="var(--action-primary)" />
            <span>{titleText}</span>
          </h2>
          {kindBadge && <span style={{ fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, padding: "2px 8px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-selected)", color: "var(--action-primary)" }}>{kindBadge}</span>}
        </div>
        <p style={{ margin: "3px 0 0", fontSize: "11px", color: "var(--text-muted)" }}>{subtitleText}</p>
      </div>
      <button type="button" data-testid="launchpad-close-btn" onClick={onClose} style={{ background: "transparent", border: "none", color: "var(--text-muted)", cursor: "pointer", padding: "4px", borderRadius: "var(--radius-xs)", display: "flex" }}>
        <BrandIcon name="close" size={ICON_HEADER_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
      </button>
    </div>
  );
};

const LaunchpadFooter: React.FC<{
  hasMaterialContext: boolean;
  onClose: () => void;
  onLaunch: () => void;
}> = ({ hasMaterialContext, onClose, onLaunch }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "12px 20px", borderTop: "1px solid var(--border-default)", display: "flex", justifyContent: hasMaterialContext ? "space-between" : "flex-end", alignItems: "center", gap: "8px", backgroundColor: "var(--bg-workspace)" }}>
      {hasMaterialContext && (
        <button type="button" data-testid="launchpad-skip-btn" onClick={onClose} style={{ padding: "6px 14px", fontSize: "12px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-secondary)", cursor: "pointer" }}>
          {t("launchpad.skipBtn", "Bỏ qua")}
        </button>
      )}
      <div style={{ display: "flex", gap: "8px" }}>
        {!hasMaterialContext && (
          <button type="button" data-testid="launchpad-cancel-btn" onClick={onClose} style={{ padding: "6px 14px", fontSize: "12px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--text-secondary)", cursor: "pointer" }}>
            {t("common.cancel", "Hủy Bỏ")}
          </button>
        )}
        <button type="button" data-testid="launchpad-submit-btn" onClick={onLaunch} style={{ padding: "6px 16px", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "6px" }}>
          <BrandIcon name="check" size={ICON_BTN_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
          <span>{hasMaterialContext ? t("launchpad.applyMaterialBtn", "Áp dụng tài liệu") : t("launchpad.proceedBtn", "Khởi Tạo Luồng")}</span>
        </button>
      </div>
    </div>
  );
};

function useLaunchpadFormState(isOpen: boolean, materialContext?: LaunchpadMaterialContext | null) {
  const [activePath, setActivePath] = useState<LaunchpadEntryPath>("prd");
  const [prdText, setPrdText] = useState("");
  const [prdFileName, setPrdFileName] = useState<string | null>(null);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>("tpl-fintech");
  const [designUrl, setDesignUrl] = useState("");
  const [clipboardSnippet, setClipboardSnippet] = useState("");
  const [extractedTokens, setExtractedTokens] = useState<string | null>(null);
  const [gitRepoUrl, setGitRepoUrl] = useState("");
  const [gitBranch, setGitBranch] = useState("main");

  useEffect(() => {
    if (!isOpen || !materialContext) return;
    setSelectedTemplateId(materialContext.entryKind === "presentation" ? "tpl-slide-deck" : "tpl-fintech");
  }, [isOpen, materialContext]);

  const payloadByPath: Record<LaunchpadEntryPath, Record<string, unknown>> = {
    prd: { prdText, prdFileName },
    templates: { templateId: selectedTemplateId },
    design: { designUrl, clipboardSnippet, extractedTokens },
    git: { gitRepoUrl, gitBranch },
  };
  return {
    activePath, setActivePath, prdText, setPrdText, prdFileName, setPrdFileName,
    selectedTemplateId, setSelectedTemplateId, designUrl, setDesignUrl, clipboardSnippet,
    setClipboardSnippet, extractedTokens, setExtractedTokens, gitRepoUrl, setGitRepoUrl,
    gitBranch, setGitBranch, payloadByPath,
  };
}

export const ProjectLaunchpadModal: React.FC<ProjectLaunchpadModalProps> = ({ isOpen, onClose, materialContext, onLaunchPath }) => {
  const { t } = useTranslation();
  const s = useLaunchpadFormState(isOpen, materialContext);
  if (!isOpen) return null;

  const extractTokens = () => s.setExtractedTokens(JSON.stringify({ primaryColor: "var(--action-primary)", surfaceColor: "var(--bg-surface)", fontFamily: "var(--font-sans)", borderRadius: "var(--radius-md)", detectedLayers: ["Header", "MetricsGrid", "CtaAction"] }, null, 2));
  const loadSample = () => {
    s.setPrdFileName("product_requirement_document.md");
    s.setPrdText(t("launchpad.samplePrdText", "# PRD: FinTech Multi-Sig Wallet\n- Feature 1: Key Management\n- Feature 2: Biometrics"));
  };

  return (
    <div role="dialog" aria-modal="true" data-testid="project-launchpad-modal" style={OVERLAY_STYLE}>
      <div style={DIALOG_STYLE}>
        <LaunchpadHeader materialContext={materialContext} onClose={onClose} />
        <div style={{ display: "flex", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-default)", padding: "0 12px", gap: "4px" }}>
          {PATH_TABS.map((tab) => (
            <button key={tab.id} type="button" data-testid={`launchpad-tab-${tab.id}`} onClick={() => s.setActivePath(tab.id)} style={tabButtonStyle(s.activePath === tab.id)}>
              <BrandIcon name={tab.icon} size={ICON_TAB_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
              <span>{t(tab.labelKey, tab.fallbackLabel)}</span>
            </button>
          ))}
        </div>
        <div style={{ padding: "20px", minHeight: "260px", maxHeight: "420px", overflowY: "auto" }}>
          {s.activePath === "prd" && <LaunchpadPrdPanel prdText={s.prdText} prdFileName={s.prdFileName} onChangeText={s.setPrdText} onSelectFile={(name, content) => { s.setPrdFileName(name); s.setPrdText(content); }} onLoadSamplePrd={loadSample} />}
          {s.activePath === "templates" && <LaunchpadTemplatesPanel selectedTemplateId={s.selectedTemplateId} onSelectTemplate={s.setSelectedTemplateId} />}
          {s.activePath === "design" && <LaunchpadDesignPanel designUrl={s.designUrl} clipboardSnippet={s.clipboardSnippet} extractedTokens={s.extractedTokens} onChangeUrl={s.setDesignUrl} onChangeClipboard={s.setClipboardSnippet} onExtractTokens={extractTokens} />}
          {s.activePath === "git" && <LaunchpadGitPanel gitRepoUrl={s.gitRepoUrl} gitBranch={s.gitBranch} onChangeUrl={s.setGitRepoUrl} onChangeBranch={s.setGitBranch} />}
        </div>
        <LaunchpadFooter hasMaterialContext={Boolean(materialContext)} onClose={onClose} onLaunch={() => { onLaunchPath?.(s.activePath, s.payloadByPath[s.activePath]); onClose(); }} />
      </div>
    </div>
  );
};
