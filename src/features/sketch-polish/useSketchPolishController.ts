// @story 05.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { useCallback, useEffect, useRef, useState } from "react";
import { getClarifySession, postClarifyDirection } from "../../api/sketch-polish";
import {
  computeEffectivePatchStack,
  enqueueSerializedScreenPatch,
  hydrateScreenOccHistory,
  type LockedRuleReceiptNotice,
  type PendingLockedPatchDescriptor,
} from "../../lib/sketch-polish-occ-store";
import type {
  ClarifySessionData,
  LockedRuleChangeReceipt,
} from "../../lib/validations/sketch-polish";
import { evaluateClientContrast } from "./oklchGuardClient";
import type { CognitivePhase, StageSkinMode } from "./StageSkinToggleBar";
import type { SteeringChipId } from "./SteeringChipsBar";
import {
  useCognitivePhaseFlow,
  useStyleTasteState,
  type PopularComponentId,
  type ThemeModeOption,
} from "./useCognitivePhaseFlow";

export type { PopularComponentId, ThemeModeOption };

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const VALID_CSS_COLOR_RE = /^(?:#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})|rgba?\(\s*\d{1,3}\s*,\s*\d{1,3}\s*,\s*\d{1,3}(?:\s*,\s*[\d.]+)?\s*\))$/i;
const STEERING_TARGET_FF_ID = "sec-header";
const DEFAULT_SKIN_FG_HEX = "#2E2D2B";
const DEFAULT_SKIN_BG_HEX = "#FAF9F6";
const MIN_SWAP_LATENCY_MS = 0.1;

type StageSkinSwapFn = (skin: StageSkinMode, cssVars?: Record<string, string>) => void;
type StageApplyHtmlFn = (html: string) => void;

export interface SketchPolishControllerParams {
  projectId?: string;
  screenId?: string;
  locale: "vi" | "en";
  hasExistingBrandTokens?: boolean;
}

function readComputedBrandHexPair(): { fgHex: string; bgHex: string } {
  if (typeof document === "undefined" || typeof getComputedStyle !== "function") {
    return { fgHex: DEFAULT_SKIN_FG_HEX, bgHex: DEFAULT_SKIN_BG_HEX };
  }
  const cs = getComputedStyle(document.documentElement);
  const fg = cs.getPropertyValue("--text-primary").trim();
  const bg = cs.getPropertyValue("--bg-surface").trim();
  return {
    fgHex: VALID_CSS_COLOR_RE.test(fg) ? fg : DEFAULT_SKIN_FG_HEX,
    bgHex: VALID_CSS_COLOR_RE.test(bg) ? bg : DEFAULT_SKIN_BG_HEX,
  };
}

function applyHydratedPatchSnapshot(opts: {
  data: { patches: ReadonlyArray<{ astRevision: number; ffId: string; opKind: string; payloadJson: Record<string, unknown>; lockedRuleReceipt?: { applied: boolean } | null }>; htmlSnapshot: string };
  stageApplyHtmlRef: React.MutableRefObject<StageApplyHtmlFn | null>;
  pendingHydratedHtmlRef: React.MutableRefObject<string | null>;
  setCanUndo: (v: boolean) => void;
  setActiveChipId: (v: SteeringChipId | null) => void;
}): void {
  const activeStack = computeEffectivePatchStack(opts.data.patches);
  opts.setCanUndo(activeStack.length > 0);
  const topApplied = activeStack[activeStack.length - 1];
  if (topApplied?.opKind === "STEERING_CHIP" && typeof topApplied.payloadJson?.chipId === "string") {
    opts.setActiveChipId(topApplied.payloadJson.chipId as SteeringChipId);
  }
  if (activeStack.length > 0 && opts.data.htmlSnapshot.trim().length > 0) {
    if (opts.stageApplyHtmlRef.current) {
      opts.stageApplyHtmlRef.current(opts.data.htmlSnapshot);
    } else {
      opts.pendingHydratedHtmlRef.current = opts.data.htmlSnapshot;
    }
  }
}

function useSketchPolishHydration(opts: {
  isValidProject: boolean;
  projectId?: string;
  screenId: string;
  stageApplyHtmlRef: React.MutableRefObject<StageApplyHtmlFn | null>;
  pendingHydratedHtmlRef: React.MutableRefObject<string | null>;
  setCanUndo: (v: boolean) => void;
  setActiveChipId: (v: SteeringChipId | null) => void;
  setLockedRuleReceipt: (v: LockedRuleChangeReceipt | null) => void;
  setPendingLockedPatch: (v: PendingLockedPatchDescriptor | null) => void;
  setClarifySession: (v: ClarifySessionData | null) => void;
  setGoalPrompt: (v: string) => void;
  setClarifyLocale: (v: "vi" | "en") => void;
  setSyncError: (v: string | null) => void;
}) {
  const {
    isValidProject, projectId, screenId, stageApplyHtmlRef, pendingHydratedHtmlRef,
    setCanUndo, setActiveChipId, setLockedRuleReceipt, setPendingLockedPatch,
    setClarifySession, setGoalPrompt, setClarifyLocale, setSyncError,
  } = opts;
  useEffect(() => {
    setActiveChipId(null);
    setLockedRuleReceipt(null);
    setPendingLockedPatch(null);
    pendingHydratedHtmlRef.current = null;
    if (!isValidProject || !projectId || typeof fetch !== "function") return;
    let cancelled = false;
    void Promise.allSettled([hydrateScreenOccHistory(projectId, screenId), getClarifySession(projectId)]).then(
      ([patchRes, clarifyRes]) => {
        if (cancelled) return;
        if (patchRes.status === "fulfilled") {
          applyHydratedPatchSnapshot({ data: patchRes.value.data, stageApplyHtmlRef, pendingHydratedHtmlRef, setCanUndo, setActiveChipId });
        } else {
          setSyncError(patchRes.reason instanceof Error ? patchRes.reason.message : "patch_hydrate_failed");
        }
        if (clarifyRes.status === "fulfilled" && clarifyRes.value.data.session) {
          const s = clarifyRes.value.data.session;
          setClarifySession(s);
          setGoalPrompt(s.goalPrompt);
          setClarifyLocale(s.locale);
        }
      }
    );
    return () => { cancelled = true; };
  }, [isValidProject, pendingHydratedHtmlRef, projectId, screenId, setActiveChipId, setCanUndo, setClarifyLocale, setClarifySession, setGoalPrompt, setLockedRuleReceipt, setPendingLockedPatch, setSyncError, stageApplyHtmlRef]);
}

function useChipPatchAction(opts: {
  isValidProject: boolean;
  projectId?: string;
  screenId: string;
  stageApplyHtmlRef: React.MutableRefObject<StageApplyHtmlFn | null>;
  setActiveChipId: (v: SteeringChipId | null) => void;
  setIsApplying: (v: boolean) => void;
  setCanUndo: (v: boolean) => void;
  setOklchWarning: (v: string | null) => void;
  setLockedRuleReceipt: (v: LockedRuleChangeReceipt | null) => void;
  setPendingLockedPatch: (v: PendingLockedPatchDescriptor | null) => void;
}) {
  const { isValidProject, projectId, screenId, stageApplyHtmlRef, setActiveChipId, setIsApplying, setCanUndo, setOklchWarning, setLockedRuleReceipt, setPendingLockedPatch } = opts;
  return useCallback(async (chipId: SteeringChipId, confirmOverride = false) => {
    setActiveChipId(chipId);
    if (!isValidProject || !projectId) return;
    setIsApplying(true);
    try {
      const payloadJson = { chipId };
      const res = await enqueueSerializedScreenPatch({
        projectId, screenId, ffId: STEERING_TARGET_FF_ID, opKind: "STEERING_CHIP", payloadJson, confirmLockedRuleOverride: confirmOverride,
      });
      setCanUndo(res.data.undoAvailable);
      if (res.data.oklchGuard?.warning) setOklchWarning(res.data.oklchGuard.warning);
      if (res.data.htmlSnapshot && (!res.data.lockedRuleReceipt || res.data.lockedRuleReceipt.applied)) {
        stageApplyHtmlRef.current?.(res.data.htmlSnapshot);
      }
      if (res.data.lockedRuleReceipt?.requiresReview) {
        setLockedRuleReceipt(res.data.lockedRuleReceipt);
        setPendingLockedPatch({ projectId, screenId, ffId: STEERING_TARGET_FF_ID, opKind: "STEERING_CHIP", payloadJson });
      } else {
        setLockedRuleReceipt(null);
        setPendingLockedPatch(null);
      }
    } catch { /* keep optimistic selection if offline */ } finally {
      setIsApplying(false);
    }
  }, [isValidProject, projectId, screenId, setActiveChipId, setCanUndo, setIsApplying, setLockedRuleReceipt, setOklchWarning, setPendingLockedPatch, stageApplyHtmlRef]);
}

function useUndoPatchAction(opts: {
  isValidProject: boolean;
  projectId?: string;
  screenId: string;
  stageApplyHtmlRef: React.MutableRefObject<StageApplyHtmlFn | null>;
  setActiveChipId: (v: SteeringChipId | null) => void;
  setIsApplying: (v: boolean) => void;
}) {
  const { isValidProject, projectId, screenId, stageApplyHtmlRef, setActiveChipId, setIsApplying } = opts;
  return useCallback(async () => {
    if (!isValidProject || !projectId) { setActiveChipId(null); return; }
    setIsApplying(true);
    try {
      const res = await enqueueSerializedScreenPatch({ projectId, screenId, ffId: STEERING_TARGET_FF_ID, opKind: "UNDO", payloadJson: {} });
      if (res.data.htmlSnapshot) stageApplyHtmlRef.current?.(res.data.htmlSnapshot);
      setActiveChipId(null);
    } catch { /* ignore offline undo error */ } finally {
      setIsApplying(false);
    }
  }, [isValidProject, projectId, screenId, setActiveChipId, setIsApplying, stageApplyHtmlRef]);
}

const SKETCH_CSS_VAR_KEYS = [
  "--stage-sketch-bg",
  "--stage-sketch-card",
  "--stage-sketch-ink",
  "--stage-sketch-muted",
  "--stage-sketch-border",
  "--stage-sketch-grid",
  "--stage-sketch-shadow",
] as const;

function readActiveSketchThemeVars(): Record<string, string> {
  const defaults: Record<string, string> = {
    "--stage-sketch-bg": DEFAULT_SKIN_BG_HEX,
    "--stage-sketch-card": "#FFFDF8",
    "--stage-sketch-ink": DEFAULT_SKIN_FG_HEX,
    "--stage-sketch-muted": "#5C5850",
    "--stage-sketch-border": "#3E3B35",
    "--stage-sketch-grid": "rgba(44, 42, 38, 0.09)",
    "--stage-sketch-shadow": "rgba(44, 42, 38, 0.16)",
  };
  if (typeof document === "undefined" || typeof getComputedStyle !== "function") return defaults;
  const hostEl = document.querySelector(".workbench-shell") ?? document.documentElement;
  const cs = getComputedStyle(hostEl);
  const out: Record<string, string> = { ...defaults };
  for (const key of SKETCH_CSS_VAR_KEYS) {
    const val = cs.getPropertyValue(key).trim();
    if (val) out[key] = val;
  }
  return out;
}

function useSketchThemeSync(
  stageSkin: StageSkinMode,
  stageSkinSwapRef: React.MutableRefObject<StageSkinSwapFn | null>
): void {
  useEffect(() => {
    if (stageSkin !== "sketch-paper") return;
    const sync = () => {
      stageSkinSwapRef.current?.("sketch-paper", readActiveSketchThemeVars());
    };
    sync();
    if (typeof document === "undefined" || typeof MutationObserver === "undefined") return;
    const observer = new MutationObserver(sync);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "class"] });
    const shell = document.querySelector(".workbench-shell");
    if (shell) observer.observe(shell, { attributes: true, attributeFilter: ["data-theme", "class"] });
    return () => observer.disconnect();
  }, [stageSkin, stageSkinSwapRef]);
}

function useSkinSwapAction(
  stageSkinSwapRef: React.MutableRefObject<StageSkinSwapFn | null>,
  setStageSkin: (skin: StageSkinMode) => void,
  setOklchWarning: (w: string | null) => void,
  setLastSwapMs: (ms: number | null) => void
) {
  return useCallback((nextSkin: StageSkinMode) => {
    const start = typeof performance !== "undefined" ? performance.now() : Date.now();
    setStageSkin(nextSkin);
    const sketchVars = nextSkin === "sketch-paper" ? readActiveSketchThemeVars() : {};
    const pair = nextSkin === "sketch-paper"
      ? { fgHex: sketchVars["--stage-sketch-ink"] ?? DEFAULT_SKIN_FG_HEX, bgHex: sketchVars["--stage-sketch-bg"] ?? DEFAULT_SKIN_BG_HEX }
      : readComputedBrandHexPair();
    stageSkinSwapRef.current?.(nextSkin, sketchVars);
    const check = evaluateClientContrast(pair.fgHex, pair.bgHex);
    setOklchWarning(check.passesAa ? null : `WCAG AA (${check.ratio}:1)`);
    const elapsed = (typeof performance !== "undefined" ? performance.now() : Date.now()) - start;
    setLastSwapMs(Math.max(MIN_SWAP_LATENCY_MS, elapsed));
  }, [setLastSwapMs, setOklchWarning, setStageSkin, stageSkinSwapRef]);
}

function useLiveDirectionAction(opts: {
  isValidProject: boolean;
  projectId?: string;
  goalPrompt: string;
  clarifyLocale: "vi" | "en";
  clarifySession: ClarifySessionData | null;
  setClarifySession: (s: ClarifySessionData | null) => void;
}) {
  const { isValidProject, projectId, goalPrompt, clarifyLocale, clarifySession, setClarifySession } = opts;
  return useCallback(async (directionId: string) => {
    if (!isValidProject || !projectId) return;
    const trimmedGoal = goalPrompt.trim() || "Design executive SaaS workspace";
    if (
      clarifySession &&
      clarifySession.activeDirectionId === directionId &&
      clarifySession.goalPrompt === trimmedGoal &&
      clarifySession.locale === clarifyLocale
    ) {
      return;
    }
    try {
      const res = await postClarifyDirection({
        projectId,
        goalPrompt: trimmedGoal,
        locale: clarifyLocale,
        activeDirectionId: directionId,
        expectedGenerationEpoch: clarifySession?.generationEpoch,
      });
      setClarifySession(res.data);
    } catch { /* keep modal interactive if offline */ }
  }, [clarifyLocale, clarifySession, goalPrompt, isValidProject, projectId, setClarifySession]);
}

function useConfirmLockedOverrideAction(opts: {
  pendingLockedPatch: PendingLockedPatchDescriptor | null;
  stageApplyHtmlRef: React.MutableRefObject<StageApplyHtmlFn | null>;
  setIsApplying: (v: boolean) => void;
  setLockedRuleReceipt: (v: LockedRuleChangeReceipt | null) => void;
  setPendingLockedPatch: (v: PendingLockedPatchDescriptor | null) => void;
}) {
  const { pendingLockedPatch, stageApplyHtmlRef, setIsApplying, setLockedRuleReceipt, setPendingLockedPatch } = opts;
  return useCallback(() => {
    if (!pendingLockedPatch) { setLockedRuleReceipt(null); return; }
    setIsApplying(true);
    void enqueueSerializedScreenPatch({ ...pendingLockedPatch, confirmLockedRuleOverride: true })
      .then((res) => {
        if (res.data.htmlSnapshot) stageApplyHtmlRef.current?.(res.data.htmlSnapshot);
        pendingLockedPatch.onApplied?.();
        setLockedRuleReceipt(null);
        setPendingLockedPatch(null);
      })
      .catch(() => undefined)
      .finally(() => setIsApplying(false));
  }, [pendingLockedPatch, setIsApplying, setLockedRuleReceipt, setPendingLockedPatch, stageApplyHtmlRef]);
}

function useStageBridgeRegistrations(stageSkin: StageSkinMode) {
  const stageSkinRef = useRef<StageSkinMode>(stageSkin);
  stageSkinRef.current = stageSkin;
  const stageSkinSwapRef = useRef<StageSkinSwapFn | null>(null);
  const stageApplyHtmlRef = useRef<StageApplyHtmlFn | null>(null);
  const pendingHydratedHtmlRef = useRef<string | null>(null);
  const onRegisterStageSkinSwap = useCallback((fn: StageSkinSwapFn | null) => {
    stageSkinSwapRef.current = fn;
    if (fn && stageSkinRef.current === "sketch-paper") {
      fn("sketch-paper", readActiveSketchThemeVars());
    }
  }, []);
  const onRegisterStageApplyHtml = useCallback((fn: StageApplyHtmlFn | null) => {
    stageApplyHtmlRef.current = fn;
    if (fn && pendingHydratedHtmlRef.current) {
      fn(pendingHydratedHtmlRef.current);
      pendingHydratedHtmlRef.current = null;
    }
  }, []);
  return { stageSkinSwapRef, stageApplyHtmlRef, pendingHydratedHtmlRef, onRegisterStageSkinSwap, onRegisterStageApplyHtml };
}

export function useSketchPolishController(params: SketchPolishControllerParams) {
  const { projectId, screenId = "default", locale, hasExistingBrandTokens = false } = params;
  const isValidProject = Boolean(projectId && UUID_RE.test(projectId));
  const [cognitivePhase, setCognitivePhase] = useState<CognitivePhase>(hasExistingBrandTokens ? "polish" : "sketch");
  const [stageSkin, setStageSkin] = useState<StageSkinMode>(hasExistingBrandTokens ? "brand-color" : "sketch-paper");
  const [activeChipId, setActiveChipId] = useState<SteeringChipId | null>(null);
  const [isApplying, setIsApplying] = useState(false);
  const [canUndo, setCanUndo] = useState(false);
  const [oklchWarning, setOklchWarning] = useState<string | null>(null);
  const [lastSwapMs, setLastSwapMs] = useState<number | null>(null);
  const [lockedRuleReceipt, setLockedRuleReceipt] = useState<LockedRuleChangeReceipt | null>(null);
  const [pendingLockedPatch, setPendingLockedPatch] = useState<PendingLockedPatchDescriptor | null>(null);
  const [isClarifyOpen, setIsClarifyOpen] = useState(false);
  const [clarifySession, setClarifySession] = useState<ClarifySessionData | null>(null);
  const [goalPrompt, setGoalPrompt] = useState("");
  const [clarifyLocale, setClarifyLocale] = useState<"vi" | "en">(locale);
  const [syncError, setSyncError] = useState<string | null>(null);

  const taste = useStyleTasteState();
  const { stageSkinSwapRef, stageApplyHtmlRef, pendingHydratedHtmlRef, onRegisterStageSkinSwap, onRegisterStageApplyHtml } = useStageBridgeRegistrations(stageSkin);
  useSketchThemeSync(stageSkin, stageSkinSwapRef);
  useSketchPolishHydration({ isValidProject, projectId, screenId, stageApplyHtmlRef, pendingHydratedHtmlRef, setCanUndo, setActiveChipId, setLockedRuleReceipt, setPendingLockedPatch, setClarifySession, setGoalPrompt, setClarifyLocale, setSyncError });
  const executeChipPatch = useChipPatchAction({ isValidProject, projectId, screenId, stageApplyHtmlRef, setActiveChipId, setIsApplying, setCanUndo, setOklchWarning, setLockedRuleReceipt, setPendingLockedPatch });
  const handleUndo = useUndoPatchAction({ isValidProject, projectId, screenId, stageApplyHtmlRef, setActiveChipId, setIsApplying });
  const handleChangeSkin = useSkinSwapAction(stageSkinSwapRef, setStageSkin, setOklchWarning, setLastSwapMs);
  const handleSwitchLiveDirection = useLiveDirectionAction({ isValidProject, projectId, goalPrompt, clarifyLocale, clarifySession, setClarifySession });
  const handleConfirmRuleOverride = useConfirmLockedOverrideAction({ pendingLockedPatch, stageApplyHtmlRef, setIsApplying, setLockedRuleReceipt, setPendingLockedPatch });
  const phaseFlow = useCognitivePhaseFlow({ cognitivePhase, stageSkin, setCognitivePhase, setIsClarifyOpen, handleChangeSkin, handleSwitchLiveDirection });
  const handleLockedRuleNotice = useCallback((notice: LockedRuleReceiptNotice) => { setLockedRuleReceipt(notice.receipt); setPendingLockedPatch(notice.pendingPatch); }, []);

  return {
    cognitivePhase, stageSkin, activeChipId, isApplying, canUndo, oklchWarning, lastSwapMs, lockedRuleReceipt, syncError,
    isClarifyOpen, clarifySession, goalPrompt, clarifyLocale, setGoalPrompt, setClarifyLocale, setIsClarifyOpen,
    ...taste, ...phaseFlow,
    dismissLockedRuleReceipt: () => { setLockedRuleReceipt(null); setPendingLockedPatch(null); },
    onRegisterStageSkinSwap, onRegisterStageApplyHtml, handleLockedRuleNotice, handleChangeSkin,
    handleSelectChip: (chipId: SteeringChipId) => { void executeChipPatch(chipId, false); },
    handleUndo: () => { void handleUndo(); }, handleSwitchLiveDirection, handleConfirmRuleOverride,
  };
}
