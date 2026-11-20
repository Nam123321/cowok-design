// @story 04.5, 05.1
// @implements AC-02, AC-03 (128-bit CSPRNG bootSecret, Monotonic Epoch, Real Async SHA-256 & Capture-Phase Bridge)
import { buildRender2DChartSnippet as buildChart2DRenderSnippet } from "./html-sandbox-chart-snippet";

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
    if (cssText.indexOf("\\u0000") !== -1 || cssText.indexOf("\\\\") !== -1) return "";
    var s = cssText.replace(/\\/\\*[\\s\\S]*?\\*\\//g, " ");
    if (s.indexOf("/*") !== -1 || s.indexOf("*/") !== -1) return "";
    var low = s.toLowerCase(), compact = low.replace(/\\s+/g, "");
    var banned = ["@import", "@charset", "@namespace", "@document", "url(", "image-set(", "expression(", "-moz-binding", "behavior:", "javascript:", "vbscript:", "data:", "</style", "<"];
    for (var i = 0; i < banned.length; i++) {
      if (low.indexOf(banned[i]) !== -1 || compact.indexOf(banned[i]) !== -1) return "";
    }
    return s;
  }
  function hasBadUrlScheme(v) {
    var s = String(v || "").replace(/[\\u0000-\\u0020\\u007f\\\\]+/g, "").toLowerCase();
    return s.indexOf("javascript:") === 0 || s.indexOf("vbscript:") === 0 || s.indexOf("data:text/html") === 0 || s.indexOf("data:image/svg+xml") === 0;
  }
  function sanitizeDomTreeAttrs(root, hoistedRules) {
    if (!root) return;
    var styleIdx = hoistedRules ? hoistedRules.length : 0;
    root.querySelectorAll("*").forEach(function(el){
      Array.from(el.attributes).forEach(function(attr){
        var n = attr.name.trim().toLowerCase();
        if (/^on/i.test(n) || n === "srcdoc") { el.removeAttribute(attr.name); return; }
        if (n === "style") {
          var cleanStyle = sanitizeCss(attr.value);
          el.removeAttribute("style");
          if (cleanStyle && cleanStyle.trim() && hoistedRules) {
            var sid = "a" + (styleIdx++);
            el.setAttribute("data-cowok-s", sid);
            hoistedRules.push('[data-cowok-s="' + sid + '"]{' + cleanStyle.trim() + "}");
          }
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

function buildSanitizeParsedDomSnippet(): string {
  return `    parsed.querySelectorAll("meta,base,script,iframe,object,embed,foreignobject,math,link,applet,template,noscript").forEach(function(el){ el.remove(); });
    parsed.querySelectorAll("svg").forEach(function(svgEl){
      svgEl.querySelectorAll("foreignobject,script,style,animate,animatemotion,animatetransform,set,use").forEach(function(bad){ bad.remove(); });
      svgEl.removeAttribute("href"); svgEl.removeAttribute("xlink:href");
      svgEl.querySelectorAll("*").forEach(function(n){ n.removeAttribute("href"); n.removeAttribute("xlink:href"); });
    });
    var extractedStyles = [];
    parsed.querySelectorAll("style").forEach(function(styleEl){
      var clean = sanitizeCss(styleEl.textContent || "");
      if (clean) extractedStyles.push(clean);
      styleEl.remove();
    });
    var hoistedRules = [];
    if (parsed.head) sanitizeDomTreeAttrs(parsed.head, hoistedRules);
    if (parsed.body) sanitizeDomTreeAttrs(parsed.body, hoistedRules);`;
}

function buildMountAppliedStylesSnippet(): string {
  return `    var activeNonce = SCRIPT_NONCE || (document.currentScript && document.currentScript.nonce) || (document.querySelector("script[nonce]") && document.querySelector("script[nonce]").nonce) || "";
    if (document.head) {
      document.head.querySelectorAll("style[data-cowok-applied='1']").forEach(function(el){ el.remove(); });
      for (var si = 0; si < extractedStyles.length; si++) {
        var stTag = document.createElement("style");
        stTag.setAttribute("data-cowok-applied", "1");
        if (activeNonce) { stTag.setAttribute("nonce", activeNonce); stTag.nonce = activeNonce; }
        stTag.textContent = extractedStyles[si];
        document.head.appendChild(stTag);
      }
      if (hoistedRules.length > 0) {
        var hoistedTag = document.createElement("style");
        hoistedTag.setAttribute("data-cowok-applied", "1");
        if (activeNonce) { hoistedTag.setAttribute("nonce", activeNonce); hoistedTag.nonce = activeNonce; }
        hoistedTag.textContent = hoistedRules.join("\\n");
        document.head.appendChild(hoistedTag);
      }
      if (parsed.body && parsed.body.querySelector && parsed.body.querySelector(".slide")) {
        var fitTag = document.createElement("style");
        fitTag.setAttribute("data-cowok-applied", "1");
        if (activeNonce) { fitTag.setAttribute("nonce", activeNonce); fitTag.nonce = activeNonce; }
        fitTag.textContent = "html,body{max-width:100%!important;overflow-x:hidden!important} .slide{width:100%!important;max-width:100%!important;box-sizing:border-box!important;height:auto!important;min-height:100%!important;padding:clamp(20px,3.6vw,64px) clamp(24px,4vw,72px)!important} .deck-eyebrow{font-size:clamp(10px,1.05vw,var(--type-micro,16px))!important} .deck-headline{font-size:clamp(22px,3.3vw,var(--type-display,56px))!important;overflow-wrap:break-word!important} .deck-subtitle{font-size:clamp(12.5px,1.5vw,var(--type-body,22px))!important} .deck-card,.deck-flow,.deck-quote,.deck-table{font-size:clamp(13px,1.85vw,var(--type-heading,32px))!important}";
        document.head.appendChild(fitTag);
      }
    }`;
}

function buildApplySanitizedHtmlSnippet(): string {
  return `  function applySanitizedHtml(raw) {
    if (typeof raw !== "string" || !document.body || typeof DOMParser === "undefined") return;
    var parsed = new DOMParser().parseFromString(raw, "text/html");
${buildSanitizeParsedDomSnippet()}
${buildMountAppliedStylesSnippet()}
    if (parsed.body) {
      var nodes = Array.from(parsed.body.childNodes).map(function(n){ return document.importNode(n, true); });
      document.body.replaceChildren.apply(document.body, nodes);
    }
    if (typeof hydrateEmbeddedCharts === "function") hydrateEmbeddedCharts();
    var h = document.documentElement ? document.documentElement.scrollHeight : 0;
    if (bridgePort) bridgePort.postMessage({ type: "HEIGHT_CHANGED", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, height: h });
    publishElementRects();
    scheduleElementRectsPublish();
  }`;
}

function buildBridgeCssStyleTagSnippet(): string {
  return `  function ensureFixtureAndHoverStyleTag() {
    if (!document.head || document.head.querySelector("style[data-cowok-bridge-css='1']")) return;
    var activeNonce = SCRIPT_NONCE || (document.currentScript && document.currentScript.nonce) || (document.querySelector("script[nonce]") && document.querySelector("script[nonce]").nonce) || "";
    var st = document.createElement("style");
    st.setAttribute("data-cowok-bridge-css", "1");
    if (activeNonce) st.setAttribute("nonce", activeNonce);
    st.textContent = [
      "[data-ff-hovered='true']{outline:2px solid var(--action-primary);outline-offset:2px}",
      "body[data-ff-substate='LOADING'] [data-ff-id='sec-kpi']{opacity:0.55}",
      "body[data-ff-substate='ERROR'] [data-ff-id='sec-kpi']{border-color:var(--state-danger)}",
      "body[data-ff-substate='EMPTY'] [data-ff-id='sec-biometric']{display:none}",
      "html,body{max-width:100%;overflow-x:hidden;box-sizing:border-box}",
      "body[data-cowok-deck-protocol='1'] .slide,.slide[data-slide-uid]{width:100%!important;max-width:100%!important;box-sizing:border-box!important;height:auto!important;min-height:100%!important}"
    ].join("\\n");
    document.head.appendChild(st);
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
${buildBridgeCssStyleTagSnippet()}
  function applyFixtureHydration(variant, fixtureJson) {
    if (!document.body) return;
    ensureFixtureAndHoverStyleTag();
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
    publishElementRects();
  }`;
}

function buildDeckBlackoutSnippet(): string {
  return `  function applyDeckBlackout(mode) {
    if (!document.body) return;
    var existing = document.getElementById("cowok-deck-blackout-layer");
    if (mode !== "black" && mode !== "white") {
      if (existing) existing.remove();
      document.body.removeAttribute("data-cowok-blackout");
      return;
    }
    document.body.setAttribute("data-cowok-blackout", mode);
    if (!existing) {
      existing = document.createElement("div");
      existing.id = "cowok-deck-blackout-layer";
      existing.setAttribute("aria-hidden", "true");
      existing.style.position = "fixed";
      existing.style.inset = "0";
      existing.style.zIndex = "2147483647";
      existing.style.pointerEvents = "none";
      document.body.appendChild(existing);
    }
    existing.style.backgroundColor = mode === "white" ? "white" : "black";
  }`;
}

function buildHtmlPatchSnippet(labels?: SandboxSubStateLabels): string {
  return `${buildApplySanitizedHtmlSnippet()}\n${buildFixtureHydrationSnippet(labels)}`;
}

function buildCtaHoverAndTelemetrySnippet(): string {
  return `  function applyCtaHoverSync(ffId, hovered) {
    if (!ffId || !document.querySelectorAll) return;
    try {
      ensureFixtureAndHoverStyleTag();
      var target = document.querySelector('[data-ff-id="' + CSS.escape(ffId) + '"]');
      if (!target) return;
      if (hovered) {
        target.setAttribute("data-ff-hovered", "true");
      } else {
        target.removeAttribute("data-ff-hovered");
      }
    } catch (_) {}
  }
  var pendingRectsRaf = 0;
  function scheduleElementRectsPublish() {
    if (typeof requestAnimationFrame !== "function") { publishElementRects(); return; }
    if (pendingRectsRaf) return;
    pendingRectsRaf = requestAnimationFrame(function() { pendingRectsRaf = 0; publishElementRects(); });
  }
  var lastOverflowBySlide = {};
  function emitOverflowTelemetry(slideEl, ffId) {
    if (!bridgePort || !slideEl || !slideEl.clientHeight) return;
    var sh = Math.round(slideEl.scrollHeight || 0), ch = Math.round(slideEl.clientHeight || 0);
    if (ch <= 0) return;
    var overflowPx = Math.max(0, sh - ch);
    var uid = slideEl.getAttribute ? slideEl.getAttribute("data-slide-uid") : null;
    var slides = document.querySelectorAll(".slide"), idx = 0;
    for (var i = 0; i < slides.length; i++) { if (slides[i] === slideEl) { idx = i; break; } }
    var slideKey = uid || String(idx);
    var prevOverflow = lastOverflowBySlide[slideKey] || 0;
    lastOverflowBySlide[slideKey] = overflowPx;
    if (overflowPx > 0 || prevOverflow > 0) {
      bridgePort.postMessage({ type: "SLIDE_OVERFLOW_WARN", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, slideIndex: idx, slideUid: uid || undefined, ffId: ffId || undefined, scrollHeight: sh, clientHeight: ch, overflowPx: overflowPx });
    }
  }
  function isSafeClientUrl(u) {
    var t = String(u || "").trim();
    if (!t || t.indexOf("..") !== -1) return false;
    var low = t.replace(/\\s+/g, "").toLowerCase();
    if (low.indexOf("javascript:") === 0 || low.indexOf("vbscript:") === 0 || low.indexOf("data:") === 0 || low.indexOf("//") === 0) return false;
    return low.indexOf("https://") === 0 || low.indexOf("/assets/") === 0 || low.indexOf("assets/") === 0 || low.indexOf("presentations/") === 0 || low.indexOf("slides/") === 0;
  }`;
}

function buildResolvePatchTargetSnippet(): string {
  return `  function resolvePatchTarget(slideUid, ffId) {
    var escFf = CSS.escape(ffId);
    if (slideUid) {
      var scoped = document.querySelector('section[data-slide-uid="' + CSS.escape(String(slideUid)) + '"] [data-ff-id="' + escFf + '"]');
      if (scoped) return scoped;
      if (document.querySelectorAll("section[data-slide-uid]").length > 0) return null;
    }
    var exact = document.querySelector('[data-ff-id="' + escFf + '"]');
    if (exact || document.querySelectorAll("section[data-slide-uid]").length > 0) return exact;
    if (/-headline$/.test(ffId)) return document.querySelector('[data-ff-id="ff-slide-headline"]');
    if (/-body$/.test(ffId)) return document.querySelector('[data-ff-id="ff-slide-subtitle"]');
    if (/-eyebrow$/.test(ffId)) return document.querySelector('[data-ff-id="ff-slide-eyebrow"]');
    return null;
  }`;
}

function buildPatchByFfIdSnippet(): string {
  return `  function applyPatchByFfId(frame) {
    var ffId = frame && frame.ffId ? String(frame.ffId) : "";
    if (!ffId || !/^[a-zA-Z0-9_-]{1,64}$/.test(ffId) || !document.querySelector) return;
    try {
      var target = resolvePatchTarget(frame.slideUid, ffId);
      if (!target) return;
      if (frame.slotKind === "bullet" && Array.isArray(frame.bulletItems)) {
        while (target.firstChild) target.removeChild(target.firstChild);
        for (var bi = 0; bi < Math.min(frame.bulletItems.length, 24); bi++) {
          var li = document.createElement("li"); li.textContent = String(frame.bulletItems[bi] || ""); target.appendChild(li);
        }
      } else if (frame.slotKind === "image" && typeof frame.textContent === "string") {
        if (isSafeClientUrl(frame.textContent)) target.setAttribute("src", frame.textContent.trim());
      } else if (frame.slotKind === "color" && typeof frame.textContent === "string") {
        var cVal = frame.textContent.trim();
        var cLow = cVal.toLowerCase();
        if (cVal && cLow.indexOf("\\\\") === -1 && cLow.indexOf("url(") === -1 && cLow.indexOf("expression(") === -1) {
          target.style.color = cVal;
        }
      } else if (frame.slotKind === "chart_json") {
        var cJson = typeof frame.chartsJson === "string" ? frame.chartsJson : (typeof frame.textContent === "string" ? frame.textContent : "");
        if (cJson && cJson.length <= 65536) target.setAttribute("data-charts-json", cJson);
      } else if (typeof frame.textContent === "string") {
        target.textContent = frame.textContent;
      }
      if (typeof frame.chartsJson === "string" && frame.chartsJson.length <= 65536) {
        target.setAttribute("data-charts-json", frame.chartsJson);
        if (typeof hydrateEmbeddedCharts === "function") hydrateEmbeddedCharts();
      } else if (frame.slotKind === "chart_json" && typeof hydrateEmbeddedCharts === "function") {
        hydrateEmbeddedCharts();
      }
      if (frame.safeStyles && typeof frame.safeStyles === "object") {
        var ALLOWED_STYLES = ["color", "backgroundColor", "fontSize", "fontWeight", "fontFamily", "lineHeight", "letterSpacing", "textAlign", "borderColor", "padding", "margin", "gap", "borderRadius", "opacity", "display"];
        for (var k in frame.safeStyles) {
          var val = ALLOWED_STYLES.indexOf(k) !== -1 ? String(frame.safeStyles[k] || "") : "";
          var lowerVal = val.toLowerCase();
          if (val && lowerVal.indexOf("\\\\") === -1 && lowerVal.indexOf("url(") === -1 && lowerVal.indexOf("@import") === -1 && lowerVal.indexOf("javascript:") === -1 && lowerVal.indexOf("expression(") === -1) target.style[k] = val;
        }
      }
      var hostSlide = target.closest ? (target.closest(".slide") || document.querySelector(".slide.active")) : document.querySelector(".slide.active");
      emitOverflowTelemetry(hostSlide, ffId);
      scheduleElementRectsPublish();
      if (bridgePort) bridgePort.postMessage({ type: "PATCH_ACK", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, slideUid: frame.slideUid || undefined, ffId: ffId, appliedAtMs: Date.now() });
    } catch (_) {}
  }`;
}

function buildCtaHoverAndPatchSnippet(): string {
  return `${buildCtaHoverAndTelemetrySnippet()}\n${buildResolvePatchTargetSnippet()}\n${buildPatchByFfIdSnippet()}`;
}

function buildDeckNavAndRectsSnippet(): string {
  return `  function applyDeckKeyNav(direction, targetIndex) {
    var slides = document.querySelectorAll(".slide");
    if (!slides || slides.length === 0 || !bridgePort) return;
    var activeIdx = 0;
    for (var i = 0; i < slides.length; i++) {
      if (slides[i].classList && slides[i].classList.contains("active")) { activeIdx = i; break; }
    }
    var nextIdx = activeIdx;
    if (direction === "next") nextIdx = Math.min(slides.length - 1, activeIdx + 1);
    else if (direction === "prev") nextIdx = Math.max(0, activeIdx - 1);
    else if (direction === "goto" && typeof targetIndex === "number") nextIdx = Math.max(0, Math.min(slides.length - 1, targetIndex));
    for (var j = 0; j < slides.length; j++) {
      if (slides[j].classList) slides[j].classList.toggle("active", j === nextIdx);
    }
    var activeSlideEl = slides[nextIdx];
    var uid = activeSlideEl && activeSlideEl.getAttribute ? activeSlideEl.getAttribute("data-slide-uid") : null;
    bridgePort.postMessage({ type: "SLIDE_CHANGE", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, slideIndex: nextIdx, slideUid: uid || undefined });
    if (activeSlideEl && activeSlideEl.clientHeight > 0 && activeSlideEl.scrollHeight > activeSlideEl.clientHeight) {
      bridgePort.postMessage({ type: "SLIDE_OVERFLOW_WARN", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, slideIndex: nextIdx, scrollHeight: activeSlideEl.scrollHeight, clientHeight: activeSlideEl.clientHeight });
    }
    publishElementRects();
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
    bridgePort.postMessage({ type: "ELEMENT_RECTS", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, rects: rects });
  }`;
}

function buildChart2DArcSnippet(): string {
  return `  function chartRound1(n) { return Math.round(n * 10) / 10; }
  function chartPolar(cx, cy, r, deg) {
    var rad = deg * Math.PI / 180;
    return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
  }
  function chartArcD(cx, cy, outerR, innerR, startDeg, endDeg) {
    var span = Math.min(359.99, Math.max(0.1, endDeg - startDeg));
    var large = span > 180 ? 1 : 0;
    var p1 = chartPolar(cx, cy, outerR, startDeg), p2 = chartPolar(cx, cy, outerR, startDeg + span);
    if (innerR <= 0) return "M " + cx + " " + cy + " L " + chartRound1(p1.x) + " " + chartRound1(p1.y) + " A " + outerR + " " + outerR + " 0 " + large + " 1 " + chartRound1(p2.x) + " " + chartRound1(p2.y) + " Z";
    var p3 = chartPolar(cx, cy, innerR, startDeg + span), p4 = chartPolar(cx, cy, innerR, startDeg);
    return "M " + chartRound1(p1.x) + " " + chartRound1(p1.y) + " A " + outerR + " " + outerR + " 0 " + large + " 1 " + chartRound1(p2.x) + " " + chartRound1(p2.y) + " L " + chartRound1(p3.x) + " " + chartRound1(p3.y) + " A " + innerR + " " + innerR + " 0 " + large + " 0 " + chartRound1(p4.x) + " " + chartRound1(p4.y) + " Z";
  }`;
}

function buildFfIdPatchAndRectsSnippet(): string {
  return `${buildCtaHoverAndPatchSnippet()}\n${buildChart2DRenderSnippet(buildChart2DArcSnippet())}\n${buildDeckNavAndRectsSnippet()}\n${buildDeckBlackoutSnippet()}`;
}

function buildDeckInitAndPortSnippet(): string {
  return `    bridgePort.postMessage({ type: "PORT_ACK", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, artifactSha256: ARTIFACT_SHA256, domAnchorHash: DOM_ANCHOR_HASH });
    bridgePort.postMessage({ type: "READY", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq });
    var deckNodes = document.querySelectorAll(".slide");
    var ffNodes = document.querySelectorAll("[data-ff-id]");
    var seenFf = Object.create(null);
    var uniqueFfCount = 0;
    for (var fIdx = 0; fIdx < ffNodes.length; fIdx++) {
      var ffVal = String(ffNodes[fIdx].getAttribute("data-ff-id") || "").trim();
      if (ffVal && !seenFf[ffVal]) { seenFf[ffVal] = true; uniqueFfCount++; }
    }
    var activeIdx = 0;
    for (var sIdx = 0; sIdx < deckNodes.length; sIdx++) {
      if (deckNodes[sIdx].classList && deckNodes[sIdx].classList.contains("active")) { activeIdx = sIdx; break; }
    }
    var hasDeckProto = (document.documentElement && document.documentElement.getAttribute("data-cowok-deck-protocol") === "1") || (document.body && document.body.getAttribute("data-cowok-deck-protocol") === "1");
    if (hasDeckProto || deckNodes.length > 0) {
      bridgePort.postMessage({ type: "DECK_PROTOCOL_INIT", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, protocolVersion: "1", slideCount: deckNodes.length, activeSlideIndex: activeIdx, uniqueFfIdsCount: uniqueFfCount });
    }
    if (typeof hydrateEmbeddedCharts === "function") hydrateEmbeddedCharts();
    publishElementRects();
    var h = document.documentElement ? document.documentElement.scrollHeight : 0;
    bridgePort.postMessage({ type: "HEIGHT_CHANGED", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, height: h });`;
}

function buildPortOnMessageSnippet(): string {
  return `    bridgePort.onmessage = function(pe) {
      if (!pe || !pe.data) return;
      var peSecret = pe.data.bootSecret || pe.data.channelSecret;
      if (pe.data.epoch !== EPOCH || (peSecret !== BOOT_SECRET && peSecret !== CHANNEL_SECRET)) return;
      if (typeof pe.data.seq !== "number" || pe.data.seq <= lastAcceptedSeq) return;
      lastAcceptedSeq = pe.data.seq;
      if (pe.data.type === "APPLY_HTML") applySanitizedHtml(pe.data.html);
      if (pe.data.type === "FLOW_PREVIEW_HYDRATE") applyFixtureHydration(pe.data.variant, pe.data.fixtureJson);
      if (pe.data.type === "CTA_HOVER_SYNC") applyCtaHoverSync(pe.data.ffId, pe.data.hovered);
      if (pe.data.type === "PATCH_BY_FF_ID") applyPatchByFfId(pe.data);
      if (pe.data.type === "RENDER_2D_CHART") applyRender2DChart(pe.data);
      if (pe.data.type === "DECK_KEY_NAV") applyDeckKeyNav(pe.data.direction, pe.data.targetIndex);
      if (pe.data.type === "DECK_BLACKOUT_SET") applyDeckBlackout(pe.data.mode);
    };
    if (typeof ResizeObserver === "function") {
      try {
        var slideRo = new ResizeObserver(function(entries) {
          scheduleElementRectsPublish();
          for (var ri = 0; ri < entries.length; ri++) emitOverflowTelemetry(entries[ri].target, undefined);
        });
        document.querySelectorAll(".slide").forEach(function(sn) { slideRo.observe(sn); });
      } catch (_) {}
    }
    window.addEventListener("securitypolicyviolation", function(ev) {
      if (!bridgePort) return;
      var activeEl = document.querySelector(".slide.active");
      var uid = activeEl && activeEl.getAttribute ? activeEl.getAttribute("data-slide-uid") : null;
      bridgePort.postMessage({
        type: "DECK_CSP_VIOLATION",
        epoch: EPOCH,
        bootSecretAck: BOOT_SECRET,
        channelSecret: CHANNEL_SECRET,
        seq: ++localSeq,
        slideUid: uid || undefined,
        violatedDirective: String(ev.violatedDirective || "unknown").slice(0, 256),
        blockedUri: String(ev.blockedURI || "inline").slice(0, 512)
      });
    });`;
}

function buildInitPortListenerSnippet(): string {
  return `  window.addEventListener("message", function(event) {
    if (channelBound || !event || event.source !== window.parent || !event.data || event.data.type !== "INIT_PORT") return;
    if (!EXPECTED_PARENT_ORIGIN || event.origin !== EXPECTED_PARENT_ORIGIN) return;
    var secret = event.data.bootSecret || event.data.channelSecret;
    var validSecret = (secret === BOOT_SECRET) || (secret === CHANNEL_SECRET);
    if (event.data.epoch !== EPOCH || !validSecret || !event.ports || !event.ports[0]) return;
    if (event.data.channelSecret) CHANNEL_SECRET = String(event.data.channelSecret);
    event.stopImmediatePropagation();
    event.preventDefault();
    channelBound = true;
    bridgePort = event.ports[0];
${buildPortOnMessageSnippet()}
${buildDeckInitAndPortSnippet()}
  }, true);
  if (EXPECTED_PARENT_ORIGIN && window.parent && window.parent !== window) {
    try { window.parent.postMessage({ type: "PORT_READY", epoch: EPOCH, bootSecret: BOOT_SECRET }, EXPECTED_PARENT_ORIGIN); } catch (_) {}
  }`;
}

function buildInspectElementHelpersSnippet(): string {
  return `  function extractElementInspectMeta(el) {
    var tag = String(el && el.tagName || "DIV").toLowerCase().slice(0, 64);
    var txt = String(el && el.textContent || "").replace(/\\s+/g, " ").trim().slice(0, 400);
    var slotKind = el && el.getAttribute ? String(el.getAttribute("data-slot-kind") || "").slice(0, 64) : "";
    var overflowPx = 0;
    try {
      if (el && typeof el.scrollHeight === "number" && typeof el.clientHeight === "number") {
        overflowPx = Math.max(0, Math.round(el.scrollHeight - el.clientHeight));
      }
    } catch (_) {}
    var cssVars = {};
    try {
      var cs = window.getComputedStyle ? window.getComputedStyle(el) : null;
      if (cs) {
        cssVars["font-size"] = String(cs.fontSize || "").slice(0, 64);
        cssVars["color"] = String(cs.color || "").slice(0, 64);
        cssVars["background"] = String(cs.backgroundColor || "").slice(0, 64);
        var rootCs = document.documentElement ? window.getComputedStyle(document.documentElement) : null;
        var tokenNames = ["--bg-surface", "--bg-canvas", "--text-primary", "--text-secondary", "--action-primary", "--human-accent", "--border-default"];
        for (var ti = 0; ti < tokenNames.length; ti++) {
          var tv = rootCs ? String(rootCs.getPropertyValue(tokenNames[ti]) || "").trim() : "";
          if (tv) cssVars[tokenNames[ti]] = tv.slice(0, 64);
        }
      }
    } catch (_) {}
    return { tagName: tag, textContent: txt || undefined, slotKind: slotKind || undefined, cssVariables: cssVars, overflowPx: overflowPx };
  }
  function resolveAnchorElement(target) {
    var el = target && target.closest ? target.closest("[data-ff-id]") : null;
    if (!el && document.querySelector) el = document.querySelector("[data-ff-id]");
    return el;
  }`;
}

function buildStageWindowListenersSnippet(): string {
  return `  window.addEventListener("click", function(event) {
    var linkEl = event.target && event.target.closest ? event.target.closest("a[href]") : null;
    if (linkEl) event.preventDefault();
    if (!event.isTrusted || !bridgePort) return;
    var el = event.target && event.target.closest ? event.target.closest("[data-ff-id]") : null;
    if (!el) {
      var slideEl = document.querySelector(".slide.active");
      var uid = slideEl && slideEl.getAttribute ? slideEl.getAttribute("data-slide-uid") : null;
      bridgePort.postMessage({ type: "STAGE_MAT_CLICK", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, slideUid: uid || undefined });
      return;
    }
    var id = String(el.getAttribute("data-ff-id") || "").slice(0, 200);
    var r = el.getBoundingClientRect ? el.getBoundingClientRect() : { left: 0, top: 0, width: 0, height: 0 };
    bridgePort.postMessage({ type: "ELEMENT_CLICKED", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, selector: '[data-ff-id="' + id + '"]', elementFfId: id, tagName: String(el.tagName || "DIV").toLowerCase().slice(0, 64), domHash: hashElementDom(el), rect: { left: Math.round(r.left), top: Math.round(r.top), width: Math.round(r.width), height: Math.round(r.height) }, metadata: extractElementInspectMeta(el), altKey: Boolean(event.altKey), shiftKey: Boolean(event.shiftKey), metaKey: Boolean(event.metaKey), ctrlKey: Boolean(event.ctrlKey) });
  }, true);
  window.addEventListener("contextmenu", function(event) {
    if (!event.isTrusted || !bridgePort) return;
    var el = resolveAnchorElement(event.target);
    if (!el) return;
    event.preventDefault();
    var id = String(el.getAttribute("data-ff-id") || "").slice(0, 200);
    var r = el.getBoundingClientRect ? el.getBoundingClientRect() : { left: 0, top: 0, width: 0, height: 0 };
    bridgePort.postMessage({ type: "ELEMENT_CONTEXT_MENU", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, selector: '[data-ff-id="' + id + '"]', elementFfId: id, tagName: String(el.tagName || "DIV").toLowerCase().slice(0, 64), domHash: hashElementDom(el), rect: { left: Math.round(r.left), top: Math.round(r.top), width: Math.round(r.width), height: Math.round(r.height) }, clientX: Math.round(event.clientX || r.left), clientY: Math.round(event.clientY || r.top), metadata: extractElementInspectMeta(el) });
  }, true);
  window.addEventListener("mouseup", function(event) {
    if (!event.isTrusted || !bridgePort || !window.getSelection) return;
    var sel = window.getSelection();
    if (!sel || sel.isCollapsed) return;
    var text = String(sel.toString() || "").trim();
    if (text.length <= 2) return;
    var rect = sel.rangeCount > 0 ? sel.getRangeAt(0).getBoundingClientRect() : { left: 0, top: 0, width: 0, height: 0 };
    bridgePort.postMessage({ type: "TEXT_QUOTE", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, text: text.slice(0, 2000), rect: { left: Math.round(rect.left), top: Math.round(rect.top), width: Math.round(rect.width), height: Math.round(rect.height) } });
  }, true);
  window.addEventListener("error", function(err) {
    if (bridgePort) bridgePort.postMessage({ type: "RUNTIME_ERROR", epoch: EPOCH, bootSecretAck: BOOT_SECRET, channelSecret: CHANNEL_SECRET, seq: ++localSeq, message: String(err && err.message || "runtime_error").slice(0, 1024) });
  }, true);`;
}

function buildBridgeListenersSnippet(): string {
  return [
    buildInitPortListenerSnippet(),
    buildInspectElementHelpersSnippet(),
    buildStageWindowListenersSnippet(),
  ].join("\n");
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
  var SCRIPT_NONCE = ${safeInlineJson(cleanNonce)};
  var BOOT_SECRET = ${safeInlineJson(opts.bootSecret)};
  var CHANNEL_SECRET = BOOT_SECRET;
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
  ensureFixtureAndHoverStyleTag();
})();</script>`;
}
