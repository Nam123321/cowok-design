// @story 04.1, 04.2
import type { ChatSession } from "../types";

export const PINNED_SESSION_IDS_KEY = "cowok.pinnedSessionIds";

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
