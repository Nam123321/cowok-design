// @story 04.1, 05.1, 06.2
// @implements AC-03, AC-07 (Pure Helpers for Per-Screen App HTML CMS State)
import type { WordingSlotDto } from "../../../lib/validations/presentation";
import type { WorkbenchNode } from "../store/flow-workbench-types";
import {
  resolveScreenPreviewData,
  type ScreenPreviewData,
  type SlidePhaseItem,
} from "../components/companion-canvas-data";

export interface AppScreenUndoEntry {
  screenKey: string;
  ffId: string;
  previousValue: string;
  nextValue: string;
}

export const DEFAULT_PREVIEW_CURRENCY = "USD";

export function applyScreenFieldOverrides(
  base: ScreenPreviewData,
  overrides: Record<string, string>,
  formattedAmount: string
): ScreenPreviewData {
  return {
    ...base,
    eyebrow: overrides["ff-slide-eyebrow"] ?? base.eyebrow,
    headline: overrides["ff-slide-headline"] ?? base.headline,
    subtitle: overrides["ff-slide-subtitle"] ?? base.subtitle,
    kpi1Label: overrides["ff-kpi-1-label"] ?? base.kpi1Label,
    kpi2Label: overrides["ff-kpi-2-label"] ?? base.kpi2Label,
    kpi3Label: overrides["ff-kpi-3-label"] ?? base.kpi3Label,
    kpi1Display: overrides["ff-kpi-1-val"] ?? base.kpi1Display ?? formattedAmount,
    statusText: overrides["ff-kpi-2-val"] ?? base.statusText,
    latency: overrides["ff-kpi-3-val"] ?? base.latency,
    actionTitle: overrides["ff-action-title"] ?? base.actionTitle,
    actionHint: overrides["ff-action-hint"] ?? base.actionHint,
    ctaLabel: overrides["ff-action-cta"] ?? base.ctaLabel,
  };
}

export function buildAppScreenSlots(opts: {
  screenKey: string;
  data: ScreenPreviewData;
  formattedAmount: string;
  kpiLabels: { kpi1Label: string; kpi2Label: string; kpi3Label: string };
  t: (k: string, f: string) => string;
}): WordingSlotDto[] {
  const { screenKey, data, formattedAmount, kpiLabels, t } = opts;
  const defs: Array<{ ffId: string; slotLabel: string; slotKind: WordingSlotDto["slotKind"]; textValue: string }> = [
    { ffId: "ff-slide-eyebrow", slotLabel: t("presentation.cms.slotKind.eyebrow", "Eyebrow Tag"), slotKind: "text", textValue: data.eyebrow },
    { ffId: "ff-slide-headline", slotLabel: t("presentation.cms.slotKind.heading", "Headline"), slotKind: "heading", textValue: data.headline },
    { ffId: "ff-slide-subtitle", slotLabel: t("presentation.cms.slotKind.text", "Subtitle"), slotKind: "text", textValue: data.subtitle },
    { ffId: "ff-kpi-1-label", slotLabel: t("canvas.previewKpiLabelSlot1", "KPI 1 Label"), slotKind: "text", textValue: data.kpi1Label || kpiLabels.kpi1Label },
    { ffId: "ff-kpi-1-val", slotLabel: data.kpi1Label || kpiLabels.kpi1Label, slotKind: "metric", textValue: data.kpi1Display ?? formattedAmount },
    { ffId: "ff-kpi-2-label", slotLabel: t("canvas.previewKpiLabelSlot2", "KPI 2 Label"), slotKind: "text", textValue: data.kpi2Label || kpiLabels.kpi2Label },
    { ffId: "ff-kpi-2-val", slotLabel: data.kpi2Label || kpiLabels.kpi2Label, slotKind: "metric", textValue: data.statusText },
    { ffId: "ff-kpi-3-label", slotLabel: t("canvas.previewKpiLabelSlot3", "KPI 3 Label"), slotKind: "text", textValue: data.kpi3Label || kpiLabels.kpi3Label },
    { ffId: "ff-kpi-3-val", slotLabel: data.kpi3Label || kpiLabels.kpi3Label, slotKind: "metric", textValue: data.latency },
    { ffId: "ff-action-title", slotLabel: t("canvas.previewActionTitle", "Primary Action Title"), slotKind: "text", textValue: data.actionTitle },
    { ffId: "ff-action-hint", slotLabel: t("canvas.previewActionHint", "Action Hint"), slotKind: "text", textValue: data.actionHint ?? "" },
    { ffId: "ff-action-cta", slotLabel: t("canvas.previewActionCta", "CTA Button Label"), slotKind: "text", textValue: data.ctaLabel },
  ];
  return defs.map((d, idx) => ({
    id: `app-${screenKey}-${d.ffId}`,
    slideId: screenKey,
    ffId: d.ffId,
    slotLabel: d.slotLabel,
    slotKind: d.slotKind,
    textValue: d.textValue,
    chartsJson: "",
    safeStylesJson: "{}",
    updatedBy: "human",
    astRevision: idx + 1,
    updatedAt: 0,
  }));
}

export function buildAppScreenCards(opts: {
  nodes: WorkbenchNode[];
  activeScreenKey: string;
  baseScreenData: ScreenPreviewData;
  overridesByScreen: Record<string, Record<string, string>>;
  synthLabels: { kpiRoute: string; kpiState: string; kpiType: string; actionHint: string; ctaOpen: string };
}): SlidePhaseItem[] {
  const { nodes, activeScreenKey, baseScreenData, overridesByScreen, synthLabels } = opts;
  if (nodes.length === 0) {
    const ov = overridesByScreen[activeScreenKey] ?? {};
    return [
      {
        id: activeScreenKey,
        code: baseScreenData.eyebrow.split(" ")[0] || "SCR",
        title: ov["ff-slide-headline"] ?? baseScreenData.headline,
        headline: ov["ff-slide-headline"] ?? baseScreenData.headline,
        subtitle: ov["ff-slide-subtitle"] ?? baseScreenData.subtitle,
        kpi1: baseScreenData.amount,
        kpi2: ov["ff-kpi-2-val"] ?? baseScreenData.statusText,
        kpi3: ov["ff-kpi-3-val"] ?? baseScreenData.latency,
      },
    ];
  }
  return nodes.map((n) => {
    const preview = resolveScreenPreviewData(n, n.id, synthLabels);
    const ov = overridesByScreen[n.id] ?? {};
    const headline = ov["ff-slide-headline"] ?? preview.headline;
    return {
      id: n.id,
      code: `${n.screenKey} · ${n.routePath || "/"}`,
      title: n.title || headline,
      headline,
      subtitle: ov["ff-slide-subtitle"] ?? preview.subtitle,
      kpi1: preview.amount,
      kpi2: ov["ff-kpi-2-val"] ?? preview.statusText,
      kpi3: ov["ff-kpi-3-val"] ?? preview.latency,
    };
  });
}
