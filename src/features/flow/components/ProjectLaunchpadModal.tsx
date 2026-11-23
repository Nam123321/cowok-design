// @story 05.1
// @story 06.1
// @story 09.1
// @implements AC-01, AC-02, AC-04, AC-06 (Ultra-Clean Chat-at-Center Quick-Start Project Launchpad Modal)
import React, { useEffect, useState } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ReaCatalogItemDto, ReaIngestOutcomeDto, ReaSearchHitDto, ReaSourceFormat } from "../../../lib/validations/rea";
import {
  LaunchpadDesignPanel,
  LaunchpadGitPanel,
  LaunchpadPrdPanel,
  LaunchpadTemplatesPanel,
} from "./LaunchpadPanels";
import { formatReaIngestError, runSilentReaIngest } from "./LaunchpadReaPanel";

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

const PATH_TABS: Array<{ id: LaunchpadEntryPath; icon: "folder" | "figma" | "git-branch" | "layout"; labelKey: string; fallbackLabel: string }> = [
  { id: "prd", icon: "folder", labelKey: "launchpad.tabPrd", fallbackLabel: "Máy tính" },
  { id: "design", icon: "figma", labelKey: "launchpad.tabDesign", fallbackLabel: "Công cụ thiết kế" },
  { id: "git", icon: "git-branch", labelKey: "launchpad.tabGit", fallbackLabel: "GitHub" },
  { id: "templates", icon: "layout", labelKey: "launchpad.tabTemplates", fallbackLabel: "Thư viện mẫu" },
];

const OVERLAY_STYLE: React.CSSProperties = {
  position: "fixed", inset: 0, zIndex: MODAL_Z_INDEX, backgroundColor: "var(--bg-overlay)", display: "flex", alignItems: "center", justifyContent: "center", padding: "16px",
};

const DIALOG_STYLE: React.CSSProperties = {
  width: "100%", maxWidth: "600px", backgroundColor: "var(--bg-surface)", border: "1px solid var(--border-default)", borderRadius: "var(--radius-lg)", boxShadow: "var(--shadow-lg)", display: "flex", flexDirection: "column", overflow: "hidden",
};

const tabButtonStyle = (isActive: boolean): React.CSSProperties => ({
  padding: "9px 12px", fontSize: "12px", fontWeight: isActive ? FONT_WEIGHT_SEMIBOLD : FONT_WEIGHT_MEDIUM, backgroundColor: isActive ? "var(--bg-selected)" : "transparent", color: isActive ? "var(--action-primary)" : "var(--text-secondary)", border: "none", borderBottom: isActive ? "2px solid var(--action-primary)" : "2px solid transparent", cursor: "pointer", display: "inline-flex", alignItems: "center", gap: "6px", whiteSpace: "nowrap",
});

const LaunchpadHeader: React.FC<{
  materialContext?: LaunchpadMaterialContext | null;
  onClose: () => void;
}> = ({ materialContext, onClose }) => {
  const { t } = useTranslation();
  const titleText = materialContext ? `${t("launchpad.loadMaterialTitle", "Thêm nguồn vào dự án")}: ${materialContext.projectName}` : t("launchpad.title", "Khởi tạo nhanh dự án");
  const subtitleText = materialContext
    ? t("launchpad.loadMaterialSubtitle", "Chọn nhanh thư mục, tệp, GitHub hoặc bỏ qua để trao đổi trực tiếp trong Chat.")
    : t("launchpad.subtitle", "Chọn nhanh thư mục trên máy, GitHub hoặc công cụ thiết kế — mọi bước tiếp theo diễn ra ngay trong Chat.");
  const kindBadge = materialContext ? (materialContext.entryKind === "presentation" ? t("workbench.entryKindPresentation", "Tạo Presentation") : t("workbench.entryKindFeDag", "Tạo Ứng dụng")) : null;

  return (
    <div style={{ padding: "14px 18px", borderBottom: "1px solid var(--border-default)", display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px" }}>
      <div>
        <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
          <h2 style={{ margin: 0, fontSize: "15px", fontWeight: FONT_WEIGHT_SEMIBOLD, color: "var(--text-primary)", display: "flex", alignItems: "center", gap: "7px" }}>
            <BrandIcon name="play" size={ICON_HEADER_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="var(--action-primary)" />
            <span>{titleText}</span>
          </h2>
          {kindBadge && <span style={{ fontSize: "10.5px", fontWeight: FONT_WEIGHT_SEMIBOLD, padding: "2px 8px", borderRadius: "var(--radius-pill)", backgroundColor: "var(--bg-selected)", color: "var(--action-primary)" }}>{kindBadge}</span>}
        </div>
        <p style={{ margin: "3px 0 0", fontSize: "11.5px", color: "var(--text-muted)" }}>{subtitleText}</p>
      </div>
      <button type="button" data-testid="launchpad-close-btn" onClick={onClose} style={{ background: "transparent", border: "none", color: "var(--text-muted)", cursor: "pointer", padding: "4px", borderRadius: "var(--radius-xs)", display: "flex" }}>
        <BrandIcon name="close" size={ICON_HEADER_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
      </button>
    </div>
  );
};

const LaunchpadFooter: React.FC<{
  hasMaterialContext: boolean;
  submitting?: boolean;
  onClose: () => void;
  onLaunch: () => void;
}> = ({ hasMaterialContext, submitting = false, onClose, onLaunch }) => {
  const { t } = useTranslation();
  let submitLabel = hasMaterialContext
    ? t("launchpad.applyMaterialBtn", "Áp dụng vào Chat")
    : t("launchpad.proceedBtn", "Mở vào Chat");
  if (submitting) submitLabel = t("rea.ingestingBtn", "Đang kiểm tra...");

  return (
    <div style={{ padding: "12px 18px", borderTop: "1px solid var(--border-default)", display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", backgroundColor: "var(--bg-workspace)" }}>
      {hasMaterialContext ? (
        <button type="button" data-testid="launchpad-skip-btn" onClick={onClose} style={{ padding: "6px 12px", fontSize: "12px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "var(--bg-surface)", color: "var(--text-secondary)", cursor: "pointer" }}>
          {t("launchpad.skipBtn", "Bỏ qua · Chat ngay")}
        </button>
      ) : (
        <button type="button" data-testid="launchpad-cancel-btn" onClick={onClose} style={{ padding: "6px 12px", fontSize: "12px", fontWeight: FONT_WEIGHT_MEDIUM, borderRadius: "var(--radius-xs)", border: "1px solid var(--border-default)", backgroundColor: "transparent", color: "var(--text-secondary)", cursor: "pointer" }}>
          {t("launchpad.startInChatBtn", "Bỏ qua · Chat ngay")}
        </button>
      )}
      <button type="button" disabled={submitting} data-testid="launchpad-submit-btn" onClick={onLaunch} style={{ padding: "7px 16px", fontSize: "12px", fontWeight: FONT_WEIGHT_SEMIBOLD, borderRadius: "var(--radius-xs)", border: "none", backgroundColor: "var(--action-primary)", color: "var(--action-primary-fg)", cursor: submitting ? "wait" : "pointer", display: "inline-flex", alignItems: "center", gap: "6px" }}>
        <BrandIcon name="check" size={ICON_BTN_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
        <span>{submitLabel}</span>
      </button>
    </div>
  );
};

function useLaunchpadFormState(isOpen: boolean, materialContext?: LaunchpadMaterialContext | null) {
  const [activePath, setActivePath] = useState<LaunchpadEntryPath>("prd");
  const [localFolderPath, setLocalFolderPath] = useState("");
  const [prdText, setPrdText] = useState("");
  const [prdFileName, setPrdFileName] = useState<string | null>(null);
  const [selectedTemplateId, setSelectedTemplateId] = useState<string>("");
  const [selectedCatalogApp, setSelectedCatalogApp] = useState<ReaCatalogItemDto | null>(null);
  const [selectedReaRef, setSelectedReaRef] = useState<ReaSearchHitDto | null>(null);
  const [designUrl, setDesignUrl] = useState("");
  const [clipboardSnippet, setClipboardSnippet] = useState("");
  const [designFormat, setDesignFormat] = useState<ReaSourceFormat>("url");
  const [designLicense, setDesignLicense] = useState("MIT");
  const [ingestedReaOutcome, setIngestedReaOutcome] = useState<ReaIngestOutcomeDto | null>(null);
  const [designRejectError, setDesignRejectError] = useState<{ code: string; message: string } | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [gitRepoUrl, setGitRepoUrl] = useState("");
  const [gitBranch, setGitBranch] = useState("main");

  useEffect(() => {
    if (!isOpen || !materialContext) return;
    setSelectedTemplateId(materialContext.entryKind === "presentation" ? "tpl-slide-deck" : "");
  }, [isOpen, materialContext]);

  const payloadByPath: Record<LaunchpadEntryPath, Record<string, unknown>> = {
    prd: { localFolderPath, prdText, prdFileName },
    templates: { templateId: selectedTemplateId, catalogApp: selectedCatalogApp, reaReference: selectedReaRef },
    design: { designUrl, clipboardSnippet, reaIngestOutcome: ingestedReaOutcome },
    git: { gitRepoUrl, gitBranch },
  };
  return {
    activePath, setActivePath, localFolderPath, setLocalFolderPath,
    prdText, setPrdText, prdFileName, setPrdFileName,
    selectedTemplateId, setSelectedTemplateId, selectedCatalogApp, setSelectedCatalogApp,
    selectedReaRef, setSelectedReaRef,
    designUrl, setDesignUrl, clipboardSnippet, setClipboardSnippet, designFormat, setDesignFormat,
    designLicense, setDesignLicense, ingestedReaOutcome, setIngestedReaOutcome,
    designRejectError, setDesignRejectError, submitting, setSubmitting,
    gitRepoUrl, setGitRepoUrl, gitBranch, setGitBranch, payloadByPath,
  };
}

const LaunchpadActiveBody: React.FC<{ s: ReturnType<typeof useLaunchpadFormState> }> = ({ s }) => (
  <div style={{ padding: "16px 18px", minHeight: "120px", maxHeight: "400px", overflowY: "auto" }}>
    {s.activePath === "prd" && (
      <LaunchpadPrdPanel localFolderPath={s.localFolderPath} onChangeFolderPath={s.setLocalFolderPath} prdFileName={s.prdFileName} onSelectFile={(name, content) => { s.setPrdFileName(name); s.setPrdText(content); }} />
    )}
    {s.activePath === "templates" && (
      <LaunchpadTemplatesPanel selectedTemplateId={s.selectedTemplateId} onSelectTemplate={s.setSelectedTemplateId} onSelectReaReference={(hit) => { s.setSelectedReaRef(hit); s.setSelectedCatalogApp(null); }} onSelectCatalogApp={(app) => { s.setSelectedCatalogApp(app); s.setSelectedReaRef(null); }} />
    )}
    {s.activePath === "design" && (
      <LaunchpadDesignPanel designUrl={s.designUrl} clipboardSnippet={s.clipboardSnippet} externalRejectError={s.designRejectError} onChangeUrl={(v) => { s.setDesignUrl(v); s.setIngestedReaOutcome(null); s.setDesignRejectError(null); }} onChangeClipboard={s.setClipboardSnippet} onFormatChange={s.setDesignFormat} onLicenseChange={s.setDesignLicense} onReaIngested={(out) => { s.setIngestedReaOutcome(out); s.setDesignRejectError(null); }} />
    )}
    {s.activePath === "git" && <LaunchpadGitPanel gitRepoUrl={s.gitRepoUrl} gitBranch={s.gitBranch} onChangeUrl={s.setGitRepoUrl} onChangeBranch={s.setGitBranch} />}
  </div>
);

export const ProjectLaunchpadModal: React.FC<ProjectLaunchpadModalProps> = ({ isOpen, onClose, materialContext, onLaunchPath }) => {
  const { t } = useTranslation();
  const s = useLaunchpadFormState(isOpen, materialContext);
  if (!isOpen) return null;

  const handleLaunch = async () => {
    if (s.activePath === "design" && s.designUrl.trim() && !s.ingestedReaOutcome) {
      s.setSubmitting(true);
      s.setDesignRejectError(null);
      try {
        const outcome = await runSilentReaIngest({ designUrl: s.designUrl, clipboardSnippet: s.clipboardSnippet, detectedFormat: s.designFormat, licenseSpdx: s.designLicense });
        s.setIngestedReaOutcome(outcome);
        onLaunchPath?.("design", { ...s.payloadByPath.design, reaIngestOutcome: outcome });
        onClose();
      } catch (err) {
        s.setDesignRejectError(formatReaIngestError(err, t("rea.ingestError", "Không thể nhập nguồn thiết kế.")));
      } finally {
        s.setSubmitting(false);
      }
      return;
    }
    onLaunchPath?.(s.activePath, s.payloadByPath[s.activePath]);
    onClose();
  };

  return (
    <div role="dialog" aria-modal="true" data-testid="project-launchpad-modal" style={OVERLAY_STYLE}>
      <div style={DIALOG_STYLE}>
        <LaunchpadHeader materialContext={materialContext} onClose={onClose} />
        <div style={{ display: "flex", backgroundColor: "var(--bg-workspace)", borderBottom: "1px solid var(--border-default)", padding: "0 12px", gap: "4px", overflowX: "auto" }}>
          {PATH_TABS.map((tab) => (
            <button key={tab.id} type="button" data-testid={`launchpad-tab-${tab.id}`} onClick={() => s.setActivePath(tab.id)} style={tabButtonStyle(s.activePath === tab.id)}>
              <BrandIcon name={tab.icon} size={ICON_TAB_SIZE} strokeWidth={STROKE_WIDTH_MONO} color="currentColor" />
              <span>{t(tab.labelKey, tab.fallbackLabel)}</span>
            </button>
          ))}
        </div>
        <LaunchpadActiveBody s={s} />
        <LaunchpadFooter hasMaterialContext={Boolean(materialContext)} submitting={s.submitting} onClose={onClose} onLaunch={() => void handleLaunch()} />
      </div>
    </div>
  );
};
