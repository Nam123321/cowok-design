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
  isPinned?: boolean;
  updatedAt: string;
  subSessions?: NodeSubSessionItem[];
}

export interface PersistedWorkbenchSettings {
  theme: "warm" | "dim" | "night" | "night-low" | "monochrome";
  locale: string;
  fontSize: string;
  defaultModel: string;
  byosEndpoint: string;
  autoContextSlice: boolean;
}
