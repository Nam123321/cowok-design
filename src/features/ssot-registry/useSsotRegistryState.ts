// @story 02.4
// @implements AC-01, AC-02, AC-04
import { useCallback, useEffect, useRef, useState } from "react";
import { exportSsotBundle, fetchSsotComponents, mutateAppShellLock } from "@/api/ssot-registry";
import { useTranslation } from "@/hooks/useTranslation";
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

type TranslateFn = (key: string, fallback: string) => string;

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
    expectedRevision: lock?.revision ?? 0,
    expectedFencingToken: isLocked ? (lock?.fencingToken ?? 1) : null,
    lockedSlots: DEFAULT_LOCKED_SLOTS,
    ttlMs: DEFAULT_LOCK_TTL_MS,
  };
}

function isLockActiveAt(lock: AppShellLockDto | null, nowMs: number): boolean {
  if (!lock || lock.isExpired) return false;
  if (lock.lockOwnerId.trim().length === 0) return false;
  return lock.expiresAt > nowMs;
}

async function runLoadSnapshot(params: {
  pid: string;
  portalId: SsotPortalId;
  preserveConflict: boolean;
  seq: number;
  seqRef: { current: number };
  mountedRef: { current: boolean };
  t: TranslateFn;
  setIsLoading: (v: boolean) => void;
  setPatterns: (v: ComponentPatternDto[]) => void;
  setLock: (v: AppShellLockDto | null) => void;
  setQuarantinedCount: (v: number) => void;
  setHasConflict: (v: boolean) => void;
  setToast: (v: SsotToastNotice | null) => void;
}) {
  params.setIsLoading(true);
  try {
    const outcome = await fetchSsotComponents({
      projectId: params.pid,
      portalId: params.portalId,
      limit: DEFAULT_QUERY_LIMIT,
    });
    if (!params.mountedRef.current || params.seqRef.current !== params.seq) return;
    if (!outcome.ok) {
      clearWorkspaceSnapshot(params.setPatterns, params.setLock, params.setQuarantinedCount);
      if (outcome.status !== HTTP_UNAUTHORIZED && outcome.status !== HTTP_NOT_FOUND) {
        params.setToast({
          tone: "warning",
          message: params.t("ssot.toast.loadFailed", "Could not load workspace patterns."),
        });
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
    params.setToast({
      tone: "warning",
      message: params.t("ssot.toast.loadFailed", "Could not load workspace patterns."),
    });
  } finally {
    if (params.mountedRef.current && params.seqRef.current === params.seq) {
      params.setIsLoading(false);
    }
  }
}

function handleLockToggleError(params: {
  code: string;
  t: TranslateFn;
  setHasConflict: (v: boolean) => void;
  setToast: (v: SsotToastNotice | null) => void;
  reloadPreservingConflict: () => Promise<void>;
}): Promise<void> {
  if (params.code === "LOCK_FENCING_CONFLICT") {
    params.setHasConflict(true);
    params.setToast({
      tone: "warning",
      message: params.t("ssot.toast.lockConflict", "Locked by another session. Refresh to sync."),
    });
    return params.reloadPreservingConflict();
  }
  params.setToast({
    tone: "danger",
    message: params.t("ssot.toast.lockFailed", "Could not update shell lock."),
  });
  return Promise.resolve();
}

async function runToggleLock(params: {
  pid: string;
  portalId: SsotPortalId;
  scopeKey: string;
  scopeRef: { current: string };
  isLocked: boolean;
  lock: AppShellLockDto | null;
  mountedRef: { current: boolean };
  t: TranslateFn;
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
    if (!params.mountedRef.current || params.scopeRef.current !== params.scopeKey) return;
    if (outcome.ok) {
      params.setLock(outcome.data.data.lock);
      const msg = payload.action === "ACQUIRE"
        ? params.t("ssot.toast.lockAcquired", "Shell locked for this workspace.")
        : params.t("ssot.toast.lockReleased", "Shell unlocked.");
      params.setToast({ tone: "success", message: msg });
      return;
    }
    await handleLockToggleError({ code: outcome.error.code, t: params.t, setHasConflict: params.setHasConflict, setToast: params.setToast, reloadPreservingConflict: params.reloadPreservingConflict });
  } catch {
    if (params.mountedRef.current && params.scopeRef.current === params.scopeKey) {
      params.setToast({ tone: "danger", message: params.t("ssot.toast.lockFailed", "Could not update shell lock.") });
    }
  } finally {
    if (params.mountedRef.current && params.scopeRef.current === params.scopeKey) {
      params.setIsMutatingLock(false);
    }
  }
}

async function runExportBundle(params: {
  pid: string;
  portalId: SsotPortalId;
  scopeKey: string;
  scopeRef: { current: string };
  mountedRef: { current: boolean };
  t: TranslateFn;
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
    if (!params.mountedRef.current || params.scopeRef.current !== params.scopeKey) return;
    if (!outcome.ok) {
      params.setToast({
        tone: "danger",
        message: params.t("ssot.toast.exportFailed", "Export could not finish right now."),
      });
      return;
    }
    params.setLastExport(outcome.data.data);
    const template = params.t("ssot.toast.exportSuccess", "Design kit exported (#{rev}).");
    params.setToast({
      tone: "success",
      message: template.replace("#{rev}", `#${outcome.data.data.revision}`),
    });
  } catch {
    if (params.mountedRef.current && params.scopeRef.current === params.scopeKey) {
      params.setToast({
        tone: "danger",
        message: params.t("ssot.toast.exportFailed", "Export could not finish right now."),
      });
    }
  } finally {
    if (params.mountedRef.current && params.scopeRef.current === params.scopeKey) {
      params.setIsExporting(false);
    }
  }
}

function useLockExpiryTick(lock: AppShellLockDto | null): void {
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (!lock || lock.isExpired || lock.expiresAt <= Date.now()) return undefined;
    const delayMs = Math.max(0, lock.expiresAt - Date.now()) + 1;
    const timer = globalThis.setTimeout(() => setTick((n) => n + 1), delayMs);
    return () => globalThis.clearTimeout(timer);
  }, [lock, tick]);
}

function useMountedScopeRefs(scopeKey: string) {
  const mountedRef = useRef(true);
  const seqRef = useRef(0);
  const scopeRef = useRef(scopeKey);
  scopeRef.current = scopeKey;
  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);
  return { mountedRef, seqRef, scopeRef };
}

function useReloadOnScopeChange(params: {
  loadSnapshot: (preserve: boolean) => Promise<void>;
  setIsMutatingLock: (v: boolean) => void;
  setIsExporting: (v: boolean) => void;
  setHasConflict: (v: boolean) => void;
  setLastExport: (v: SsotExportResponse["data"] | null) => void;
}): void {
  const { loadSnapshot, setIsMutatingLock, setIsExporting, setHasConflict, setLastExport } = params;
  useEffect(() => {
    setIsMutatingLock(false);
    setIsExporting(false);
    setHasConflict(false);
    setLastExport(null);
    void loadSnapshot(false);
  }, [loadSnapshot, setHasConflict, setIsExporting, setIsMutatingLock, setLastExport]);
}

export function useSsotRegistryState(
  projectId: string | undefined,
  initialPortal: SsotPortalId = "DeveloperPortal"
): UseSsotRegistryResult {
  const { t } = useTranslation();
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

  const pid = projectId?.trim() || "";
  const scopeKey = `${pid}\0${portalId}`;
  const { mountedRef, seqRef, scopeRef } = useMountedScopeRefs(scopeKey);
  useLockExpiryTick(lock);

  const dismissToast = useCallback(() => setToast(null), []);
  const loadSnapshot = useCallback((preserve: boolean) => {
    const seq = ++seqRef.current;
    if (!pid) {
      clearWorkspaceSnapshot(setPatterns, setLock, setQuarantinedCount);
      setHasConflict(false); setIsLoading(false);
      return Promise.resolve();
    }
    return runLoadSnapshot({ pid, portalId, preserveConflict: preserve, seq, seqRef, mountedRef, t, setIsLoading, setPatterns, setLock, setQuarantinedCount, setHasConflict, setToast });
  }, [mountedRef, pid, portalId, seqRef, t]);

  const refresh = useCallback(() => loadSnapshot(false), [loadSnapshot]);
  useReloadOnScopeChange({ loadSnapshot, setIsMutatingLock, setIsExporting, setHasConflict, setLastExport });

  const isLocked = isLockActiveAt(lock, Date.now());
  const toggleLock = useCallback(
    () => (!pid || isLoading || isMutatingLock ? Promise.resolve() : runToggleLock({ pid, portalId, scopeKey, scopeRef, isLocked, lock, mountedRef, t, setIsMutatingLock, setHasConflict, setLock, setToast, reloadPreservingConflict: () => loadSnapshot(true) })),
    [isLoading, isLocked, isMutatingLock, loadSnapshot, lock, mountedRef, pid, portalId, scopeKey, scopeRef, t]
  );
  const triggerExport = useCallback(
    () => (!pid || isExporting ? Promise.resolve() : runExportBundle({ pid, portalId, scopeKey, scopeRef, mountedRef, t, setIsExporting, setLastExport, setToast })),
    [isExporting, mountedRef, pid, portalId, scopeKey, scopeRef, t]
  );

  return { portalId, setPortalId, patterns, lock, quarantinedCount, isLocked, hasConflict, isLoading, isMutatingLock, isExporting, lastExport, toast, dismissToast, refresh, toggleLock, triggerExport };
}
