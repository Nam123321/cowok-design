// @story 06.4
// @implements AC-01, AC-02, AC-03, AC-04
import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import type {
  PresenterBlackoutMode,
  PresenterSlideSnapshotDto,
  PresenterTransportMode,
} from "../../../lib/validations/presentation-presenter";
import {
  PresenterBlackoutBanner,
  PresenterCurrentSlideCard,
  PresenterRightCockpitColumn,
  PresenterShortcutsPopover,
  PresenterSlideControlsBar,
  PresenterTopBar,
} from "./PresenterCockpitCards";

export interface PresenterCockpitViewProps {
  variant: "window" | "inline";
  deckTitle: string;
  slides: PresenterSlideSnapshotDto[];
  activeSlideIndex: number;
  currentSlideHtml?: string;
  nextSlideHtml?: string | null;
  blackoutMode: PresenterBlackoutMode;
  isAudienceFullscreen: boolean;
  isAuthenticated: boolean;
  transportMode: PresenterTransportMode;
  syncLatencyMs: number | null;
  isHistoricalReadOnly: boolean;
  snapshotTag: string | null;
  elapsedMs: number;
  isTimerRunning: boolean;
  notesFontPx: number;
  isShortcutsOpen: boolean;
  onToggleShortcuts: () => void;
  onStepNotesFont: (delta: number) => void;
  onPrevSlide: () => void;
  onNextSlide: () => void;
  onToggleBlackout: (mode: "black" | "white", trigger: "button") => void;
  onClearBlackout: () => void;
  onToggleFullscreen: () => void;
  onTimerCommand: (action: "start" | "pause" | "reset") => void;
  onReopenWindow?: () => void;
  onClose: () => void;
}

export const PresenterCockpitView: React.FC<PresenterCockpitViewProps> = (p) => {
  const { t } = useTranslation();
  const currentSlide = p.slides[p.activeSlideIndex] || p.slides[0] || null;
  const nextSlide = p.slides[p.activeSlideIndex + 1] || null;
  const variantClass = p.variant === "inline" ? "ff-presenter-cockpit--inline" : "ff-presenter-cockpit--window";

  return (
    <div data-theme="night" data-presenter-variant={p.variant} aria-label={t("presentation.presenter.consoleAria", "Dual-Screen Presenter Console")} className={`ff-presenter-cockpit ${variantClass}`}>
      <PresenterTopBar
        deckTitle={p.deckTitle} activeSlideIndex={p.activeSlideIndex} slidesCount={p.slides.length}
        isAuthenticated={p.isAuthenticated} transportMode={p.transportMode} syncLatencyMs={p.syncLatencyMs}
        isHistoricalReadOnly={p.isHistoricalReadOnly} snapshotTag={p.snapshotTag}
        isShortcutsOpen={p.isShortcutsOpen} onToggleShortcuts={p.onToggleShortcuts}
        onReopenWindow={p.onReopenWindow} onClose={p.onClose}
      />
      <PresenterShortcutsPopover isOpen={p.isShortcutsOpen} />
      {p.variant === "inline" && (
        <>
          <PresenterBlackoutBanner blackoutMode={p.blackoutMode} isHistoricalReadOnly={p.isHistoricalReadOnly} onClearBlackout={p.onClearBlackout} />
          <PresenterSlideControlsBar
            activeSlideIndex={p.activeSlideIndex} slidesCount={p.slides.length}
            blackoutMode={p.blackoutMode} isAudienceFullscreen={p.isAudienceFullscreen}
            isHistoricalReadOnly={p.isHistoricalReadOnly} onPrev={p.onPrevSlide}
            onNext={p.onNextSlide} onToggleBlackout={p.onToggleBlackout} onToggleFullscreen={p.onToggleFullscreen}
          />
        </>
      )}
      <div className="ff-presenter-grid">
        {p.variant === "window" && (
          <PresenterCurrentSlideCard
            slide={currentSlide} slideHtml={p.currentSlideHtml} activeSlideIndex={p.activeSlideIndex} slidesCount={p.slides.length}
            blackoutMode={p.blackoutMode} isAudienceFullscreen={p.isAudienceFullscreen} isHistoricalReadOnly={p.isHistoricalReadOnly}
            onPrev={p.onPrevSlide} onNext={p.onNextSlide} onToggleBlackout={p.onToggleBlackout}
            onClearBlackout={p.onClearBlackout} onToggleFullscreen={p.onToggleFullscreen}
          />
        )}
        <PresenterRightCockpitColumn
          nextSlide={nextSlide} nextSlideHtml={p.nextSlideHtml} elapsedMs={p.elapsedMs}
          isTimerRunning={p.isTimerRunning} speakerNotes={currentSlide?.speakerNotes ?? ""}
          notesFontPx={p.notesFontPx} onStepNotesFont={p.onStepNotesFont} onTimerCommand={p.onTimerCommand}
        />
      </div>
    </div>
  );
};
