// @story 04.1, 04.2
// @implements AC-01, AC-02, AC-03, AC-04
import React from "react";
import { WorkbenchNode, WorkbenchTransition } from "../../features/flow/store/flow-workbench-store";
import { WorkbenchTopNav, ThemeMode } from "./components/WorkbenchTopNav";
import { SettingsModal } from "./components/SettingsModal";
import { NewConversationModal } from "./components/NewConversationModal";
import { OnboardingWizardModal } from "./components/OnboardingWizardModal";
import { DaemonFolderPickerDialog } from "./components/DaemonFolderPickerDialog";
import { WorkbenchBodyLayout } from "./components/WorkbenchBodyLayout";
import { useWorkbenchShellState } from "./hooks/useWorkbenchShellState";
import { DEFAULT_FLOW_PROJECT_ID, DEFAULT_INITIAL_NODES, DEFAULT_INITIAL_TRANSITIONS } from "./constants";
import { useTranslation } from "../../hooks/useTranslation";

export type { ThemeMode };

export interface WorkbenchShellProps {
  projectId?: string;
  projectName?: string;
  initialNodes?: WorkbenchNode[];
  initialTransitions?: WorkbenchTransition[];
}

function createWorkbenchShellStyle(fontSize: string): React.CSSProperties {
  return {
    display: "flex", flexDirection: "column", width: "100vw", height: "100vh",
    overflow: "hidden", backgroundColor: "var(--bg-canvas)", color: "var(--text-primary)",
    fontFamily: "var(--font-sans)", fontSize,
  };
}

export const WorkbenchSettingsDialog: React.FC<{
  settings: ReturnType<typeof useWorkbenchShellState>["settings"];
}> = ({ settings }) => (
  <SettingsModal
    isOpen={settings.isSettingsOpen}
    onClose={() => settings.setIsSettingsOpen(false)}
    currentTheme={settings.themeMode}
    onThemeChange={settings.setThemeMode}
    currentLocale={settings.currentLocale}
    onLocaleChange={settings.setCurrentLocale}
    currentFontSize={settings.currentFontSize}
    onFontSizeChange={settings.setCurrentFontSize}
    activeTab={settings.settingsActiveTab}
  />
);

const WorkbenchConnectedBody: React.FC<{
  shell: ReturnType<typeof useWorkbenchShellState>;
  resolvedProjectName: string;
  resolvedProjectId: string;
  defaultSessionTitle: string;
}> = ({ shell, resolvedProjectName, resolvedProjectId, defaultSessionTitle }) => {
  const { settings, sessionManager: sm, store, state } = shell;
  return (
    <WorkbenchBodyLayout
      projectName={resolvedProjectName} projectId={resolvedProjectId} projects={sm.projects} sessions={sm.sessions}
      activeSessionId={sm.activeSessionId} onSelectSession={sm.setActiveSessionId}
      onOpenNewConversationModal={() => sm.setIsNewConvoModalOpen(true)} onOpenSettings={settings.openSettings}
      currentLocale={settings.currentLocale} themeMode={settings.themeMode} onThemeChange={settings.setThemeMode}
      isSidebarCollapsed={shell.isSidebarCollapsed} onToggleSidebar={shell.toggleSidebar}
      onToggleProjectFolder={sm.toggleProjectFolder}
      onCreateSessionInProject={(id) => sm.handleCreateInProject(id, defaultSessionTitle)}
      onTogglePinSession={sm.handleTogglePinSession} onRenameSession={sm.handleRenameSession}
      onDeleteSession={sm.handleDeleteSession} onRenameProject={sm.handleRenameProject}
      onDeleteProject={sm.handleDeleteProject}
      onLinkProjectFolder={(id) => {
        const target = sm.projects.find((p) => p.id === id);
        sm.openPromoteFolderPicker(id, target?.name);
      }}
      activeTabId={shell.activeTabId}
      onTabChange={(id, format) => { shell.setActiveTabId(id); store.setActiveFormatTab(format); }}
      splitter={shell.splitter} attachments={shell.attachments} graphHandlers={shell.graphHandlers}
      state={state} store={store}
    />
  );
};

const WorkbenchToastBar: React.FC<{
  kind: "success" | "danger" | "warning";
  message: string;
  undoProjectId?: string;
  testId: string;
  onUndo?: (id: string) => void;
  onDismiss: () => void;
}> = ({ kind, message, undoProjectId, testId, onUndo, onDismiss }) => {
  const { t } = useTranslation();
  return (
    <div role="alert" data-testid={testId} className={`ff-toast ff-toast-${kind}`}>
      <span>{message}</span>
      {undoProjectId && onUndo && (
        <button
          type="button"
          onClick={() => onUndo(undoProjectId)}
          data-testid="toast-undo-restore-btn"
          className="cowok-btn cowok-btn-secondary"
          style={{ fontSize: "11px", padding: "2px 8px", flexShrink: 0 }}
        >
          {t("workbench.undoRestoreBtn", "Hoàn tác")}
        </button>
      )}
      <button
        type="button"
        onClick={onDismiss}
        aria-label={t("common.close", "Đóng")}
        style={{ border: "none", background: "transparent", color: "currentColor", cursor: "pointer" }}
      >
        ✕
      </button>
    </div>
  );
};

const WorkbenchModalsAndToasts: React.FC<{
  shell: ReturnType<typeof useWorkbenchShellState>;
  isOnboardingOpen: boolean;
  daemonWarning: string | null;
  onCloseOnboarding: () => void;
  onSetDaemonWarning: (msg: string | null) => void;
}> = ({ shell, isOnboardingOpen, daemonWarning, onCloseOnboarding, onSetDaemonWarning }) => {
  const { settings, sessionManager } = shell;
  return (
    <>
      <NewConversationModal
        isOpen={sessionManager.isNewConvoModalOpen} onClose={() => sessionManager.setIsNewConvoModalOpen(false)}
        projects={sessionManager.projects} activeProjectId={sessionManager.activeProjectId}
        onCreateInProject={sessionManager.handleCreateInProject} onCreateNewProject={sessionManager.handleCreateNewProject}
        onCreateStandalone={sessionManager.handleCreateStandalone}
      />
      <DaemonFolderPickerDialog
        isOpen={sessionManager.pickerState.isOpen} mode={sessionManager.pickerState.mode}
        projectId={sessionManager.pickerState.projectId} initialSlug={sessionManager.pickerState.initialSlug}
        onClose={sessionManager.closeFolderPicker}
        onConfirmPromote={async (projId, targetDir) => { await sessionManager.handleConfirmPromote(projId, targetDir); }}
      />
      <OnboardingWizardModal
        isOpen={isOnboardingOpen} onDaemonSyncWarning={onSetDaemonWarning}
        onClose={onCloseOnboarding} onComplete={onCloseOnboarding}
      />
      {daemonWarning && <WorkbenchToastBar kind="warning" message={daemonWarning} testId="workbench-daemon-warning" onDismiss={() => onSetDaemonWarning(null)} />}
      {sessionManager.toastState && (
        <WorkbenchToastBar
          kind={sessionManager.toastState.kind} message={sessionManager.toastState.message}
          undoProjectId={sessionManager.toastState.undoProjectId} testId="workbench-status-toast"
          onUndo={(id) => void sessionManager.handleRestoreProject(id)} onDismiss={sessionManager.dismissToast}
        />
      )}
      <WorkbenchSettingsDialog settings={settings} />
    </>
  );
};

export const WorkbenchShell: React.FC<WorkbenchShellProps> = ({
  projectId = DEFAULT_FLOW_PROJECT_ID,
  projectName = "cowok-design",
  initialNodes = DEFAULT_INITIAL_NODES,
  initialTransitions = DEFAULT_INITIAL_TRANSITIONS,
}) => {
  const { t } = useTranslation();
  const shell = useWorkbenchShellState(initialNodes, initialTransitions, projectId);
  const { settings, sessionManager } = shell;
  const activeSession = shell.activeSession;
  const activeProjObj = sessionManager.projects.find((p) => p.id === (activeSession?.projectId || sessionManager.activeProjectId || projectId));
  const resolvedProjectName = activeProjObj?.name || projectName;
  const resolvedProjectId = activeProjObj?.id || sessionManager.activeProjectId || projectId;

  const [isOnboardingOpen, setIsOnboardingOpen] = React.useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    return !window.localStorage.getItem("cowok_first_run_completed");
  });
  const [daemonWarning, setDaemonWarning] = React.useState<string | null>(null);
  const closeOnboarding = () => {
    if (typeof window !== "undefined") window.localStorage.setItem("cowok_first_run_completed", "true");
    setIsOnboardingOpen(false);
  };

  return (
    <div className={`workbench-shell theme-${settings.themeMode}`} data-theme={settings.themeMode} data-flow-hydrated={shell.isFlowHydrated ? "true" : "false"} style={createWorkbenchShellStyle(settings.currentFontSize)}>
      <WorkbenchTopNav projectName={resolvedProjectName} activeSessionTitle={activeSession?.title} isStandaloneConversation={!activeSession?.projectId} themeMode={settings.themeMode} />
      <WorkbenchConnectedBody shell={shell} resolvedProjectName={resolvedProjectName} resolvedProjectId={resolvedProjectId} defaultSessionTitle={t("workbench.defaultNewSessionTitle", "Phiên thiết kế mới")} />
      <WorkbenchModalsAndToasts shell={shell} isOnboardingOpen={isOnboardingOpen} daemonWarning={daemonWarning} onCloseOnboarding={closeOnboarding} onSetDaemonWarning={setDaemonWarning} />
    </div>
  );
};
