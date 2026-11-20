// @story 04.4
// @implements AC-02, AC-04 (Dual-Surface Chat Store bridging Left-Open Chat, Floating Chat, SSE & Sub-Sessions)
import type { SdiClarificationCardData } from "@/components/workbench/ChatStreamPrimitives";

export interface SharedChatMessage {
  id: string;
  sender: "human" | "agent";
  text: string;
  timestamp: string;
  createdAtMs: number;
  nodeId?: string;
  jobId?: string;
  projectSeq?: number;
  fencingToken?: number;
  sdiClarification?: SdiClarificationCardData;
}

export interface SharedChatSnapshot {
  version: number;
  subSessionsVersion: number;
}

export const SESSION_FORKED_EVENT = "cowok:session-forked";

export interface SessionForkedEventDetail {
  projectId?: string;
  sourceSessionId: string;
  forkedSessionId: string;
  forkedTitle?: string;
  entryKind?: "fe-dag" | "presentation";
  detachedNodeIds?: string[];
  cutoffMessageId?: string;
  cutoffCreatedAtMs?: number;
  copiedMessagesCount?: number;
}

export function emitSessionForked(detail: SessionForkedEventDetail): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent<SessionForkedEventDetail>(SESSION_FORKED_EVENT, { detail }));
}

export function toSessionChatKey(projectId: string | undefined, sessionId: string | undefined): string {
  const sess = sessionId || "session-1";
  return projectId ? `${projectId}::${sess}` : `default::${sess}`;
}

const SHARED_CHAT_STORAGE_KEY = "cowok:shared-chat:v1";
const MAX_PERSISTED_MSGS_PER_SCOPE = 80;

function readPersistedChatStore(): { messages: Record<string, SharedChatMessage[]>; drafts: Record<string, string> } {
  if (typeof window === "undefined") return { messages: {}, drafts: {} };
  try {
    const raw = window.sessionStorage?.getItem(SHARED_CHAT_STORAGE_KEY);
    if (!raw) return { messages: {}, drafts: {} };
    const parsed = JSON.parse(raw) as { messages?: Record<string, SharedChatMessage[]>; drafts?: Record<string, string> };
    return {
      messages: parsed?.messages && typeof parsed.messages === "object" ? parsed.messages : {},
      drafts: parsed?.drafts && typeof parsed.drafts === "object" ? parsed.drafts : {},
    };
  } catch {
    return { messages: {}, drafts: {} };
  }
}

class SharedChatSyncStore {
  private messagesByScope: Map<string, SharedChatMessage[]>;
  private draftByScope: Map<string, string>;
  private clearedScopes = new Set<string>();
  private listeners = new Set<() => void>();
  private snapshot: SharedChatSnapshot = { version: 0, subSessionsVersion: 0 };

  constructor() {
    const initial = readPersistedChatStore();
    this.messagesByScope = new Map(Object.entries(initial.messages));
    this.draftByScope = new Map(Object.entries(initial.drafts));
  }

  private persist(): void {
    if (typeof window === "undefined") return;
    try {
      const messagesObj: Record<string, SharedChatMessage[]> = {};
      for (const [k, list] of this.messagesByScope.entries()) {
        if (list.length > 0) messagesObj[k] = list.slice(-MAX_PERSISTED_MSGS_PER_SCOPE);
      }
      const draftsObj: Record<string, string> = {};
      for (const [k, v] of this.draftByScope.entries()) {
        if (v) draftsObj[k] = v;
      }
      window.sessionStorage?.setItem(SHARED_CHAT_STORAGE_KEY, JSON.stringify({ messages: messagesObj, drafts: draftsObj }));
    } catch { /* ignore storage quota errors */ }
  }

  public subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };

  public getSnapshot = (): SharedChatSnapshot => this.snapshot;

  private emit(bumpSubSessions = false): void {
    this.persist();
    this.snapshot = {
      version: this.snapshot.version + 1,
      subSessionsVersion: this.snapshot.subSessionsVersion + (bumpSubSessions ? 1 : 0),
    };
    for (const listener of this.listeners) {
      try { listener(); } catch { /* ignore listener errors */ }
    }
  }

  public getDraft(scopeKey: string): string {
    return this.draftByScope.get(scopeKey) ?? "";
  }

  public setDraft(scopeKey: string, val: string): void {
    if ((this.draftByScope.get(scopeKey) ?? "") === val) return;
    if (val) this.draftByScope.set(scopeKey, val);
    else this.draftByScope.delete(scopeKey);
    this.emit(false);
  }

  public getMessages(scopeKey: string): SharedChatMessage[] {
    return this.messagesByScope.get(scopeKey) ?? [];
  }

  public appendMessage(scopeKey: string, msg: SharedChatMessage): void {
    const list = this.messagesByScope.get(scopeKey) ?? [];
    if (list.some((item) => item.id === msg.id)) return;
    this.messagesByScope.set(scopeKey, [...list, msg]);
    if (msg.nodeId) {
      this.clearedScopes.delete(`${scopeKey}::${msg.nodeId}`);
    }
    this.emit(false);
  }

  public resolveSdiClarification(scopeKey: string, intentId: string, selectedVariantId: string, status: "ready" | "executed"): void {
    const list = this.messagesByScope.get(scopeKey);
    if (!list) return;
    let changed = false;
    const next = list.map((item) => {
      if (!item.sdiClarification || item.sdiClarification.intentId !== intentId) return item;
      changed = true;
      return { ...item, sdiClarification: { ...item.sdiClarification, status, selectedVariantId } };
    });
    if (!changed) return;
    this.messagesByScope.set(scopeKey, next);
    this.emit(false);
  }

  public bumpSubSessionsVersion(): void {
    this.emit(true);
  }

  public markScopeCleared(compositeScopeKey: string): void {
    this.clearedScopes.add(compositeScopeKey);
    this.emit(true);
  }

  public unmarkScopeCleared(compositeScopeKey: string): void {
    if (this.clearedScopes.delete(compositeScopeKey)) {
      this.emit(true);
    }
  }

  public isScopeCleared(compositeScopeKey: string): boolean {
    return this.clearedScopes.has(compositeScopeKey);
  }

  public copyBranchMessages(
    sourceScopeKey: string,
    targetScopeKey: string,
    inclusiveMessageId?: string,
    cutoffCreatedAtMs?: number
  ): void {
    const src = this.messagesByScope.get(sourceScopeKey) ?? [];
    if (src.length === 0) return;
    let slice = src;
    const idx = inclusiveMessageId ? src.findIndex((m) => m.id === inclusiveMessageId) : -1;
    if (idx >= 0) {
      slice = src.slice(0, idx + 1);
    } else if (typeof cutoffCreatedAtMs === "number" && Number.isFinite(cutoffCreatedAtMs)) {
      slice = src.filter((m) => m.createdAtMs <= cutoffCreatedAtMs);
    }
    if (slice.length === 0) return;
    this.messagesByScope.set(targetScopeKey, slice.map((m) => ({ ...m })));
    this.emit(false);
  }

  public clearAll(): void {
    this.messagesByScope.clear();
    this.draftByScope.clear();
    this.clearedScopes.clear();
    if (typeof window !== "undefined") {
      try { window.sessionStorage?.removeItem(SHARED_CHAT_STORAGE_KEY); } catch { /* ignore */ }
    }
    this.emit(true);
  }
}

export const sharedChatStore = new SharedChatSyncStore();
