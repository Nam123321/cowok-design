// @story 05.3
import React, { useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ProjectIntegrationsStatus } from "../../../lib/validations/github-ide-integration";
import type {
  IdeTarget,
  OpenIdeHandoffResponse,
} from "../../../lib/validations/version-snapshot";

export interface HandoffManifestPanelProps {
  handoffResult: OpenIdeHandoffResponse | null;
  isLaunching?: boolean;
  errorMessage?: string | null;
  integrationsStatus?: ProjectIntegrationsStatus | null;
  isFocused?: boolean;
  onOpenIde: (ide: IdeTarget) => void;
  onSetDefaultIde?: (ide: IdeTarget) => void;
}

const IDE_TARGET_OPTIONS: ReadonlyArray<{ id: IdeTarget; labelKey: string; fallback: string; setupCmd: string }> = [
  { id: "antigravity", labelKey: "snapshots.ideAntigravity", fallback: "Open in Antigravity", setupCmd: "antigravity --version" },
  { id: "cursor", labelKey: "snapshots.ideCursor", fallback: "Open in Cursor", setupCmd: "Install Cursor CLI: Cmd+Shift+P -> Shell Command: Install 'cursor' command" },
  { id: "vscode", labelKey: "snapshots.ideVsCode", fallback: "Open in VS Code", setupCmd: "Install VS Code CLI: Cmd+Shift+P -> Shell Command: Install 'code' command" },
  { id: "claude-code", labelKey: "snapshots.ideClaudeCode", fallback: "Open in Claude Code", setupCmd: "npm install -g @anthropic-ai/claude-code" },
];

const IdeTargetGrid: React.FC<{
  activeIde?: IdeTarget;
  isLaunching: boolean;
  integrationsStatus?: ProjectIntegrationsStatus | null;
  onSelectIde: (ide: IdeTarget) => void;
}> = ({ activeIde, isLaunching, integrationsStatus, onSelectIde }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-handoff-grid">
      {IDE_TARGET_OPTIONS.map((opt) => {
        const info = integrationsStatus?.ides.find((item) => item.id === opt.id);
        const isDefault = integrationsStatus?.preferredIde === opt.id;
        return (
          <button
            key={opt.id}
            type="button"
            disabled={isLaunching}
            data-testid={`ide-handoff-btn-${opt.id}`}
            className={`snap-ide-pill${activeIde === opt.id ? " snap-ide-pill--active" : ""}`}
            onClick={() => onSelectIde(opt.id)}
          >
            <span>{t(opt.labelKey, opt.fallback)}</span>
            {info && (
              <span className="snap-ide-meta-badge">
                {info.installed
                  ? (isDefault ? t("snapshots.defaultIdeBadge", "Default") : t("snapshots.ideReadyBadge", "Ready"))
                  : t("snapshots.ideSetupNeededBadge", "Setup")}
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
};

const IdeInlineSetupCard: React.FC<{
  selectedIde: IdeTarget;
  isDefault: boolean;
  onSetDefaultIde?: (ide: IdeTarget) => void;
}> = ({ selectedIde, isDefault, onSetDefaultIde }) => {
  const { t } = useTranslation();
  const opt = IDE_TARGET_OPTIONS.find((item) => item.id === selectedIde);
  return (
    <div className="snap-setup-card" data-testid="ide-inline-setup-card">
      <div className="snap-item-header">
        <strong>{t("snapshots.ideMissingCliTitle", "IDE CLI not detected on PATH")}</strong>
        {onSetDefaultIde && !isDefault && (
          <button
            type="button"
            className="snap-btn snap-btn--ghost snap-btn--compact"
            data-testid={`set-default-ide-${selectedIde}`}
            onClick={() => onSetDefaultIde(selectedIde)}
          >
            {t("snapshots.setDefaultIdeBtn", "Set Default")}
          </button>
        )}
      </div>
      <span>{t("snapshots.ideMissingCliHint", "You can still generate the manifest & launch URI below, or install the CLI:")}</span>
      {opt && <div className="snap-cli-box"><code className="snap-cli-code">{opt.setupCmd}</code></div>}
    </div>
  );
};

const HandoffResultCard: React.FC<{
  handoffResult: OpenIdeHandoffResponse;
  copiedCli: boolean;
  onCopyCli: (cmd: string) => void;
}> = ({ handoffResult, copiedCli, onCopyCli }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-meta-card" data-testid="ide-handoff-result">
      <div className="snap-meta-row">
        <span className="snap-meta-label">{t("snapshots.manifestPathLabel", "Manifest Path:")}</span>
        <span className="snap-meta-value">{handoffResult.manifestPath}</span>
      </div>
      <div className="snap-meta-row">
        <span className="snap-meta-label">{t("snapshots.launchUriLabel", "Launch URI:")}</span>
        <a href={handoffResult.launchUri} className="snap-meta-value" data-testid="ide-handoff-launch-uri">
          {handoffResult.launchUri}
        </a>
      </div>
      <div className="snap-manifest-sha">
        {t("snapshots.manifestShaLabel", "Checksum SHA-256:")} {handoffResult.manifestSha256}
      </div>
      <div className="snap-cli-box">
        <code className="snap-cli-code" data-testid="ide-handoff-cli-fallback">
          {handoffResult.cliFallbackCommand}
        </code>
        <button
          type="button"
          className="snap-btn snap-btn--ghost snap-btn--compact"
          data-testid="ide-handoff-copy-cli-btn"
          onClick={() => onCopyCli(handoffResult.cliFallbackCommand)}
        >
          {copiedCli ? t("snapshots.copiedCliBtn", "Copied") : t("snapshots.copyCliBtn", "Copy CLI")}
        </button>
      </div>
    </div>
  );
};

async function copyCliToClipboard(cmd: string): Promise<boolean> {
  try {
    if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(cmd);
    }
    return true;
  } catch {
    return false;
  }
}

export const HandoffManifestPanel: React.FC<HandoffManifestPanelProps> = ({
  handoffResult,
  isLaunching = false,
  errorMessage = null,
  integrationsStatus = null,
  isFocused = false,
  onOpenIde,
  onSetDefaultIde,
}) => {
  const { t } = useTranslation();
  const [copiedCli, setCopiedCli] = useState(false);
  const [selectedIde, setSelectedIde] = useState<IdeTarget | null>(null);
  const effectiveIde = selectedIde ?? handoffResult?.ide ?? null;
  const selectedInfo = effectiveIde ? integrationsStatus?.ides.find((item) => item.id === effectiveIde) : undefined;

  return (
    <div className={`snap-handoff-panel${isFocused ? " snap-section-focus" : ""}`} data-testid="handoff-manifest-panel">
      <div className="snap-item-header">
        <span className="snap-item-label">{t("snapshots.ideHandoffTitle", "1-Click Open in IDE")}</span>
        <span className="snap-badge snap-badge-manual"><code>cowok-handoff-manifest.json</code></span>
      </div>
      <p className="snap-modal-desc">
        {t("snapshots.ideHandoffDesc", "Generate cowok-handoff-manifest.json inside the project sandbox and launch your IDE directly.")}
      </p>
      <IdeTargetGrid
        activeIde={handoffResult?.ide}
        isLaunching={isLaunching}
        integrationsStatus={integrationsStatus}
        onSelectIde={(ide) => { setCopiedCli(false); setSelectedIde(ide); onOpenIde(ide); }}
      />
      {selectedInfo && !selectedInfo.installed && (
        <IdeInlineSetupCard selectedIde={selectedInfo.id} isDefault={integrationsStatus?.preferredIde === selectedInfo.id} onSetDefaultIde={onSetDefaultIde} />
      )}
      {errorMessage && <p className="snap-form-error" data-testid="ide-handoff-error">{errorMessage}</p>}
      {handoffResult && (
        <HandoffResultCard handoffResult={handoffResult} copiedCli={copiedCli} onCopyCli={(cmd) => { void copyCliToClipboard(cmd).then(setCopiedCli); }} />
      )}
    </div>
  );
};
