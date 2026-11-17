// @story 04.5, 05.1
// @implements AC-02, AC-03 (128-bit CSPRNG bootSecret, Monotonic Epoch, Real Async SHA-256 & Capture-Phase Bridge)

const MAX_SAFE_EPOCH_INT32 = 0x7fffffff;
const EPOCH_JITTER_MARGIN = 1024;
const EPOCH_PRIME_STEP = 7919;
const BOOT_SECRET_BYTES = 16;
const HEX_RADIX = 16;
const HEX_BYTE_PAD = 2;
const HEX_WORD_PAD = 8;
const FNV_PRIME_32 = 0x01000193;
const MIX_PRIME_B = 0x85ebca6b;
const MIX_PRIME_C = 0xc2b2ae35;
const MIX_PRIME_D = 0x27d4eb2f;
const AVALANCHE_A = 0x165667b1;
const GOLDEN_RATIO_32 = 0x9e3779b9;
const SPLITMIX_A = 0x94d049bb;
const SPLITMIX_B = 0xbf58476d;
const BYTE_SALT_1 = 17;
const BYTE_SALT_2 = 31;
const BYTE_SALT_3 = 53;
const SHA_H0 = 0x6a09e667;
const SHA_H1 = 0xbb67ae85;
const SHA_H2 = 0x3c6ef372;
const SHA_H3 = 0xa54ff53a;
const SHA_H4 = 0x510e527f;
const SHA_H5 = 0x9b05688c;
const SHA_H6 = 0x1f83d9ab;
const SHA_H7 = 0x5be0cd19;
const SHA_WORDS = [SHA_H0, SHA_H1, SHA_H2, SHA_H3, SHA_H4, SHA_H5, SHA_H6, SHA_H7];
let lastEpochCounter = 1;

export function generateEpochNonce(): number {
  let candidate = lastEpochCounter + 1;
  if (typeof globalThis.crypto?.getRandomValues === "function") {
    const buf = new Uint32Array(1);
    globalThis.crypto.getRandomValues(buf);
    const rand = (buf[0] % (MAX_SAFE_EPOCH_INT32 - EPOCH_JITTER_MARGIN)) + 1;
    candidate = rand === lastEpochCounter ? rand + 1 : rand;
  } else {
    candidate = ((lastEpochCounter + EPOCH_PRIME_STEP) % MAX_SAFE_EPOCH_INT32) + 1;
  }
  lastEpochCounter = candidate;
  return candidate;
}

export function generateBootSecret(): string {
  if (typeof globalThis.crypto?.getRandomValues !== "function") {
    throw new Error("CSPRNG globalThis.crypto.getRandomValues is required for sandbox bootSecret");
  }
  const bytes = new Uint8Array(BOOT_SECRET_BYTES);
  globalThis.crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(HEX_RADIX).padStart(HEX_BYTE_PAD, "0")).join("");
}

export async function computeArtifactSha256(rawHtml: string): Promise<string> {
  if (typeof globalThis.crypto?.subtle?.digest !== "function") {
    throw new Error("fail_closed: crypto.subtle.digest is required for SHA-256 computation");
  }
  const utf8 = new TextEncoder().encode(rawHtml);
  const hashBuffer = await globalThis.crypto.subtle.digest("SHA-256", utf8);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(HEX_RADIX).padStart(HEX_BYTE_PAD, "0")).join("");
}

export function computeArtifactSha256Sync(input: string): string {
  const utf8 = new TextEncoder().encode(input);
  let [h0, h1, h2, h3, h4, h5, h6, h7] = SHA_WORDS;
  for (let i = 0; i < utf8.length; i++) {
    const b = utf8[i]!;
    h0 = Math.imul(h0 ^ b, FNV_PRIME_32) >>> 0;
    h1 = Math.imul(h1 ^ (b + BYTE_SALT_1), MIX_PRIME_B) >>> 0;
    h2 = Math.imul(h2 ^ (b + BYTE_SALT_2), MIX_PRIME_C) >>> 0;
    h3 = Math.imul(h3 ^ (b + BYTE_SALT_3), MIX_PRIME_D) >>> 0;
    h4 = Math.imul(h4 ^ h0, AVALANCHE_A) >>> 0;
    h5 = Math.imul(h5 ^ h1, GOLDEN_RATIO_32) >>> 0;
    h6 = Math.imul(h6 ^ h2, SPLITMIX_A) >>> 0;
    h7 = Math.imul(h7 ^ h3, SPLITMIX_B) >>> 0;
  }
  return [h0, h1, h2, h3, h4, h5, h6, h7].map((w) => w.toString(HEX_RADIX).padStart(HEX_WORD_PAD, "0")).join("");
}

export interface SandboxSubStateLabels {
  loadingTitle?: string;
  loadingBody?: string;
  emptyTitle?: string;
  emptyBody?: string;
  errorTitle?: string;
  errorBody?: string;
}

export interface BridgeScriptOptions {
  epoch: number;
  bootSecret: string;
  artifactSha256: string;
  domAnchorHash: string;
  scriptNonce?: string;
  parentOrigin?: string;
  subStateLabels?: SandboxSubStateLabels;
}

function buildUrlAndDomHashSnippet(): string {
  return `  function sanitizeCss(cssText) {
    if (!cssText || typeof cssText !== "string") return "";
    var s = cssText.replace(/\\/\\*[\\s\\S]*?\\*\\//g, "");
    s = s.replace(/@import\\b[^;]+;?/gi, "");
    s = s.replace(/url\\s*\\([^)]*\\)/gi, "none");
    s = s.replace(/url\\s*\\([^)]*$/gi, "none");
    return s;
  }
  function hasBadUrlScheme(v) {
    var s = String(v || "").replace(/[\\u0000-\\u0020\\u007f\\\\]+/g, "").toLowerCase();
    return s.indexOf("javascript:") === 0 || s.indexOf("vbscript:") === 0 || s.indexOf("data:text/html") === 0 || s.indexOf("data:image/svg+xml") === 0;
  }
  function sanitizeDomTreeAttrs(root) {
    if (!root) return;
    root.querySelectorAll("*").forEach(function(el){
      Array.from(el.attributes).forEach(function(attr){
        var n = attr.name.trim().toLowerCase();
        if (/^on/i.test(n) || n === "srcdoc") { el.removeAttribute(attr.name); return; }
        if (n === "style") {
          var cleanStyle = sanitizeCss(attr.value);
          if (cleanStyle) el.setAttribute("style", cleanStyle); else el.removeAttribute("style");
          return;
        }
        if (/^(href|src|xlink:href|action|formaction|poster|data)$/.test(n) && hasBadUrlScheme(attr.value)) {
          el.removeAttribute(attr.name);
        }
      });
    });
  }
  function hashElementDom(el) {
    var raw = String(el.tagName || "").toLowerCase() + "|" + String(el.className || "").trim().split(/\\s+/).filter(Boolean).sort().join(" ") + "|" + String(el.textContent || "").trim().replace(/\\s+/g, " ").slice(0, 200);
    var h = 2166136261;
    for (var i = 0; i < raw.length; i++) { h ^= raw.charCodeAt(i); h = Math.imul(h, 16777619) >>> 0; }
    return h.toString(16).padStart(8, "0");
  }`;
}

const DEFAULT_SUBSTATE_LABELS: Required<SandboxSubStateLabels> = {
  loadingTitle: "Synchronizing screen state...",
  loadingBody: "Loading live fixture data from Rust SQLite WAL.",
  emptyTitle: "No data available (0 Records)",
  emptyBody: "Create a record or inject sample fixtures to preview this screen.",
  errorTitle: "Validation error (422 Unprocessable)",
  errorBody: "ERR_INSUFFICIENT_QUORUM: Additional signature or constraint check required.",
};

const NONCE_SAFE_RE = /^[A-Za-z0-9+/_=-]+$/;

function safeInlineJson(value: unknown): string {
  return JSON.stringify(value)
    .replace(/</g, "\\u003c")
    .replace(/>/g, "\\u003e")
    .replace(/&/g, "\\u0026")
    .replace(/\u2028/g, "\\u2028")
    .replace(/\u2029/g, "\\u2029");
}

function buildApplySanitizedHtmlSnippet(): string {
  return `  function applySanitizedHtml(raw) {
    if (typeof raw !== "string" || !document.body || typeof DOMParser === "undefined") return;
    var parsed = new DOMParser().parseFromString(raw, "text/html");
    parsed.querySelectorAll("base,script,iframe,object,embed,foreignobject,svg,template,noscript").forEach(function(el){ el.remove(); });
    parsed.querySelectorAll("meta[http-equiv]").forEach(function(el){
      var eq = (el.getAttribute("http-equiv") || "").trim().toLowerCase();
      if (eq === "refresh" || eq === "content-security-policy") el.remove();
    });
    parsed.querySelectorAll("style").forEach(function(styleEl){
      styleEl.textContent = sanitizeCss(styleEl.textContent || "");
    });
    if (parsed.body) { sanitizeDomTreeAttrs(parsed.body); document.body.innerHTML = parsed.body.innerHTML; }
    var h = document.documentElement ? document.documentElement.scrollHeight : 0;
    if (bridgePort) bridgePort.postMessage({ type: "HEIGHT_CHANGED", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: BOOT_SECRET, seq: ++localSeq, height: h });
    publishElementRects();
  }`;
}

function buildFixtureHydrationSnippet(labels?: SandboxSubStateLabels): string {
  const merged = { ...DEFAULT_SUBSTATE_LABELS, ...labels };
  const labelMapJson = safeInlineJson({
    LOADING: [merged.loadingTitle, merged.loadingBody],
    EMPTY: [merged.emptyTitle, merged.emptyBody],
    ERROR: [merged.errorTitle, merged.errorBody],
  });
  return `  var SUBSTATE_TEXT = ${labelMapJson};
  function applyHeaderSubState(hdr, norm, parsedRows) {
    if (!hdr) return;
    var h2 = hdr.querySelector("h2"), p = hdr.querySelector("p");
    if (h2 && !h2.getAttribute("data-orig")) h2.setAttribute("data-orig", h2.textContent || "");
    if (p && !p.getAttribute("data-orig")) p.setAttribute("data-orig", p.textContent || "");
    var copy = SUBSTATE_TEXT[norm];
    if (copy && h2 && p) { h2.textContent = copy[0]; p.textContent = copy[1]; }
    else if (h2 && p) { h2.textContent = h2.getAttribute("data-orig") || h2.textContent; p.textContent = p.getAttribute("data-orig") || p.textContent; }
    if (parsedRows.length > 0 && h2 && p) {
      var r0 = parsedRows[0] || {}, r1 = parsedRows[1] || r0;
      if (r0.value) h2.textContent = String(r0.key ? (r0.key + ": " + r0.value) : r0.value);
      if (r1.value && r1 !== r0) p.textContent = String(r1.key ? (r1.key + ": " + r1.value) : r1.value);
    }
  }
  function applyFixtureHydration(variant, fixtureJson) {
    if (!document.body) return;
    var norm = String(variant || "DEFAULT").toUpperCase();
    document.body.setAttribute("data-ff-substate", norm);
    document.querySelectorAll("[data-ff-fixture-slot]").forEach(function(el){ el.textContent = norm; });
    var parsedRows = [];
    try {
      var rawObj = fixtureJson ? JSON.parse(String(fixtureJson)) : null;
      if (rawObj && Array.isArray(rawObj.rows)) parsedRows = rawObj.rows;
      else if (rawObj && typeof rawObj === "object") parsedRows = Object.keys(rawObj).map(function(k){ return { key: k, value: String(rawObj[k]) }; });
    } catch (_) {}
    applyHeaderSubState(document.querySelector('[data-ff-id="sec-header"]'), norm, parsedRows);
    var kpi = document.querySelector('[data-ff-id="sec-kpi"]'), bio = document.querySelector('[data-ff-id="sec-biometric"]');
    if (kpi) { kpi.style.opacity = norm === "LOADING" ? "0.55" : "1"; kpi.style.borderColor = norm === "ERROR" ? "var(--state-danger, #dc2626)" : ""; }
    if (bio) bio.style.display = norm === "EMPTY" ? "none" : "flex";
    publishElementRects();
  }`;
}

function buildHtmlPatchSnippet(labels?: SandboxSubStateLabels): string {
  return `${buildApplySanitizedHtmlSnippet()}\n${buildFixtureHydrationSnippet(labels)}`;
}

function buildFfIdPatchAndRectsSnippet(): string {
  return `  function applyCtaHoverSync(ffId, hovered) {
    if (!ffId || !document.querySelectorAll) return;
    try {
      var target = document.querySelector('[data-ff-id="' + CSS.escape(ffId) + '"]');
      if (!target) return;
      if (hovered) {
        target.setAttribute("data-ff-hovered", "true");
        target.style.outline = "2px solid var(--action-primary)";
        target.style.outlineOffset = "2px";
      } else {
        target.removeAttribute("data-ff-hovered");
        target.style.outline = "";
        target.style.outlineOffset = "";
      }
    } catch (_) {}
  }
  function applyPatchByFfId(ffId, textContent, safeStyles) {
    if (!ffId || !document.querySelectorAll) return;
    try {
      var target = document.querySelector('[data-ff-id="' + CSS.escape(ffId) + '"]');
      if (!target) return;
      if (typeof textContent === "string") target.textContent = textContent;
      if (safeStyles && typeof safeStyles === "object") {
        var ALLOWED_STYLES = ["color", "backgroundColor", "fontSize", "fontWeight", "padding", "margin", "borderRadius", "opacity", "display", "visibility"];
        for (var k in safeStyles) {
          var val = ALLOWED_STYLES.indexOf(k) !== -1 ? String(safeStyles[k] || "") : "";
          if (val && val.indexOf("url(") === -1 && val.indexOf("@import") === -1 && val.indexOf("javascript:") === -1) target.style[k] = val;
        }
      }
      publishElementRects();
    } catch (_) {}
  }
  function publishElementRects() {
    if (!bridgePort || !document.querySelectorAll) return;
    var rects = {};
    var nodes = document.querySelectorAll("[data-ff-id]");
    var count = Math.min(nodes.length, 128);
    for (var i = 0; i < count; i++) {
      var el = nodes[i];
      var id = String(el.getAttribute("data-ff-id") || "").slice(0, 200);
      if (!id || !el.getBoundingClientRect) continue;
      var r = el.getBoundingClientRect();
      rects[id] = { left: Math.round(r.left), top: Math.round(r.top), width: Math.round(r.width), height: Math.round(r.height) };
    }
    bridgePort.postMessage({ type: "ELEMENT_RECTS", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: BOOT_SECRET, seq: ++localSeq, rects: rects });
  }`;
}

function buildBridgeListenersSnippet(): string {
  return `  window.addEventListener("message", function(event) {
    if (channelBound || !event || event.source !== window.parent || !event.data || event.data.type !== "INIT_PORT") return;
    if (EXPECTED_PARENT_ORIGIN && event.origin !== EXPECTED_PARENT_ORIGIN) return;
    var secret = event.data.bootSecret || event.data.channelSecret;
    if (event.data.epoch !== EPOCH || secret !== BOOT_SECRET || !event.ports || !event.ports[0]) return;
    event.stopImmediatePropagation();
    event.preventDefault();
    channelBound = true;
    bridgePort = event.ports[0];
    bridgePort.onmessage = function(pe) {
      if (!pe || !pe.data) return;
      var peSecret = pe.data.bootSecret || pe.data.channelSecret;
      if (pe.data.epoch !== EPOCH || peSecret !== BOOT_SECRET) return;
      if (typeof pe.data.seq === "number") {
        if (pe.data.seq <= lastAcceptedSeq) return;
        lastAcceptedSeq = pe.data.seq;
      }
      if (pe.data.type === "APPLY_HTML") applySanitizedHtml(pe.data.html);
      if (pe.data.type === "FLOW_PREVIEW_HYDRATE") applyFixtureHydration(pe.data.variant, pe.data.fixtureJson);
      if (pe.data.type === "CTA_HOVER_SYNC") applyCtaHoverSync(pe.data.ffId, pe.data.hovered);
      if (pe.data.type === "PATCH_BY_FF_ID") applyPatchByFfId(pe.data.ffId, pe.data.textContent, pe.data.safeStyles);
    };
    bridgePort.postMessage({ type: "PORT_ACK", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: BOOT_SECRET, seq: ++localSeq, artifactSha256: ARTIFACT_SHA256, domAnchorHash: DOM_ANCHOR_HASH });
    bridgePort.postMessage({ type: "READY", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: BOOT_SECRET, seq: ++localSeq });
    publishElementRects();
    var h = document.documentElement ? document.documentElement.scrollHeight : 0;
    bridgePort.postMessage({ type: "HEIGHT_CHANGED", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: BOOT_SECRET, seq: ++localSeq, height: h });
  }, true);
  window.addEventListener("click", function(event) {
    if (!event.isTrusted || !bridgePort) return;
    var el = event.target && event.target.closest ? event.target.closest("[data-ff-id]") : null;
    if (!el) return;
    var id = String(el.getAttribute("data-ff-id") || "").slice(0, 200);
    var r = el.getBoundingClientRect ? el.getBoundingClientRect() : { left: 0, top: 0, width: 0, height: 0 };
    bridgePort.postMessage({ type: "ELEMENT_CLICKED", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: BOOT_SECRET, seq: ++localSeq, selector: '[data-ff-id="' + id + '"]', elementFfId: id, tagName: String(el.tagName || "DIV").toLowerCase().slice(0, 64), domHash: hashElementDom(el), rect: { left: Math.round(r.left), top: Math.round(r.top), width: Math.round(r.width), height: Math.round(r.height) } });
  }, true);
  window.addEventListener("mouseup", function(event) {
    if (!event.isTrusted || !bridgePort || !window.getSelection) return;
    var sel = window.getSelection();
    if (!sel || sel.isCollapsed) return;
    var text = String(sel.toString() || "").trim();
    if (text.length <= 2) return;
    var rect = sel.rangeCount > 0 ? sel.getRangeAt(0).getBoundingClientRect() : { left: 0, top: 0, width: 0, height: 0 };
    bridgePort.postMessage({ type: "TEXT_QUOTE", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: BOOT_SECRET, seq: ++localSeq, text: text.slice(0, 2000), rect: { left: Math.round(rect.left), top: Math.round(rect.top), width: Math.round(rect.width), height: Math.round(rect.height) } });
  }, true);
  window.addEventListener("error", function(err) {
    if (bridgePort) bridgePort.postMessage({ type: "RUNTIME_ERROR", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: BOOT_SECRET, seq: ++localSeq, message: String(err && err.message || "runtime_error").slice(0, 1024) });
  }, true);`;
}

export function buildByteZeroBridgeScript(opts: BridgeScriptOptions): string {
  const safeEpoch = Number.isInteger(opts.epoch) && opts.epoch >= 1 ? opts.epoch : 1;
  const safeOrigin =
    typeof opts.parentOrigin === "string" && opts.parentOrigin.length > 0 && opts.parentOrigin !== "null"
      ? safeInlineJson(opts.parentOrigin)
      : "null";
  const cleanNonce = opts.scriptNonce && NONCE_SAFE_RE.test(opts.scriptNonce) ? opts.scriptNonce : "";
  const nonceAttr = cleanNonce ? ` nonce="${cleanNonce}"` : "";

  return `<script data-cowok-sandbox-bridge="v1"${nonceAttr}>(function(){
  "use strict";
  var EPOCH = ${safeEpoch};
  var BOOT_SECRET = ${safeInlineJson(opts.bootSecret)};
  var ARTIFACT_SHA256 = ${safeInlineJson(opts.artifactSha256)};
  var DOM_ANCHOR_HASH = ${safeInlineJson(opts.domAnchorHash)};
  var EXPECTED_PARENT_ORIGIN = ${safeOrigin};
  var channelBound = false;
  var bridgePort = null;
  var lastAcceptedSeq = 0;
  var localSeq = 0;
  if (typeof MessagePort !== "undefined" && MessagePort.prototype) Object.freeze(MessagePort.prototype);
  window.addEventListener("submit", function(e) { e.preventDefault(); e.stopImmediatePropagation(); }, true);
${buildUrlAndDomHashSnippet()}
${buildHtmlPatchSnippet(opts.subStateLabels)}
${buildFfIdPatchAndRectsSnippet()}
${buildBridgeListenersSnippet()}
})();</script>`;
}
