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
import { apiWorkbenchSettings } from "../../api/workbench-settings";
import { CowokToast } from "../../components/ui/CowokToast";
import { SupportRequestDrawer } from "../../features/support/SupportRequestDrawer";
import { ProjectReadinessModal } from "../../features/visual-qc/ProjectReadinessModal";
import { DesignTournamentStudioDrawer } from "../../features/tournament/DesignTournamentStudioDrawer";
import { useVisualQcController } from "../../features/visual-qc/useVisualQcController";
import { resolveStageTabKind, resolveStageTabTitle } from "./components/ContextualStageToolbar";

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
  projectId: string;
}> = ({ settings, projectId }) => (
  <SettingsModal
    isOpen={settings.isSettingsOpen}
    onClose={() => settings.setIsSettingsOpen(false)}
    projectId={projectId}
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
  isOnboardingOpen: boolean;
  isSupportDrawerOpen: boolean;
  isTournamentOpen: boolean;
  tournamentRunCount: number;
  onOpenVisualQc: () => void;
  onOpenTournament: () => void;
  resolvedProjectName: string;
  resolvedProjectId: string;
  defaultSessionTitle: string;
}> = ({
  shell,
  isOnboardingOpen,
  isSupportDrawerOpen,
  isTournamentOpen,
  tournamentRunCount,
  onOpenVisualQc,
  onOpenTournament,
  resolvedProjectName,
  resolvedProjectId,
  defaultSessionTitle,
}) => {
  const { settings, sessionManager: sm, store, state } = shell;
  const anyShellModalOpen = Boolean(
    settings.isSettingsOpen || sm.isNewConvoModalOpen || sm.pickerState.isOpen || isOnboardingOpen || isSupportDrawerOpen || isTournamentOpen
  );
  return (
    <WorkbenchBodyLayout
      projectName={resolvedProjectName} projectId={resolvedProjectId} projects={sm.projects} sessions={sm.sessions}
      activeSessionId={sm.activeSessionId} onSelectSession={sm.setActiveSessionId}
      onOpenNewConversationModal={() => sm.setIsNewConvoModalOpen(true)} onOpenSettings={settings.openSettings}
      currentLocale={settings.currentLocale} themeMode={settings.themeMode} onThemeChange={settings.setThemeMode}
      isSidebarCollapsed={shell.isSidebarCollapsed} onToggleSidebar={shell.toggleSidebar}
      onToggleProjectFolder={sm.toggleProjectFolder}
      onCreateSessionInProject={(id, customTitle, entryKind) => sm.handleCreateInProject(id, customTitle || defaultSessionTitle, entryKind)}
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
      isModalOpen={anyShellModalOpen}
      onOpenVisualQc={onOpenVisualQc}
      onOpenTournament={onOpenTournament}
      activeTournamentRunCount={tournamentRunCount}
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
  const hasUndo = Boolean(undoProjectId && onUndo);
  return (
    <CowokToast
      tone={kind}
      message={message}
      testId={testId}
      actionLabel={hasUndo ? t("workbench.undoRestoreBtn", "Hoàn tác") : undefined}
      actionTestId={hasUndo ? "toast-undo-restore-btn" : undefined}
      onAction={hasUndo && undoProjectId && onUndo ? () => onUndo(undoProjectId) : undefined}
      onDismiss={onDismiss}
      durationMs={hasUndo ? 0 : 5000}
    />
  );
};

const WorkbenchModalsAndToasts: React.FC<{
  shell: ReturnType<typeof useWorkbenchShellState>;
  resolvedProjectId: string;
  isOnboardingOpen: boolean;
  daemonWarning: string | null;
  onCloseOnboarding: () => void;
  onSetDaemonWarning: (msg: string | null) => void;
}> = ({ shell, resolvedProjectId, isOnboardingOpen, daemonWarning, onCloseOnboarding, onSetDaemonWarning }) => {
  const { settings, sessionManager } = shell;
  return (
    <>
      <NewConversationModal
        isOpen={sessionManager.isNewConvoModalOpen} onClose={() => sessionManager.setIsNewConvoModalOpen(false)}
        projects={sessionManager.projects} activeProjectId={sessionManager.activeProjectId}
        existingSessions={sessionManager.sessions}
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
      <WorkbenchSettingsDialog settings={settings} projectId={resolvedProjectId} />
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

  const [isOnboardingOpen, setIsOnboardingOpen] = React.useState<boolean>(false);
  const [isSupportDrawerOpen, setIsSupportDrawerOpen] = React.useState<boolean>(false);
  const [isTournamentOpen, setIsTournamentOpen] = React.useState<boolean>(false);
  const [tournamentRunCount, setTournamentRunCount] = React.useState<number>(0);
  const [daemonWarning, setDaemonWarning] = React.useState<string | null>(null);
  const activeTabKind = resolveStageTabKind(shell.activeTabId, shell.state.activeFormatTab);
  const activeTabTitle = resolveStageTabTitle(shell.activeTabId, shell.state.activeFormatTab);
  const visualQc = useVisualQcController({
    projectId: resolvedProjectId,
    targetId: shell.activeTabId || "workbench-stage",
    targetKind: "APP_SCREEN",
    html: "<main><section data-ff-id='workbench-root'><h1>Workspace</h1></section></main>",
  });
  React.useEffect(() => {
    if (typeof window === "undefined") return;
    let cancelled = false;
    void apiWorkbenchSettings
      .listAgentSettings()
      .then((rows) => {
        if (cancelled) return;
        const alreadyOnboarded = rows.some((row) => Boolean(row.onboardingDone));
        setIsOnboardingOpen(!alreadyOnboarded);
      })
      .catch(() => {
        // Keep wizard closed if daemon is unreachable or tests mock partial endpoints.
      });
    return () => {
      cancelled = true;
    };
  }, []);
  const closeOnboarding = () => {
    setIsOnboardingOpen(false);
  };

  React.useEffect(() => {
    if (!isTournamentOpen) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsTournamentOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isTournamentOpen]);

  return (
    <div className={`workbench-shell theme-${settings.themeMode}`} data-theme={settings.themeMode} data-flow-hydrated={shell.isFlowHydrated ? "true" : "false"} style={createWorkbenchShellStyle(settings.currentFontSize)}>
      <WorkbenchTopNav projectId={resolvedProjectId} projectName={resolvedProjectName} activeSessionTitle={activeSession?.title} isStandaloneConversation={!activeSession?.projectId} themeMode={settings.themeMode} onOpenSupportDrawer={() => setIsSupportDrawerOpen(true)} />
      <WorkbenchConnectedBody shell={shell} isOnboardingOpen={isOnboardingOpen} isSupportDrawerOpen={isSupportDrawerOpen} isTournamentOpen={isTournamentOpen} tournamentRunCount={tournamentRunCount} onOpenVisualQc={() => visualQc.setIsReadinessOpen(true)} onOpenTournament={() => setIsTournamentOpen(true)} resolvedProjectName={resolvedProjectName} resolvedProjectId={resolvedProjectId} defaultSessionTitle={t("workbench.defaultNewSessionTitle", "Phiên thiết kế mới")} />
      <WorkbenchModalsAndToasts shell={shell} resolvedProjectId={resolvedProjectId} isOnboardingOpen={isOnboardingOpen} daemonWarning={daemonWarning} onCloseOnboarding={closeOnboarding} onSetDaemonWarning={setDaemonWarning} />
      <ProjectReadinessModal open={visualQc.isReadinessOpen} onClose={() => visualQc.setIsReadinessOpen(false)} runResult={visualQc.runResult} isRunning={visualQc.isRunning} onRunQc={() => void visualQc.runQc()} onDeterministicFix={() => void visualQc.runDeterministicFix()} />
      <DesignTournamentStudioDrawer
        isOpen={isTournamentOpen}
        onClose={() => setIsTournamentOpen(false)}
        projectId={resolvedProjectId}
        activeTabTitle={activeTabTitle}
        activeTabKind={activeTabKind}
        onActiveRunCountChange={setTournamentRunCount}
        onCommittedHtml={(mergedHtml) => {
          if (typeof window !== "undefined") {
            window.dispatchEvent(new CustomEvent("cowok:tournament-committed", { detail: { projectId: resolvedProjectId, mergedHtml } }));
          }
        }}
      />
      <SupportRequestDrawer isOpen={isSupportDrawerOpen} onClose={() => setIsSupportDrawerOpen(false)} locale={settings.currentLocale} />
    </div>
  );
};
