// @story 06.2
// @implements AC-06
import {
  CmsRecoveryDraftSchema,
  type CmsRecoveryDraft,
} from "../../../lib/validations/presentation";

export const CMS_DRAFT_STORAGE_PREFIX = "cowok:cms-draft:";
export const IME_COMPOSING_KEY_CODE = 229;
export const E3_DANGER_OVERFLOW_PX = 100;
const IDB_NAME = "cowok-cms-recovery";
const IDB_STORE = "drafts";

function buildDraftStorageKey(deckId: string, slideUid: string): string {
  return `${CMS_DRAFT_STORAGE_PREFIX}${deckId}:${slideUid}`;
}

function openDraftIdb(): Promise<IDBDatabase | null> {
  if (typeof window === "undefined" || typeof window.indexedDB === "undefined") {
    return Promise.resolve(null);
  }
  return new Promise((resolve) => {
    try {
      const req = window.indexedDB.open(IDB_NAME, 1);
      req.onupgradeneeded = () => {
        if (!req.result.objectStoreNames.contains(IDB_STORE)) {
          req.result.createObjectStore(IDB_STORE);
        }
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
}

async function runDraftIdbOp<T>(
  mode: IDBTransactionMode,
  op: (store: IDBObjectStore) => IDBRequest<T>
): Promise<T | null> {
  const db = await openDraftIdb();
  if (!db) return null;
  return new Promise((resolve) => {
    try {
      const tx = db.transaction(IDB_STORE, mode);
      const req = op(tx.objectStore(IDB_STORE));
      req.onsuccess = () => resolve(req.result ?? null);
      req.onerror = () => resolve(null);
      tx.oncomplete = () => db.close();
      tx.onerror = () => db.close();
    } catch {
      db.close();
      resolve(null);
    }
  });
}

export function saveRecoveryDraft(draft: CmsRecoveryDraft): boolean {
  const parsed = CmsRecoveryDraftSchema.safeParse(draft);
  if (!parsed.success) return false;
  const key = buildDraftStorageKey(parsed.data.deckId, parsed.data.slideUid);
  const serialized = JSON.stringify(parsed.data);
  try {
    if (typeof window !== "undefined" && window.sessionStorage) {
      window.sessionStorage.setItem(key, serialized);
      return true;
    }
  } catch {
    // Evict stale sessionStorage entry so newer IndexedDB draft is never shadowed on recovery
    try {
      window.sessionStorage?.removeItem(key);
    } catch {
      // Ignore storage removal errors
    }
    void runDraftIdbOp("readwrite", (store) => store.put(serialized, key));
  }
  return false;
}

export function loadRecoveryDraft(deckId: string, slideUid: string): CmsRecoveryDraft | null {
  const key = buildDraftStorageKey(deckId, slideUid);
  try {
    if (typeof window === "undefined" || !window.sessionStorage) return null;
    const raw = window.sessionStorage.getItem(key);
    if (!raw) return null;
    const parsed = CmsRecoveryDraftSchema.safeParse(JSON.parse(raw));
    return parsed.success ? parsed.data : null;
  } catch {
    return null;
  }
}

export async function loadRecoveryDraftWithIdbFallback(
  deckId: string,
  slideUid: string
): Promise<CmsRecoveryDraft | null> {
  const fromSession = loadRecoveryDraft(deckId, slideUid);
  if (fromSession) return fromSession;
  const key = buildDraftStorageKey(deckId, slideUid);
  const raw = await runDraftIdbOp<unknown>("readonly", (store) => store.get(key));
  if (typeof raw !== "string") return null;
  try {
    const parsed = CmsRecoveryDraftSchema.safeParse(JSON.parse(raw));
    return parsed.success ? parsed.data : null;
  } catch {
    return null;
  }
}

export function clearRecoveryDraft(deckId: string, slideUid: string): void {
  const key = buildDraftStorageKey(deckId, slideUid);
  try {
    if (typeof window !== "undefined" && window.sessionStorage) {
      window.sessionStorage.removeItem(key);
    }
  } catch {
    // Ignore storage unavailability
  }
  void runDraftIdbOp("readwrite", (store) => store.delete(key));
}
