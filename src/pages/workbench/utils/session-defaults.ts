// @story 04.1, 04.2, 06.1
// @implements AC-01, AC-06 (4-5 Digit Project-Scoped Session Alias & Path Basename Extraction)

const DEFAULT_MAX_ATTEMPTS = 10000;
const GIT_SUFFIX_LEN = 4;
const SESSION_SEQ_PAD_DIGITS = 4;

export function extractFolderBasename(rawPathOrUrl: string): string {
  const trimmed = rawPathOrUrl.trim().replace(/[\\/]+$/, "");
  if (!trimmed) return "";
  const withoutGitSuffix = trimmed.toLowerCase().endsWith(".git")
    ? trimmed.slice(0, -GIT_SUFFIX_LEN)
    : trimmed;
  const segments = withoutGitSuffix.split(/[\\/]+/).filter(Boolean);
  return segments[segments.length - 1] ?? "";
}

export function formatSessionSequenceSuffix(seq: number): string {
  return String(Math.max(1, seq)).padStart(SESSION_SEQ_PAD_DIGITS, "0");
}

export function generateUniqueSessionTitle(opts: {
  projectName?: string;
  existingTitles: Iterable<string>;
  existingCount?: number;
  fallbackPrefix?: string;
}): string {
  const base = opts.projectName?.trim() || opts.fallbackPrefix?.trim() || "Session";
  const normalizedTaken = new Set<string>();
  let totalTakenCount = 0;
  for (const title of opts.existingTitles) {
    const clean = title.trim().toLowerCase();
    if (clean) {
      normalizedTaken.add(clean);
      totalTakenCount += 1;
    }
  }
  const startSeq = Math.max(1, (opts.existingCount ?? totalTakenCount) + 1);
  for (let offset = 0; offset < DEFAULT_MAX_ATTEMPTS; offset += 1) {
    const candidate = `${base} #${formatSessionSequenceSuffix(startSeq + offset)}`;
    if (!normalizedTaken.has(candidate.toLowerCase())) {
      return candidate;
    }
  }
  return `${base} #${formatSessionSequenceSuffix(startSeq + DEFAULT_MAX_ATTEMPTS)}`;
}
