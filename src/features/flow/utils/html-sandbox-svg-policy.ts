// @story 04.6
// @implements AC-03, AC-05 (SVG ViewBox Inference, Strict SVG URL Allowlist & Fail-Closed CDN CSP)
const SAFE_CDN_HOST_RE = /^https:\/\/[a-z0-9.-]+$/;
const SAFE_DATA_IMAGE_RE = /^data:image\/(?:png|jpeg|webp);base64,[A-Za-z0-9+/=]+$/;
const SAFE_SVG_FRAGMENT_RE = /^#[A-Za-z_][A-Za-z0-9_.:-]*$/;
const SAFE_SVG_RELATIVE_RE = /^\/(?:[^/\s\x22\x27<>?#][^\s\x22\x27<>]*)?$/;
const DEFAULT_SVG_VIEWBOX = "0 0 1200 800";
const FORBIDDEN_SVG_CHILDREN = "foreignobject,script,style,animate,animatemotion,animatetransform,set,use";
const INLINE_EVENT_ATTR_RE = /^on/i;

export function parseDesignCdnCspAllowlist(raw?: string): string[] {
  const value = (raw ?? "").trim();
  if (!value) return [];
  const tokens = value.split(/[\s,]+/).filter(Boolean);
  if (tokens.length === 0) return [];
  for (const token of tokens) {
    if (!SAFE_CDN_HOST_RE.test(token)) return [];
  }
  return Array.from(new Set(tokens));
}

const STARTUP_DESIGN_CDN_CSP_ALLOWLIST: readonly string[] = Object.freeze(
  parseDesignCdnCspAllowlist(typeof process !== "undefined" ? process.env?.DESIGN_CDN_CSP : undefined)
);

export function getStartupDesignCdnCspAllowlist(): readonly string[] {
  return STARTUP_DESIGN_CDN_CSP_ALLOWLIST;
}

export function isAllowedSvgUrl(rawValue: string): boolean {
  if (!rawValue || typeof rawValue !== "string") return false;
  if (/[\u0000-\u0020\u007f\\]/.test(rawValue)) return false;
  const trimmed = rawValue.trim();
  if (SAFE_DATA_IMAGE_RE.test(trimmed) || SAFE_SVG_FRAGMENT_RE.test(trimmed) || SAFE_SVG_RELATIVE_RE.test(trimmed)) {
    return true;
  }
  try {
    const u = new URL(trimmed);
    return u.protocol === "https:" || u.protocol === "http:";
  } catch {
    return false;
  }
}

export function inferSvgViewBox(svgEl: Element): string {
  const existing = (svgEl.getAttribute("viewBox") || "").trim();
  if (existing) return existing;
  const parseDim = (attr: string | null): number | null => {
    const m = attr ? /^(\d+(?:\.\d+)?)\s*(?:px)?$/i.exec(attr.trim()) : null;
    const n = m ? Number(m[1]) : NaN;
    return Number.isFinite(n) && n > 0 ? n : null;
  };
  const w = parseDim(svgEl.getAttribute("width"));
  const h = parseDim(svgEl.getAttribute("height"));
  const resolved = w !== null && h !== null ? `0 0 ${w} ${h}` : DEFAULT_SVG_VIEWBOX;
  svgEl.setAttribute("viewBox", resolved);
  return resolved;
}

export function sanitizeSvgElements(doc: Document): void {
  doc.querySelectorAll("svg").forEach((svgEl) => {
    const hasEvent = Array.from(svgEl.attributes).some((a) => INLINE_EVENT_ATTR_RE.test(a.name.trim()));
    if (hasEvent || svgEl.querySelector(FORBIDDEN_SVG_CHILDREN)) {
      svgEl.remove();
      return;
    }
    inferSvgViewBox(svgEl);
    [svgEl, ...Array.from(svgEl.querySelectorAll("*"))].forEach((node) => {
      for (const attrName of ["href", "xlink:href", "src"]) {
        const val = node.getAttribute(attrName);
        if (val !== null && !isAllowedSvgUrl(val)) node.removeAttribute(attrName);
      }
    });
  });
}
