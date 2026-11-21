// @story 02.4
// @implements AC-01, AC-02, AC-04
import { useCallback, useEffect, useRef, useState } from "react";
import { exportSsotBundle, fetchSsotComponents, mutateAppShellLock } from "@/api/ssot-registry";
import type {
  AppShellLockDto,
  ComponentPatternDto,
  SsotExportResponse,
  SsotPortalId,
} from "@/lib/validations/ssot-registry";

const DEFAULT_LOCKED_SLOTS = ["TopBar", "LeftNav"];
const DEFAULT_QUERY_LIMIT = 50;
const DEFAULT_LOCK_TTL_MS = 30_000;
const HTTP_UNAUTHORIZED = 401;
const HTTP_NOT_FOUND = 404;

export interface SsotToastNotice {
  tone: "success" | "danger" | "warning";
  message: string;
}

export interface UseSsotRegistryResult {
  portalId: SsotPortalId;
  setPortalId: (portal: SsotPortalId) => void;
  patterns: ComponentPatternDto[];
  lock: AppShellLockDto | null;
  quarantinedCount: number;
  isLocked: boolean;
  hasConflict: boolean;
  isLoading: boolean;
  isMutatingLock: boolean;
  isExporting: boolean;
  lastExport: SsotExportResponse["data"] | null;
  toast: SsotToastNotice | null;
  dismissToast: () => void;
  refresh: () => Promise<void>;
  toggleLock: () => Promise<void>;
  triggerExport: () => Promise<void>;
}

function clearWorkspaceSnapshot(
  setPatterns: (v: ComponentPatternDto[]) => void,
  setLock: (v: AppShellLockDto | null) => void,
  setQuarantinedCount: (v: number) => void
) {
  setPatterns([]);
  setLock(null);
  setQuarantinedCount(0);
}

function buildLockRequestPayload(
  projectId: string,
  portalId: SsotPortalId,
  isLocked: boolean,
  lock: AppShellLockDto | null
) {
  const action: "RELEASE" | "ACQUIRE" = isLocked ? "RELEASE" : "ACQUIRE";
  return {
    projectId,
    portalId,
    action,
    ...(isLocked && lock?.lockOwnerId ? { lockOwnerId: lock.lockOwnerId } : {}),
    expectedRevision: lock?.revision ?? 0,
    expectedFencingToken: isLocked ? (lock?.fencingToken ?? 1) : null,
    lockedSlots: DEFAULT_LOCKED_SLOTS,
    ttlMs: DEFAULT_LOCK_TTL_MS,
  };
}

async function runLoadSnapshot(params: {
  pid: string;
  portalId: SsotPortalId;
  preserveConflict: boolean;
  seq: number;
  seqRef: { current: number };
  mountedRef: { current: boolean };
  setIsLoading: (v: boolean) => void;
  setPatterns: (v: ComponentPatternDto[]) => void;
  setLock: (v: AppShellLockDto | null) => void;
  setQuarantinedCount: (v: number) => void;
  setHasConflict: (v: boolean) => void;
  setToast: (v: SsotToastNotice | null) => void;
}) {
  params.setIsLoading(true);
  try {
    const outcome = await fetchSsotComponents({ projectId: params.pid, portalId: params.portalId, limit: DEFAULT_QUERY_LIMIT });
    if (!params.mountedRef.current || params.seqRef.current !== params.seq) return;
    if (!outcome.ok) {
      clearWorkspaceSnapshot(params.setPatterns, params.setLock, params.setQuarantinedCount);
      if (outcome.status !== HTTP_UNAUTHORIZED && outcome.status !== HTTP_NOT_FOUND) {
        params.setToast({ tone: "warning", message: "Could not load workspace patterns." });
      }
      return;
    }
    params.setPatterns(outcome.data.data.patterns);
    params.setLock(outcome.data.data.lock);
    params.setQuarantinedCount(outcome.data.data.quarantinedCount);
    if (!params.preserveConflict) params.setHasConflict(false);
  } catch {
    if (!params.mountedRef.current || params.seqRef.current !== params.seq) return;
    clearWorkspaceSnapshot(params.setPatterns, params.setLock, params.setQuarantinedCount);
  } finally {
    if (params.mountedRef.current && params.seqRef.current === params.seq) params.setIsLoading(false);
  }
}

async function runToggleLock(params: {
  pid: string;
  portalId: SsotPortalId;
  isLocked: boolean;
  lock: AppShellLockDto | null;
  mountedRef: { current: boolean };
  setIsMutatingLock: (v: boolean) => void;
  setHasConflict: (v: boolean) => void;
  setLock: (v: AppShellLockDto | null) => void;
  setToast: (v: SsotToastNotice | null) => void;
  reloadPreservingConflict: () => Promise<void>;
}) {
  params.setIsMutatingLock(true);
  params.setHasConflict(false);
  try {
    const payload = buildLockRequestPayload(params.pid, params.portalId, params.isLocked, params.lock);
    const outcome = await mutateAppShellLock(payload);
    if (!params.mountedRef.current) return;
    if (outcome.ok) {
      params.setLock(outcome.data.data.lock);
      const msg = payload.action === "ACQUIRE" ? "Shell locked for this workspace." : "Shell unlocked.";
      params.setToast({ tone: "success", message: msg });
      return;
    }
    if (outcome.error.code === "LOCK_FENCING_CONFLICT") {
      params.setHasConflict(true);
      params.setToast({ tone: "warning", message: "Locked by another session. Refresh to sync." });
      await params.reloadPreservingConflict();
      return;
    }
    params.setToast({ tone: "danger", message: "Could not update shell lock." });
  } catch {
    if (params.mountedRef.current) params.setToast({ tone: "danger", message: "Could not update shell lock." });
  } finally {
    if (params.mountedRef.current) params.setIsMutatingLock(false);
  }
}

async function runExportBundle(params: {
  pid: string;
  portalId: SsotPortalId;
  mountedRef: { current: boolean };
  setIsExporting: (v: boolean) => void;
  setLastExport: (v: SsotExportResponse["data"] | null) => void;
  setToast: (v: SsotToastNotice | null) => void;
}) {
  params.setIsExporting(true);
  try {
    const outcome = await exportSsotBundle({
      projectId: params.pid,
      portalId: params.portalId,
      formats: ["markdown", "json"],
      includeQuarantined: false,
    });
    if (!params.mountedRef.current) return;
    if (!outcome.ok) {
      params.setToast({ tone: "danger", message: "Export could not finish right now." });
      return;
    }
    params.setLastExport(outcome.data.data);
    params.setToast({ tone: "success", message: `SSOT exported cleanly (#${outcome.data.data.revision}).` });
  } catch {
    if (params.mountedRef.current) params.setToast({ tone: "danger", message: "Export could not finish right now." });
  } finally {
    if (params.mountedRef.current) params.setIsExporting(false);
  }
}

export function useSsotRegistryState(
  projectId: string | undefined,
  initialPortal: SsotPortalId = "DeveloperPortal"
): UseSsotRegistryResult {
  const [portalId, setPortalId] = useState<SsotPortalId>(initialPortal);
  const [patterns, setPatterns] = useState<ComponentPatternDto[]>([]);
  const [lock, setLock] = useState<AppShellLockDto | null>(null);
  const [quarantinedCount, setQuarantinedCount] = useState(0);
  const [hasConflict, setHasConflict] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [isMutatingLock, setIsMutatingLock] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [lastExport, setLastExport] = useState<SsotExportResponse["data"] | null>(null);
  const [toast, setToast] = useState<SsotToastNotice | null>(null);

  const mountedRef = useRef(true);
  const seqRef = useRef(0);
  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  const pid = projectId?.trim() || "";
  const dismissToast = useCallback(() => setToast(null), []);
  const loadSnapshot = useCallback((preserve: boolean) => {
    const seq = ++seqRef.current;
    if (!pid) { clearWorkspaceSnapshot(setPatterns, setLock, setQuarantinedCount); return Promise.resolve(); }
    return runLoadSnapshot({ pid, portalId, preserveConflict: preserve, seq, seqRef, mountedRef, setIsLoading, setPatterns, setLock, setQuarantinedCount, setHasConflict, setToast });
  }, [pid, portalId]);

  const refresh = useCallback(() => loadSnapshot(false), [loadSnapshot]);
  useEffect(() => { void loadSnapshot(false); }, [loadSnapshot]);

  const isLocked = Boolean(lock && !lock.isExpired && lock.lockOwnerId.trim().length > 0);
  const toggleLock = useCallback(
    () => (!pid || isLoading || isMutatingLock ? Promise.resolve() : runToggleLock({ pid, portalId, isLocked, lock, mountedRef, setIsMutatingLock, setHasConflict, setLock, setToast, reloadPreservingConflict: () => loadSnapshot(true) })),
    [isLoading, isLocked, isMutatingLock, loadSnapshot, lock, pid, portalId]
  );
  const triggerExport = useCallback(
    () => (!pid || isExporting ? Promise.resolve() : runExportBundle({ pid, portalId, mountedRef, setIsExporting, setLastExport, setToast })),
    [isExporting, pid, portalId]
  );

  return { portalId, setPortalId, patterns, lock, quarantinedCount, isLocked, hasConflict, isLoading, isMutatingLock, isExporting, lastExport, toast, dismissToast, refresh, toggleLock, triggerExport };
}
