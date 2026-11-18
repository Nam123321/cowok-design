// @story 08.1
// @implements AC-04, AC-08
import type { LintIssue } from "./types";

function mapSmartQuote(ch: string, inAttr: boolean): { out: string; changed: boolean } {
  if (ch === "“" || ch === "”") return { out: inAttr ? "&quot;" : '"', changed: true };
  if (ch === "‘" || ch === "’") return { out: inAttr ? "&#39;" : "'", changed: true };
  return { out: ch, changed: false };
}

function replaceSmartQuotesSafe(content: string): { result: string; changed: boolean } {
  let out = "";
  let changed = false;
  let inTag = false;
  let attrQuote: string | null = null;
  for (let i = 0; i < content.length; i++) {
    const ch = content[i];
    if (!inTag && ch === "<" && /[a-z!/?]/i.test(content[i + 1] ?? "")) {
      inTag = true;
      attrQuote = null;
    } else if (inTag && attrQuote === null && (ch === '"' || ch === "'")) {
      attrQuote = ch;
    } else if (inTag && attrQuote !== null && ch === attrQuote) {
      attrQuote = null;
    } else if (inTag && attrQuote === null && ch === ">") {
      inTag = false;
    }
    const mapped = mapSmartQuote(ch, inTag && attrQuote !== null);
    out += mapped.out;
    if (mapped.changed) changed = true;
  }
  return { result: out, changed };
}

/** Performs deterministic single-pass auto-fixes on HTML/CSS markup. */
export function autoFixIssues(content: string): { fixed: string; count: number } {
  let fixed = content;
  let count = 0;
  if (/[“”‘’]/.test(fixed)) {
    const rq = replaceSmartQuotesSafe(fixed);
    fixed = rq.result;
    if (rq.changed) count++;
  }
  if (/#6366f1|#8b5cf6|#7c3aed/i.test(fixed)) {
    fixed = fixed
      .replace(/#6366f1/gi, "var(--action-primary, #2563eb)")
      .replace(/#8b5cf6/gi, "var(--action-primary-hover, #1d4ed8)")
      .replace(/#7c3aed/gi, "var(--action-primary, #2563eb)");
    count++;
  }
  const tabRe = /\btabindex\s*=\s*(?:["'][1-9]\d*["']|[1-9]\d*(?=[\s>]))/gi;
  if (tabRe.test(fixed)) {
    fixed = fixed.replace(/\btabindex\s*=\s*(?:["'][1-9]\d*["']|[1-9]\d*(?=[\s>]))/gi, 'tabindex="0"');
    count++;
  }
  let svgFixed = false;
  fixed = fixed.replace(/<svg\b([^>]*)>/gi, (full, attrs: string) => {
    if (/\baria-(?:hidden|label)\b/i.test(attrs)) return full;
    svgFixed = true;
    return `<svg aria-hidden="true"${attrs}>`;
  });
  if (svgFixed) count++;
  if (/line-height:\s*1;/i.test(fixed)) {
    fixed = fixed.replace(/line-height:\s*1;/gi, "line-height: 1.5;");
    count++;
  }
  if (/<head>/i.test(fixed) && !/name=["']viewport["']/i.test(fixed)) {
    fixed = fixed.replace(/<head>/i, '<head>\n    <meta name="viewport" content="width=device-width, initial-scale=1.0">');
    count++;
  }
  return { fixed, count };
}

/** Counts only newly introduced lint issues per (rule_id, message) multiset after auto-fix. */
export function countNewlyIntroducedIssues(beforeIssues: LintIssue[], afterIssues: LintIssue[]): number {
  const beforeCounts = new Map<string, number>();
  for (const issue of beforeIssues) {
    const key = `${issue.rule_id}\0${issue.message}`;
    beforeCounts.set(key, (beforeCounts.get(key) ?? 0) + 1);
  }
  const afterCounts = new Map<string, number>();
  for (const issue of afterIssues) {
    const key = `${issue.rule_id}\0${issue.message}`;
    afterCounts.set(key, (afterCounts.get(key) ?? 0) + 1);
  }
  let added = 0;
  for (const [key, afterCount] of afterCounts.entries()) {
    const beforeCount = beforeCounts.get(key) ?? 0;
    if (afterCount > beforeCount) added += afterCount - beforeCount;
  }
  return added;
}
