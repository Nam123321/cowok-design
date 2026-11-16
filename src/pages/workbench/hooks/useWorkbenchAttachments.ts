// @story 04.1, 04.3, 04.5
// @implements AC-03 (3-Way Contextual Feedback & Attachment Tray)
import { useState, useCallback, useEffect } from "react";
import { MIN_SELECTION_LENGTH } from "../constants";

const MAX_QUOTE_LEN = 90;
const QUOTE_SLICE_LEN = 87;
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

export function useWorkbenchAttachments() {
  const [attachmentChips, setAttachmentChips] = useState<string[]>([]);
  const [quoteTooltip, setQuoteTooltip] = useState<QuoteTooltipState | null>(null);

  const handleCommentOnNode = useCallback((nodeKeyOrChip: string) => {
    const trimmed = nodeKeyOrChip.trim();
    if (!trimmed) return;
    const chip = trimmed.startsWith("@[") || trimmed.startsWith("[") ? trimmed : `[Node ${trimmed}]`;
    setAttachmentChips((prev) => (prev.includes(chip) ? prev : [...prev, chip]));
  }, []);

  const handleRemoveAttachmentChip = useCallback((chip: string) => {
    setAttachmentChips((prev) => prev.filter((c) => c !== chip));
  }, []);

  const handleAddQuoteChip = useCallback((rawText: string) => {
    const trimmed = rawText.trim();
    if (!trimmed) return;
    const truncated = trimmed.length > MAX_QUOTE_LEN ? `${trimmed.slice(0, QUOTE_SLICE_LEN)}...` : trimmed;
    const chip = `@[Quote: "${truncated}"]`;
    setAttachmentChips((prev) => (prev.includes(chip) ? prev : [...prev, chip]));
    setQuoteTooltip(null);
  }, []);

  const handleDismissQuote = useCallback(() => setQuoteTooltip(null), []);
  useWindowQuoteListener(setQuoteTooltip);

  return { attachmentChips, quoteTooltip, handleCommentOnNode, handleRemoveAttachmentChip, handleAddQuoteChip, handleDismissQuote };
}
