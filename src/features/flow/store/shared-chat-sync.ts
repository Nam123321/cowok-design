// @story 04.4, 04.6
// @implements AC-01, AC-02, AC-04 (Dual-Surface SQLite-Backed Chat Store Replacing sessionStorage History)
import { apiChatVault } from "@/api/chat-vault";
import type { SdiClarificationCardData } from "@/components/workbench/ChatStreamPrimitives";
import type { VaultReceiptDto } from "@/lib/validations/chat-vault";

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
  vaultReceipt?: VaultReceiptDto | null;
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

class SharedChatSyncStore {
  private messagesByScope = new Map<string, SharedChatMessage[]>();
  private draftByScope = new Map<string, string>();
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

  public async hydrateFromSqlite(
    scopeKey: string,
    sessionId: string,
    formatTime: (v: number) => string
  ): Promise<SharedChatMessage[]> {
    const res = await apiChatVault.getMessages(sessionId);
    const loaded: SharedChatMessage[] = res.messages.map((m) => ({
      id: m.id,
      sender: m.role === "user" ? "human" : "agent",
      text: m.proseMarkdown,
      timestamp: formatTime(m.createdAt),
      createdAtMs: m.createdAt,
      projectSeq: m.seqNo,
      vaultReceipt: m.vaultReceipt ?? null,
    }));
    this.messagesByScope.set(scopeKey, loaded);
    this.emit(false);
    return loaded;
  }

  public async persistAndAppend(
    scopeKey: string,
    sessionId: string,
    msg: SharedChatMessage,
    projectId?: string
  ): Promise<void> {
    this.appendMessage(scopeKey, msg);
    const saved = await apiChatVault.appendMessage(sessionId, {
      role: msg.sender === "human" ? "user" : "assistant",
      proseMarkdown: msg.text,
      projectId: projectId ?? null,
      vaultReceipt: msg.vaultReceipt ?? undefined,
    });
    const list = this.messagesByScope.get(scopeKey);
    if (!list) return;
    const deduped = list.filter((item) => item.id !== saved.id || item.id === msg.id);
    const reconciled = deduped.map((item) =>
      item.id === msg.id ? { ...item, id: saved.id, projectSeq: saved.seqNo } : item
    );
    this.messagesByScope.set(scopeKey, reconciled);
    this.emit(false);
  }

  public updateVaultReceipt(
    scopeKey: string,
    receiptId: string,
    nextReceipt: VaultReceiptDto | null
  ): void {
    const list = this.messagesByScope.get(scopeKey);
    if (!list) return;
    let changed = false;
    const updated = list.map((item) => {
      if (item.vaultReceipt?.receiptId !== receiptId) return item;
      changed = true;
      return { ...item, vaultReceipt: nextReceipt };
    });
    if (!changed) return;
    this.messagesByScope.set(scopeKey, updated);
    this.emit(false);
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
    this.emit(true);
  }
}

export const sharedChatStore = new SharedChatSyncStore();
