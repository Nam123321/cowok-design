// @story 04.6
// @implements AC-04, AC-05 (Client-Side Pre-Persistence 3-Layer Secret Interceptor)
export type InterceptedProvider =
  | "google_ai_studio"
  | "typesafe_jev"
  | "anthropic"
  | "openai"
  | "custom";

export interface DetectedSecretEntry {
  provider: InterceptedProvider;
  rawSecretEnvelope: string;
}

export interface SecretInterceptResult {
  detected: boolean;
  provider: InterceptedProvider | null;
  rawSecretEnvelope: string | null;
  detectedSecrets: DetectedSecretEntry[];
  redactedPreview: string;
}

const TYPESAFE_TOKEN_PREFIX = ["jev", "_"].join("");

const PROVIDER_PATTERNS: ReadonlyArray<{
  provider: InterceptedProvider;
  regex: RegExp;
}> = [
  { provider: "google_ai_studio", regex: /\bAIza[0-9A-Za-z_-]{20,128}\b/g },
  { provider: "typesafe_jev", regex: new RegExp(`\\b${TYPESAFE_TOKEN_PREFIX}[0-9A-Za-z_-]{12,128}\\b`, "g") },
  { provider: "anthropic", regex: /\bsk-ant-[0-9A-Za-z_-]{16,180}\b/g },
  { provider: "openai", regex: /\bsk-[0-9A-Za-z_-]{16,180}\b/g },
];

interface SecretMatchSpan {
  start: number;
  end: number;
  provider: InterceptedProvider;
  raw: string;
}

function collectSecretSpans(input: string): SecretMatchSpan[] {
  const spans: SecretMatchSpan[] = [];
  for (const entry of PROVIDER_PATTERNS) {
    entry.regex.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = entry.regex.exec(input)) !== null) {
      const start = m.index;
      const end = start + m[0].length;
      const overlaps = spans.some((s) => start < s.end && end > s.start);
      if (!overlaps) spans.push({ start, end, provider: entry.provider, raw: m[0] });
    }
  }
  return spans.sort((a, b) => a.start - b.start);
}

function buildRedactedAndEntries(input: string, spans: SecretMatchSpan[]) {
  let redacted = "";
  let cursor = 0;
  const seen = new Set<string>();
  const detectedSecrets: DetectedSecretEntry[] = [];
  for (const sp of spans) {
    redacted += input.slice(cursor, sp.start) + `[VAULTED:${sp.provider}]`;
    cursor = sp.end;
    if (!seen.has(sp.raw)) {
      seen.add(sp.raw);
      detectedSecrets.push({ provider: sp.provider, rawSecretEnvelope: sp.raw });
    }
  }
  return { redactedPreview: redacted + input.slice(cursor), detectedSecrets };
}

export function inspectChatSecretCandidate(rawText: string): SecretInterceptResult {
  const input = rawText || "";
  const spans = collectSecretSpans(input);
  if (spans.length === 0) {
    return {
      detected: false,
      provider: null,
      rawSecretEnvelope: null,
      detectedSecrets: [],
      redactedPreview: input,
    };
  }
  const { redactedPreview, detectedSecrets } = buildRedactedAndEntries(input, spans);
  return {
    detected: true,
    provider: detectedSecrets[0].provider,
    rawSecretEnvelope: detectedSecrets[0].rawSecretEnvelope,
    detectedSecrets,
    redactedPreview,
  };
}
