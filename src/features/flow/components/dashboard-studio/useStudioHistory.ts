// @story 06.5
// @implements AC-04 (Unified Snapshot Undo/Redo History Engine capped at 50 entries with Cmd+Z / Cmd+Shift+Z)
import { useCallback, useEffect, useReducer } from "react";
import { HISTORY_MAX_ENTRIES, type StudioSnapshot } from "./types";

interface HistoryState {
  past: StudioSnapshot[];
  present: StudioSnapshot;
  future: StudioSnapshot[];
}

type HistoryAction =
  | { type: "commit"; updater: (prev: StudioSnapshot) => StudioSnapshot }
  | { type: "preview"; updater: (prev: StudioSnapshot) => StudioSnapshot }
  | { type: "commitFromBase"; base: StudioSnapshot; next: StudioSnapshot }
  | { type: "undo" }
  | { type: "redo" }
  | { type: "reset"; initial: StudioSnapshot };

export interface StudioHistoryApi {
  snapshot: StudioSnapshot;
  canUndo: boolean;
  canRedo: boolean;
  commit: (updater: (prev: StudioSnapshot) => StudioSnapshot) => void;
  preview: (updater: (prev: StudioSnapshot) => StudioSnapshot) => void;
  commitFromBase: (base: StudioSnapshot, next: StudioSnapshot) => void;
  undo: () => void;
  redo: () => void;
  reset: (initial: StudioSnapshot) => void;
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || target.isContentEditable;
}

function stepUndo(cur: HistoryState): HistoryState {
  if (cur.past.length === 0) return cur;
  const previous = cur.past[cur.past.length - 1];
  return {
    past: cur.past.slice(0, -1),
    present: previous,
    future: [cur.present, ...cur.future].slice(0, HISTORY_MAX_ENTRIES),
  };
}

function stepRedo(cur: HistoryState): HistoryState {
  if (cur.future.length === 0) return cur;
  const next = cur.future[0];
  return {
    past: [...cur.past, cur.present].slice(-HISTORY_MAX_ENTRIES),
    present: next,
    future: cur.future.slice(1),
  };
}

function historyReducer(cur: HistoryState, action: HistoryAction): HistoryState {
  if (action.type === "undo") return stepUndo(cur);
  if (action.type === "redo") return stepRedo(cur);
  if (action.type === "reset") return { past: [], present: action.initial, future: [] };
  if (action.type === "preview") return { ...cur, present: action.updater(cur.present) };
  if (action.type === "commitFromBase") {
    return { past: [...cur.past, action.base].slice(-HISTORY_MAX_ENTRIES), present: action.next, future: [] };
  }
  const next = action.updater(cur.present);
  if (next === cur.present) return cur;
  return { past: [...cur.past, cur.present].slice(-HISTORY_MAX_ENTRIES), present: next, future: [] };
}

function useUndoRedoKeybinding(isActive: boolean, undo: () => void, redo: () => void): void {
  useEffect(() => {
    if (!isActive) return;
    const onKeyDown = (ev: KeyboardEvent) => {
      const mod = ev.metaKey || ev.ctrlKey;
      if (!mod || ev.key.toLowerCase() !== "z" || isEditableTarget(ev.target)) return;
      ev.preventDefault();
      if (ev.shiftKey) redo();
      else undo();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isActive, redo, undo]);
}

export function useStudioHistory(
  initialSnapshot: StudioSnapshot,
  isActive: boolean
): StudioHistoryApi {
  const [state, dispatch] = useReducer(historyReducer, {
    past: [],
    present: initialSnapshot,
    future: [],
  });
  const commit = useCallback((updater: (prev: StudioSnapshot) => StudioSnapshot) => dispatch({ type: "commit", updater }), []);
  const preview = useCallback((updater: (prev: StudioSnapshot) => StudioSnapshot) => dispatch({ type: "preview", updater }), []);
  const commitFromBase = useCallback((base: StudioSnapshot, next: StudioSnapshot) => dispatch({ type: "commitFromBase", base, next }), []);
  const undo = useCallback(() => dispatch({ type: "undo" }), []);
  const redo = useCallback(() => dispatch({ type: "redo" }), []);
  const reset = useCallback((initial: StudioSnapshot) => dispatch({ type: "reset", initial }), []);
  useUndoRedoKeybinding(isActive, undo, redo);

  return {
    snapshot: state.present,
    canUndo: state.past.length > 0,
    canRedo: state.future.length > 0,
    commit,
    preview,
    commitFromBase,
    undo,
    redo,
    reset,
  };
}
