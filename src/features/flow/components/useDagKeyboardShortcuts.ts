// @story 04.1
// @implements AC-04, AC-07, AC-08
import React, { useEffect } from "react";
import { FlowWorkbenchState, WorkbenchViewport } from "../store/flow-workbench-store";

const ZOOM_STEP = 0.25;
const ZOOM_MIN = 0.2;
const ZOOM_MAX = 2.0;
const FIT_VIEW_ZOOM = 0.85;
const FIT_VIEW_PAN = 40;
const KEYBOARD_PAN_STEP = 80;

export interface DagKeyboardShortcutHandlers {
  state: FlowWorkbenchState;
  handleUpdateVp: (viewport: Partial<WorkbenchViewport>) => void;
  setIsSpacePressed: React.Dispatch<React.SetStateAction<boolean>>;
  onUndo?: () => void;
  onRedo?: () => void;
  onDuplicateNode?: (nodeId: string) => void;
  onDeleteNode?: (nodeId: string) => void;
  onDeleteTransition?: (transitionId: string) => void;
  onToggleMinimap?: () => void;
}

function handleModifierShortcut(e: KeyboardEvent, h: DagKeyboardShortcutHandlers): boolean {
  const isMod = e.metaKey || e.ctrlKey;
  if (!isMod) return false;
  const key = e.key.toLowerCase();
  if (key === "z") {
    e.preventDefault();
    if (e.shiftKey) h.onRedo?.();
    else h.onUndo?.();
    return true;
  }
  if (key === "d" && h.state.selectedNodeId) {
    e.preventDefault();
    h.onDuplicateNode?.(h.state.selectedNodeId);
    return true;
  }
  if (e.key === "+" || e.key === "=") {
    e.preventDefault();
    h.handleUpdateVp({ zoom: Math.min(h.state.viewport.zoom + ZOOM_STEP, ZOOM_MAX) });
    return true;
  }
  if (e.key === "-" || e.key === "_") {
    e.preventDefault();
    h.handleUpdateVp({ zoom: Math.max(h.state.viewport.zoom - ZOOM_STEP, ZOOM_MIN) });
    return true;
  }
  if (e.key === "0") {
    e.preventDefault();
    h.handleUpdateVp({ zoom: 1.0, panX: 0, panY: 0 });
    return true;
  }
  return false;
}

function handlePlainKeyShortcut(e: KeyboardEvent, h: DagKeyboardShortcutHandlers): void {
  const { viewport, selectedNodeId, selectedTransitionId } = h.state;
  if ((e.key === "Delete" || e.key === "Backspace") && (selectedNodeId || selectedTransitionId)) {
    e.preventDefault();
    if (selectedNodeId) h.onDeleteNode?.(selectedNodeId);
    else if (selectedTransitionId) h.onDeleteTransition?.(selectedTransitionId);
    return;
  }
  switch (e.key) {
    case "+":
    case "=":
      h.handleUpdateVp({ zoom: Math.min(viewport.zoom + ZOOM_STEP, ZOOM_MAX) });
      break;
    case "-":
    case "_":
      h.handleUpdateVp({ zoom: Math.max(viewport.zoom - ZOOM_STEP, ZOOM_MIN) });
      break;
    case "0":
      h.handleUpdateVp({ zoom: 1.0, panX: 0, panY: 0 });
      break;
    case "f":
    case "F":
      h.handleUpdateVp({ zoom: FIT_VIEW_ZOOM, panX: FIT_VIEW_PAN, panY: FIT_VIEW_PAN });
      break;
    case "m":
    case "M":
      h.onToggleMinimap?.();
      break;
    case "ArrowLeft":
      h.handleUpdateVp({ panX: viewport.panX + KEYBOARD_PAN_STEP });
      break;
    case "ArrowRight":
      h.handleUpdateVp({ panX: viewport.panX - KEYBOARD_PAN_STEP });
      break;
    case "ArrowUp":
      h.handleUpdateVp({ panY: viewport.panY + KEYBOARD_PAN_STEP });
      break;
    case "ArrowDown":
      h.handleUpdateVp({ panY: viewport.panY - KEYBOARD_PAN_STEP });
      break;
  }
}

export function useDagKeyboardShortcuts(handlers: DagKeyboardShortcutHandlers) {
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement ||
        e.target instanceof HTMLSelectElement
      ) {
        return;
      }
      if (e.code === "Space" && !e.repeat) {
        handlers.setIsSpacePressed(true);
        return;
      }
      if (!handleModifierShortcut(e, handlers)) {
        handlePlainKeyShortcut(e, handlers);
      }
    };
    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.code === "Space") handlers.setIsSpacePressed(false);
    };
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [handlers]);
}
