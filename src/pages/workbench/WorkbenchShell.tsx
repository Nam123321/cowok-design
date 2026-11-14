// @story 04.1
// @implements AC-01, AC-02
import React from "react";
import {
  WorkbenchNode,
  WorkbenchTransition,
} from "../../features/flow/store/flow-workbench-store";
import { WorkbenchTopNav, ThemeMode } from "./components/WorkbenchTopNav";
import { SettingsModal } from "./components/SettingsModal";
import { NewConversationModal } from "./components/NewConversationModal";
import { OnboardingWizardModal } from "./components/OnboardingWizardModal";
import { WorkbenchBodyLayout } from "./components/WorkbenchBodyLayout";
import { useWorkbenchShellState } from "./hooks/useWorkbenchShellState";
import { DEFAULT_FLOW_PROJECT_ID, DEFAULT_INITIAL_NODES, DEFAULT_INITIAL_TRANSITIONS } from "./constants";
import { useTranslation } from "../../hooks/useTranslation";
import { apiHarnessDaemon } from "../../api/harness-daemon";

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

async function pickAndLinkProjectFolder(
  projId: string,
  onLink: (id: string, path: string) => void
): Promise<void> {
  if (typeof window !== "undefined" && "showDirectoryPicker" in window) {
    try {
      const dirHandle = await (window as unknown as { showDirectoryPicker: () => Promise<{ name?: string }> }).showDirectoryPicker();
      if (dirHandle?.name) {
        try {
          const data = await apiHarnessDaemon.issuePathToken(projId, {
            relative_path: dirHandle.name,
          });
          onLink(projId, data.normalized_relative_path || `./projects/${dirHandle.name}`);
          return;
        } catch {
          // Fallback to relative path
        }
        onLink(projId, `./projects/${dirHandle.name}`);
        return;
      }
    } catch {
      return;
    }
  }
  onLink(projId, `./projects/${projId}`);
}

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
      onLinkProjectFolder={(id) => void pickAndLinkProjectFolder(id, sm.handleLinkProjectFolder)}
      activeTabId={shell.activeTabId}
      onTabChange={(id, format) => { shell.setActiveTabId(id); store.setActiveFormatTab(format); }}
      splitter={shell.splitter} attachments={shell.attachments} graphHandlers={shell.graphHandlers}
      state={state} store={store}
    />
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
  const activeProjObj = sessionManager.projects.find(
    (p) => p.id === (activeSession?.projectId || sessionManager.activeProjectId || projectId)
  );
  const resolvedProjectName = activeProjObj?.name || projectName;
  const resolvedProjectId = activeProjObj?.id || sessionManager.activeProjectId || projectId;

  const [isOnboardingOpen, setIsOnboardingOpen] = React.useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    return !window.localStorage.getItem("cowok_first_run_completed");
  });

  return (
    <div
      className={`workbench-shell theme-${settings.themeMode}`}
      data-theme={settings.themeMode}
      data-flow-hydrated={shell.isFlowHydrated ? "true" : "false"}
      style={createWorkbenchShellStyle(settings.currentFontSize)}
    >
      <WorkbenchTopNav projectName={resolvedProjectName} activeSessionTitle={activeSession?.title} isStandaloneConversation={!activeSession?.projectId} themeMode={settings.themeMode} />
      <WorkbenchConnectedBody shell={shell} resolvedProjectName={resolvedProjectName} resolvedProjectId={resolvedProjectId} defaultSessionTitle={t("workbench.defaultNewSessionTitle", "Phiên thiết kế mới")} />
      <NewConversationModal
        isOpen={sessionManager.isNewConvoModalOpen} onClose={() => sessionManager.setIsNewConvoModalOpen(false)}
        projects={sessionManager.projects} activeProjectId={sessionManager.activeProjectId}
        onCreateInProject={sessionManager.handleCreateInProject} onCreateNewProject={sessionManager.handleCreateNewProject}
        onCreateStandalone={sessionManager.handleCreateStandalone}
      />
      <OnboardingWizardModal
        isOpen={isOnboardingOpen}
        onClose={() => {
          if (typeof window !== "undefined") {
            window.localStorage.setItem("cowok_first_run_completed", "true");
          }
          setIsOnboardingOpen(false);
        }}
        onComplete={(_cfg) => {
          if (typeof window !== "undefined") {
            window.localStorage.setItem("cowok_first_run_completed", "true");
          }
          setIsOnboardingOpen(false);
        }}
      />
      <WorkbenchSettingsDialog settings={settings} />
    </div>
  );
};
