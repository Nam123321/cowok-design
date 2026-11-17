// @story 08.4
// @implements AC-05 (XSS & external image-beacon stripping for LEARN Markdown preview)

const HTML_TAG_REGEX = /<\/?[a-zA-Z][^>]*>/g;
const MARKDOWN_IMAGE_BEACON_REGEX = /!\[([^\]]*)\]\(([^)]*)\)/g;
const DANGEROUS_SCHEME_REGEX = /\b(javascript|vbscript|data):/gi;
const EVENT_HANDLER_REGEX = /\bon[a-z]+\s*=/gi;

/**
 * Strips raw HTML tags, inline event handlers, scriptable URI schemes, and Markdown image beacons
 * so LEARN knowledge cards render strictly as safe inert text with zero external pixel beacons or XSS.
 */
export function sanitizeLearnMarkdownPreview(rawMarkdown: string): string {
  if (!rawMarkdown) return "";
  return rawMarkdown
    .replace(MARKDOWN_IMAGE_BEACON_REGEX, (_full, alt: string) => {
      const cleanAlt = String(alt || "image").trim();
      return `[blocked-image-beacon: ${cleanAlt || "external"}]`;
    })
    .replace(HTML_TAG_REGEX, "")
    .replace(DANGEROUS_SCHEME_REGEX, "[blocked-scheme]:")
    .replace(EVENT_HANDLER_REGEX, "[blocked-handler]=")
    .trim();
}
