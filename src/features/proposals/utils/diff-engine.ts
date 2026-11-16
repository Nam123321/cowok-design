import { FileDiffItem, VisualDiffResult } from "../../../lib/validations/change-proposal";

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

export interface ElementRectLike {
  left: number;
  top: number;
  width: number;
  height: number;
}

export type PopoverPlacement = "bottom" | "top" | "right" | "left";

export interface PopoverGeometry {
  placement: PopoverPlacement;
  left: number;
  top: number;
  maxHeight: number;
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

export function computeProposalDiffStats(files: FileDiffItem[]): {
  fileCount: number;
  additions: number;
  deletions: number;
} {
  let additions = 0;
  let deletions = 0;
  for (const f of files) {
    additions += f.additions;
    deletions += f.deletions;
  }
  return { fileCount: files.length, additions, deletions };
}

const PERCENT_SCALE_FACTOR = 10000;
const PERCENT_ROUND_DIVISOR = 100;

export function formatVisualDiffMetrics(diff: VisualDiffResult | null | undefined): {
  ssimValue: number | null;
  deltaPercent: number | null;
  isComputed: boolean;
} {
  if (!diff || diff.status !== "COMPUTED") {
    return { ssimValue: null, deltaPercent: null, isComputed: false };
  }
  return {
    ssimValue: diff.ssim,
    deltaPercent: Math.round(diff.mismatchRatio * PERCENT_SCALE_FACTOR) / PERCENT_ROUND_DIVISOR,
    isComputed: true,
  };
}

const EDITABLE_TAGS = new Set(["INPUT", "TEXTAREA", "SELECT"]);

export interface HotkeyGuardEvent {
  isComposing?: boolean;
  metaKey?: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
  target?: EventTarget | null;
}

export function shouldSuppressProposalHotkey(
  event: HotkeyGuardEvent,
  options: { isModalOpen?: boolean; isInlineEditorOpen?: boolean } = {}
): boolean {
  if (event.isComposing || event.metaKey || event.ctrlKey || event.altKey) {
    return true;
  }
  if (options.isModalOpen || options.isInlineEditorOpen) {
    return true;
  }
  const target = event.target as {
    tagName?: string;
    isContentEditable?: boolean;
    getAttribute?: (name: string) => string | null;
    closest?: (selector: string) => Element | null;
  } | null;

  if (!target) return false;
  const tag = (target.tagName || "").toUpperCase();
  if (EDITABLE_TAGS.has(tag)) return true;
  if (target.isContentEditable) return true;
  const ceAttr = target.getAttribute?.("contenteditable");
  if (ceAttr === "true" || ceAttr === "") return true;
  if (
    typeof target.closest === "function" &&
    target.closest('[contenteditable="true"], [contenteditable=""], [role="dialog"], [aria-modal="true"], [data-inline-code-editor="true"]')
  ) {
    return true;
  }
  return false;
}

export function resolveIframeElementStageRects(
  rawRects: Record<string, ElementRectLike>,
  iframeOffset: { left: number; top: number } = { left: 0, top: 0 },
  zoom = 1
): Record<string, ElementRectLike> {
  const scale = Number.isFinite(zoom) && zoom > 0 ? zoom : 1;
  const resolved: Record<string, ElementRectLike> = {};
  for (const [ffId, r] of Object.entries(rawRects)) {
    resolved[ffId] = {
      left: r.left * scale + iframeOffset.left,
      top: r.top * scale + iframeOffset.top,
      width: r.width * scale,
      height: r.height * scale,
    };
  }
  return resolved;
}

const DEFAULT_POPOVER_WIDTH = 288;
const DEFAULT_POPOVER_HEIGHT = 220;
const MIN_POPOVER_MAX_HEIGHT = 120;
const POPOVER_MARGIN = 8;

function clampCoord(val: number, min: number, max: number): number {
  return Math.min(Math.max(val, min), max);
}

function buildPlacementCandidates(
  anchorRect: ElementRectLike,
  maxLeft: number,
  maxTop: number,
  popoverSize: { width: number; height: number }
): Array<{ placement: PopoverPlacement; left: number; top: number }> {
  const gap = POPOVER_MARGIN;
  return [
    { placement: "bottom", left: clampCoord(anchorRect.left, POPOVER_MARGIN, maxLeft), top: anchorRect.top + anchorRect.height + gap },
    { placement: "top", left: clampCoord(anchorRect.left, POPOVER_MARGIN, maxLeft), top: anchorRect.top - popoverSize.height - gap },
    { placement: "right", left: anchorRect.left + anchorRect.width + gap, top: clampCoord(anchorRect.top, POPOVER_MARGIN, maxTop) },
    { placement: "left", left: anchorRect.left - popoverSize.width - gap, top: clampCoord(anchorRect.top, POPOVER_MARGIN, maxTop) },
  ];
}

export function computeAutoFlipPopoverPosition(
  anchorRect: ElementRectLike,
  container: { width: number; height: number; drawerWidth?: number },
  popoverSize: { width: number; height: number } = { width: DEFAULT_POPOVER_WIDTH, height: DEFAULT_POPOVER_HEIGHT }
): PopoverGeometry {
  const usableRight = Math.max(popoverSize.width + POPOVER_MARGIN * 2, container.width - (container.drawerWidth ?? 0));
  const maxLeft = Math.max(POPOVER_MARGIN, usableRight - popoverSize.width - POPOVER_MARGIN);
  const maxTop = Math.max(POPOVER_MARGIN, container.height - popoverSize.height - POPOVER_MARGIN);
  const candidates = buildPlacementCandidates(anchorRect, maxLeft, maxTop, popoverSize);

  for (const cand of candidates) {
    const fitsX = cand.left >= POPOVER_MARGIN && cand.left + popoverSize.width <= usableRight - POPOVER_MARGIN;
    const fitsY = cand.top >= POPOVER_MARGIN && cand.top + popoverSize.height <= container.height - POPOVER_MARGIN;
    if (fitsX && fitsY) {
      return {
        placement: cand.placement,
        left: Math.round(cand.left),
        top: Math.round(cand.top),
        maxHeight: Math.max(MIN_POPOVER_MAX_HEIGHT, container.height - Math.round(cand.top) - POPOVER_MARGIN),
      };
    }
  }

  const fallbackLeft = clampCoord(anchorRect.left, POPOVER_MARGIN, maxLeft);
  const fallbackTop = clampCoord(anchorRect.top + anchorRect.height + POPOVER_MARGIN, POPOVER_MARGIN, maxTop);
  return {
    placement: fallbackLeft < anchorRect.left ? "left" : "bottom",
    left: Math.round(fallbackLeft),
    top: Math.round(fallbackTop),
    maxHeight: Math.max(MIN_POPOVER_MAX_HEIGHT, container.height - Math.round(fallbackTop) - POPOVER_MARGIN),
  };
}

export type DesignerTalkHotkeyAction =
  | { type: "close" }
  | { type: "step"; delta: 1 | -1 }
  | { type: "select_index"; badgeIndex: number }
  | { type: "decide"; decision: "agreed" | "disagreed_instead" | "skipped" };

export function resolveDesignerTalkHotkeyAction(key: string): DesignerTalkHotkeyAction | null {
  const lower = key.toLowerCase();
  if (lower === "escape") return { type: "close" };
  if (lower === "j" || key === "ArrowDown") return { type: "step", delta: 1 };
  if (lower === "k" || key === "ArrowUp") return { type: "step", delta: -1 };
  if (/^[1-9]$/.test(lower)) return { type: "select_index", badgeIndex: Number(lower) };
  if (lower === "a") return { type: "decide", decision: "agreed" };
  if (lower === "d") return { type: "decide", decision: "disagreed_instead" };
  if (lower === "s") return { type: "decide", decision: "skipped" };
  return null;
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
