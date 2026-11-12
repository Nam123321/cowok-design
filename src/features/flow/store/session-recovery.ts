// @story 04.1
// @implements AC-06, AC-08
import type { StructuralSnapshot, WorkbenchNode, WorkbenchTransition } from "./flow-workbench-types";

export const MAX_UNDO_HISTORY = 100;
export const MAX_SESSION_DRAFT_BYTES = 1_048_576;

export interface PersistedDraftEnvelope {
  schema: "cowok-design.draft-session.v2";
  projectId: string;
  draftId: string;
  baseSnapshot: StructuralSnapshot;
  nodes: WorkbenchNode[];
  transitions: WorkbenchTransition[];
  savedAt: number;
}

function draftStorageKey(projectId: string): string {
  return `cowok:flow:draft:${projectId}`;
}

export function generateDraftUuid(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  const hex = "0123456789abcdef";
  let out = "";
  for (let i = 0; i < 36; i++) {
    if (i === 8 || i === 13 || i === 18 || i === 23) out += "-";
    else if (i === 14) out += "4";
    else if (i === 19) out += hex[(Math.random() * 4) | 8];
    else out += hex[(Math.random() * 16) | 0];
  }
  return out;
}

export function capUndoStack(stack: StructuralSnapshot[]): StructuralSnapshot[] {
  if (stack.length <= MAX_UNDO_HISTORY) return stack;
  return stack.slice(stack.length - MAX_UNDO_HISTORY);
}

export function saveSessionDraft(envelope: PersistedDraftEnvelope): boolean {
  if (typeof globalThis.sessionStorage === "undefined") return false;
  try {
    const raw = JSON.stringify(envelope);
    if (new TextEncoder().encode(raw).length > MAX_SESSION_DRAFT_BYTES) {
      return false;
    }
    globalThis.sessionStorage.setItem(draftStorageKey(envelope.projectId), raw);
    return true;
  } catch {
    return false;
  }
}

export function loadSessionDraft(projectId: string): PersistedDraftEnvelope | null {
  if (typeof globalThis.sessionStorage === "undefined" || !projectId) return null;
  try {
    const raw = globalThis.sessionStorage.getItem(draftStorageKey(projectId));
    if (!raw) return null;
    if (new TextEncoder().encode(raw).length > MAX_SESSION_DRAFT_BYTES) {
      globalThis.sessionStorage.removeItem(draftStorageKey(projectId));
      return null;
    }
    const parsed = JSON.parse(raw) as PersistedDraftEnvelope;
    if (parsed.schema !== "cowok-design.draft-session.v2" || parsed.projectId !== projectId || !parsed.draftId) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

export function clearSessionDraft(projectId: string): void {
  if (typeof globalThis.sessionStorage === "undefined" || !projectId) return;
  try {
    globalThis.sessionStorage.removeItem(draftStorageKey(projectId));
  } catch {
    // ignore storage restriction
  }
}
