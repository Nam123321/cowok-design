// @story 04.1, 04.5, 05.1, 04.6
// @implements AC-02 (DOMParser-Only Zero-Network HTML/CSS/SVG Double-Parse mXSS Scrubber)
import {
  getStartupDesignCdnCspAllowlist,
  inferSvgViewBox,
  isAllowedSvgUrl,
  parseDesignCdnCspAllowlist,
  sanitizeSvgElements,
} from "./html-sandbox-svg-policy";

export { getStartupDesignCdnCspAllowlist, inferSvgViewBox, isAllowedSvgUrl, parseDesignCdnCspAllowlist };

const MAX_HTML_PAYLOAD_BYTES = 1_048_576;
const SAFE_ATTR_NAME_RE = /^[a-zA-Z_:][a-zA-Z0-9_.:-]*$/;
const INLINE_EVENT_ATTR_RE = /^on/i;
const URL_CHECKED_ATTRS = new Set(["href", "src", "xlink:href", "action", "formaction", "poster", "data"]);
const FORBIDDEN_TAGS = [
  "script", "iframe", "object", "embed", "foreignobject", "math",
  "link", "applet", "template", "noscript", "base", "meta",
];
const FORBIDDEN_CSS_NEEDLES = [
  "@import", "@charset", "@namespace", "@document", "url(", "image-set(",
  "expression(", "-moz-binding", "behavior:", "javascript:", "vbscript:", "data:", "</style", "<",
];

export function escapeHtmlAttr(raw: string): string {
  return raw.replace(/&/g, "&amp;").replace(/\x22/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

export function validateUserHtmlPayload(raw: string): { valid: boolean; reason?: "empty" | "too_large" | "invalid_html" } {
  const trimmed = raw.trim();
  if (!trimmed) return { valid: false, reason: "empty" };
  if (new TextEncoder().encode(trimmed).length > MAX_HTML_PAYLOAD_BYTES) return { valid: false, reason: "too_large" };
  const lastOpen = trimmed.lastIndexOf("<");
  const lastClose = trimmed.lastIndexOf(">");
  if (lastOpen === -1 || lastClose === -1 || lastOpen > lastClose || !/<[a-zA-Z][a-zA-Z0-9-]{0,31}(?=[\s/>])/.test(trimmed)) {
    return { valid: false, reason: "invalid_html" };
  }
  if (typeof DOMParser !== "undefined") {
    const doc = new DOMParser().parseFromString(trimmed, "text/html");
    const hasElements = Boolean(doc.body?.children.length || doc.head?.children.length);
    if (!hasElements) return { valid: false, reason: "invalid_html" };
  }
  return { valid: true };
}

export function sanitizeCssContent(cssText: string, inlineMode = false): string {
  if (!cssText || typeof cssText !== "string") return "";
  if (cssText.includes("\u0000") || cssText.includes("\\")) return "";
  if (inlineMode && (cssText.includes("{") || cssText.includes("}"))) return "";
  let s = cssText;
  let prev = "";
  while (s !== prev) {
    prev = s;
    s = s.replace(/\/\*[\s\S]*?\*\//g, "");
    s = s.replace(/@(?:import|charset|namespace|document)\b[^;{}]*;?/gi, "");
    s = s.replace(/(?:url|image-set|expression)\s*\([^)]*\)/gi, "none");
    s = s.replace(/(?:url|image-set|expression)\s*\([^)]*$/gi, "none");
    s = s.replace(/(?:-moz-binding|behavior)\s*:[^;{}]*;?/gi, "");
  }
  if (s.includes("/*") || s.includes("*/")) return "";
  const lower = s.toLowerCase();
  const compact = lower.replace(/\s+/g, "");
  return FORBIDDEN_CSS_NEEDLES.some((n) => lower.includes(n) || compact.includes(n)) ? "" : s;
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
  if (URL_CHECKED_ATTRS.has(lowerName) && hasDangerousUrlScheme(attr.value)) el.removeAttribute(attr.name);
}

function collectSafeElementAttributes(el: Element | null): string {
  if (!el) return "";
  return Array.from(el.attributes)
    .filter((a) => SAFE_ATTR_NAME_RE.test(a.name) && !INLINE_EVENT_ATTR_RE.test(a.name.trim()) && a.name.trim().toLowerCase() !== "srcdoc")
    .map((a) => ` ${a.name}="${escapeHtmlAttr(a.value)}"`)
    .join("");
}

function scrubStyleElements(doc: Document, styleNonce: string): void {
  doc.querySelectorAll("meta[http-equiv]").forEach((el) => {
    const eq = (el.getAttribute("http-equiv") || "").trim().toLowerCase();
    if (eq === "refresh" || eq === "content-security-policy") el.remove();
  });
  doc.querySelectorAll("style").forEach((styleEl) => {
    const cleanCss = sanitizeCssContent(styleEl.textContent || "");
    if (!cleanCss.trim()) styleEl.remove();
    else {
      styleEl.textContent = cleanCss;
      styleEl.setAttribute("nonce", styleNonce);
    }
  });
}

function hoistInlineStyles(doc: Document, styleNonce: string): void {
  const hoistedRules: string[] = [];
  let styleIdx = 0;
  doc.querySelectorAll("*").forEach((el) => {
    for (const attr of Array.from(el.attributes)) sanitizeSingleAttribute(el, attr);
    const inlineStyle = el.getAttribute("style");
    if (!inlineStyle?.trim()) return;
    const cleanInline = sanitizeCssContent(inlineStyle, true).trim();
    el.removeAttribute("style");
    if (cleanInline) {
      const id = `s${styleIdx++}`;
      el.setAttribute("data-cowok-s", id);
      hoistedRules.push(`[data-cowok-s="${id}"]{${cleanInline}}`);
    }
  });
  if (hoistedRules.length > 0 && doc.head) {
    const hoistedStyle = doc.createElement("style");
    hoistedStyle.setAttribute("nonce", styleNonce);
    hoistedStyle.textContent = hoistedRules.join("\n");
    doc.head.appendChild(hoistedStyle);
  }
}

function scrubParsedDocument(doc: Document, styleNonce: string): void {
  doc.querySelectorAll(FORBIDDEN_TAGS.join(",")).forEach((el) => el.remove());
  sanitizeSvgElements(doc);
  scrubStyleElements(doc, styleNonce);
  hoistInlineStyles(doc, styleNonce);
}

export function sanitizeViaDomParser(rawHtml: string, headPrefixHtml: string, styleNonce: string): string {
  if (typeof DOMParser === "undefined") {
    return `<!DOCTYPE html><html><head>${headPrefixHtml}</head><body><pre>${escapeHtmlAttr(rawHtml)}</pre></body></html>`;
  }
  const parser = new DOMParser();
  const pass1 = parser.parseFromString(rawHtml, "text/html");
  scrubParsedDocument(pass1, styleNonce);
  const pass1Html = `<!DOCTYPE html><html${collectSafeElementAttributes(pass1.documentElement)}><head>${pass1.head?.innerHTML || ""}</head><body${collectSafeElementAttributes(pass1.body)}>${pass1.body?.innerHTML || ""}</body></html>`;
  const pass2 = parser.parseFromString(pass1Html, "text/html");
  scrubParsedDocument(pass2, styleNonce);
  return `<!DOCTYPE html><html${collectSafeElementAttributes(pass2.documentElement)}><head>${headPrefixHtml}${pass2.head?.innerHTML || ""}</head><body${collectSafeElementAttributes(pass2.body)}>${pass2.body?.innerHTML || ""}</body></html>`;
}

export function prepareHtmlForApplyBridge(rawHtml: string, scriptNonce?: string): string {
  if (typeof DOMParser === "undefined" || !scriptNonce) return rawHtml;
  const doc = new DOMParser().parseFromString(rawHtml, "text/html");
  scrubParsedDocument(doc, scriptNonce);
  return `<!DOCTYPE html><html${collectSafeElementAttributes(doc.documentElement)}><head>${doc.head?.innerHTML || ""}</head><body${collectSafeElementAttributes(doc.body)}>${doc.body?.innerHTML || ""}</body></html>`;
}
