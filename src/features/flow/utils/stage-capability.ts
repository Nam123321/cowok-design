// @story 06.1
// @implements AC-03, AC-06 (Shared Capability Map for FE-DAG & Presentation Workstreams)
export type StageWorkstreamMode = "fe-dag" | "presentation";

export interface StageCapabilityConfig {
  mode: StageWorkstreamMode;
  showDualScreenToggle: boolean;
  showViewportPresets: boolean;
  showSlideAspectAndZoom: boolean;
  defaultWalkthroughView: "dual" | "full";
}

export const STAGE_CAPABILITY_BY_MODE: Record<StageWorkstreamMode, StageCapabilityConfig> = {
  "fe-dag": {
    mode: "fe-dag",
    showDualScreenToggle: true,
    showViewportPresets: true,
    showSlideAspectAndZoom: false,
    defaultWalkthroughView: "dual",
  },
  presentation: {
    mode: "presentation",
    showDualScreenToggle: false,
    showViewportPresets: false,
    showSlideAspectAndZoom: true,
    defaultWalkthroughView: "full",
  },
};

export function resolveStageMode(structureLens?: "userflow" | "slide-cms"): StageWorkstreamMode {
  return structureLens === "slide-cms" ? "presentation" : "fe-dag";
}

export function buildSlideWalkthroughLabel(opts: {
  slideId?: string;
  activeIndex: number;
  totalSlides: number;
  formatNumber: (n: number) => string;
}): string {
  const safeTotal = Math.max(1, opts.totalSlides);
  const safeIdx = Math.max(0, Math.min(safeTotal - 1, opts.activeIndex));
  const idLabel = opts.slideId || "SLD";
  return `${idLabel} (${opts.formatNumber(safeIdx + 1)}/${opts.formatNumber(safeTotal)})`;
}
