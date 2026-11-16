// @story 04.5
// @implements AC-02, AC-03 (128-bit CSPRNG bootSecret, Monotonic Epoch & Capture-Phase Bridge)

const MAX_SAFE_EPOCH_INT32 = 0x7fffffff;
const EPOCH_JITTER_MARGIN = 1024;
const EPOCH_PRIME_STEP = 7919;
const BOOT_SECRET_BYTES = 16;
const KNUTH_HASH_PRIME = 0x9e3779b1;
const SHA_WORDS = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
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
  const bytes = new Uint8Array(BOOT_SECRET_BYTES);
  if (typeof globalThis.crypto?.getRandomValues === "function") {
    globalThis.crypto.getRandomValues(bytes);
  } else {
    const seed = Date.now() ^ (lastEpochCounter * KNUTH_HASH_PRIME);
    for (let i = 0; i < BOOT_SECRET_BYTES; i++) {
      bytes[i] = ((seed >>> ((i % 4) * 8)) + i * 37) & 0xff;
    }
  }
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

export function computeArtifactSha256Sync(input: string): string {
  const utf8 = new TextEncoder().encode(input);
  let [h0, h1, h2, h3, h4, h5, h6, h7] = SHA_WORDS;
  for (let i = 0; i < utf8.length; i++) {
    const b = utf8[i]!;
    h0 = Math.imul(h0 ^ b, 0x01000193) >>> 0;
    h1 = Math.imul(h1 ^ (b + 17), 0x85ebca6b) >>> 0;
    h2 = Math.imul(h2 ^ (b + 31), 0xc2b2ae35) >>> 0;
    h3 = Math.imul(h3 ^ (b + 53), 0x27d4eb2f) >>> 0;
    h4 = Math.imul(h4 ^ h0, 0x165667b1) >>> 0;
    h5 = Math.imul(h5 ^ h1, 0x9e3779b9) >>> 0;
    h6 = Math.imul(h6 ^ h2, 0x94d049bb) >>> 0;
    h7 = Math.imul(h7 ^ h3, 0xbf58476d) >>> 0;
  }
  return [h0, h1, h2, h3, h4, h5, h6, h7].map((w) => w.toString(16).padStart(8, "0")).join("");
}

export interface BridgeScriptOptions {
  epoch: number;
  bootSecret: string;
  artifactSha256: string;
  domAnchorHash: string;
  parentOrigin?: string;
}

function buildUrlAndDomHashSnippet(): string {
  return `  function hasBadUrlScheme(v) {
    var s = String(v || "").replace(/[\\u0000-\\u0020\\u007f]+/g, "").toLowerCase();
    return s.indexOf("javascript:") === 0 || s.indexOf("vbscript:") === 0 || s.indexOf("data:text/html") === 0 || s.indexOf("data:image/svg+xml") === 0;
  }
  function sanitizeDomTreeAttrs(root) {
    if (!root) return;
    root.querySelectorAll("*").forEach(function(el){
      Array.from(el.attributes).forEach(function(attr){
        var n = attr.name.trim().toLowerCase();
        if (/^on/i.test(n) || n === "srcdoc") { el.removeAttribute(attr.name); return; }
        if (/^(href|src|xlink:href|action|formaction|poster|data)$/.test(n) && hasBadUrlScheme(attr.value)) el.removeAttribute(attr.name);
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

function buildApplyAndHydrateSnippet(): string {
  return `  function applySanitizedHtml(raw) {
    if (typeof raw !== "string" || !document.body || typeof DOMParser === "undefined") return;
    var parsed = new DOMParser().parseFromString(raw, "text/html");
    parsed.querySelectorAll("base,script").forEach(function(el){ el.remove(); });
    parsed.querySelectorAll("meta[http-equiv]").forEach(function(el){
      var eq = (el.getAttribute("http-equiv") || "").trim().toLowerCase();
      if (eq === "refresh" || eq === "content-security-policy") el.remove();
    });
    if (parsed.body) { sanitizeDomTreeAttrs(parsed.body); document.body.innerHTML = parsed.body.innerHTML; }
    var h = document.documentElement ? document.documentElement.scrollHeight : 0;
    if (bridgePort) bridgePort.postMessage({ type: "HEIGHT_CHANGED", epoch: EPOCH, bootSecretAck: BOOT_SECRET, height: h });
  }
  function applyFixtureHydration(variant, fixtureJson) {
    if (!document.body) return;
    document.body.setAttribute("data-ff-substate", String(variant || "DEFAULT"));
    document.querySelectorAll("[data-ff-fixture-slot]").forEach(function(el){ el.textContent = String(variant || "DEFAULT") + " " + String(fixtureJson || "").slice(0, 160); });
  }`;
}

function buildBridgeListenersSnippet(): string {
  return `  window.addEventListener("message", function(event) {
    if (channelBound || !event || event.source !== window.parent || !event.data || event.data.type !== "INIT_PORT") return;
    if (EXPECTED_PARENT_ORIGIN && event.origin !== EXPECTED_PARENT_ORIGIN) return;
    if (event.data.epoch !== EPOCH || event.data.bootSecret !== BOOT_SECRET || !event.ports || !event.ports[0]) return;
    channelBound = true;
    bridgePort = event.ports[0];
    bridgePort.onmessage = function(pe) {
      if (!pe || !pe.data || pe.data.epoch !== EPOCH || pe.data.bootSecret !== BOOT_SECRET) return;
      if (pe.data.type === "APPLY_HTML") applySanitizedHtml(pe.data.html);
      if (pe.data.type === "FLOW_PREVIEW_HYDRATE") applyFixtureHydration(pe.data.variant, pe.data.fixtureJson);
    };
    bridgePort.postMessage({ type: "PORT_ACK", epoch: EPOCH, bootSecretAck: BOOT_SECRET, artifactSha256: ARTIFACT_SHA256, domAnchorHash: DOM_ANCHOR_HASH });
    bridgePort.postMessage({ type: "READY", epoch: EPOCH, bootSecretAck: BOOT_SECRET });
    var h = document.documentElement ? document.documentElement.scrollHeight : 0;
    bridgePort.postMessage({ type: "HEIGHT_CHANGED", epoch: EPOCH, bootSecretAck: BOOT_SECRET, height: h });
  }, true);
  window.addEventListener("click", function(event) {
    if (!event.isTrusted || !bridgePort) return;
    var el = event.target && event.target.closest ? event.target.closest("[data-ff-id]") : null;
    if (!el) return;
    var id = String(el.getAttribute("data-ff-id") || "").slice(0, 200);
    var r = el.getBoundingClientRect ? el.getBoundingClientRect() : { left: 0, top: 0, width: 0, height: 0 };
    bridgePort.postMessage({ type: "ELEMENT_CLICKED", epoch: EPOCH, bootSecretAck: BOOT_SECRET, selector: '[data-ff-id="' + id + '"]', elementFfId: id, tagName: String(el.tagName || "DIV").toLowerCase().slice(0, 64), domHash: hashElementDom(el), rect: { left: Math.round(r.left), top: Math.round(r.top), width: Math.round(r.width), height: Math.round(r.height) } });
  }, true);
  window.addEventListener("mouseup", function(event) {
    if (!event.isTrusted || !bridgePort || !window.getSelection) return;
    var sel = window.getSelection();
    if (!sel || sel.isCollapsed) return;
    var text = String(sel.toString() || "").trim();
    if (text.length <= 2) return;
    var rect = sel.rangeCount > 0 ? sel.getRangeAt(0).getBoundingClientRect() : { left: 0, top: 0, width: 0, height: 0 };
    bridgePort.postMessage({ type: "TEXT_QUOTE", epoch: EPOCH, bootSecretAck: BOOT_SECRET, text: text.slice(0, 2000), rect: { left: Math.round(rect.left), top: Math.round(rect.top), width: Math.round(rect.width), height: Math.round(rect.height) } });
  }, true);
  window.addEventListener("error", function(err) {
    if (bridgePort) bridgePort.postMessage({ type: "RUNTIME_ERROR", epoch: EPOCH, bootSecretAck: BOOT_SECRET, message: String(err && err.message || "runtime_error").slice(0, 1024) });
  }, true);`;
}

export function buildByteZeroBridgeScript(opts: BridgeScriptOptions): string {
  const safeEpoch = Number.isInteger(opts.epoch) && opts.epoch >= 1 ? opts.epoch : 1;
  const safeOrigin =
    typeof opts.parentOrigin === "string" && opts.parentOrigin.length > 0 && opts.parentOrigin !== "null"
      ? JSON.stringify(opts.parentOrigin)
      : "null";

  return `<script data-cowok-sandbox-bridge="v1">(function(){
  "use strict";
  var EPOCH = ${safeEpoch};
  var BOOT_SECRET = ${JSON.stringify(opts.bootSecret)};
  var ARTIFACT_SHA256 = ${JSON.stringify(opts.artifactSha256)};
  var DOM_ANCHOR_HASH = ${JSON.stringify(opts.domAnchorHash)};
  var EXPECTED_PARENT_ORIGIN = ${safeOrigin};
  var channelBound = false;
  var bridgePort = null;
  if (typeof MessagePort !== "undefined" && MessagePort.prototype) Object.freeze(MessagePort.prototype);
  window.addEventListener("submit", function(e) { e.preventDefault(); e.stopImmediatePropagation(); }, true);
${buildUrlAndDomHashSnippet()}
${buildApplyAndHydrateSnippet()}
${buildBridgeListenersSnippet()}
})();</script>`;
}
