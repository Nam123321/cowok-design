// @story 04.1, 05.1, 06.1
// @implements AC-01, AC-04 (Live Preview HTML & Sample Fixture Sync Hooks)
import { useEffect, useMemo, useRef } from "react";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type { FixtureVariant } from "../../../lib/validations/screen-fixture";
import type { ScreenPreviewData, SlidePhaseItem } from "../components/companion-canvas-data";
import { buildPreviewDocumentHtml } from "../utils/html-sandbox-wrapper";

const PREVIEW_HOST_TOKEN_KEYS = [
  "--bg-canvas",
  "--bg-surface",
  "--text-primary",
  "--text-secondary",
  "--text-muted",
  "--border-default",
  "--action-primary",
  "--text-on-action",
  "--human-accent",
  "--state-success",
  "--font-sans",
] as const;

export function readHostThemeCssVars(): Record<string, string> | undefined {
  if (typeof window === "undefined" || typeof document === "undefined" || !document.documentElement) return undefined;
  const styles = window.getComputedStyle(document.documentElement);
  const resolved: Record<string, string> = {};
  for (const key of PREVIEW_HOST_TOKEN_KEYS) {
    const val = styles.getPropertyValue(key).trim();
    if (val) resolved[key] = val;
  }
  return Object.keys(resolved).length > 0 ? resolved : undefined;
}

export function useRawPreviewHtml(
  opts: {
    structureLens: "userflow" | "slide-cms";
    activeSlide: SlidePhaseItem;
    activeScreenData: ScreenPreviewData;
  },
  activeSubState: FixtureVariant,
  effectiveCustomHtml?: string
): string {
  const { t, locale } = useTranslation();
  const { formatCurrency } = useFormatter({ locale });
  return useMemo(() => {
    if (effectiveCustomHtml) return effectiveCustomHtml;
    const rawAmount = opts.structureLens === "slide-cms" ? (opts.activeSlide?.kpi1 ?? "$0") : (opts.activeScreenData?.amount ?? "$0");
    const formattedAmount = typeof rawAmount === "number" ? formatCurrency(rawAmount, "USD") : rawAmount;
    const fallbackScreenData: ScreenPreviewData = opts.activeScreenData || {
      eyebrow: t("canvas.fallbackEyebrow", "SCREEN"),
      headline: t("canvas.fallbackHeadline", "Screen State"),
      subtitle: t("canvas.fallbackSubtitle", "Preview"),
      statusText: t("canvas.fallbackStatus", "Ready"),
      latency: "12ms",
      actionTitle: t("canvas.fallbackActionTitle", "Action"),
      ctaLabel: t("canvas.fallbackCtaLabel", "Open"),
    };
    return buildPreviewDocumentHtml({
      structureLens: opts.structureLens,
      activeSlide: opts.activeSlide,
      activeScreenData: fallbackScreenData,
      formattedAmount,
      subState: activeSubState,
      cssVars: opts.structureLens === "slide-cms" ? undefined : readHostThemeCssVars(),
      labels: {
        lang: locale,
        kpi1Label: t("canvas.previewKpiLimitLabel", "Transaction Limit"),
        kpi2Label: t("canvas.previewKpiSecurityLabel", "Security Status"),
        kpi3Label: t("canvas.previewKpiLatencyLabel", "Response Time"),
        actionHint: t("canvas.previewFallbackHint", "Auto-synced live HTML preview from current graph state."),
      },
    });
  }, [activeSubState, effectiveCustomHtml, formatCurrency, locale, opts.activeScreenData, opts.activeSlide, opts.structureLens, t]);
}

export function useSampleFixtureSyncEffect(opts: {
  projectId?: string;
  selectedNodeId?: string | null;
  postFixtureHydrate: (variant: FixtureVariant, fixtureJson?: string) => void;
  setLocalSubState: (v: FixtureVariant) => void;
  onSelectSubState?: (v: FixtureVariant) => void;
}) {
  const { projectId, selectedNodeId, postFixtureHydrate, setLocalSubState, onSelectSubState } = opts;
  const hydrateRef = useRef(postFixtureHydrate);
  hydrateRef.current = postFixtureHydrate;
  const selectRef = useRef(onSelectSubState);
  selectRef.current = onSelectSubState;

  useEffect(() => {
    const onSampleFixture = (ev: Event) => {
      const detail = (ev as CustomEvent<{ projectId?: string; nodeId?: string; variant?: FixtureVariant; fixtureJson?: string }>).detail;
      if (!detail?.fixtureJson) return;
      if (detail.projectId && projectId && detail.projectId !== projectId) return;
      if (detail.nodeId && selectedNodeId && detail.nodeId !== selectedNodeId) return;
      const nextVar = detail.variant || "DEFAULT";
      setLocalSubState(nextVar);
      selectRef.current?.(nextVar);
      hydrateRef.current(nextVar, detail.fixtureJson);
    };
    window.addEventListener("cowok:sample-fixture-applied", onSampleFixture);
    return () => window.removeEventListener("cowok:sample-fixture-applied", onSampleFixture);
  }, [projectId, selectedNodeId, setLocalSubState]);
}
