import { FileDiffItem } from "../../../lib/validations/change-proposal";

export {
  runValidationChecks,
  checkAstLint,
  hasTypeMismatch,
  hasSyntaxErrors,
  checkMissingRelativeImports,
  checkTypeAndBuild,
} from "./validation-checker";

export interface ParsedDiffLine {
  type: "added" | "deleted" | "context";
  content: string;
}

function determineFileStatus(origLen: number, newLen: number): "added" | "modified" | "deleted" {
  if (origLen === 0) return "added";
  if (newLen === 0) return "deleted";
  return "modified";
}

function buildLcsMatrix(a: string[], b: string[]): number[][] {
  const dp: number[][] = Array.from({ length: a.length + 1 }, () => new Array(b.length + 1).fill(0));
  for (let i = 0; i < a.length; i++) {
    for (let j = 0; j < b.length; j++) {
      if (a[i] === b[j]) {
        dp[i + 1][j + 1] = dp[i][j] + 1;
      } else {
        dp[i + 1][j + 1] = Math.max(dp[i + 1][j], dp[i][j + 1]);
      }
    }
  }
  return dp;
}

export function computeUnifiedDiff(
  filePath: string,
  originalContent: string,
  newContent: string
): FileDiffItem {
  const orig = originalContent ? originalContent.trimEnd().split("\n") : [];
  const updated = newContent ? newContent.trimEnd().split("\n") : [];
  const dp = buildLcsMatrix(orig, updated);

  let i = orig.length;
  let j = updated.length;
  let additions = 0;
  let deletions = 0;
  const lines: string[] = [];

  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && orig[i - 1] === updated[j - 1]) {
      lines.push(` ${orig[i - 1]}`);
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      lines.push(`+${updated[j - 1]}`);
      additions++;
      j--;
    } else if (i > 0) {
      lines.push(`-${orig[i - 1]}`);
      deletions++;
      i--;
    }
  }

  lines.reverse();
  const status = determineFileStatus(orig.length, updated.length);

  return {
    path: filePath,
    status,
    additions,
    deletions,
    patch: lines.join("\n"),
  };
}

export function parsePatchLines(patch: string): ParsedDiffLine[] {
  if (!patch) return [];
  return patch.split("\n").map((line) => {
    if (line.startsWith("+")) return { type: "added", content: line.slice(1) };
    if (line.startsWith("-")) return { type: "deleted", content: line.slice(1) };
    return { type: "context", content: line.startsWith(" ") ? line.slice(1) : line };
  });
}

export const sandboxedFilesStore = new Map<string, string>();

export function applyProposalFiles(files: FileDiffItem[]): Map<string, string> {
  for (const file of files) {
    if (file.status === "deleted") {
      sandboxedFilesStore.delete(file.path);
    } else {
      const content = file.patch
        .split("\n")
        .filter((l) => l.startsWith("+") || l.startsWith(" "))
        .map((l) => (l.startsWith("+") ? l.slice(1) : l.slice(1)))
        .join("\n");
      sandboxedFilesStore.set(file.path, content);
    }
  }
  return sandboxedFilesStore;
}