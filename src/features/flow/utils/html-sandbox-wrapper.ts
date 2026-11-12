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

function escapeHtmlAttr(raw: string): string {
  return raw
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function buildByteZeroBridgeScript(epoch: number): string {
  const safeEpoch = Number.isInteger(epoch) && epoch >= 1 ? epoch : 1;
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
  window.addEventListener("message", function(event) {
    if (channelBound || !event || !event.data || event.data.type !== "INIT_PORT") return;
    if (event.data.epoch !== EPOCH || !event.ports || !event.ports[0]) return;
    channelBound = true;
    bridgePort = event.ports[0];
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
        .map((a) => ` ${a.name}="${escapeHtmlAttr(a.value)}"`)
        .join("")
    : "";
  const bodyHtml = doc.body ? doc.body.innerHTML : "";
  return `<!DOCTYPE html><html><head>${headPrefixHtml}${headHtml}</head><body${bodyAttrs}>${bodyHtml}</body></html>`;
}

export function wrapUserHtmlSrcDoc(rawHtml: string, opts?: WrapUserHtmlOptions): string {
  const csp = opts?.allowDesignCdn ? DESIGN_CDN_CSP : STRICT_OFFLINE_CSP;
  const epoch = opts?.epoch ?? 1;
  const headPrefix = `<meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${escapeHtmlAttr(csp)}">${buildByteZeroBridgeScript(epoch)}`;

  const domResult = sanitizeViaDomParser(rawHtml, headPrefix);
  if (domResult !== null) return domResult;

  const cleaned = stripForbiddenHeadTagsRegex(rawHtml);
  if (/<head\b[^>]*>/i.test(cleaned)) {
    return cleaned.replace(/<head\b[^>]*>/i, (match) => `${match}${headPrefix}`);
  }
  return `<!DOCTYPE html><html><head>${headPrefix}</head><body>${cleaned}</body></html>`;
}

export function buildPreviewDocumentHtml(params: {
  structureLens: "userflow" | "slide-cms";
  activeSlide: SlidePhaseItem;
  activeScreenData: ScreenPreviewData;
  formattedAmount: string;
}): string {
  const eyebrow = params.structureLens === "slide-cms" ? params.activeSlide.code : params.activeScreenData.eyebrow;
  const headline = params.structureLens === "slide-cms" ? params.activeSlide.headline : params.activeScreenData.headline;
  const subtitle = params.structureLens === "slide-cms" ? params.activeSlide.subtitle : params.activeScreenData.subtitle;
  const statusVal = params.structureLens === "slide-cms" ? params.activeSlide.kpi2 : params.activeScreenData.statusText;
  const latencyVal = params.structureLens === "slide-cms" ? params.activeSlide.kpi3 : params.activeScreenData.latency;

  return `<!DOCTYPE html><html lang="vi"><head><style>
body{margin:0;padding:16px;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;background:#FFFFFF;color:#0F172A;display:flex;flex-direction:column;gap:12px}
.card{padding:12px;border-radius:8px;background:#F8FAFC;border:1px solid #E2E8F0;cursor:pointer}
.kpi-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:8px}
.kpi{padding:8px 10px;border-radius:6px;background:#FFFFFF;border:1px solid #E2E8F0}
.cta{padding:6px 12px;font-size:12px;font-weight:600;border-radius:6px;border:none;background:#0F172A;color:#FFFFFF;cursor:pointer}
</style></head><body>
<section class="card" data-ff-id="sec-header"><div style="font-size:10px;font-weight:600;color:#2563EB;margin-bottom:4px">${escapeHtmlAttr(eyebrow)}</div><h2 style="margin:0 0 4px;font-size:16px">${escapeHtmlAttr(headline)}</h2><p style="margin:0;font-size:12px;color:#475467">${escapeHtmlAttr(subtitle)}</p></section>
<section class="card kpi-grid" data-ff-id="sec-kpi"><div class="kpi"><div style="font-size:10px;color:#64748B">Hạn mức Giải ngân</div><div style="font-size:13.5px;font-weight:600">${escapeHtmlAttr(params.formattedAmount)}</div></div><div class="kpi"><div style="font-size:10px;color:#64748B">Trạng thái Bảo mật</div><div style="font-size:13.5px;font-weight:600;color:#16A34A">${escapeHtmlAttr(statusVal)}</div></div><div class="kpi"><div style="font-size:10px;color:#64748B">Độ trễ Đồng bộ</div><div style="font-size:13.5px;font-weight:600">${escapeHtmlAttr(latencyVal)}</div></div></section>
<section class="card" data-ff-id="sec-biometric" style="display:flex;align-items:center;justify-content:space-between;gap:12px"><div><div style="font-size:12.5px;font-weight:600">${escapeHtmlAttr(params.activeScreenData.actionTitle)}</div><div style="font-size:11px;color:#475467;margin-top:2px">Nếu xác thực thất bại sẽ tự động rẽ nhánh sang SCR-02B (Fallback OTP).</div></div><button type="button" class="cta">${escapeHtmlAttr(params.activeScreenData.ctaLabel)}</button></section>
</body></html>`;
}
