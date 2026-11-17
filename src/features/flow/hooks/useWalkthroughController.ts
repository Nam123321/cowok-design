// @story 05.1
// @implements AC-03, AC-06 (Dual-Screen & Full-Design Walkthrough Controller)
import type React from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { WorkbenchNode } from "../store/flow-workbench-types";

const WALKTHROUGH_INTERVAL_MS = 2500;

interface WalkthroughAutoPlayOpts {
  isActive: boolean;
  autoPlay: boolean;
  nodes: WorkbenchNode[] | undefined;
  stepIndexRef: React.MutableRefObject<number>;
  jumpTo: (idx: number) => void;
}

function useWalkthroughAutoPlay(opts: WalkthroughAutoPlayOpts) {
  const { isActive, autoPlay, nodes, stepIndexRef, jumpTo } = opts;
  const total = nodes?.length ?? 0;
  useEffect(() => {
    if (!isActive || !autoPlay || total === 0) return;
    const timer = setInterval(() => {
      jumpTo((stepIndexRef.current + 1) % total);
    }, WALKTHROUGH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [autoPlay, isActive, jumpTo, stepIndexRef, total]);
}

function useWalkthroughEscapeKey(isActive: boolean, onExit: () => void) {
  useEffect(() => {
    if (!isActive) return;
    const onKey = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") onExit();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isActive, onExit]);
}

function formatWalkthroughLabel(
  nodes: WorkbenchNode[] | undefined,
  stepIndex: number,
  formatNumber: (n: number) => string
): string | undefined {
  const total = nodes?.length ?? 0;
  if (total === 0) return undefined;
  const currentIdx = stepIndex % total;
  const currentNode = nodes?.[currentIdx];
  return `${currentNode?.screenKey || currentNode?.id || "SCR"} (${formatNumber(currentIdx + 1)}/${formatNumber(total)})`;
}

function selectBoundedNode(opts: {
  nodes: WorkbenchNode[] | undefined;
  nextIdx: number;
  stepIndexRef: React.MutableRefObject<number>;
  setStepIndex: React.Dispatch<React.SetStateAction<number>>;
  onSelectNode: ((nodeId: string) => void) | undefined;
}) {
  const { nodes, nextIdx, stepIndexRef, setStepIndex, onSelectNode } = opts;
  if (!nodes || nodes.length === 0) return;
  const bounded = ((nextIdx % nodes.length) + nodes.length) % nodes.length;
  stepIndexRef.current = bounded;
  setStepIndex(bounded);
  const target = nodes[bounded];
  if (target) onSelectNode?.(target.id);
}

export function useWalkthroughController(
  nodes: WorkbenchNode[] | undefined,
  onSelectNode: ((nodeId: string) => void) | undefined,
  setIsPresenting: React.Dispatch<React.SetStateAction<boolean>>
) {
  const [isActive, setIsActive] = useState(false);
  const [autoPlay, setAutoPlay] = useState(true);
  const [viewMode, setViewModeState] = useState<"dual" | "full">("dual");
  const [stepIndex, setStepIndex] = useState(0);
  const isActiveRef = useRef(isActive);
  const stepIndexRef = useRef(stepIndex);
  isActiveRef.current = isActive;
  stepIndexRef.current = stepIndex;
  const { locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });

  const jumpTo = useCallback((idx: number) => selectBoundedNode({ nodes, nextIdx: idx, stepIndexRef, setStepIndex, onSelectNode }), [nodes, onSelectNode]);
  const exitWalkthrough = useCallback(() => { setIsActive(false); setIsPresenting(false); }, [setIsPresenting]);

  useWalkthroughAutoPlay({ isActive, autoPlay, nodes, stepIndexRef, jumpTo });
  useWalkthroughEscapeKey(isActive, exitWalkthrough);

  const toggleWalkthrough = useCallback(() => {
    const next = !isActiveRef.current;
    setIsActive(next);
    setAutoPlay(next);
    setViewModeState("dual");
    setIsPresenting(false);
    if (next) jumpTo(0);
  }, [jumpTo, setIsPresenting]);

  return {
    isActive, autoPlay, viewMode,
    stepLabel: formatWalkthroughLabel(nodes, stepIndex, formatNumber),
    toggleAutoPlay: () => setAutoPlay((v) => !v),
    prevStep: () => { setAutoPlay(false); jumpTo(stepIndexRef.current - 1); },
    nextStep: () => { setAutoPlay(false); jumpTo(stepIndexRef.current + 1); },
    toggleWalkthrough,
    setViewMode: useCallback((m: "dual" | "full") => { setViewModeState(m); setIsPresenting(m === "full"); }, [setIsPresenting]),
    exitWalkthrough,
  };
}
