// @story 04.1, 04.5, 05.1
// @implements AC-02 (Option CSP-B: DOMParser-Only Zero-Network default-src 'none' + CSPRNG Script Nonce & Real Async SHA-256)
import {
  HostToIframeMessageSchema,
  IframeToHostMessageSchema,
  type HostToIframeMessage,
  type IframeToHostMessage,
} from "../../../lib/validations/flow-workbench";
import type { ScreenPreviewData, SlidePhaseItem } from "../components/companion-canvas-data";
import {
  buildByteZeroBridgeScript,
  computeArtifactSha256,
  computeArtifactSha256Sync,
  generateBootSecret,
  generateEpochNonce,
  type SandboxSubStateLabels,
} from "./html-sandbox-bridge";

export {
  HostToIframeMessageSchema,
  IframeToHostMessageSchema,
  computeArtifactSha256,
  computeArtifactSha256Sync,
  generateBootSecret,
  generateEpochNonce,
};
export type { HostToIframeMessage, IframeToHostMessage };

export const IFRAME_SANDBOX_PERMISSIONS = "allow-scripts allow-forms" as const;
const DOM_ANCHOR_HASH_LEN = 16;

export function buildStrictOfflineCsp(scriptNonce: string): string {
  return `default-src 'none'; base-uri 'none'; connect-src 'none'; form-action 'none'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; script-src 'nonce-${scriptNonce}';`;
}

export const STRICT_OFFLINE_CSP =
  "default-src 'none'; base-uri 'none'; connect-src 'none'; form-action 'none'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; script-src 'unsafe-inline';";

export interface WrapUserHtmlOptions {
  epoch?: number;
  bootSecret?: string;
  scriptNonce?: string;
  artifactSha256?: string;
  domAnchorHash?: string;
  parentOrigin?: string;
  subStateLabels?: SandboxSubStateLabels;
}

export type { SandboxSubStateLabels };

export interface PreviewDocumentLabels {
  lang?: string;
  kpi1Label?: string;
  kpi2Label?: string;
  kpi3Label?: string;
  actionHint?: string;
}

const MAX_HTML_PAYLOAD_BYTES = 1_048_576;
const SAFE_ATTR_NAME_RE = /^[a-zA-Z_:][a-zA-Z0-9_.:-]*$/;
const INLINE_EVENT_ATTR_RE = /^on/i;
const URL_CHECKED_ATTRS = new Set(["href", "src", "xlink:href", "action", "formaction", "poster", "data"]);
const FORBIDDEN_TAGS = ["script", "iframe", "object", "embed", "foreignobject", "svg", "template", "noscript", "base"];

export function validateUserHtmlPayload(raw: string): {
  valid: boolean;
  reason?: "empty" | "too_large" | "invalid_html";
} {
  const trimmed = raw.trim();
  if (!trimmed) return { valid: false, reason: "empty" };
  if (new TextEncoder().encode(trimmed).length > MAX_HTML_PAYLOAD_BYTES) {
    return { valid: false, reason: "too_large" };
  }
  if (!/<[a-zA-Z][a-zA-Z0-9-]*(?:\s[^>]*)?>/.test(trimmed) || /<[^>]*$/.test(trimmed)) {
    return { valid: false, reason: "invalid_html" };
  }
  if (typeof DOMParser !== "undefined") {
    const doc = new DOMParser().parseFromString(trimmed, "text/html");
    const hasElements =
      (doc.body && doc.body.children.length > 0) || (doc.head && doc.head.children.length > 0);
    if (!hasElements) return { valid: false, reason: "invalid_html" };
  }
  return { valid: true };
}

function escapeHtmlAttr(raw: string): string {
  return raw.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

export function sanitizeCssContent(cssText: string): string {
  if (!cssText || typeof cssText !== "string") return "";
  let s = cssText.replace(/\/\*[\s\S]*?\*\//g, "");
  s = s.replace(/@import\b[^;]+;?/gi, "");
  s = s.replace(/url\s*\([^)]*\)/gi, "none");
  s = s.replace(/url\s*\([^)]*$/gi, "none");
  return s;
}

function hasDangerousUrlScheme(value: string): boolean {
  const normalized = value.replace(/[\u0000-\u0020\u007f\\]+/g, "").toLowerCase();
  return (
    normalized.startsWith("javascript:") ||
    normalized.startsWith("vbscript:") ||
    normalized.startsWith("data:text/html") ||
    normalized.startsWith("data:image/svg+xml")
  );
}

function sanitizeSingleAttribute(el: Element, attr: Attr): void {
  const lowerName = attr.name.trim().toLowerCase();
  if (INLINE_EVENT_ATTR_RE.test(lowerName) || lowerName === "srcdoc") {
    el.removeAttribute(attr.name);
    return;
  }
  if (lowerName === "style") {
    const cleanStyle = sanitizeCssContent(attr.value);
    if (cleanStyle) el.setAttribute("style", cleanStyle);
    else el.removeAttribute("style");
    return;
  }
  if (URL_CHECKED_ATTRS.has(lowerName) && hasDangerousUrlScheme(attr.value)) {
    el.removeAttribute(attr.name);
  }
}

function collectSafeBodyAttributes(body: Element | null): string {
  if (!body) return "";
  return Array.from(body.attributes)
    .filter((a) => SAFE_ATTR_NAME_RE.test(a.name) && !INLINE_EVENT_ATTR_RE.test(a.name.trim()))
    .map((a) => ` ${a.name}="${escapeHtmlAttr(a.value)}"`)
    .join("");
}

function sanitizeViaDomParser(rawHtml: string, headPrefixHtml: string): string {
  if (typeof DOMParser === "undefined") {
    const safeText = escapeHtmlAttr(rawHtml);
    return `<!DOCTYPE html><html><head>${headPrefixHtml}</head><body><pre>${safeText}</pre></body></html>`;
  }
  const doc = new DOMParser().parseFromString(rawHtml, "text/html");
  doc.querySelectorAll(FORBIDDEN_TAGS.join(",")).forEach((el) => el.remove());
  doc.querySelectorAll("meta[http-equiv]").forEach((el) => {
    const eq = (el.getAttribute("http-equiv") || "").trim().toLowerCase();
    if (eq === "refresh" || eq === "content-security-policy") el.remove();
  });
  doc.querySelectorAll("style").forEach((styleEl) => {
    styleEl.textContent = sanitizeCssContent(styleEl.textContent || "");
  });
  doc.querySelectorAll("*").forEach((el) => {
    for (const attr of Array.from(el.attributes)) sanitizeSingleAttribute(el, attr);
  });
  const headHtml = doc.head ? doc.head.innerHTML : "";
  const bodyAttrs = collectSafeBodyAttributes(doc.body);
  const bodyHtml = doc.body ? doc.body.innerHTML : "";
  return `<!DOCTYPE html><html><head>${headPrefixHtml}${headHtml}</head><body${bodyAttrs}>${bodyHtml}</body></html>`;
}

export function wrapUserHtmlSrcDoc(rawHtml: string, opts?: WrapUserHtmlOptions): string {
  const epoch = opts?.epoch ?? generateEpochNonce();
  const bootSecret = opts?.bootSecret ?? generateBootSecret();
  const scriptNonce = opts?.scriptNonce ?? generateBootSecret();
  const artifactSha256 = opts?.artifactSha256 ?? computeArtifactSha256Sync(rawHtml);
  const domAnchorHash = opts?.domAnchorHash ?? artifactSha256.slice(0, DOM_ANCHOR_HASH_LEN);
  const parentOrigin =
    opts?.parentOrigin ??
    (typeof window !== "undefined" && window.location?.origin && window.location.origin !== "null"
      ? window.location.origin
      : undefined);

  const bridgeScript = buildByteZeroBridgeScript({
    epoch,
    bootSecret,
    scriptNonce,
    artifactSha256,
    domAnchorHash,
    parentOrigin,
    subStateLabels: opts?.subStateLabels,
  });
  const cspDirective = buildStrictOfflineCsp(scriptNonce);
  const headPrefix = `<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${escapeHtmlAttr(cspDirective)}">${bridgeScript}`;
  return sanitizeViaDomParser(rawHtml, headPrefix);
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

  return `<!DOCTYPE html><html lang="${escapeHtmlAttr(c.lang)}"><head><style>
${rootVars}
body{margin:0;padding:16px;font-family:var(--font-sans,-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif);background:var(--bg-surface,Canvas);color:var(--text-primary,CanvasText);display:flex;flex-direction:column;gap:12px}
.card{padding:12px;border-radius:8px;background:var(--bg-canvas,Canvas);border:1px solid var(--border-default,GrayText);cursor:pointer}
.kpi-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:8px}
.kpi{padding:8px 10px;border-radius:6px;background:var(--bg-surface,Canvas);border:1px solid var(--border-default,GrayText)}
.cta{padding:6px 12px;font-size:12px;font-weight:600;border-radius:6px;border:none;background:var(--action-primary,ButtonText);color:var(--text-on-action,ButtonFace);cursor:pointer}
</style></head><body data-ff-substate="${subStateAttr}">
<section class="card" data-ff-id="sec-header"><div style="font-size:10px;font-weight:600;color:var(--human-accent,LinkText);margin-bottom:4px">${escapeHtmlAttr(c.eyebrow)} • <span data-ff-fixture-slot="substate">${subStateAttr}</span></div><h2 style="margin:0 0 4px;font-size:16px">${escapeHtmlAttr(c.headline)}</h2><p style="margin:0;font-size:12px;color:var(--text-secondary,GrayText)">${escapeHtmlAttr(c.subtitle)}</p></section>
<section class="card kpi-grid" data-ff-id="sec-kpi"><div class="kpi"><div style="font-size:10px;color:var(--text-muted,GrayText)">${escapeHtmlAttr(c.kpi1Label)}</div><div style="font-size:13.5px;font-weight:600">${escapeHtmlAttr(c.kpi1Val)}</div></div><div class="kpi"><div style="font-size:10px;color:var(--text-muted,GrayText)">${escapeHtmlAttr(c.kpi2Label)}</div><div style="font-size:13.5px;font-weight:600;color:var(--state-success,Green)">${escapeHtmlAttr(c.statusVal)}</div></div><div class="kpi"><div style="font-size:10px;color:var(--text-muted,GrayText)">${escapeHtmlAttr(c.kpi3Label)}</div><div style="font-size:13.5px;font-weight:600">${escapeHtmlAttr(c.latencyVal)}</div></div></section>
<section class="card" data-ff-id="sec-biometric" style="display:flex;align-items:center;justify-content:space-between;gap:12px"><div><div style="font-size:12.5px;font-weight:600">${escapeHtmlAttr(params.activeScreenData.actionTitle)}</div>${c.actionHint ? `<div style="font-size:11px;color:var(--text-secondary,GrayText);margin-top:2px">${escapeHtmlAttr(c.actionHint)}</div>` : ""}</div><button type="button" class="cta">${escapeHtmlAttr(params.activeScreenData.ctaLabel)}</button></section>
</body></html>`;
}
