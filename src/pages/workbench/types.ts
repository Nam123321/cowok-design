// @story 04.1, 04.3, 04.4
export interface NodeSubSessionItem {
  nodeId: string;
  nodeTitle: string;
  screenKey: string;
  compositeScopeKey?: string;
  messageCount?: number;
  status?: "idle" | "running" | "queued" | "paused" | "interrupted" | "completed";
}

export interface ChatSession {
  id: string;
  title: string;
  projectId?: string;
  entryKind?: "fe-dag" | "presentation";
  isPinned?: boolean;
  updatedAt: string;
  subSessions?: NodeSubSessionItem[];
}

export interface PostCreateProjectOptions {
  openMaterialLaunchpad?: boolean;
  githubRepoUrl?: string;
}

export interface PersistedWorkbenchSettings {
  theme: "warm" | "dim" | "night" | "night-low" | "monochrome";
  locale: string;
  fontSize: string;
  defaultModel: string;
  byosEndpoint: string;
  autoContextSlice: boolean;
}
