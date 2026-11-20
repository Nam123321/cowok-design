// @story 04.1, 04.5, 05.1
// @implements AC-02 (Synthesized Live Preview HTML Document Builder)
import type { ScreenPreviewData, SlidePhaseItem } from "../components/companion-canvas-data";
import { escapeHtmlAttr } from "./html-sandbox-sanitizer";

export interface PreviewDocumentLabels {
  lang?: string;
  kpi1Label?: string;
  kpi2Label?: string;
  kpi3Label?: string;
  actionHint?: string;
}

function buildPreviewRootCssVars(cssVars?: Record<string, string>): string {
  if (!cssVars) return "";
  const pairs = Object.entries(cssVars)
    .filter(([k, v]) => /^--[a-zA-Z0-9-]+$/.test(k) && v.trim().length > 0 && !/[;{}]/.test(v))
    .map(([k, v]) => `${k}:${v.trim()}`);
  return pairs.length > 0 ? `:root{${pairs.join(";")}}` : "";
}

function resolveCommonPreviewLabels(labels?: PreviewDocumentLabels) {
  return {
    lang: labels?.lang ?? "en",
    kpi1Label: labels?.kpi1Label ?? "Metric",
    kpi2Label: labels?.kpi2Label ?? "Status",
    kpi3Label: labels?.kpi3Label ?? "Latency",
    actionHint: labels?.actionHint ?? "",
  };
}

function resolvePreviewContent(params: {
  structureLens: "userflow" | "slide-cms";
  activeSlide: SlidePhaseItem;
  activeScreenData: ScreenPreviewData;
  formattedAmount: string;
  labels?: PreviewDocumentLabels;
}) {
  const base = resolveCommonPreviewLabels(params.labels);
  if (params.structureLens === "slide-cms") {
    const slide = params.activeSlide;
    return {
      ...base,
      eyebrow: slide.code,
      headline: slide.headline,
      subtitle: slide.subtitle,
      kpi1Val: params.formattedAmount,
      statusVal: slide.kpi2,
      latencyVal: slide.kpi3,
    };
  }
  const scr = params.activeScreenData;
  return {
    eyebrow: scr.eyebrow,
    headline: scr.headline,
    subtitle: scr.subtitle,
    kpi1Val: scr.kpi1Display ?? params.formattedAmount,
    statusVal: scr.statusText,
    latencyVal: scr.latency,
    lang: base.lang,
    kpi1Label: scr.kpi1Label ?? base.kpi1Label,
    kpi2Label: scr.kpi2Label ?? base.kpi2Label,
    kpi3Label: scr.kpi3Label ?? base.kpi3Label,
    actionHint: scr.actionHint ?? base.actionHint,
  };
}

export function buildPreviewDocumentHtml(params: {
  structureLens: "userflow" | "slide-cms";
  activeSlide: SlidePhaseItem;
  activeScreenData: ScreenPreviewData;
  formattedAmount: string;
  labels?: PreviewDocumentLabels;
  cssVars?: Record<string, string>;
  subState?: string;
}): string {
  const c = resolvePreviewContent(params);
  const rootVars = buildPreviewRootCssVars(params.cssVars);
  const subStateAttr = escapeHtmlAttr((params.subState || "DEFAULT").toUpperCase());

  const isDeck = params.structureLens === "slide-cms";
  const deckBodyAttrs = isDeck ? ' data-cowok-deck-protocol="1"' : "";
  const deckHtmlStyle = isDeck ? ' style="color-scheme:dark"' : "";
  const slideSectionClass = isDeck ? "card slide active" : "card";

  return `<!DOCTYPE html><html lang="${escapeHtmlAttr(c.lang)}"${deckBodyAttrs}${deckHtmlStyle}><head><style>
${rootVars}
body{margin:0;padding:16px;font-family:var(--font-sans,-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif);background:var(--bg-surface,Canvas);color:var(--text-primary,CanvasText);display:flex;flex-direction:column;gap:12px}
.card{padding:12px;border-radius:8px;background:var(--bg-canvas,Canvas);border:1px solid var(--border-default,GrayText);cursor:pointer}
.slide{box-sizing:border-box;max-height:var(--slide-h,1080px);overflow-y:auto}
.kpi-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:8px}
.kpi{padding:8px 10px;border-radius:6px;background:var(--bg-surface,Canvas);border:1px solid var(--border-default,GrayText)}
.cta{padding:6px 12px;font-size:12px;font-weight:600;border-radius:6px;border:none;background:var(--action-primary,ButtonText);color:var(--text-on-action,ButtonFace);cursor:pointer}
</style></head><body${deckBodyAttrs} data-ff-substate="${subStateAttr}">
<section class="${slideSectionClass}" data-slide-index="0" data-ff-id="sec-header"><div style="font-size:10px;font-weight:600;color:var(--human-accent,LinkText);margin-bottom:4px"><span data-ff-id="ff-slide-eyebrow">${escapeHtmlAttr(c.eyebrow)}</span> • <span data-ff-fixture-slot="substate">${subStateAttr}</span></div><h2 data-ff-id="ff-slide-headline" style="margin:0 0 4px;font-size:16px">${escapeHtmlAttr(c.headline)}</h2><p data-ff-id="ff-slide-subtitle" style="margin:0;font-size:12px;color:var(--text-secondary,GrayText)">${escapeHtmlAttr(c.subtitle)}</p></section>
<section class="card kpi-grid" data-ff-id="sec-kpi"><div class="kpi"><div data-ff-id="ff-kpi-1-label" style="font-size:10px;color:var(--text-muted,GrayText)">${escapeHtmlAttr(c.kpi1Label)}</div><div data-ff-id="ff-kpi-1-val" style="font-size:13.5px;font-weight:600">${escapeHtmlAttr(c.kpi1Val)}</div></div><div class="kpi"><div data-ff-id="ff-kpi-2-label" style="font-size:10px;color:var(--text-muted,GrayText)">${escapeHtmlAttr(c.kpi2Label)}</div><div data-ff-id="ff-kpi-2-val" style="font-size:13.5px;font-weight:600;color:var(--state-success,Green)">${escapeHtmlAttr(c.statusVal)}</div></div><div class="kpi"><div data-ff-id="ff-kpi-3-label" style="font-size:10px;color:var(--text-muted,GrayText)">${escapeHtmlAttr(c.kpi3Label)}</div><div data-ff-id="ff-kpi-3-val" style="font-size:13.5px;font-weight:600">${escapeHtmlAttr(c.latencyVal)}</div></div></section>
<section class="card" data-ff-id="sec-biometric" style="display:flex;align-items:center;justify-content:space-between;gap:12px"><div><div data-ff-id="ff-action-title" style="font-size:12.5px;font-weight:600">${escapeHtmlAttr(params.activeScreenData.actionTitle)}</div>${c.actionHint ? `<div data-ff-id="ff-action-hint" style="font-size:11px;color:var(--text-secondary,GrayText);margin-top:2px">${escapeHtmlAttr(c.actionHint)}</div>` : ""}</div><button type="button" data-ff-id="ff-action-cta" class="cta">${escapeHtmlAttr(params.activeScreenData.ctaLabel)}</button></section>
</body></html>`;
}
