// @story 08.1
// @implements AC-02, AC-03, AC-06
import { getEncoding, Tiktoken } from "js-tiktoken";
import { parseDocument, isAlias, isMap, isPair, isScalar, isSeq } from "yaml";

export const MAX_FRONTMATTER_TOKENS = 100;
export const MAX_FRONTMATTER_BYTES = 1024;

let cachedCl100k: Tiktoken | null = null;

function getCl100kBpe(): Tiktoken {
  if (!cachedCl100k) cachedCl100k = getEncoding("cl100k_base");
  return cachedCl100k;
}

export function countExactCl100kTokens(text: string): number {
  return text ? getCl100kBpe().encode(text, [], []).length : 0;
}

export function estimateTokenCount(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  const bpeTokens = getCl100kBpe().encode(trimmed, [], []).length;
  const charEstimate = Math.ceil(trimmed.length / 4);
  const wordEstimate = Math.ceil(trimmed.split(/\s+/).filter(Boolean).length * 1.33);
  return Math.max(bpeTokens, charEstimate, wordEstimate);
}

export interface ValidationResult {
  valid: boolean;
  tokens: number;
  bytes?: number;
  error?: string;
  fields?: {
    name?: string;
    version?: string;
    description?: string;
    entry_point?: string;
    risk_tier?: number;
    capabilities_required?: string[];
  };
}

function extractBoundedFrontmatter(rawContent: string): string | null {
  const lines = rawContent.replace(/^\uFEFF/, "").split(/\r?\n/);
  if (lines.length < 3 || lines[0] !== "---") return null;
  const closeIdx = lines.indexOf("---", 1);
  return closeIdx === -1 ? null : lines.slice(1, closeIdx).join("\n").trim();
}

function findUnsafeYamlAstConstruct(node: unknown): string | null {
  if (!node || typeof node !== "object") return null;
  if (isAlias(node)) return "Unsafe YAML construct disallowed: aliases (*)";
  const tagged = node as { anchor?: string; tag?: string };
  if (tagged.anchor) return "Unsafe YAML construct disallowed: anchors (&)";
  if (tagged.tag && !tagged.tag.startsWith("tag:yaml.org,2002:")) {
    return `Unsafe YAML construct disallowed: custom tag (${tagged.tag})`;
  }
  const children = isMap(node) ? node.items.flatMap((p) => (isPair(p) ? [p.key, p.value] : [])) : isSeq(node) ? node.items : [];
  for (const child of children) {
    const err = findUnsafeYamlAstConstruct(child);
    if (err) return err;
  }
  return null;
}

function getStrictString(map: ReturnType<typeof parseDocument>["contents"], keys: string[]): { value?: string; error?: string } {
  if (!isMap(map)) return {};
  for (const key of keys) {
    const pair = map.items.find((p) => isPair(p) && isScalar(p.key) && String(p.key.value) === key);
    if (!pair || !isPair(pair)) continue;
    if (!isScalar(pair.value) || typeof pair.value.value !== "string") return { error: `Field '${key}' must be a string` };
    const trimmed = pair.value.value.trim();
    if (trimmed) return { value: trimmed };
  }
  return {};
}

function getStringList(doc: ReturnType<typeof parseDocument>, keys: string[]): { values: string[]; error?: string } {
  for (const key of keys) {
    const node = doc.get(key, true);
    if (node === undefined) continue;
    if (!isSeq(node)) return { values: [], error: `Field '${key}' must be a YAML sequence` };
    const out: string[] = [];
    for (const item of node.items) {
      if (!isScalar(item) || typeof item.value !== "string" || !item.value.trim()) {
        return { values: [], error: `Field '${key}' items must be non-empty strings` };
      }
      out.push(item.value.trim());
    }
    return { values: out };
  }
  return { values: [] };
}

function parseRiskTier(doc: ReturnType<typeof parseDocument>): { value?: number; error?: string } {
  const tierNode = doc.get("risk_tier", true);
  if (tierNode === undefined) return {};
  const rawTier = isScalar(tierNode) ? tierNode.value : tierNode;
  if (typeof rawTier !== "number" || !Number.isInteger(rawTier) || rawTier < 1 || rawTier > 3) {
    return { error: "Invalid risk_tier: must be integer 1, 2, or 3" };
  }
  return { value: rawTier };
}

/** Validates agentskills.io YAML frontmatter against strict YAML 1.2, 1024-byte, and 100-token budgets. */
export function validateSkillFrontmatter(rawContent: string): ValidationResult {
  const yamlBody = extractBoundedFrontmatter(rawContent);
  if (yamlBody === null) return { valid: false, tokens: 0, error: "Frontmatter must start and end with '---' delimiters" };
  const byteLen = Buffer.byteLength(yamlBody, "utf8");
  if (byteLen > MAX_FRONTMATTER_BYTES) {
    return { valid: false, tokens: 0, bytes: byteLen, error: `Frontmatter exceeds ${MAX_FRONTMATTER_BYTES}-byte hard ceiling: ${byteLen} bytes` };
  }
  const doc = parseDocument(yamlBody, { strict: true, uniqueKeys: true, merge: false, schema: "core", customTags: [] });
  if (doc.errors.length > 0 || doc.warnings.length > 0) {
    return { valid: false, tokens: 0, bytes: byteLen, error: `YAML parse error: ${doc.errors[0]?.message || doc.warnings[0]?.message || "Invalid YAML frontmatter"}` };
  }
  const unsafeErr = findUnsafeYamlAstConstruct(doc.contents);
  if (unsafeErr || !isMap(doc.contents)) return { valid: false, tokens: 0, bytes: byteLen, error: unsafeErr ?? "Frontmatter must be a YAML mapping" };

  const tokens = estimateTokenCount(yamlBody);
  if (tokens > MAX_FRONTMATTER_TOKENS) {
    return { valid: false, tokens, bytes: byteLen, error: `Frontmatter exceeds 100-token budget: ${tokens} tokens estimated (limit: 100)` };
  }
  const nameRes = getStrictString(doc.contents, ["name", "id", "title"]);
  const descRes = getStrictString(doc.contents, ["description", "desc"]);
  const verRes = getStrictString(doc.contents, ["version"]);
  const entryRes = getStrictString(doc.contents, ["entry_point"]);
  const entryNode = doc.get("entry_point", true);
  const emptyEntryErr =
    entryNode !== undefined && !entryRes.error && !entryRes.value
      ? "Field 'entry_point' must not be empty"
      : undefined;
  const capsRes = getStringList(doc, ["capabilities_required", "capabilities", "permissions"]);
  const tierRes = parseRiskTier(doc);
  const firstErr = nameRes.error ?? (!nameRes.value ? "Missing required field in frontmatter: name" : undefined) ?? descRes.error ?? (!descRes.value ? "Missing required field in frontmatter: description" : undefined) ?? verRes.error ?? entryRes.error ?? emptyEntryErr ?? capsRes.error ?? tierRes.error;
  if (firstErr) return { valid: false, tokens, bytes: byteLen, error: firstErr };

  return {
    valid: true,
    tokens,
    bytes: byteLen,
    fields: { name: nameRes.value, version: verRes.value, description: descRes.value, entry_point: entryRes.value, risk_tier: tierRes.value, capabilities_required: capsRes.values },
  };
}
