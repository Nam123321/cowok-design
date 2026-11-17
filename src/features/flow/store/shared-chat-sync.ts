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

export function toSessionChatKey(projectId: string | undefined, sessionId: string | undefined): string {
  const sess = sessionId || "session-1";
  return projectId ? `${projectId}::${sess}` : `default::${sess}`;
}

class SharedChatSyncStore {
  private messagesByScope = new Map<string, SharedChatMessage[]>();
  private clearedScopes = new Set<string>();
  private listeners = new Set<() => void>();
  private snapshot: SharedChatSnapshot = { version: 0, subSessionsVersion: 0 };

  public subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };

  public getSnapshot = (): SharedChatSnapshot => this.snapshot;

  private emit(bumpSubSessions = false): void {
    this.snapshot = {
      version: this.snapshot.version + 1,
      subSessionsVersion: this.snapshot.subSessionsVersion + (bumpSubSessions ? 1 : 0),
    };
    for (const listener of this.listeners) {
      try { listener(); } catch { /* ignore listener errors */ }
    }
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
    this.clearedScopes.delete(compositeScopeKey);
  }

  public isScopeCleared(compositeScopeKey: string): boolean {
    return this.clearedScopes.has(compositeScopeKey);
  }

  public clearAll(): void {
    this.messagesByScope.clear();
    this.clearedScopes.clear();
    this.emit(true);
  }
}

export const sharedChatStore = new SharedChatSyncStore();
