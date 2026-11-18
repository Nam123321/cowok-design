// @story 08.4
// @implements AC-05 (XSS & external image-beacon stripping for LEARN Markdown preview)

const HTML_TAG_REGEX = /<\/?[a-zA-Z][^>]*>/g;
const UNCLOSED_HTML_TAG_REGEX =
  /<(?:!--|\/?(?:script|img|iframe|object|embed|svg|style|link|meta|input|form|button|div|span|body|html|head|video|audio|a|p|h[1-6])(?:\s[^>\n]*|\/)?)$/gim;
const MARKDOWN_IMAGE_BEACON_REGEX = /!\[([^\]]*)\]\s*\((?:[^()\n]|\([^()\n]*\))*\)/g;
const MARKDOWN_REF_IMAGE_REGEX = /!\[([^\]]*)\]\s*\[[^\]]*\]/g;
const MARKDOWN_REF_DEF_REGEX = /^\s*\[([^\]]+)\]:\s*\S+.*$/gm;
const DANGEROUS_SCHEME_REGEX = /\b(javascript|vbscript|data):/gi;
const EVENT_HANDLER_REGEX = /\bon[a-z]+\s*=/gi;
const MAX_SANITIZE_PASSES = 8;

/**
 * Strips raw HTML tags (including nested/unclosed tags), inline event handlers, scriptable URI schemes,
 * and inline or reference-style Markdown image beacons so LEARN knowledge cards render strictly as safe inert text.
 */
export function sanitizeLearnMarkdownPreview(rawMarkdown: string): string {
  if (!rawMarkdown) return "";
  let out = rawMarkdown
    .replace(MARKDOWN_IMAGE_BEACON_REGEX, (_full, alt: string) => {
      const cleanAlt = String(alt || "image").trim();
      return `[blocked-image-beacon: ${cleanAlt || "external"}]`;
    })
    .replace(MARKDOWN_REF_IMAGE_REGEX, (_full, alt: string) => {
      const cleanAlt = String(alt || "image").trim();
      return `[blocked-image-beacon: ${cleanAlt || "external"}]`;
    })
    .replace(MARKDOWN_REF_DEF_REGEX, "[blocked-reference-target]");

  for (let i = 0; i < MAX_SANITIZE_PASSES; i += 1) {
    const next = out
      .replace(HTML_TAG_REGEX, "")
      .replace(UNCLOSED_HTML_TAG_REGEX, "")
      .replace(DANGEROUS_SCHEME_REGEX, "[blocked-scheme]:")
      .replace(EVENT_HANDLER_REGEX, "[blocked-handler]=");
    if (next === out) break;
    out = next;
  }
  return out.trim();
}
