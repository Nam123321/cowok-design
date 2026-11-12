// @story 04.1, 04.3
// @implements AC-04 (3-Way Contextual Feedback & Attachment Tray)
import { useState, useCallback, useEffect } from "react";
import { MIN_SELECTION_LENGTH } from "../constants";

export interface QuoteTooltipState {
  visible: boolean;
  x: number;
  y: number;
  text: string;
}

export function useWorkbenchAttachments() {
  const [attachmentChips, setAttachmentChips] = useState<string[]>([]);
  const [quoteTooltip, setQuoteTooltip] = useState<QuoteTooltipState | null>(null);

  const handleCommentOnNode = useCallback((nodeKey: string) => {
    const chip = `[Node ${nodeKey}]`;
    setAttachmentChips((prev) => (prev.includes(chip) ? prev : [...prev, chip]));
  }, []);

  const handleRemoveAttachmentChip = useCallback((chip: string) => {
    setAttachmentChips((prev) => prev.filter((c) => c !== chip));
  }, []);

  const handleAddQuoteChip = useCallback((rawText: string) => {
    const trimmed = rawText.trim();
    if (!trimmed) return;
    const truncated = trimmed.length > 90 ? `${trimmed.slice(0, 87)}...` : trimmed;
    const chip = `@[Quote: "${truncated}"]`;
    setAttachmentChips((prev) => (prev.includes(chip) ? prev : [...prev, chip]));
    setQuoteTooltip(null);
  }, []);

  const handleDismissQuote = useCallback(() => {
    setQuoteTooltip(null);
  }, []);

  useEffect(() => {
    const handleMouseUp = (e: MouseEvent) => {
      // If clicking inside the quote tooltip itself, ignore
      if ((e.target as HTMLElement).closest(".quote-floating-pill")) return;

      const selection = window.getSelection();
      if (!selection || selection.isCollapsed) {
        setQuoteTooltip(null);
        return;
      }

      const selectedText = selection.toString().trim();
      if (selectedText.length > MIN_SELECTION_LENGTH) {
        const range = selection.getRangeAt(0);
        const rect = range.getBoundingClientRect();
        if (rect.width > 0 && rect.height > 0) {
          setQuoteTooltip({
            visible: true,
            x: Math.min(window.innerWidth - 140, Math.max(12, rect.right - 60)),
            y: Math.max(10, rect.top - 36),
            text: selectedText,
          });
          return;
        }
      }
      setQuoteTooltip(null);
    };

    window.addEventListener("mouseup", handleMouseUp);
    return () => window.removeEventListener("mouseup", handleMouseUp);
  }, []);

  return {
    attachmentChips,
    quoteTooltip,
    handleCommentOnNode,
    handleRemoveAttachmentChip,
    handleAddQuoteChip,
    handleDismissQuote,
  };
}
