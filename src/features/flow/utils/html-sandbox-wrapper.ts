// @story 04.1
// @implements AC-03, AC-07
import {
  HostToIframeMessageSchema,
  IframeToHostMessageSchema,
  type HostToIframeMessage,
  type IframeToHostMessage,
} from "../../../lib/validations/flow-workbench";
import type { ScreenPreviewData, SlidePhaseItem } from "../components/companion-canvas-data";

export { HostToIframeMessageSchema, IframeToHostMessageSchema };
export type { HostToIframeMessage, IframeToHostMessage };

export const IFRAME_SANDBOX_PERMISSIONS = "allow-scripts allow-forms" as const;

export const STRICT_OFFLINE_CSP =
  "default-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; script-src 'unsafe-inline'; connect-src 'none'; form-action 'none';";

export const DESIGN_CDN_CSP =
  "default-src 'none'; style-src 'unsafe-inline' https://fonts.googleapis.com https://cdn.tailwindcss.com https://unpkg.com https://cdn.jsdelivr.net https://cdnjs.cloudflare.com; img-src data: blob:; font-src data: https://fonts.gstatic.com; script-src 'unsafe-inline' https://cdn.tailwindcss.com https://unpkg.com https://cdn.jsdelivr.net https://cdnjs.cloudflare.com; connect-src 'none'; form-action 'none';";

export interface WrapUserHtmlOptions {
  allowDesignCdn?: boolean;
  epoch?: number;
}

export interface PreviewDocumentLabels {
  lang?: string;
  kpi1Label?: string;
  kpi2Label?: string;
  kpi3Label?: string;
  actionHint?: string;
}

const MAX_HTML_PAYLOAD_BYTES = 1_048_576;
const MAX_SAFE_EPOCH_INT32 = 0x7fffffff;
const SAFE_ATTR_NAME_RE = /^[a-zA-Z_:][a-zA-Z0-9_.:-]*$/;
const INLINE_EVENT_ATTR_RE = /^on/i;

export function generateEpochNonce(): number {
  if (typeof globalThis.crypto?.getRandomValues === "function") {
    const buf = new Uint32Array(1);
    globalThis.crypto.getRandomValues(buf);
    return (buf[0] % MAX_SAFE_EPOCH_INT32) + 1;
  }
  return Math.floor(Math.random() * MAX_SAFE_EPOCH_INT32) + 1;
}

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
  return raw
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function buildBridgeApplyHtmlSnippet(): string {
  return `  function applySanitizedHtml(raw) {
    if (typeof raw !== "string" || !document.body) return;
    if (typeof DOMParser !== "undefined") {
      var parsed = new DOMParser().parseFromString(raw, "text/html");
      parsed.querySelectorAll("base,script").forEach(function(el){ el.remove(); });
      parsed.querySelectorAll("meta[http-equiv]").forEach(function(el){
        var eq = (el.getAttribute("http-equiv") || "").trim().toLowerCase();
        if (eq === "refresh" || eq === "content-security-policy") el.remove();
      });
      if (parsed.body) {
        parsed.body.querySelectorAll("*").forEach(function(el){
          Array.from(el.attributes).forEach(function(attr){
            if (/^on/i.test(attr.name)) el.removeAttribute(attr.name);
          });
        });
        document.body.innerHTML = parsed.body.innerHTML;
      }
    }
    var h = document.documentElement ? document.documentElement.scrollHeight : 0;
    if (bridgePort) bridgePort.postMessage({ type: "HEIGHT_CHANGED", epoch: EPOCH, height: h });
  }`;
}

function buildByteZeroBridgeScript(epoch: number): string {
  const safeEpoch = Number.isInteger(epoch) && epoch >= 1 ? epoch : 1;
  const applySnippet = buildBridgeApplyHtmlSnippet();
  return `<script data-cowok-sandbox-bridge="v1">(function(){
  "use strict";
  var EPOCH = ${safeEpoch};
  var channelBound = false;
  var bridgePort = null;
  if (typeof MessagePort !== "undefined" && MessagePort.prototype) {
    Object.freeze(MessagePort.prototype);
  }
  window.addEventListener("submit", function(e) {
    e.preventDefault();
    e.stopImmediatePropagation();
  }, true);
${applySnippet}
  window.addEventListener("message", function(event) {
    if (channelBound || !event || !event.data || event.data.type !== "INIT_PORT") return;
    if (event.data.epoch !== EPOCH || !event.ports || !event.ports[0]) return;
    channelBound = true;
    bridgePort = event.ports[0];
    bridgePort.onmessage = function(portEv) {
      if (!portEv || !portEv.data || portEv.data.epoch !== EPOCH) return;
      if (portEv.data.type === "APPLY_HTML") applySanitizedHtml(portEv.data.html);
    };
    bridgePort.postMessage({ type: "READY", epoch: EPOCH });
    var h = document.documentElement ? document.documentElement.scrollHeight : 0;
    bridgePort.postMessage({ type: "HEIGHT_CHANGED", epoch: EPOCH, height: h });
  });
  window.addEventListener("click", function(event) {
    if (!event.isTrusted || !bridgePort) return;
    var el = event.target && event.target.closest ? event.target.closest("[data-ff-id]") : null;
    if (!el) return;
    var id = String(el.getAttribute("data-ff-id") || "").slice(0, 200);
    var tag = String(el.tagName || "DIV").toLowerCase().slice(0, 64);
    bridgePort.postMessage({ type: "ELEMENT_CLICKED", epoch: EPOCH, selector: '[data-ff-id="' + id + '"]', tagName: tag });
  }, true);
  window.addEventListener("error", function(err) {
    if (!bridgePort) return;
    bridgePort.postMessage({ type: "RUNTIME_ERROR", epoch: EPOCH, message: String(err && err.message || "runtime_error").slice(0, 1024) });
  });
})();</script>`;
}

function stripForbiddenHeadTagsRegex(html: string): string {
  return html
    .replace(/<base\b[^>]*>/gi, "")
    .replace(/<meta\b[^>]*http-equiv\s*=\s*["']?\s*(?:refresh|content-security-policy)\s*["']?[^>]*>/gi, "");
}

function sanitizeViaDomParser(rawHtml: string, headPrefixHtml: string): string | null {
  if (typeof DOMParser === "undefined") return null;
  const parser = new DOMParser();
  const doc = parser.parseFromString(rawHtml, "text/html");
  doc.querySelectorAll("base").forEach((el) => el.remove());
  doc.querySelectorAll("meta[http-equiv]").forEach((el) => {
    const eq = (el.getAttribute("http-equiv") || "").trim().toLowerCase();
    if (eq === "refresh" || eq === "content-security-policy") el.remove();
  });
  const headHtml = doc.head ? doc.head.innerHTML : "";
  const bodyAttrs = doc.body
    ? Array.from(doc.body.attributes)
        .filter((a) => SAFE_ATTR_NAME_RE.test(a.name) && !INLINE_EVENT_ATTR_RE.test(a.name.trim()))
        .map((a) => ` ${a.name}="${escapeHtmlAttr(a.value)}"`)
        .join("")
    : "";
  const bodyHtml = doc.body ? doc.body.innerHTML : "";
  return `<!DOCTYPE html><html><head>${headPrefixHtml}${headHtml}</head><body${bodyAttrs}>${bodyHtml}</body></html>`;
}

export function wrapUserHtmlSrcDoc(rawHtml: string, opts?: WrapUserHtmlOptions): string {
  const csp = opts?.allowDesignCdn ? DESIGN_CDN_CSP : STRICT_OFFLINE_CSP;
  const epoch = opts?.epoch ?? generateEpochNonce();
  const headPrefix = `<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${escapeHtmlAttr(csp)}">${buildByteZeroBridgeScript(epoch)}`;

  const domResult = sanitizeViaDomParser(rawHtml, headPrefix);
  if (domResult !== null) return domResult;

  const cleaned = stripForbiddenHeadTagsRegex(rawHtml);
  if (/<head\b[^>]*>/i.test(cleaned)) {
    return cleaned.replace(/<head\b[^>]*>/i, (match) => `${match}${headPrefix}`);
  }
  return `<!DOCTYPE html><html><head>${headPrefix}</head><body>${cleaned}</body></html>`;
}

function buildPreviewRootCssVars(cssVars?: Record<string, string>): string {
  if (!cssVars) return "";
  const pairs = Object.entries(cssVars)
    .filter(([k, v]) => /^--[a-zA-Z0-9-]+$/.test(k) && v.trim().length > 0 && !/[;{}]/.test(v))
    .map(([k, v]) => `${k}:${v.trim()}`);
  return pairs.length > 0 ? `:root{${pairs.join(";")}}` : "";
}

function resolveSlidePreviewContent(slide: SlidePhaseItem, formattedAmount: string, labels?: PreviewDocumentLabels) {
  return {
    eyebrow: slide.code,
    headline: slide.headline,
    subtitle: slide.subtitle,
    kpi1Val: formattedAmount,
    statusVal: slide.kpi2,
    latencyVal: slide.kpi3,
    lang: labels?.lang ?? "en",
    kpi1Label: labels?.kpi1Label ?? "Metric",
    kpi2Label: labels?.kpi2Label ?? "Status",
    kpi3Label: labels?.kpi3Label ?? "Latency",
    actionHint: labels?.actionHint ?? "",
  };
}

function resolveUserflowPreviewContent(scr: ScreenPreviewData, formattedAmount: string, labels?: PreviewDocumentLabels) {
  return {
    eyebrow: scr.eyebrow,
    headline: scr.headline,
    subtitle: scr.subtitle,
    kpi1Val: scr.kpi1Display ?? formattedAmount,
    statusVal: scr.statusText,
    latencyVal: scr.latency,
    lang: labels?.lang ?? "en",
    kpi1Label: scr.kpi1Label ?? labels?.kpi1Label ?? "Metric",
    kpi2Label: scr.kpi2Label ?? labels?.kpi2Label ?? "Status",
    kpi3Label: scr.kpi3Label ?? labels?.kpi3Label ?? "Latency",
    actionHint: scr.actionHint ?? labels?.actionHint ?? "",
  };
}

function resolvePreviewContent(params: {
  structureLens: "userflow" | "slide-cms";
  activeSlide: SlidePhaseItem;
  activeScreenData: ScreenPreviewData;
  formattedAmount: string;
  labels?: PreviewDocumentLabels;
}) {
  if (params.structureLens === "slide-cms") {
    return resolveSlidePreviewContent(params.activeSlide, params.formattedAmount, params.labels);
  }
  return resolveUserflowPreviewContent(params.activeScreenData, params.formattedAmount, params.labels);
}

export function buildPreviewDocumentHtml(params: {
  structureLens: "userflow" | "slide-cms";
  activeSlide: SlidePhaseItem;
  activeScreenData: ScreenPreviewData;
  formattedAmount: string;
  labels?: PreviewDocumentLabels;
  cssVars?: Record<string, string>;
}): string {
  const c = resolvePreviewContent(params);
  const rootVars = buildPreviewRootCssVars(params.cssVars);

  return `<!DOCTYPE html><html lang="${escapeHtmlAttr(c.lang)}"><head><style>
${rootVars}
body{margin:0;padding:16px;font-family:var(--font-sans,-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif);background:var(--bg-surface,Canvas);color:var(--text-primary,CanvasText);display:flex;flex-direction:column;gap:12px}
.card{padding:12px;border-radius:8px;background:var(--bg-canvas,Canvas);border:1px solid var(--border-default,GrayText);cursor:pointer}
.kpi-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:8px}
.kpi{padding:8px 10px;border-radius:6px;background:var(--bg-surface,Canvas);border:1px solid var(--border-default,GrayText)}
.cta{padding:6px 12px;font-size:12px;font-weight:600;border-radius:6px;border:none;background:var(--action-primary,ButtonText);color:var(--text-on-action,ButtonFace);cursor:pointer}
</style></head><body>
<section class="card" data-ff-id="sec-header"><div style="font-size:10px;font-weight:600;color:var(--human-accent,LinkText);margin-bottom:4px">${escapeHtmlAttr(c.eyebrow)}</div><h2 style="margin:0 0 4px;font-size:16px">${escapeHtmlAttr(c.headline)}</h2><p style="margin:0;font-size:12px;color:var(--text-secondary,GrayText)">${escapeHtmlAttr(c.subtitle)}</p></section>
<section class="card kpi-grid" data-ff-id="sec-kpi"><div class="kpi"><div style="font-size:10px;color:var(--text-muted,GrayText)">${escapeHtmlAttr(c.kpi1Label)}</div><div style="font-size:13.5px;font-weight:600">${escapeHtmlAttr(c.kpi1Val)}</div></div><div class="kpi"><div style="font-size:10px;color:var(--text-muted,GrayText)">${escapeHtmlAttr(c.kpi2Label)}</div><div style="font-size:13.5px;font-weight:600;color:var(--state-success,Green)">${escapeHtmlAttr(c.statusVal)}</div></div><div class="kpi"><div style="font-size:10px;color:var(--text-muted,GrayText)">${escapeHtmlAttr(c.kpi3Label)}</div><div style="font-size:13.5px;font-weight:600">${escapeHtmlAttr(c.latencyVal)}</div></div></section>
<section class="card" data-ff-id="sec-biometric" style="display:flex;align-items:center;justify-content:space-between;gap:12px"><div><div style="font-size:12.5px;font-weight:600">${escapeHtmlAttr(params.activeScreenData.actionTitle)}</div>${c.actionHint ? `<div style="font-size:11px;color:var(--text-secondary,GrayText);margin-top:2px">${escapeHtmlAttr(c.actionHint)}</div>` : ""}</div><button type="button" class="cta">${escapeHtmlAttr(params.activeScreenData.ctaLabel)}</button></section>
</body></html>`;
}
