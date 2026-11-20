// @story 06.4
// @implements AC-03
import React, { useState } from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type {
  PresenterBlackoutMode,
  PresenterSlideSnapshotDto,
  PresenterTransportMode,
} from "../../../lib/validations/presentation-presenter";
import { PresenterCockpitView } from "./PresenterCockpitView";

const DEFAULT_INLINE_NOTES_FONT_PX = 18;
const MIN_INLINE_NOTES_FONT_PX = 15;
const MAX_INLINE_NOTES_FONT_PX = 26;

export interface PresenterInlineSplitFallbackProps {
  deckTitle: string;
  slides: PresenterSlideSnapshotDto[];
  activeSlideIndex: number;
  currentSlideHtml?: string;
  nextSlideHtml?: string | null;
  blackoutMode: PresenterBlackoutMode;
  isAudienceFullscreen: boolean;
  transportMode: PresenterTransportMode;
  syncLatencyMs: number | null;
  isHistoricalReadOnly: boolean;
  snapshotTag: string | null;
  elapsedMs: number;
  isTimerRunning: boolean;
  onPrevSlide: () => void;
  onNextSlide: () => void;
  onToggleBlackout: (mode: "black" | "white", trigger: "button") => void;
  onClearBlackout: () => void;
  onToggleFullscreen: () => void;
  onTimerCommand: (action: "start" | "pause" | "reset") => void;
  onReopenWindow: () => void;
  onClose: () => void;
}

export const PresenterInlineSplitFallback: React.FC<PresenterInlineSplitFallbackProps> = (p) => {
  const { t } = useTranslation();
  const [notesFontPx, setNotesFontPx] = useState(DEFAULT_INLINE_NOTES_FONT_PX);
  const [isShortcutsOpen, setIsShortcutsOpen] = useState(false);

  return (
    <aside
      role="region"
      data-theme="night"
      data-testid="presenter-inline-fallback"
      aria-label={t("presentation.presenter.inlineFallbackAria", "Inline Presenter Fallback")}
      className="ff-presenter-inline-fallback"
    >
      <PresenterCockpitView
        variant="inline"
        deckTitle={p.deckTitle}
        slides={p.slides}
        activeSlideIndex={p.activeSlideIndex}
        currentSlideHtml={p.currentSlideHtml}
        nextSlideHtml={p.nextSlideHtml}
        blackoutMode={p.blackoutMode}
        isAudienceFullscreen={p.isAudienceFullscreen}
        isAuthenticated={true}
        transportMode={p.transportMode}
        syncLatencyMs={p.syncLatencyMs}
        isHistoricalReadOnly={p.isHistoricalReadOnly}
        snapshotTag={p.snapshotTag}
        elapsedMs={p.elapsedMs}
        isTimerRunning={p.isTimerRunning}
        notesFontPx={notesFontPx}
        isShortcutsOpen={isShortcutsOpen}
        onToggleShortcuts={() => setIsShortcutsOpen((v) => !v)}
        onStepNotesFont={(delta) =>
          setNotesFontPx((prev) => Math.max(MIN_INLINE_NOTES_FONT_PX, Math.min(MAX_INLINE_NOTES_FONT_PX, prev + delta)))
        }
        onPrevSlide={p.onPrevSlide}
        onNextSlide={p.onNextSlide}
        onToggleBlackout={p.onToggleBlackout}
        onClearBlackout={p.onClearBlackout}
        onToggleFullscreen={p.onToggleFullscreen}
        onTimerCommand={p.onTimerCommand}
        onReopenWindow={p.onReopenWindow}
        onClose={p.onClose}
      />
    </aside>
  );
};
