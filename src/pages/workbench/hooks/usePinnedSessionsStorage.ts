// @story 04.1, 04.2
import type { ChatSession } from "../types";

export const PINNED_SESSION_IDS_KEY = "cowok.pinnedSessionIds";
export const STANDALONE_SESSIONS_KEY = "cowok.standaloneSessions.v1";

export function readStandaloneSessions(): ChatSession[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(STANDALONE_SESSIONS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (s): s is ChatSession =>
        Boolean(s) && typeof s.id === "string" && typeof s.title === "string" && !s.projectId
    );
  } catch {
    return [];
  }
}

export function writeStandaloneSessions(sessions: ChatSession[]): void {
  if (typeof window === "undefined") return;
  try {
    const standalone = sessions.filter((s) => !s.projectId || s.projectId === "general" || s.projectId === "standalone");
    window.localStorage.setItem(STANDALONE_SESSIONS_KEY, JSON.stringify(standalone));
  } catch {
    // Ignore storage quota / private mode failures
  }
}

export function readPinnedSessionIds(): Set<string> {
  if (typeof window === "undefined") return new Set();
  try {
    const raw = window.localStorage.getItem(PINNED_SESSION_IDS_KEY);
    if (!raw) return new Set();
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? new Set(parsed.filter((v): v is string => typeof v === "string")) : new Set();
  } catch {
    return new Set();
  }
}

export function writePinnedSessionIds(sessions: ChatSession[]): void {
  if (typeof window === "undefined") return;
  try {
    const ids = sessions.filter((s) => s.isPinned).map((s) => s.id);
    window.localStorage.setItem(PINNED_SESSION_IDS_KEY, JSON.stringify(ids));
  } catch {
    // Ignore storage quota / private mode failures
  }
}
