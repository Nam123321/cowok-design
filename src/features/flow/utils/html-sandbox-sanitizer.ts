// @story 04.1, 04.5, 05.1
// @implements AC-02 (DOMParser-Only Zero-Network HTML/CSS/SVG Double-Parse mXSS Scrubber)
const MAX_HTML_PAYLOAD_BYTES = 1_048_576;
const SAFE_ATTR_NAME_RE = /^[a-zA-Z_:][a-zA-Z0-9_.:-]*$/;
const INLINE_EVENT_ATTR_RE = /^on/i;
const URL_CHECKED_ATTRS = new Set(["href", "src", "xlink:href", "action", "formaction", "poster", "data"]);
const FORBIDDEN_TAGS = [
  "script",
  "iframe",
  "object",
  "embed",
  "foreignobject",
  "math",
  "link",
  "applet",
  "template",
  "noscript",
  "base",
  "meta",
];
const FORBIDDEN_SVG_CHILDREN = "foreignobject,script,style,animate,animatemotion,animatetransform,set,use";
const FORBIDDEN_CSS_NEEDLES = [
  "@import",
  "@charset",
  "@namespace",
  "@document",
  "url(",
  "image-set(",
  "expression(",
  "-moz-binding",
  "behavior:",
  "javascript:",
  "vbscript:",
  "data:",
  "</style",
  "<",
];

export function escapeHtmlAttr(raw: string): string {
  return raw.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
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
  const lastOpen = trimmed.lastIndexOf("<");
  const lastClose = trimmed.lastIndexOf(">");
  if (lastOpen === -1 || lastClose === -1 || lastOpen > lastClose || !/<[a-zA-Z][a-zA-Z0-9-]{0,31}(?=[\s/>])/.test(trimmed)) {
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
  for (const needle of FORBIDDEN_CSS_NEEDLES) {
    if (lower.includes(needle) || compact.includes(needle)) {
      return "";
    }
  }
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

function collectSafeElementAttributes(el: Element | null): string {
  if (!el) return "";
  return Array.from(el.attributes)
    .filter((a) => {
      const lower = a.name.trim().toLowerCase();
      return SAFE_ATTR_NAME_RE.test(a.name) && !INLINE_EVENT_ATTR_RE.test(lower) && lower !== "srcdoc";
    })
    .map((a) => ` ${a.name}="${escapeHtmlAttr(a.value)}"`)
    .join("");
}

function scrubParsedDocument(doc: Document, styleNonce: string): void {
  doc.querySelectorAll(FORBIDDEN_TAGS.join(",")).forEach((el) => el.remove());
  doc.querySelectorAll("svg").forEach((svgEl) => {
    const hasEventOrNoViewBox =
      !svgEl.hasAttribute("viewBox") ||
      Array.from(svgEl.attributes).some((a) => INLINE_EVENT_ATTR_RE.test(a.name.trim()));
    if (hasEventOrNoViewBox || svgEl.querySelector(FORBIDDEN_SVG_CHILDREN)) {
      svgEl.remove();
      return;
    }
    [svgEl, ...Array.from(svgEl.querySelectorAll("*"))].forEach((node) => {
      node.removeAttribute("href");
      node.removeAttribute("xlink:href");
    });
  });
  doc.querySelectorAll("meta[http-equiv]").forEach((el) => {
    const eq = (el.getAttribute("http-equiv") || "").trim().toLowerCase();
    if (eq === "refresh" || eq === "content-security-policy") el.remove();
  });
  doc.querySelectorAll("style").forEach((styleEl) => {
    const cleanCss = sanitizeCssContent(styleEl.textContent || "");
    if (!cleanCss.trim()) {
      styleEl.remove();
      return;
    }
    styleEl.textContent = cleanCss;
    styleEl.setAttribute("nonce", styleNonce);
  });
  const hoistedRules: string[] = [];
  let styleIdx = 0;
  doc.querySelectorAll("*").forEach((el) => {
    for (const attr of Array.from(el.attributes)) sanitizeSingleAttribute(el, attr);
    const inlineStyle = el.getAttribute("style");
    if (inlineStyle && inlineStyle.trim()) {
      const cleanInline = sanitizeCssContent(inlineStyle, true).trim();
      el.removeAttribute("style");
      if (cleanInline) {
        const id = `s${styleIdx++}`;
        el.setAttribute("data-cowok-s", id);
        hoistedRules.push(`[data-cowok-s="${id}"]{${cleanInline}}`);
      }
    }
  });
  if (hoistedRules.length > 0 && doc.head) {
    const hoistedStyle = doc.createElement("style");
    hoistedStyle.setAttribute("nonce", styleNonce);
    hoistedStyle.textContent = hoistedRules.join("\n");
    doc.head.appendChild(hoistedStyle);
  }
}

export function sanitizeViaDomParser(rawHtml: string, headPrefixHtml: string, styleNonce: string): string {
  if (typeof DOMParser === "undefined") {
    const safeText = escapeHtmlAttr(rawHtml);
    return `<!DOCTYPE html><html><head>${headPrefixHtml}</head><body><pre>${safeText}</pre></body></html>`;
  }
  const parser = new DOMParser();
  const pass1 = parser.parseFromString(rawHtml, "text/html");
  scrubParsedDocument(pass1, styleNonce);
  const pass1Html = `<!DOCTYPE html><html${collectSafeElementAttributes(pass1.documentElement)}><head>${pass1.head ? pass1.head.innerHTML : ""}</head><body${collectSafeElementAttributes(pass1.body)}>${pass1.body ? pass1.body.innerHTML : ""}</body></html>`;
  // Pass 2 double-parse mXSS convergence scrub
  const pass2 = parser.parseFromString(pass1Html, "text/html");
  scrubParsedDocument(pass2, styleNonce);
  const htmlAttrs = collectSafeElementAttributes(pass2.documentElement);
  const headHtml = pass2.head ? pass2.head.innerHTML : "";
  const bodyAttrs = collectSafeElementAttributes(pass2.body);
  const bodyHtml = pass2.body ? pass2.body.innerHTML : "";
  return `<!DOCTYPE html><html${htmlAttrs}><head>${headPrefixHtml}${headHtml}</head><body${bodyAttrs}>${bodyHtml}</body></html>`;
}

export function prepareHtmlForApplyBridge(rawHtml: string, scriptNonce?: string): string {
  if (typeof DOMParser === "undefined" || !scriptNonce) return rawHtml;
  const parser = new DOMParser();
  const doc = parser.parseFromString(rawHtml, "text/html");
  scrubParsedDocument(doc, scriptNonce);
  const htmlAttrs = collectSafeElementAttributes(doc.documentElement);
  const headHtml = doc.head ? doc.head.innerHTML : "";
  const bodyAttrs = collectSafeElementAttributes(doc.body);
  const bodyHtml = doc.body ? doc.body.innerHTML : "";
  return `<!DOCTYPE html><html${htmlAttrs}><head>${headHtml}</head><body${bodyAttrs}>${bodyHtml}</body></html>`;
}
