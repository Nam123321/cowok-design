// @story 04.1, 04.3, 04.5
// @implements AC-03 (3-Way Contextual Feedback & Attachment Tray)
import { useState, useCallback, useEffect } from "react";
import { StructuredCommentEnvelopeSchema } from "../../../lib/validations/element-comment";
import { MIN_SELECTION_LENGTH } from "../constants";

const MAX_QUOTE_LEN = 90;
const QUOTE_SLICE_LEN = 87;
const MAX_COMMENT_CHIP_LEN = 42;
const COMMENT_CHIP_SLICE_LEN = 39;
const TOOLTIP_RIGHT_MARGIN = 140;
const TOOLTIP_MIN_X = 12;
const TOOLTIP_OFFSET_X = 60;
const TOOLTIP_OFFSET_Y = 36;
const TOOLTIP_MIN_Y = 10;

export interface QuoteTooltipState {
  visible: boolean;
  x: number;
  y: number;
  text: string;
}

function resolveQuoteTooltipState(e: MouseEvent): QuoteTooltipState | null {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed) return null;
  const selectedText = selection.toString().trim();
  if (selectedText.length <= MIN_SELECTION_LENGTH) return null;
  const rect = selection.getRangeAt(0).getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return null;
  void e;
  return {
    visible: true,
    x: Math.min(window.innerWidth - TOOLTIP_RIGHT_MARGIN, Math.max(TOOLTIP_MIN_X, rect.right - TOOLTIP_OFFSET_X)),
    y: Math.max(TOOLTIP_MIN_Y, rect.top - TOOLTIP_OFFSET_Y),
    text: selectedText,
  };
}

function useWindowQuoteListener(setQuoteTooltip: React.Dispatch<React.SetStateAction<QuoteTooltipState | null>>) {
  useEffect(() => {
    const handleMouseUp = (e: MouseEvent) => {
      if ((e.target as HTMLElement).closest(".quote-floating-pill")) return;
      setQuoteTooltip(resolveQuoteTooltipState(e));
    };
    window.addEventListener("mouseup", handleMouseUp);
    return () => window.removeEventListener("mouseup", handleMouseUp);
  }, [setQuoteTooltip]);
}

export function useWorkbenchAttachments(projectId?: string) {
  const [attachmentChips, setAttachmentChips] = useState<string[]>([]);
  const [attachmentSnippets, setAttachmentSnippets] = useState<Record<string, string>>({});
  const [quoteTooltip, setQuoteTooltip] = useState<QuoteTooltipState | null>(null);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const onStructuredComment = (ev: Event) => {
      const raw = (ev as CustomEvent<unknown>).detail;
      const parsed = StructuredCommentEnvelopeSchema.safeParse(raw);
      if (!parsed.success) return;
      const env = parsed.data;
      if (env.projectId && projectId && env.projectId !== projectId) return;
      const rawBody = env.commentBody.trim().replace(/\s+/g, " ");
      const shortBody =
        rawBody.length > MAX_COMMENT_CHIP_LEN ? `${rawBody.slice(0, COMMENT_CHIP_SLICE_LEN)}...` : rawBody;
      const shortId = env.commentId.slice(-4);
      const scope =
        env.targetType === "spatial_region" && env.regionRect
          ? `${Math.round(env.regionRect.width)}×${Math.round(env.regionRect.height)}`
          : `#${env.elementFfId}`;
      const chip = `@[Comment ${scope} (${shortId}): "${shortBody}"]`;
      const payload = JSON.stringify({
        kind: "STRUCTURED_COMMENT_ENVELOPE",
        commentId: env.commentId,
        targetType: env.targetType,
        elementFfId: env.elementFfId,
        coveredFfIds: env.coveredFfIds,
        regionRect: env.regionRect,
        slideUid: env.slideUid,
        nodeId: env.nodeId,
        domHash: env.domHash,
        artifactSha256: env.artifactSha256,
        commentBody: env.commentBody,
      });
      setAttachmentChips((prev) => (prev.includes(chip) ? prev : [...prev, chip]));
      setAttachmentSnippets((prev) => ({ ...prev, [chip]: payload }));
    };
    window.addEventListener("cowok:structured-comment", onStructuredComment);
    return () => window.removeEventListener("cowok:structured-comment", onStructuredComment);
  }, [projectId]);

  const handleCommentOnNode = useCallback((nodeKeyOrChip: string) => {
    const trimmed = nodeKeyOrChip.trim();
    if (!trimmed) return;
    const chip = trimmed.startsWith("@[") || trimmed.startsWith("[") ? trimmed : `[Node ${trimmed}]`;
    setAttachmentChips((prev) => (prev.includes(chip) ? prev : [...prev, chip]));
  }, []);

  const handleRemoveAttachmentChip = useCallback((chip: string) => {
    setAttachmentChips((prev) => prev.filter((c) => c !== chip));
    setAttachmentSnippets((prev) => {
      if (!(chip in prev)) return prev;
      const next = { ...prev };
      delete next[chip];
      return next;
    });
  }, []);

  const handleAddQuoteChip = useCallback((rawText: string) => {
    const trimmed = rawText.trim();
    if (!trimmed) return;
    const truncated = trimmed.length > MAX_QUOTE_LEN ? `${trimmed.slice(0, QUOTE_SLICE_LEN)}...` : trimmed;
    const chip = `@[Quote: "${truncated}"]`;
    setAttachmentChips((prev) => (prev.includes(chip) ? prev : [...prev, chip]));
    setQuoteTooltip(null);
  }, []);

  const handleAddFileChips = useCallback((fileNames: string[], snippetsByName?: Record<string, string>) => {
    const cleanNames = fileNames.map((n) => n.trim()).filter(Boolean);
    if (cleanNames.length === 0) return;
    setAttachmentChips((prev) => {
      const next = [...prev];
      for (const name of cleanNames) {
        const chip = name.startsWith("@[") ? name : `@[File: ${name}]`;
        if (!next.includes(chip)) next.push(chip);
      }
      return next;
    });
    if (snippetsByName && Object.keys(snippetsByName).length > 0) {
      setAttachmentSnippets((prev) => {
        const next = { ...prev };
        for (const [name, snippet] of Object.entries(snippetsByName)) {
          const chip = name.startsWith("@[") ? name : `@[File: ${name}]`;
          if (snippet) next[chip] = snippet;
        }
        return next;
      });
    }
  }, []);

  const handleClearAttachments = useCallback(() => {
    setAttachmentChips([]);
    setAttachmentSnippets({});
  }, []);

  const handleDismissQuote = useCallback(() => setQuoteTooltip(null), []);
  useWindowQuoteListener(setQuoteTooltip);

  return {
    attachmentChips,
    attachmentSnippets,
    quoteTooltip,
    handleCommentOnNode,
    handleRemoveAttachmentChip,
    handleAddQuoteChip,
    handleAddFileChips,
    handleClearAttachments,
    handleDismissQuote,
  };
}
