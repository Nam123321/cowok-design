/**
 * Deterministic token estimator for YAML frontmatter (~4 chars per token or ~1.33 tokens per word)
 */
export function estimateTokenCount(text: string): number {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  const charCount = trimmed.length;
  const wordCount = trimmed.split(/\s+/).filter(Boolean).length;

  const charEstimate = Math.ceil(charCount / 4);
  const wordEstimate = Math.ceil(wordCount * 1.33);
  return Math.max(charEstimate, wordEstimate);
}

export interface ValidationResult {
  valid: boolean;
  tokens: number;
  error?: string;
  fields?: {
    name?: string;
    version?: string;
    description?: string;
    risk_tier?: number;
  };
}

/**
 * Validates that a SKILL.md frontmatter adheres to the agentskills.io standard:
 * - Delimited by `---`
 * - Estimated token count <= 100 tokens (strict AC-03 budget guard)
 * - Contains `name` and `description`
 */
export function validateSkillFrontmatter(rawContent: string): ValidationResult {
  const trimmed = rawContent.trim();
  let yamlBody = "";

  if (trimmed.startsWith("---")) {
    const afterFirst = trimmed.slice(3);
    const endIdx = afterFirst.indexOf("---");
    if (endIdx === -1) {
      return {
        valid: false,
        tokens: 0,
        error: "Frontmatter must start and end with '---' delimiters",
      };
    }
    yamlBody = afterFirst.slice(0, endIdx).trim();
  } else {
    yamlBody = trimmed;
  }

  const tokens = estimateTokenCount(yamlBody);
  if (tokens > 100) {
    return {
      valid: false,
      tokens,
      error: `Frontmatter exceeds 100-token budget: ${tokens} tokens estimated (limit: 100)`,
    };
  }

  const lines = yamlBody.split("\n");
  let name: string | undefined;
  let version: string | undefined;
  let description: string | undefined;
  let risk_tier: number | undefined;

  for (const line of lines) {
    const t = line.trim();
    if (t.startsWith("name:")) {
      name = t.slice("name:".length).trim().replace(/^['"]|['"]$/g, "");
    } else if (t.startsWith("id:") && !name) {
      name = t.slice("id:".length).trim().replace(/^['"]|['"]$/g, "");
    } else if (t.startsWith("version:")) {
      version = t.slice("version:".length).trim().replace(/^['"]|['"]$/g, "");
    } else if (t.startsWith("description:") || t.startsWith("desc:")) {
      description = t.slice(t.indexOf(":") + 1).trim().replace(/^['"]|['"]$/g, "");
    } else if (t.startsWith("risk_tier:")) {
      const parsed = parseInt(t.slice("risk_tier:".length).trim(), 10);
      if (!isNaN(parsed)) risk_tier = parsed;
    }
  }

  if (!name) {
    return {
      valid: false,
      tokens,
      error: "Missing required field in frontmatter: name",
    };
  }

  if (!description) {
    return {
      valid: false,
      tokens,
      error: "Missing required field in frontmatter: description",
    };
  }

  if (risk_tier !== undefined && (risk_tier < 1 || risk_tier > 3)) {
    return {
      valid: false,
      tokens,
      error: `Invalid risk_tier: ${risk_tier} (must be 1, 2, or 3)`,
    };
  }

  return {
    valid: true,
    tokens,
    fields: { name, version, description, risk_tier },
  };
}
