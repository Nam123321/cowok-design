// @story 05.4
// @implements AC-01, AC-02 (Cognitive Phase Transition Flow & User-Driven Style Taste State)
import { useCallback, useRef, useState } from "react";
import type { CognitivePhase, StageSkinMode } from "./StageSkinToggleBar";

export type ThemeModeOption = "light" | "warm" | "dark" | "dim";
export type PopularComponentId = "toast" | "confirm-dialog" | "tooltip" | "dropdown" | "empty-error";

const DEFAULT_THEME_MODES: ThemeModeOption[] = ["light"];
const DEFAULT_POPULAR_COMPONENTS: PopularComponentId[] = ["toast", "confirm-dialog", "empty-error"];

export function useStyleTasteState() {
  const [referenceApps, setReferenceApps] = useState("");
  const [themeModes, setThemeModes] = useState<ThemeModeOption[]>(DEFAULT_THEME_MODES);
  const [selectedPopularComponents, setSelectedPopularComponents] = useState<PopularComponentId[]>(
    DEFAULT_POPULAR_COMPONENTS
  );

  const toggleThemeMode = useCallback((mode: ThemeModeOption) => {
    setThemeModes((prev) => {
      if (prev.includes(mode)) return prev.length > 1 ? prev.filter((m) => m !== mode) : prev;
      return [...prev, mode];
    });
  }, []);

  const togglePopularComponent = useCallback((compId: PopularComponentId) => {
    setSelectedPopularComponents((prev) =>
      prev.includes(compId) ? prev.filter((c) => c !== compId) : [...prev, compId]
    );
  }, []);

  return {
    referenceApps, setReferenceApps, themeModes, toggleThemeMode, selectedPopularComponents, togglePopularComponent,
  };
}

interface PhaseFlowOptions {
  cognitivePhase: CognitivePhase;
  stageSkin: StageSkinMode;
  setCognitivePhase: (p: CognitivePhase) => void;
  setIsClarifyOpen: (open: boolean) => void;
  handleChangeSkin: (skin: StageSkinMode) => void;
  handleSwitchLiveDirection: (dirId: string) => Promise<void>;
}

function useStyleModalPhaseActions(opts: PhaseFlowOptions) {
  const { cognitivePhase, setCognitivePhase, setIsClarifyOpen, handleChangeSkin, handleSwitchLiveDirection } = opts;
  const previousPhaseRef = useRef<CognitivePhase>(cognitivePhase);

  const handleAdvanceToStyle = useCallback(() => {
    if (cognitivePhase !== "style") previousPhaseRef.current = cognitivePhase;
    setCognitivePhase("style");
    setIsClarifyOpen(true);
  }, [cognitivePhase, setCognitivePhase, setIsClarifyOpen]);

  const handleCancelStyleModal = useCallback(() => {
    setIsClarifyOpen(false);
    if (cognitivePhase === "style") setCognitivePhase(previousPhaseRef.current);
  }, [cognitivePhase, setCognitivePhase, setIsClarifyOpen]);

  const handleConfirmStyleDirection = useCallback(
    (dirId: string) => {
      void handleSwitchLiveDirection(dirId);
      setIsClarifyOpen(false);
      setCognitivePhase("polish");
      handleChangeSkin("brand-color");
    },
    [handleChangeSkin, handleSwitchLiveDirection, setCognitivePhase, setIsClarifyOpen]
  );

  return { handleAdvanceToStyle, handleCancelStyleModal, handleConfirmStyleDirection };
}

export function useCognitivePhaseFlow(opts: PhaseFlowOptions) {
  const { stageSkin, setCognitivePhase, handleChangeSkin } = opts;
  const modalActions = useStyleModalPhaseActions(opts);

  const handleApproveV1 = useCallback(() => {
    setCognitivePhase("approved");
    if (stageSkin !== "brand-color") handleChangeSkin("brand-color");
  }, [handleChangeSkin, setCognitivePhase, stageSkin]);

  const handleToggleSketchLens = useCallback(() => {
    handleChangeSkin(stageSkin === "sketch-paper" ? "brand-color" : "sketch-paper");
  }, [handleChangeSkin, stageSkin]);

  const handleSelectPhase = useCallback(
    (next: CognitivePhase) => {
      if (next === "sketch") { setCognitivePhase("sketch"); handleChangeSkin("sketch-paper"); return; }
      if (next === "style") { modalActions.handleAdvanceToStyle(); return; }
      if (next === "polish") { setCognitivePhase("polish"); handleChangeSkin("brand-color"); return; }
      handleApproveV1();
    },
    [handleApproveV1, handleChangeSkin, modalActions, setCognitivePhase]
  );

  return { ...modalActions, handleApproveV1, handleToggleSketchLens, handleSelectPhase };
}
