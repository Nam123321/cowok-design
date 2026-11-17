// @story 08.3
// @implements AC-04, AC-06
import crypto from "node:crypto";
import { LearnError } from "./lifecycle-errors";
import { scanSkillMarkdownSafety } from "./repackager-security";

const MAX_SCOPE_SEGMENT_BYTES = 128;

export const nowUnixSeconds = (): number => Math.floor(Date.now() / 1000);
export const utf8Bytes = (s: string): number => Buffer.byteLength(s, "utf8");
export const isValidSha256Hex = (s: string): boolean => /^[0-9a-f]{64}$/.test(s);

export const escapeXml = (raw: string): string =>
  raw.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&apos;");

export const computeCardSha256 = (title: string, content: string): string =>
  crypto.createHash("sha256").update(`${utf8Bytes(title)}:${title}|${utf8Bytes(content)}:${content}`, "utf8").digest("hex");

function hasDisallowedCtrlOrBidi(text: string, allowTabNewlines: boolean): boolean {
  return Array.from(text).some((ch) => {
    const code = ch.codePointAt(0) ?? 0;
    const isLineWs = code === 9 || code === 10 || code === 13;
    const asciiCtrl = (code < 0x20 && !(allowTabNewlines && isLineWs)) || code === 0x7f || (code >= 0x80 && code <= 0x9f);
    const hidden =
      (code >= 0x200b && code <= 0x200f) ||
      (code >= 0x2028 && code <= 0x202e) ||
      (code >= 0x2066 && code <= 0x2069) ||
      code === 0xfeff ||
      (code >= 0xe0001 && code <= 0xe007f);
    return asciiCtrl || hidden;
  });
}

/** Validates scope identifiers (tenant_id, project_id, card.id, kid) for length and control characters. */
export function validateScopeSegment(label: string, value: string): string {
  const trimmed = value.trim();
  if (!trimmed || utf8Bytes(trimmed) > MAX_SCOPE_SEGMENT_BYTES) {
    throw new LearnError("InvalidCardInput", `${label} must be 1..=${MAX_SCOPE_SEGMENT_BYTES} UTF-8 bytes`);
  }
  if (hasDisallowedCtrlOrBidi(trimmed, false)) {
    throw new LearnError("InvalidCardInput", `${label} contains disallowed control or bidi characters`);
  }
  return trimmed;
}

/** Rejects raw or NFKC/line-separator-obfuscated YAML '---' breakout lines in text metadata. */
export function assertNoYamlBreakout(label: string, text: string): void {
  for (const candidate of [text, text.normalize("NFKC").replace(/[\u2010-\u2015\u2212\uFE58\uFE63]/g, "-")]) {
    if (candidate.replace(/\r\n/g, "\n").replace(/[\r\u2028\u2029]/g, "\n").split("\n").some((l) => l.trim() === "---")) {
      throw new LearnError("PromptInjectionBlocked", `YAML delimiter '---' line forbidden inside ${label}`);
    }
  }
}

/** Validates LEARN card text fields against UTF-8 caps, C0/C1/bidi controls, YAML breakouts, NFKC, and IPI. */
export function validateLearnCardField(label: string, value: string, maxBytes: number): string {
  const trimmed = value.trim();
  if (!trimmed) throw new LearnError("InvalidCardInput", `${label} must not be empty`);
  if (utf8Bytes(trimmed) > maxBytes) {
    throw new LearnError("InvalidCardInput", `${label} exceeds ${maxBytes} byte ceiling`);
  }
  if (hasDisallowedCtrlOrBidi(trimmed, true)) {
    throw new LearnError("InvalidCardInput", `${label} contains disallowed control or bidi characters`);
  }
  assertNoYamlBreakout(label, trimmed);
  try {
    scanSkillMarkdownSafety(label, trimmed);
  } catch (err) {
    throw new LearnError("PromptInjectionBlocked", err instanceof Error ? err.message : String(err));
  }
  return trimmed;
}
