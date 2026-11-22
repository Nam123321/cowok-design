// @story 04.1, 05.1, 05.4, 06.2
// @implements AC-03, AC-05, AC-07 (SQLite WAL-Backed Per-Screen App HTML CMS State & Tier-1 Undo)
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  enqueueSerializedScreenPatch,
  hydrateScreenOccHistory,
  type LockedRuleReceiptNotice,
} from "../../../lib/sketch-polish-occ-store";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { ScreenPreviewData } from "../components/companion-canvas-data";
import type { WorkbenchNode } from "../store/flow-workbench-types";
import type { PostPatchByFfIdFn } from "./useCmsSlideEditorController";
import {
  applyOverrideUpdate,
  applyScreenFieldOverrides,
  buildAppScreenCards,
  buildAppScreenSlots,
  DEFAULT_PREVIEW_CURRENCY,
  extractOverridesFromPatches,
  FORBIDDEN_RECORD_KEYS,
  readPersistedAppCmsState,
  writePersistedAppCmsState,
  type AppScreenUndoEntry,
} from "./appScreenCmsHelpers";

const MAX_APP_UNDO_STEPS = 50;
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const SAFE_FF_ID_RE = /^[a-zA-Z0-9_-]{1,64}$/;

function useAppScreenDerivedView(opts: {
  nodes: WorkbenchNode[];
  activeScreenKey: string;
  baseScreenData: ScreenPreviewData;
  overridesByScreen: Record<string, Record<string, string>>;
}) {
  const { t, locale } = useTranslation();
  const { formatCurrency } = useFormatter({ locale });
  const synthLabels = useMemo(
    () => ({
      kpiRoute: t("canvas.previewSynthKpiRoute", "Route"),
      kpiState: t("canvas.previewSynthKpiState", "Node Status"),
      kpiType: t("canvas.previewSynthKpiType", "Node Type"),
      actionHint: t("canvas.previewFallbackHint", "Auto-synced live HTML preview from current graph state."),
      ctaOpen: t("canvas.previewSynthCtaOpen", "Open"),
    }),
    [t]
  );
  const screenCards = useMemo(
    () => buildAppScreenCards({ nodes: opts.nodes, activeScreenKey: opts.activeScreenKey, baseScreenData: opts.baseScreenData, overridesByScreen: opts.overridesByScreen, synthLabels }),
    [opts.activeScreenKey, opts.baseScreenData, opts.nodes, opts.overridesByScreen, synthLabels]
  );
  const activeCardIndex = Math.max(0, screenCards.findIndex((c) => c.id === opts.activeScreenKey));
  const activeOverrides = opts.overridesByScreen[opts.activeScreenKey] ?? {};
  const formattedBaseAmount = formatCurrency(Number(opts.baseScreenData.amount || 0), DEFAULT_PREVIEW_CURRENCY);
  const defaultKpiLabels = useMemo(
    () => ({
      kpi1Label: opts.baseScreenData.kpi1Label || t("canvas.previewKpiLimitLabel", "Transaction Limit"),
      kpi2Label: opts.baseScreenData.kpi2Label || t("canvas.previewKpiSecurityLabel", "Security Status"),
      kpi3Label: opts.baseScreenData.kpi3Label || t("canvas.previewKpiLatencyLabel", "Response Time"),
    }),
    [opts.baseScreenData.kpi1Label, opts.baseScreenData.kpi2Label, opts.baseScreenData.kpi3Label, t]
  );
  const mergedScreenData = useMemo(() => applyScreenFieldOverrides(opts.baseScreenData, activeOverrides, formattedBaseAmount), [activeOverrides, opts.baseScreenData, formattedBaseAmount]);
  const mergedKpiLabels = useMemo(
    () => ({
      kpi1Label: activeOverrides["ff-kpi-1-label"] ?? defaultKpiLabels.kpi1Label,
      kpi2Label: activeOverrides["ff-kpi-2-label"] ?? defaultKpiLabels.kpi2Label,
      kpi3Label: activeOverrides["ff-kpi-3-label"] ?? defaultKpiLabels.kpi3Label,
    }),
    [activeOverrides, defaultKpiLabels]
  );
  const slots = useMemo(
    () => buildAppScreenSlots({ screenKey: opts.activeScreenKey, data: mergedScreenData, formattedAmount: formattedBaseAmount, kpiLabels: mergedKpiLabels, t }),
    [formattedBaseAmount, mergedKpiLabels, mergedScreenData, opts.activeScreenKey, t]
  );
  return { screenCards, activeCardIndex, mergedScreenData, slots };
}

function shiftHistoryEntry(opts: {
  activeScreenKey: string;
  isReadOnly: boolean;
  fromRef: React.MutableRefObject<Record<string, AppScreenUndoEntry[]>>;
  setFrom: React.Dispatch<React.SetStateAction<Record<string, AppScreenUndoEntry[]>>>;
  setTo: React.Dispatch<React.SetStateAction<Record<string, AppScreenUndoEntry[]>>>;
  pickValue: (entry: AppScreenUndoEntry) => string;
  commitScreenPatch: (screenKey: string, ffId: string, nextValue: string) => void;
}) {
  if (opts.isReadOnly) return;
  const stack = opts.fromRef.current[opts.activeScreenKey] ?? [];
  if (stack.length === 0) return;
  const entry = stack[stack.length - 1]!;
  opts.setFrom((prev) => ({
    ...prev,
    [opts.activeScreenKey]: (prev[opts.activeScreenKey] ?? []).slice(0, -1),
  }));
  opts.setTo((t) => ({
    ...t,
    [opts.activeScreenKey]: [...(t[opts.activeScreenKey] ?? []).slice(-(MAX_APP_UNDO_STEPS - 1)), entry],
  }));
  opts.commitScreenPatch(entry.screenKey, entry.ffId, opts.pickValue(entry));
}

function useAppScreenHistoryActions(opts: {
  activeScreenKey: string;
  isReadOnly: boolean;
  commitScreenPatch: (screenKey: string, ffId: string, nextValue: string, fallbackPreviousValue?: string) => void;
}) {
  const { activeScreenKey, isReadOnly, commitScreenPatch } = opts;
  const [undoByScreen, setUndoByScreen] = useState<Record<string, AppScreenUndoEntry[]>>({});
  const [redoByScreen, setRedoByScreen] = useState<Record<string, AppScreenUndoEntry[]>>({});
  const undoRef = useRef(undoByScreen);
  undoRef.current = undoByScreen;
  const redoRef = useRef(redoByScreen);
  redoRef.current = redoByScreen;

  const applyScreenFieldEdit = useCallback(
    (ffId: string, nextValue: string, previousValue: string, recordUndo = true) => {
      if (isReadOnly || FORBIDDEN_RECORD_KEYS.has(ffId.toLowerCase())) return;
      if (recordUndo && previousValue !== nextValue) {
        const entry: AppScreenUndoEntry = { screenKey: activeScreenKey, ffId, previousValue, nextValue };
        setUndoByScreen((p) => ({ ...p, [activeScreenKey]: [...(p[activeScreenKey] ?? []), entry].slice(-MAX_APP_UNDO_STEPS) }));
        setRedoByScreen((p) => ({ ...p, [activeScreenKey]: [] }));
      }
      commitScreenPatch(activeScreenKey, ffId, nextValue, previousValue);
    },
    [activeScreenKey, commitScreenPatch, isReadOnly]
  );

  const undo = useCallback(
    () => shiftHistoryEntry({ activeScreenKey, isReadOnly, fromRef: undoRef, setFrom: setUndoByScreen, setTo: setRedoByScreen, pickValue: (e) => e.previousValue, commitScreenPatch }),
    [activeScreenKey, commitScreenPatch, isReadOnly]
  );
  const redo = useCallback(
    () => shiftHistoryEntry({ activeScreenKey, isReadOnly, fromRef: redoRef, setFrom: setRedoByScreen, setTo: setUndoByScreen, pickValue: (e) => e.nextValue, commitScreenPatch }),
    [activeScreenKey, commitScreenPatch, isReadOnly]
  );
  return { canUndo: (undoByScreen[activeScreenKey] ?? []).length > 0, canRedo: (redoByScreen[activeScreenKey] ?? []).length > 0, applyScreenFieldEdit, undo, redo };
}

interface FlushPatchRequest {
  screenKey: string;
  ffId: string;
  nextValue: string;
  opKind: "TEXT_EDIT" | "UNDO";
  onRollback: () => void;
  onApplied: () => void;
}

function useHydrateAppScreenSqlite(opts: {
  canSync: boolean;
  projectId?: string;
  activeScreenKey: string;
  setOverridesByScreen: React.Dispatch<React.SetStateAction<Record<string, Record<string, string>>>>;
  setRevision: React.Dispatch<React.SetStateAction<number>>;
}) {
  const { canSync, projectId, activeScreenKey, setOverridesByScreen, setRevision } = opts;
  const syncErrorRef = useRef<string | null>(null);
  useEffect(() => {
    if (!canSync || !projectId) return;
    let cancelled = false;
    void hydrateScreenOccHistory(projectId, activeScreenKey)
      .then((res) => {
        if (cancelled) return;
        syncErrorRef.current = null;
        setRevision(Math.max(1, res.data.astRevision));
        const restored = extractOverridesFromPatches(res.data.patches);
        if (Object.keys(restored).length > 0) {
          setOverridesByScreen((prev) => ({
            ...prev,
            [activeScreenKey]: { ...(prev[activeScreenKey] ?? {}), ...restored },
          }));
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) syncErrorRef.current = err instanceof Error ? err.message : "sqlite_hydrate_failed";
      });
    return () => { cancelled = true; };
  }, [activeScreenKey, canSync, projectId, setOverridesByScreen, setRevision]);
}

function useAppScreenSqliteSync(opts: {
  projectId?: string;
  activeScreenKey: string;
  isReadOnly: boolean;
  setOverridesByScreen: React.Dispatch<React.SetStateAction<Record<string, Record<string, string>>>>;
  setDirtyScreens: React.Dispatch<React.SetStateAction<Record<string, boolean>>>;
  setRevision: React.Dispatch<React.SetStateAction<number>>;
  onLockedRuleReceipt?: (notice: LockedRuleReceiptNotice) => void;
}) {
  const { projectId, activeScreenKey, isReadOnly, setOverridesByScreen, setDirtyScreens, setRevision, onLockedRuleReceipt } = opts;
  const canSync = Boolean(projectId && UUID_RE.test(projectId) && activeScreenKey && typeof fetch === "function");
  useHydrateAppScreenSqlite({ canSync, projectId, activeScreenKey, setOverridesByScreen, setRevision });

  const flushPatchToSqlite = useCallback((req: FlushPatchRequest) => {
    if (!canSync || !projectId || isReadOnly) return;
    if (!SAFE_FF_ID_RE.test(req.ffId) || FORBIDDEN_RECORD_KEYS.has(req.ffId.toLowerCase())) return;
    const payloadJson = { text: req.nextValue };
    void enqueueSerializedScreenPatch({ projectId, screenId: req.screenKey, ffId: req.ffId, opKind: req.opKind, payloadJson })
      .then((res) => {
        setRevision(res.data.astRevision);
        setDirtyScreens((prev) => ({ ...prev, [req.screenKey]: false }));
        const receipt = res.data.lockedRuleReceipt;
        if (receipt && !receipt.applied) {
          req.onRollback();
          onLockedRuleReceipt?.({
            receipt,
            pendingPatch: { projectId, screenId: req.screenKey, ffId: req.ffId, opKind: req.opKind, payloadJson, onApplied: req.onApplied },
          });
        }
      })
      .catch(() => { req.onRollback(); });
  }, [canSync, isReadOnly, onLockedRuleReceipt, projectId, setDirtyScreens, setRevision]);

  return { flushPatchToSqlite };
}

function applyClientOverrideAndDom(opts: {
  screenKey: string;
  activeScreenKey: string;
  ffId: string;
  val: string | undefined;
  domVal: string | undefined;
  dirty: boolean;
  notesRef: React.MutableRefObject<Record<string, string>>;
  postPatchByFfIdRef: React.MutableRefObject<PostPatchByFfIdFn | null>;
  setOverridesByScreen: React.Dispatch<React.SetStateAction<Record<string, Record<string, string>>>>;
  setDirtyScreens: React.Dispatch<React.SetStateAction<Record<string, boolean>>>;
}): void {
  opts.setOverridesByScreen((prev) => {
    const updated = applyOverrideUpdate(prev, opts.screenKey, opts.ffId, opts.val);
    writePersistedAppCmsState(updated, opts.notesRef.current);
    return updated;
  });
  opts.setDirtyScreens((prev) => ({ ...prev, [opts.screenKey]: opts.dirty }));
  if (opts.screenKey === opts.activeScreenKey && opts.domVal !== undefined) {
    opts.postPatchByFfIdRef.current?.(opts.ffId, { textContent: opts.domVal, slotKind: "text" });
  }
}

function useAppScreenStorageBacking(opts: {
  projectId?: string;
  activeScreenKey: string;
  isReadOnly: boolean;
  postPatchByFfIdRef: React.MutableRefObject<PostPatchByFfIdFn | null>;
  setRevision: React.Dispatch<React.SetStateAction<number>>;
  onLockedRuleReceipt?: (notice: LockedRuleReceiptNotice) => void;
}) {
  const { projectId, activeScreenKey, isReadOnly, postPatchByFfIdRef, setRevision, onLockedRuleReceipt } = opts;
  const initial = useMemo(() => readPersistedAppCmsState(), []);
  const [overridesByScreen, setOverridesByScreen] = useState(() => initial.overrides);
  const [speakerNotesByScreen, setSpeakerNotesByScreen] = useState(() => initial.notes);
  const overridesRef = useRef(overridesByScreen);
  overridesRef.current = overridesByScreen;
  const notesRef = useRef(speakerNotesByScreen);
  notesRef.current = speakerNotesByScreen;
  const [dirtyScreens, setDirtyScreens] = useState<Record<string, boolean>>({});
  const { flushPatchToSqlite } = useAppScreenSqliteSync({
    projectId, activeScreenKey, isReadOnly, setOverridesByScreen, setDirtyScreens, setRevision, onLockedRuleReceipt,
  });

  const commitScreenPatch = useCallback((screenKey: string, ffId: string, nextValue: string, fallbackPreviousValue?: string) => {
    if (FORBIDDEN_RECORD_KEYS.has(ffId.toLowerCase())) return;
    const prevScreen = overridesRef.current[screenKey] ?? {};
    const previousValue = Object.prototype.hasOwnProperty.call(prevScreen, ffId) ? prevScreen[ffId] : undefined;
    const apply = (val: string | undefined, domVal: string | undefined, dirty: boolean) =>
      applyClientOverrideAndDom({ screenKey, activeScreenKey, ffId, val, domVal, dirty, notesRef, postPatchByFfIdRef, setOverridesByScreen, setDirtyScreens });
    apply(nextValue, nextValue, true);
    flushPatchToSqlite({
      screenKey, ffId, nextValue, opKind: "TEXT_EDIT",
      onRollback: () => apply(previousValue, previousValue ?? fallbackPreviousValue, false),
      onApplied: () => apply(nextValue, nextValue, false),
    });
  }, [activeScreenKey, flushPatchToSqlite, postPatchByFfIdRef]);

  const setSpeakerNotes = useCallback((nextNotes: string) => {
    if (isReadOnly) return;
    setSpeakerNotesByScreen((prev) => {
      const updated = { ...prev, [activeScreenKey]: nextNotes };
      writePersistedAppCmsState(overridesRef.current, updated);
      return updated;
    });
    setDirtyScreens((prev) => ({ ...prev, [activeScreenKey]: true }));
  }, [activeScreenKey, isReadOnly]);

  const clearDirtyAndPersist = useCallback(() => { writePersistedAppCmsState(overridesRef.current, notesRef.current); setDirtyScreens({}); }, []);
  return { overridesByScreen, speakerNotesByScreen, dirtyScreens, commitScreenPatch, setSpeakerNotes, clearDirtyAndPersist };
}

export function useAppScreenCmsState(params: {
  projectId?: string;
  nodes?: WorkbenchNode[];
  selectedNodeId?: string | null;
  baseScreenData: ScreenPreviewData;
  isReadOnly?: boolean;
  postPatchByFfIdRef: React.MutableRefObject<PostPatchByFfIdFn | null>;
  onLockedRuleReceipt?: (notice: LockedRuleReceiptNotice) => void;
}) {
  const { projectId, nodes = [], selectedNodeId, baseScreenData, isReadOnly = false, postPatchByFfIdRef, onLockedRuleReceipt } = params;
  const activeScreenKey = selectedNodeId || nodes[0]?.id || "default";
  const [revision, setRevision] = useState(1);
  const storage = useAppScreenStorageBacking({ projectId, activeScreenKey, isReadOnly, postPatchByFfIdRef, setRevision, onLockedRuleReceipt });
  const derived = useAppScreenDerivedView({ nodes, activeScreenKey, baseScreenData, overridesByScreen: storage.overridesByScreen });
  const history = useAppScreenHistoryActions({ activeScreenKey, isReadOnly, commitScreenPatch: storage.commitScreenPatch });
  const markSaved = useCallback(() => {
    storage.clearDirtyAndPersist();
    setRevision((r) => r + 1);
  }, [storage]);

  return {
    activeScreenKey, ...derived, speakerNotes: storage.speakerNotesByScreen[activeScreenKey] ?? "",
    hasPendingSave: Object.values(storage.dirtyScreens).some(Boolean), revision, ...history, setSpeakerNotes: storage.setSpeakerNotes, markSaved,
  };
}
