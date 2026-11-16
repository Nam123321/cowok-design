// @story 05.1
// @implements 4-Path Launchpad (PRD, Templates, Design Import & Git Codebase Mode)
import React, { useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  LaunchpadDesignPanel,
  LaunchpadGitPanel,
  LaunchpadPrdPanel,
  LaunchpadTemplatesPanel,
} from "./LaunchpadPanels";

export type LaunchpadEntryPath = "prd" | "templates" | "design" | "git";

export interface ProjectLaunchpadModalProps {
  isOpen: boolean;
  onClose: () => void;
  onLaunchPath?: (pathType: LaunchpadEntryPath, payload: Record<string, unknown>) => void;
}

const MODAL_Z_INDEX = 50;
const ICON_HEADER_SIZE = 16;
const ICON_TAB_SIZE = 13;
const ICON_BTN_SIZE = 13;
const STROKE_WIDTH_MONO = 1.85;
const FONT_WEIGHT_SEMIBOLD = 600;
const FONT_WEIGHT_MEDIUM = 500;

const PATH_TABS: Array<{ id: LaunchpadEntryPath; icon: "file-text" | "layout" | "figma" | "git-branch"; label: string }> = [
  { id: "prd", icon: "file-text", label: "1. Prompt / PRD" },
  { id: "templates", icon: "layout", label: "2. Starter Templates" },
  { id: "design", icon: "figma", label: "3. Design Import" },
  { id: "git", icon: "git-branch", label: "4. Git Codebase" },
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

const LaunchpadHeader: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "16px 20px", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between" }}>
      <div>
        <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", display: "flex", alignItems: "center", gap: "8px" }}>
          <BrandIcon name="play" size={ICON_HEADER_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="var(--action-primary)" />
          <span>{t("launchpad.title", "Project Launchpad (4 Entry Paths)")}</span>
        </h2>
        <p style={{ margin: "3px 0 0", fontSize: "11px", color: "var(--text-muted)" }}>{t("launchpad.subtitle", "Khởi tạo nhanh luồng màn hình UI/UX qua 4 phương thức chuẩn hóa.")}</p>
      </div>
      <button type="button" data-testid="launchpad-close-btn" onClick={onClose} style={{ background: "transparent", border: "none", color: "var(--text-muted)", cursor: "pointer", padding: "4px", borderRadius: "var(--radius-xs)", display: "flex" }}>
        <BrandIcon name="close" size={ICON_HEADER_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
      </button>
    </div>
  );
};

const LaunchpadFooter: React.FC<{ onClose: () => void; onLaunch: () => void }> = ({ onClose, onLaunch }) => {
  const { t } = useTranslation();
  return (
    <div style={{ padding: "12px 20px", borderTop: "1px solid var(--border-default)", display: "flex", justifyContent: "flex-end", gap: "8px", backgroundColor: "var(--bg-workspace)" }}>
      <button type="button" data-testid="launchpad-cancel-btn" onClick={onClose} style={{ padding: "6px 14px", fontSize: "12px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--text-secondary)", cursor: "pointer" }}>
        {t("common.cancel", "Hủy Bỏ")}
      </button>
      <button type="button" data-testid="launchpad-submit-btn" onClick={onLaunch} style={{ padding: "6px 16px", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "6px" }}>
        <BrandIcon name="check" size={ICON_BTN_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
        <span>{t("launchpad.proceedBtn", "Khởi Tạo Luồng")}</span>
      </button>
    </div>
  );
};

export const ProjectLaunchpadModal: React.FC<ProjectLaunchpadModalProps> = ({ isOpen, onClose, onLaunchPath }) => {
  const [activePath, setActivePath] = useState<LaunchpadEntryPath>("prd");
  const [prdText, setPrdText] = useState("");
  const [prdFileName, setPrdFileName] = useState<string | null>(null);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>("tpl-fintech");
  const [designUrl, setDesignUrl] = useState("");
  const [clipboardSnippet, setClipboardSnippet] = useState("");
  const [extractedTokens, setExtractedTokens] = useState<string | null>(null);
  const [gitRepoUrl, setGitRepoUrl] = useState("");
  const [gitBranch, setGitBranch] = useState("main");

  if (!isOpen) return null;

  const payloadByPath: Record<LaunchpadEntryPath, Record<string, unknown>> = {
    prd: { prdText, prdFileName },
    templates: { templateId: selectedTemplateId },
    design: { designUrl, clipboardSnippet, extractedTokens },
    git: { gitRepoUrl, gitBranch },
  };

  const handleOfflineExtractTokens = () => {
    setExtractedTokens(JSON.stringify({ primaryColor: "var(--action-primary)", surfaceColor: "var(--bg-surface)", fontFamily: "var(--font-sans)", borderRadius: "var(--radius-md)", detectedLayers: ["Header", "MetricsGrid", "CtaAction"] }, null, 2));
  };

  return (
    <div role="dialog" aria-modal="true" data-testid="project-launchpad-modal" style={OVERLAY_STYLE}>
      <div style={DIALOG_STYLE}>
        <LaunchpadHeader onClose={onClose} />
        <div style={{ display: "flex", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-default)", padding: "0 12px", gap: "4px" }}>
          {PATH_TABS.map((tab) => (
            <button key={tab.id} type="button" data-testid={`launchpad-tab-${tab.id}`} onClick={() => setActivePath(tab.id)} style={tabButtonStyle(activePath === tab.id)}>
              <BrandIcon name={tab.icon} size={ICON_TAB_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
              <span>{tab.label}</span>
            </button>
          ))}
        </div>
        <div style={{ padding: "20px", minHeight: "260px", maxHeight: "420px", overflowY: "auto" }}>
          {activePath === "prd" && <LaunchpadPrdPanel prdText={prdText} prdFileName={prdFileName} onChangeText={setPrdText} onSelectFile={(name, content) => { setPrdFileName(name); setPrdText(content); }} onLoadSamplePrd={() => { setPrdFileName("product_requirement_document.md"); setPrdText("# PRD: FinTech Multi-Sig Wallet\n- Feature 1: Key Management\n- Feature 2: Biometrics"); }} />}
          {activePath === "templates" && <LaunchpadTemplatesPanel selectedTemplateId={selectedTemplateId} onSelectTemplate={setSelectedTemplateId} />}
          {activePath === "design" && <LaunchpadDesignPanel designUrl={designUrl} clipboardSnippet={clipboardSnippet} extractedTokens={extractedTokens} onChangeUrl={setDesignUrl} onChangeClipboard={setClipboardSnippet} onExtractTokens={handleOfflineExtractTokens} />}
          {activePath === "git" && <LaunchpadGitPanel gitRepoUrl={gitRepoUrl} gitBranch={gitBranch} onChangeUrl={setGitRepoUrl} onChangeBranch={setGitBranch} />}
        </div>
        <LaunchpadFooter onClose={onClose} onLaunch={() => { onLaunchPath?.(activePath, payloadByPath[activePath]); onClose(); }} />
      </div>
    </div>
  );
};
