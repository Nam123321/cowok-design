// @story 05.3, 07.2
// @implements AC-01, AC-02, AC-03, AC-04
import React, { useEffect, useMemo, useState } from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ProjectIntegrationsStatus } from "../../../lib/validations/github-ide-integration";
import {
  type ExportZipResponse,
  type GitHubPRInput,
  type GitHubPRResponse,
  GitHubPRSchema,
  type IdeTarget,
  type OpenIdeHandoffResponse,
} from "../../../lib/validations/version-snapshot";
import { DeveloperHandoffPanel } from "../../split-export/DeveloperHandoffPanel";
import { QcGateBannerCard } from "../../split-export/QcGateBannerCard";
import { SharePresentPanel } from "../../split-export/SharePresentPanel";
import { useSplitExport } from "../../split-export/useSplitExport";
import { GitHubAccountSection } from "./GitHubAccountSection";
import { HandoffManifestPanel } from "./HandoffManifestPanel";

const HTTP_NOT_IMPLEMENTED = 501;
const SHORT_SHA_LEN = 16;
const DEFAULT_WORKBENCH_PROJECT_ID = "11111111-1111-4111-8111-111111111111";

export type ExportInitialSection = "zip" | "github-pr" | "ide-handoff";
export type ExportAudienceTab = "share-present" | "developer-handoff";

export interface PrSubmissionError {
  status?: number;
  code?: string;
  message: string;
}

export interface ExportBundleModalProps {
  isOpen: boolean;
  metadata: ExportZipResponse | null;
  isExporting: boolean;
  initialSection?: ExportInitialSection;
  defaultAudienceTab?: ExportAudienceTab;
  projectId?: string;
  projectTitle?: string;
  previewHtml?: string;
  integrationsStatus?: ProjectIntegrationsStatus | null;
  integrationBusy?: boolean;
  prResult?: GitHubPRResponse | null;
  prError?: PrSubmissionError | null;
  handoffResult?: OpenIdeHandoffResponse | null;
  handoffError?: string | null;
  onDownloadZip: () => void;
  onCreateGitHubPR: (input: GitHubPRInput) => void;
  onOpenIde?: (ide: IdeTarget) => void;
  onConnectGitHub?: (input: { token?: string; useGhCli?: boolean }) => Promise<void>;
  onSelectGitHubAccount?: (login: string) => Promise<void>;
  onBindGitRemote?: (remoteUrl: string) => Promise<void>;
  onSetDefaultIde?: (ide: IdeTarget) => void;
  onNavigatePluginInstall?: (installAction: string) => void;
  onClose: () => void;
}

const OFFLINE_PR_CODES = new Set([
  "GH_CLI_MISSING",
  "GH_UNAUTHENTICATED",
  "GIT_REMOTE_MISSING",
  "GITHUB_INTEGRATION_NOT_CONFIGURED",
]);

function isOfflineFallbackError(err: PrSubmissionError | null | undefined): boolean {
  if (!err) return false;
  if (err.status === HTTP_NOT_IMPLEMENTED) return true;
  return Boolean(err.code && OFFLINE_PR_CODES.has(err.code));
}

function resolveInitialAudienceTab(params: {
  defaultAudienceTab?: ExportAudienceTab;
  initialSection?: ExportInitialSection;
  projectId?: string;
  hasLegacyDevContext: boolean;
}): ExportAudienceTab {
  if (params.defaultAudienceTab) return params.defaultAudienceTab;
  if (
    params.initialSection === "zip" ||
    params.initialSection === "github-pr" ||
    params.initialSection === "ide-handoff"
  ) {
    return "developer-handoff";
  }
  if (!params.projectId && params.hasLegacyDevContext) {
    return "developer-handoff";
  }
  return "share-present";
}

const RunnabilityChecklist: React.FC<{ metadata: ExportZipResponse }> = ({ metadata }) => {
  const { t } = useTranslation();
  const items = useMemo(() => {
    const lower = metadata.filesIncluded.map((p) => p.toLowerCase());
    return [
      { key: "tokens", label: "tokens.css", ok: lower.some((p) => p.endsWith("tokens.css")) },
      { key: "icons", label: "icon-manifest.json", ok: lower.some((p) => p.endsWith("icon-manifest.json")) },
      { key: "fixtures", label: "fixtures/", ok: lower.some((p) => p.includes("fixture")) },
      { key: "package", label: "package.json", ok: lower.some((p) => p === "package.json" || p.endsWith("/package.json")) },
      { key: "handoff", label: "cowok-handoff-manifest.json", ok: metadata.handoffManifestIncluded || lower.some((p) => p.endsWith("cowok-handoff-manifest.json")) },
    ];
  }, [metadata]);

  return (
    <ul className="snap-checklist" data-testid="export-runnability-checklist">
      {items.map((item) => (
        <li key={item.key} data-testid={`checklist-item-${item.key}`} className={`snap-checklist-item ${item.ok ? "snap-checklist-item--ok" : "snap-checklist-item--missing"}`}>
          <code>{item.label}</code>
          <span>{item.ok ? t("snapshots.checklistOk", "✓ Included") : t("snapshots.checklistMissing", "Missing")}</span>
        </li>
      ))}
    </ul>
  );
};

const MetadataCard: React.FC<{ metadata: ExportZipResponse }> = ({ metadata }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });

  return (
    <div className="snap-meta-card" data-testid="export-metadata-card">
      <div className="snap-meta-row">
        <span className="snap-meta-label">{t("snapshots.exportFileLabel", "Export file:")}</span>
        <span className="snap-meta-value">{metadata.fileName}</span>
      </div>
      <div className="snap-meta-row">
        <span className="snap-meta-label">{t("snapshots.byteSizeLabel", "Size:")}</span>
        <span className="snap-meta-value">{`${formatNumber(metadata.byteLength)} ${t("snapshots.bytesUnit", "bytes")}`}</span>
      </div>
      <div className="snap-meta-row">
        <span className="snap-meta-label">{t("snapshots.manifestShaLabel", "Checksum SHA-256:")}</span>
        <span className="snap-meta-value">{metadata.sha256.slice(0, SHORT_SHA_LEN)}</span>
      </div>
      <div>
        <span className={`snap-badge ${metadata.readyToRun ? "snap-badge-safety" : "snap-badge-historical"}`}>
          {metadata.readyToRun ? t("snapshots.readyToRunBadge", "✓ Ready to run (pnpm install && pnpm build)") : t("snapshots.missingAssetsBadge", "Missing assets")}
        </span>
      </div>
      <RunnabilityChecklist metadata={metadata} />
    </div>
  );
};

const PrStatusBanners: React.FC<{
  validationError: string | null;
  prError: PrSubmissionError | null;
  prResult: GitHubPRResponse | null;
  isExporting: boolean;
  onDownloadZip: () => void;
}> = ({ validationError, prError, prResult, isExporting, onDownloadZip }) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const showOfflineFallback = isOfflineFallbackError(prError);
  return (
    <>
      {validationError && <p className="snap-form-error" data-testid="pr-validation-error">{validationError}</p>}
      {prError && !showOfflineFallback && <p className="snap-form-error" data-testid="pr-submit-error">{prError.message}</p>}
      {showOfflineFallback && (
        <div className="snap-fallback-card" data-testid="pr-501-fallback-card">
          <strong>{t("snapshots.prOfflineFallbackTitle", "GitHub CLI / Remote Unavailable — Offline ZIP Fallback Ready")}</strong>
          <span>{prError?.message}</span>
          <div className="snap-cli-box"><code className="snap-cli-code">gh auth login &amp;&amp; git remote -v</code></div>
          <button type="button" onClick={onDownloadZip} disabled={isExporting} data-testid="pr-501-zip-fallback-btn" className="snap-btn snap-btn--emerald">
            {t("snapshots.downloadZipBtn", "Download Full Repo ZIP")}
          </button>
        </div>
      )}
      {prResult && (
        <div className="snap-safety-banner" data-testid="pr-created-banner">
          <p className="snap-safety-banner-title">{t("snapshots.prCreatedTitle", "Pull Request Ready")} #{formatNumber(prResult.prNumber)}</p>
          <a href={prResult.prUrl} target="_blank" rel="noreferrer" className="snap-meta-value">{prResult.prUrl}</a>
        </div>
      )}
    </>
  );
};

const PrFormInputs: React.FC<{
  branch: string;
  setBranch: (v: string) => void;
  targetBranch: string;
  setTargetBranch: (v: string) => void;
  prTitle: string;
  setPrTitle: (v: string) => void;
  prBody: string;
  setPrBody: (v: string) => void;
}> = ({ branch, setBranch, targetBranch, setTargetBranch, prTitle, setPrTitle, prBody, setPrBody }) => {
  const { t } = useTranslation();
  return (
    <>
      <div className="snap-form-group">
        <label className="snap-form-label">{t("snapshots.githubBranchLabel", "GitHub Branch")}</label>
        <input type="text" value={branch} data-testid="pr-branch-input" onChange={(e) => setBranch(e.target.value)} className="snap-form-input" />
      </div>
      <div className="snap-form-group">
        <label className="snap-form-label">{t("snapshots.targetBranchLabel", "Base Branch")}</label>
        <input type="text" value={targetBranch} data-testid="pr-target-branch-input" onChange={(e) => setTargetBranch(e.target.value)} className="snap-form-input" />
      </div>
      <div className="snap-form-group">
        <label className="snap-form-label">{t("snapshots.prTitleLabel", "Pull Request Title")}</label>
        <input type="text" value={prTitle} data-testid="pr-title-input" onChange={(e) => setPrTitle(e.target.value)} className="snap-form-input" />
      </div>
      <div className="snap-form-group">
        <label className="snap-form-label">{t("snapshots.prBodyLabel", "Pull Request Summary")}</label>
        <textarea rows={2} value={prBody} data-testid="pr-body-input" onChange={(e) => setPrBody(e.target.value)} className="snap-form-textarea" />
      </div>
    </>
  );
};

const ExportModalFooter: React.FC<{
  activeTab: ExportAudienceTab;
  isExporting: boolean;
  onClose: () => void;
  onCreatePR: () => void;
  onDownloadZip: () => void;
}> = ({ activeTab, isExporting, onClose, onCreatePR, onDownloadZip }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-modal-actions">
      <button type="button" onClick={onClose} data-testid="export-modal-close-btn" className="snap-btn snap-btn--ghost">
        {t("common.close", "Đóng")}
      </button>
      {activeTab === "developer-handoff" && (
        <>
          <button type="button" onClick={onCreatePR} disabled={isExporting} data-testid="export-create-pr-btn" className="snap-btn snap-btn--primary">
            {t("snapshots.createPrBtn", "Create GitHub PR")}
          </button>
          <button type="button" onClick={onDownloadZip} disabled={isExporting} data-testid="export-download-zip-btn" className="snap-btn snap-btn--emerald">
            {isExporting ? t("snapshots.exportingBtn", "Exporting...") : t("snapshots.downloadZipBtn", "Download Full Repo ZIP")}
          </button>
        </>
      )}
    </div>
  );
};

const AudienceTabSwitcher: React.FC<{
  activeTab: ExportAudienceTab;
  onSelectTab: (tab: ExportAudienceTab) => void;
}> = ({ activeTab, onSelectTab }) => {
  const { t } = useTranslation();
  const makeTabStyle = (active: boolean): React.CSSProperties => ({
    flex: 1,
    padding: "9px 14px",
    borderRadius: "var(--radius-sm)",
    border: active ? "1px solid var(--action-primary)" : "1px solid transparent",
    backgroundColor: active ? "var(--bg-surface)" : "transparent",
    color: active ? "var(--text-primary)" : "var(--text-secondary)",
    fontWeight: active ? 600 : 500,
    fontSize: "13px",
    cursor: "pointer",
    boxShadow: active ? "var(--shadow-sm)" : "none",
  });

  return (
    <div
      role="tablist"
      aria-label={t("splitExport.audienceTabsAria", "Chọn luồng xuất bản")}
      style={{
        display: "flex",
        gap: "6px",
        padding: "4px",
        borderRadius: "var(--radius-md)",
        backgroundColor: "var(--bg-subtle)",
        border: "1px solid var(--border-subtle)",
        marginBottom: "14px",
      }}
      data-testid="split-export-tablist"
    >
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === "share-present"}
        style={makeTabStyle(activeTab === "share-present")}
        onClick={() => onSelectTab("share-present")}
        data-ff-id="share-present-tab"
        data-testid="share-present-tab"
      >
        {t("splitExport.sharePresentTab", "Chia sẻ & Trình chiếu")}
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === "developer-handoff"}
        style={makeTabStyle(activeTab === "developer-handoff")}
        onClick={() => onSelectTab("developer-handoff")}
        data-ff-id="developer-handoff-tab"
        data-testid="developer-handoff-tab"
      >
        {t("splitExport.developerHandoffTab", "Bàn giao Lập trình viên")}
      </button>
    </div>
  );
};

const ExportModalHeader: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const { t } = useTranslation();
  return (
    <>
      <div className="snap-item-header">
        <h3 className="snap-modal-title">{t("splitExport.modalTitle", "Xuất bản & Chia sẻ dự án")}</h3>
        <button type="button" onClick={onClose} aria-label={t("common.close", "Close")} className="snap-drawer-close-btn">{"✕"}</button>
      </div>
      <p className="snap-modal-desc">
        {t("splitExport.modalDesc", "Chọn luồng trình chiếu ngoại tuyến nhanh cho người xem hoặc gói mã nguồn đầy đủ cho đội ngũ kỹ thuật.")}
      </p>
    </>
  );
};

const ExportModalBody: React.FC<{
  metadata: ExportZipResponse | null;
  handoffResult: OpenIdeHandoffResponse | null;
  handoffError: string | null;
  isExporting: boolean;
  initialSection?: ExportInitialSection;
  integrationsStatus: ProjectIntegrationsStatus | null;
  integrationBusy: boolean;
  onOpenIde?: (ide: IdeTarget) => void;
  onSetDefaultIde?: (ide: IdeTarget) => void;
  onConnectGitHub?: (input: { token?: string; useGhCli?: boolean }) => Promise<void>;
  onSelectGitHubAccount?: (login: string) => Promise<void>;
  onBindGitRemote?: (remoteUrl: string) => Promise<void>;
  branch: string;
  setBranch: (v: string) => void;
  targetBranch: string;
  setTargetBranch: (v: string) => void;
  prTitle: string;
  setPrTitle: (v: string) => void;
  prBody: string;
  setPrBody: (v: string) => void;
  validationError: string | null;
  prError: PrSubmissionError | null;
  prResult: GitHubPRResponse | null;
  onDownloadZip: () => void;
}> = (p) => (
  <div className="snap-modal-grid">
    <div className="snap-modal-col">
      {p.metadata && <MetadataCard metadata={p.metadata} />}
      {p.onOpenIde && (
        <HandoffManifestPanel
          handoffResult={p.handoffResult}
          isLaunching={p.isExporting}
          errorMessage={p.handoffError}
          integrationsStatus={p.integrationsStatus}
          isFocused={p.initialSection === "ide-handoff"}
          onOpenIde={p.onOpenIde}
          onSetDefaultIde={p.onSetDefaultIde}
        />
      )}
    </div>
    <div className={`snap-modal-col${p.initialSection === "github-pr" ? " snap-section-focus" : ""}`}>
      {p.onConnectGitHub && p.onSelectGitHubAccount && p.onBindGitRemote && (
        <GitHubAccountSection
          status={p.integrationsStatus}
          busy={p.integrationBusy}
          onConnectGitHub={p.onConnectGitHub}
          onSelectGitHubAccount={p.onSelectGitHubAccount}
          onBindGitRemote={p.onBindGitRemote}
        />
      )}
      <PrFormInputs branch={p.branch} setBranch={p.setBranch} targetBranch={p.targetBranch} setTargetBranch={p.setTargetBranch} prTitle={p.prTitle} setPrTitle={p.setPrTitle} prBody={p.prBody} setPrBody={p.setPrBody} />
      <PrStatusBanners validationError={p.validationError} prError={p.prError} prResult={p.prResult} isExporting={p.isExporting} onDownloadZip={p.onDownloadZip} />
    </div>
  </div>
);

export const ExportBundleModal: React.FC<ExportBundleModalProps> = ({
  isOpen,
  metadata,
  isExporting,
  initialSection,
  defaultAudienceTab,
  projectId,
  projectTitle,
  previewHtml = "",
  integrationsStatus = null,
  integrationBusy = false,
  prResult = null,
  prError = null,
  handoffResult = null,
  handoffError = null,
  onDownloadZip,
  onCreateGitHubPR,
  onOpenIde,
  onConnectGitHub,
  onSelectGitHubAccount,
  onBindGitRemote,
  onSetDefaultIde,
  onNavigatePluginInstall,
  onClose,
}) => {
  const { t } = useTranslation();
  const isMountedInLiveWorkbench =
    typeof document !== "undefined" && Boolean(document.querySelector(".workbench-shell"));
  const effectiveProjectId =
    projectId || (isMountedInLiveWorkbench ? DEFAULT_WORKBENCH_PROJECT_ID : undefined);
  const resolvedProjectTitle =
    projectTitle || t("splitExport.defaultProjectTitle", "Bản trình chiếu");
  const hasLegacyDevContext = Boolean(metadata || prError || prResult || handoffResult || handoffError);
  const [activeTab, setActiveTab] = useState<ExportAudienceTab>(() =>
    resolveInitialAudienceTab({
      defaultAudienceTab,
      initialSection,
      projectId,
      hasLegacyDevContext,
    })
  );
  const [branch, setBranch] = useState("feature/design-intent-export");
  const [targetBranch, setTargetBranch] = useState("main");
  const [prTitle, setPrTitle] = useState("Sync executable UI design intent");
  const [prBody, setPrBody] = useState("");
  const [validationError, setValidationError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setActiveTab(
      resolveInitialAudienceTab({
        defaultAudienceTab,
        initialSection,
        projectId,
        hasLegacyDevContext,
      })
    );
  }, [defaultAudienceTab, hasLegacyDevContext, initialSection, isOpen, projectId]);

  const splitExport = useSplitExport({
    projectId: effectiveProjectId,
    projectTitle: resolvedProjectTitle,
    previewHtml,
    enabled: isOpen && Boolean(effectiveProjectId),
  });

  if (!isOpen) return null;

  const handleCreatePR = () => {
    const parsed = GitHubPRSchema.safeParse({
      branchName: branch.trim(),
      targetBranch: targetBranch.trim(),
      prTitle: prTitle.trim(),
      prBody,
      includeVisualDiffTable: true,
    });
    if (!parsed.success) {
      setValidationError(
        parsed.error.issues[0]?.message ||
          t("snapshots.invalidPrParams", "Tham số Pull Request không hợp lệ")
      );
      return;
    }
    setValidationError(null);
    onCreateGitHubPR(parsed.data);
  };

  const formProps = { branch, setBranch, targetBranch, setTargetBranch, prTitle, setPrTitle, prBody, setPrBody, validationError };
  const intProps = { initialSection, integrationsStatus, integrationBusy, onConnectGitHub, onSelectGitHubAccount, onBindGitRemote, onSetDefaultIde };
  const resProps = { metadata, handoffResult, handoffError, isExporting, prError, prResult, onOpenIde, onDownloadZip };

  return (
    <div
      className="snap-modal-overlay"
      role="dialog"
      aria-modal="true"
      aria-label={t("splitExport.modalTitle", "Xuất bản & Chia sẻ dự án")}
      data-testid="export-bundle-modal"
      data-ff-id="export-bundle-modal"
    >
      <div className="snap-modal-card snap-modal-card--wide">
        <ExportModalHeader onClose={onClose} />
        <AudienceTabSwitcher activeTab={activeTab} onSelectTab={setActiveTab} />

        {effectiveProjectId && (
          <div style={{ marginBottom: "14px" }}>
            <QcGateBannerCard
              qcGate={splitExport.qcGate}
              overrideQcWarning={splitExport.overrideQcWarning}
              onToggleOverride={splitExport.setOverrideQcWarning}
              errorNotice={splitExport.errorNotice}
            />
          </div>
        )}

        {activeTab === "share-present" ? (
          <SharePresentPanel
            activeFormat={splitExport.activeFormat}
            disabled={!effectiveProjectId}
            htmlResult={splitExport.htmlResult}
            pdfResult={splitExport.pdfResult}
            pptxResult={splitExport.pptxResult}
            pptxPluginInstalled={splitExport.pptxPluginInstalled}
            pptxExportMode={splitExport.pptxExportMode}
            onPptxExportModeChange={splitExport.setPptxExportMode}
            onExportOfflineHtml={() => void splitExport.runOfflineHtmlExport()}
            onExportVectorPdf={() => void splitExport.runVectorPdfExport()}
            onExportPptx={(mode) => void splitExport.runPptxExport(mode)}
            onNavigatePluginInstall={onNavigatePluginInstall}
          />
        ) : (
          <DeveloperHandoffPanel
            activeFormat={splitExport.activeFormat}
            disabled={isExporting}
            hasProjectId={Boolean(effectiveProjectId)}
            devHandoffResult={splitExport.devHandoffResult}
            onExportDeveloperHandoff={() => void splitExport.runDeveloperHandoffExport()}
          >
            <ExportModalBody {...formProps} {...intProps} {...resProps} />
          </DeveloperHandoffPanel>
        )}

        <ExportModalFooter
          activeTab={activeTab}
          isExporting={isExporting}
          onClose={onClose}
          onCreatePR={handleCreatePR}
          onDownloadZip={onDownloadZip}
        />
      </div>
    </div>
  );
};
