// @story 04.1, 05.1, 06.2
// @implements AC-03, AC-07 (Per-Screen App HTML CMS State & Scoped Undo/Redo without Switching to Deck Seed)
import { useCallback, useMemo, useRef, useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import { useFormatter } from "../../../hooks/useFormatter";
import type { ScreenPreviewData } from "../components/companion-canvas-data";
import type { WorkbenchNode } from "../store/flow-workbench-types";
import type { PostPatchByFfIdFn } from "./useCmsSlideEditorController";
import {
  applyScreenFieldOverrides,
  buildAppScreenCards,
  buildAppScreenSlots,
  DEFAULT_PREVIEW_CURRENCY,
  type AppScreenUndoEntry,
} from "./appScreenCmsHelpers";

const MAX_APP_UNDO_STEPS = 50;
const APP_CMS_STORAGE_KEY = "cowok:app-cms:v1";

const FORBIDDEN_RECORD_KEYS = new Set(["__proto__", "constructor", "prototype"]);

function sanitizeStringRecord(raw: unknown): Record<string, string> {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};
  const clean: Record<string, string> = {};
  for (const [k, v] of Object.entries(raw)) {
    if (typeof k === "string" && !FORBIDDEN_RECORD_KEYS.has(k) && typeof v === "string") clean[k] = v.slice(0, 8000);
  }
  return clean;
}

function readPersistedAppCmsState(): { overrides: Record<string, Record<string, string>>; notes: Record<string, string> } {
  if (typeof window === "undefined") return { overrides: {}, notes: {} };
  try {
    const raw = window.localStorage?.getItem(APP_CMS_STORAGE_KEY);
    if (!raw) return { overrides: {}, notes: {} };
    const parsed = JSON.parse(raw) as { overrides?: unknown; notes?: unknown };
    const overrides: Record<string, Record<string, string>> = {};
    if (parsed?.overrides && typeof parsed.overrides === "object" && !Array.isArray(parsed.overrides)) {
      for (const [screenKey, fields] of Object.entries(parsed.overrides)) {
        if (typeof screenKey === "string" && !FORBIDDEN_RECORD_KEYS.has(screenKey)) {
          overrides[screenKey] = sanitizeStringRecord(fields);
        }
      }
    }
    return { overrides, notes: sanitizeStringRecord(parsed?.notes) };
  } catch {
    return { overrides: {}, notes: {} };
  }
}

function writePersistedAppCmsState(overrides: Record<string, Record<string, string>>, notes: Record<string, string>): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage?.setItem(APP_CMS_STORAGE_KEY, JSON.stringify({ overrides, notes }));
  } catch { /* ignore storage quota errors */ }
}

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
  setFrom: React.Dispatch<React.SetStateAction<Record<string, AppScreenUndoEntry[]>>>;
  setTo: React.Dispatch<React.SetStateAction<Record<string, AppScreenUndoEntry[]>>>;
  pickValue: (entry: AppScreenUndoEntry) => string;
  commitScreenPatch: (screenKey: string, ffId: string, nextValue: string) => void;
}) {
  if (opts.isReadOnly) return;
  opts.setFrom((prev) => {
    const stack = prev[opts.activeScreenKey] ?? [];
    if (stack.length === 0) return prev;
    const entry = stack[stack.length - 1]!;
    opts.setTo((t) => ({
      ...t,
      [opts.activeScreenKey]: [...(t[opts.activeScreenKey] ?? []).slice(-(MAX_APP_UNDO_STEPS - 1)), entry],
    }));
    opts.commitScreenPatch(entry.screenKey, entry.ffId, opts.pickValue(entry));
    return { ...prev, [opts.activeScreenKey]: stack.slice(0, -1) };
  });
}

function useAppScreenHistoryActions(opts: {
  activeScreenKey: string;
  isReadOnly: boolean;
  commitScreenPatch: (screenKey: string, ffId: string, nextValue: string) => void;
}) {
  const { activeScreenKey, isReadOnly, commitScreenPatch } = opts;
  const [undoByScreen, setUndoByScreen] = useState<Record<string, AppScreenUndoEntry[]>>({});
  const [redoByScreen, setRedoByScreen] = useState<Record<string, AppScreenUndoEntry[]>>({});

  const applyScreenFieldEdit = useCallback(
    (ffId: string, nextValue: string, previousValue: string, recordUndo = true) => {
      if (isReadOnly) return;
      if (recordUndo && previousValue !== nextValue) {
        const entry: AppScreenUndoEntry = { screenKey: activeScreenKey, ffId, previousValue, nextValue };
        setUndoByScreen((p) => ({ ...p, [activeScreenKey]: [...(p[activeScreenKey] ?? []), entry].slice(-MAX_APP_UNDO_STEPS) }));
        setRedoByScreen((p) => ({ ...p, [activeScreenKey]: [] }));
      }
      commitScreenPatch(activeScreenKey, ffId, nextValue);
    },
    [activeScreenKey, commitScreenPatch, isReadOnly]
  );

  const undo = useCallback(
    () => shiftHistoryEntry({ activeScreenKey, isReadOnly, setFrom: setUndoByScreen, setTo: setRedoByScreen, pickValue: (e) => e.previousValue, commitScreenPatch }),
    [activeScreenKey, commitScreenPatch, isReadOnly]
  );
  const redo = useCallback(
    () => shiftHistoryEntry({ activeScreenKey, isReadOnly, setFrom: setRedoByScreen, setTo: setUndoByScreen, pickValue: (e) => e.nextValue, commitScreenPatch }),
    [activeScreenKey, commitScreenPatch, isReadOnly]
  );
  return { canUndo: (undoByScreen[activeScreenKey] ?? []).length > 0, canRedo: (redoByScreen[activeScreenKey] ?? []).length > 0, applyScreenFieldEdit, undo, redo };
}

function useAppScreenStorageBacking(activeScreenKey: string, isReadOnly: boolean, postPatchByFfIdRef: React.MutableRefObject<PostPatchByFfIdFn | null>) {
  const initial = useMemo(() => readPersistedAppCmsState(), []);
  const [overridesByScreen, setOverridesByScreen] = useState(() => initial.overrides);
  const [speakerNotesByScreen, setSpeakerNotesByScreen] = useState(() => initial.notes);
  const overridesRef = useRef(overridesByScreen);
  overridesRef.current = overridesByScreen;
  const notesRef = useRef(speakerNotesByScreen);
  notesRef.current = speakerNotesByScreen;
  const [dirtyScreens, setDirtyScreens] = useState<Record<string, boolean>>({});

  const commitScreenPatch = useCallback((screenKey: string, ffId: string, nextValue: string) => {
    setOverridesByScreen((prev) => {
      const updated = { ...prev, [screenKey]: { ...(prev[screenKey] ?? {}), [ffId]: nextValue } };
      writePersistedAppCmsState(updated, notesRef.current);
      return updated;
    });
    setDirtyScreens((prev) => ({ ...prev, [screenKey]: true }));
    if (screenKey === activeScreenKey) postPatchByFfIdRef.current?.(ffId, { textContent: nextValue, slotKind: "text" });
  }, [activeScreenKey, postPatchByFfIdRef]);

  const setSpeakerNotes = useCallback((nextNotes: string) => {
    if (isReadOnly) return;
    setSpeakerNotesByScreen((prev) => {
      const updated = { ...prev, [activeScreenKey]: nextNotes };
      writePersistedAppCmsState(overridesRef.current, updated);
      return updated;
    });
    setDirtyScreens((prev) => ({ ...prev, [activeScreenKey]: true }));
  }, [activeScreenKey, isReadOnly]);

  const clearDirtyAndPersist = useCallback(() => {
    writePersistedAppCmsState(overridesRef.current, notesRef.current);
    setDirtyScreens({});
  }, []);

  return { overridesByScreen, speakerNotesByScreen, dirtyScreens, commitScreenPatch, setSpeakerNotes, clearDirtyAndPersist };
}

export function useAppScreenCmsState(params: {
  nodes?: WorkbenchNode[];
  selectedNodeId?: string | null;
  baseScreenData: ScreenPreviewData;
  isReadOnly?: boolean;
  postPatchByFfIdRef: React.MutableRefObject<PostPatchByFfIdFn | null>;
}) {
  const { nodes = [], selectedNodeId, baseScreenData, isReadOnly = false, postPatchByFfIdRef } = params;
  const activeScreenKey = selectedNodeId || nodes[0]?.id || "default";
  const [revision, setRevision] = useState(1);
  const storage = useAppScreenStorageBacking(activeScreenKey, isReadOnly, postPatchByFfIdRef);
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
