// @story 05.3
import React, { useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type { ProjectIntegrationsStatus } from "../../../lib/validations/github-ide-integration";

export interface GitHubAccountSectionProps {
  status: ProjectIntegrationsStatus | null;
  busy?: boolean;
  onConnectGitHub: (input: { token?: string; useGhCli?: boolean }) => Promise<void>;
  onSelectGitHubAccount: (login: string) => Promise<void>;
  onBindGitRemote: (remoteUrl: string) => Promise<void>;
}

const InlineAccountForm: React.FC<{
  ghCliInstalled: boolean;
  busy: boolean;
  errorMsg: string | null;
  onConnectPat: (token: string) => Promise<void>;
  onConnectGhCli: () => Promise<void>;
  onCancel?: () => void;
}> = ({ ghCliInstalled, busy, errorMsg, onConnectPat, onConnectGhCli, onCancel }) => {
  const { t } = useTranslation();
  const [token, setToken] = useState("");

  return (
    <div className="snap-setup-card" data-testid="github-add-account-form">
      <div className="snap-item-header">
        <strong>{t("snapshots.githubConnectTitle", "Connect GitHub Account")}</strong>
        {onCancel && (
          <button type="button" className="snap-btn snap-btn--ghost snap-btn--compact" onClick={onCancel}>
            {t("common.cancel", "Cancel")}
          </button>
        )}
      </div>
      <input
        type="password"
        value={token}
        onChange={(e) => setToken(e.target.value)}
        placeholder="ghp_... / github_pat_..."
        className="snap-form-input"
        data-testid="github-pat-input"
      />
      {errorMsg && <p className="snap-form-error" data-testid="github-setup-error">{errorMsg}</p>}
      <div className="snap-item-actions">
        <button
          type="button"
          className="snap-btn snap-btn--primary snap-btn--compact"
          data-testid="github-connect-pat-btn"
          disabled={busy || token.trim().length === 0}
          onClick={() => void onConnectPat(token.trim())}
        >
          {t("snapshots.verifySavePatBtn", "Verify & Save Token")}
        </button>
        {ghCliInstalled && (
          <button
            type="button"
            className="snap-btn snap-btn--compact"
            data-testid="github-connect-ghcli-btn"
            disabled={busy}
            onClick={() => void onConnectGhCli()}
          >
            {t("snapshots.useGhCliBtn", "Use gh CLI Session")}
          </button>
        )}
      </div>
    </div>
  );
};

const InlineRemoteForm: React.FC<{
  initialUrl?: string | null;
  busy: boolean;
  errorMsg: string | null;
  onBind: (url: string) => Promise<void>;
  onCancel?: () => void;
}> = ({ initialUrl, busy, errorMsg, onBind, onCancel }) => {
  const { t } = useTranslation();
  const [url, setUrl] = useState(initialUrl || "");

  return (
    <div className="snap-setup-card" data-testid="git-remote-bind-form">
      <div className="snap-item-header">
        <strong>{t("snapshots.bindRemoteTitle", "Link Git Repository (origin)")}</strong>
        {onCancel && (
          <button type="button" className="snap-btn snap-btn--ghost snap-btn--compact" onClick={onCancel}>
            {t("common.cancel", "Cancel")}
          </button>
        )}
      </div>
      <input
        type="text"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        placeholder="https://github.com/owner/repo.git"
        className="snap-form-input"
        data-testid="git-remote-url-input"
      />
      {errorMsg && <p className="snap-form-error" data-testid="git-remote-error">{errorMsg}</p>}
      <button
        type="button"
        className="snap-btn snap-btn--primary snap-btn--compact"
        data-testid="bind-git-remote-btn"
        disabled={busy || url.trim().length === 0}
        onClick={() => void onBind(url.trim())}
      >
        {t("snapshots.saveOriginBtn", "Bind origin Remote")}
      </button>
    </div>
  );
};

type GithubInfo = ProjectIntegrationsStatus["github"];

const GitHubInlineSetupView: React.FC<{
  gh: GithubInfo | undefined;
  busy: boolean;
  localError: string | null;
  showAddAccount: boolean;
  setShowAddAccount: React.Dispatch<React.SetStateAction<boolean>>;
  onRunConnect: (payload: { token?: string; useGhCli?: boolean }) => Promise<void>;
  onRunBindRemote: (url: string) => Promise<void>;
}> = ({ gh, busy, localError, showAddAccount, setShowAddAccount, onRunConnect, onRunBindRemote }) => {
  const { t } = useTranslation();
  return (
    <div className="snap-setup-card" data-testid="github-inline-setup-card">
      <strong>{t("snapshots.githubSetupRequiredTitle", "Setup GitHub Sync to Push Pull Request")}</strong>
      <span className="snap-meta-label">
        {t("snapshots.githubSetupRequiredDesc", "Complete setup inline below without leaving this export workflow.")}
      </span>
      {(!gh?.connected || showAddAccount) && (
        <InlineAccountForm
          ghCliInstalled={Boolean(gh?.ghCliInstalled)}
          busy={busy}
          errorMsg={localError}
          onConnectPat={(tok) => onRunConnect({ token: tok })}
          onConnectGhCli={() => onRunConnect({ useGhCli: true })}
        />
      )}
      {gh?.connected && (
        <div className="snap-meta-row">
          <span className="snap-badge snap-badge-safety">@{gh.activeLogin}</span>
          <button type="button" className="snap-btn snap-btn--ghost snap-btn--compact" onClick={() => setShowAddAccount((v) => !v)}>
            {t("snapshots.switchOrAddBtn", "Switch / Add")}
          </button>
        </div>
      )}
      {!gh?.remoteUrl && (
        <InlineRemoteForm initialUrl="" busy={busy} errorMsg={gh?.connected ? localError : null} onBind={onRunBindRemote} />
      )}
    </div>
  );
};

const GitHubAccountSwitcherPills: React.FC<{
  accounts: GithubInfo["accounts"];
  busy: boolean;
  onSelectGitHubAccount: (login: string) => Promise<void>;
}> = ({ accounts, busy, onSelectGitHubAccount }) => {
  if (accounts.length <= 1) return null;
  return (
    <div className="snap-item-actions" style={{ flexWrap: "wrap" }}>
      {accounts.map((acct) => (
        <button
          key={acct.login}
          type="button"
          disabled={busy || acct.active}
          className={`snap-btn snap-btn--compact ${acct.active ? "snap-btn--primary" : ""}`}
          data-testid={`select-github-acct-${acct.login}`}
          onClick={() => void onSelectGitHubAccount(acct.login)}
        >
          @{acct.login}
        </button>
      ))}
    </div>
  );
};

const GitHubAccountSectionHeader: React.FC<{
  gh: GithubInfo;
  setShowAddAccount: React.Dispatch<React.SetStateAction<boolean>>;
  setShowEditRemote: React.Dispatch<React.SetStateAction<boolean>>;
}> = ({ gh, setShowAddAccount, setShowEditRemote }) => {
  const { t } = useTranslation();
  return (
    <>
      <div className="snap-item-header">
        <span>
          <strong>{t("snapshots.confirmAccountPrefix", "Export as GitHub Account")}:</strong>{" "}
          <span className="snap-badge snap-badge-manual">@{gh.activeLogin}</span>
        </span>
        <button type="button" className="snap-btn snap-btn--ghost snap-btn--compact" data-testid="github-add-account-toggle" onClick={() => setShowAddAccount((v) => !v)}>
          {t("snapshots.addSwitchAccountBtn", "+ Add / Switch")}
        </button>
      </div>
      <div className="snap-meta-row">
        <span className="snap-meta-label">
          {t("snapshots.originLabel", "Origin:")} <code>{gh.remoteUrl}</code>
        </span>
        <button type="button" className="snap-btn snap-btn--ghost snap-btn--compact" data-testid="git-remote-edit-toggle" onClick={() => setShowEditRemote((v) => !v)}>
          {t("snapshots.changeRemoteBtn", "Change Repo")}
        </button>
      </div>
    </>
  );
};

const GitHubConnectedConfirmView: React.FC<{
  gh: GithubInfo;
  busy: boolean;
  localError: string | null;
  showAddAccount: boolean;
  setShowAddAccount: React.Dispatch<React.SetStateAction<boolean>>;
  showEditRemote: boolean;
  setShowEditRemote: React.Dispatch<React.SetStateAction<boolean>>;
  onRunConnect: (payload: { token?: string; useGhCli?: boolean }) => Promise<void>;
  onSelectGitHubAccount: (login: string) => Promise<void>;
  onRunBindRemote: (url: string) => Promise<void>;
}> = (p) => (
  <div className="snap-account-card" data-testid="github-account-confirm-card">
    <GitHubAccountSectionHeader gh={p.gh} setShowAddAccount={p.setShowAddAccount} setShowEditRemote={p.setShowEditRemote} />
    <GitHubAccountSwitcherPills accounts={p.gh.accounts} busy={p.busy} onSelectGitHubAccount={p.onSelectGitHubAccount} />
    {p.showAddAccount && (
      <InlineAccountForm ghCliInstalled={p.gh.ghCliInstalled} busy={p.busy} errorMsg={p.localError} onConnectPat={(tok) => p.onRunConnect({ token: tok })} onConnectGhCli={() => p.onRunConnect({ useGhCli: true })} onCancel={() => p.setShowAddAccount(false)} />
    )}
    {p.showEditRemote && (
      <InlineRemoteForm initialUrl={p.gh.remoteUrl} busy={p.busy} errorMsg={p.localError} onBind={p.onRunBindRemote} onCancel={() => p.setShowEditRemote(false)} />
    )}
  </div>
);

function useGitHubSectionState(
  onConnectGitHub: GitHubAccountSectionProps["onConnectGitHub"],
  onBindGitRemote: GitHubAccountSectionProps["onBindGitRemote"]
) {
  const { t } = useTranslation();
  const [showAddAccount, setShowAddAccount] = useState(false);
  const [showEditRemote, setShowEditRemote] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  const runConnect = async (payload: { token?: string; useGhCli?: boolean }) => {
    setLocalError(null);
    try { await onConnectGitHub(payload); setShowAddAccount(false); }
    catch (err) { setLocalError(err instanceof Error ? err.message : t("snapshots.githubConnectErr", "GitHub connection failed")); }
  };
  const runBindRemote = async (url: string) => {
    setLocalError(null);
    try { await onBindGitRemote(url); setShowEditRemote(false); }
    catch (err) { setLocalError(err instanceof Error ? err.message : t("snapshots.bindRemoteErr", "Failed binding git remote")); }
  };
  return { showAddAccount, setShowAddAccount, showEditRemote, setShowEditRemote, localError, runConnect, runBindRemote };
}

export const GitHubAccountSection: React.FC<GitHubAccountSectionProps> = ({
  status, busy = false, onConnectGitHub, onSelectGitHubAccount, onBindGitRemote,
}) => {
  const gh = status?.github;
  const st = useGitHubSectionState(onConnectGitHub, onBindGitRemote);
  if (!gh?.connected || !gh?.remoteUrl) {
    return (
      <GitHubInlineSetupView
        gh={gh} busy={busy} localError={st.localError} showAddAccount={st.showAddAccount}
        setShowAddAccount={st.setShowAddAccount} onRunConnect={st.runConnect} onRunBindRemote={st.runBindRemote}
      />
    );
  }
  return (
    <GitHubConnectedConfirmView
      gh={gh} busy={busy} localError={st.localError} showAddAccount={st.showAddAccount}
      setShowAddAccount={st.setShowAddAccount} showEditRemote={st.showEditRemote} setShowEditRemote={st.setShowEditRemote}
      onRunConnect={st.runConnect} onSelectGitHubAccount={onSelectGitHubAccount} onRunBindRemote={st.runBindRemote}
    />
  );
};
