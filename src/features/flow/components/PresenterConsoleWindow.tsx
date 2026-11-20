// @story 06.4
// @implements AC-01, AC-02, AC-04
import React from "react";
import { useTranslation } from "../../../hooks/useTranslation";
import {
  usePresenterWindowClient,
  type PresenterClientSyncedState,
} from "../hooks/usePresenterWindowClient";
import { PresenterCockpitView } from "./PresenterCockpitView";

export interface PresenterConsoleWindowProps {
  deckId: string;
  initialSnapshot?: PresenterClientSyncedState | null;
}

export const PresenterConsoleWindow: React.FC<PresenterConsoleWindowProps> = ({
  deckId,
  initialSnapshot = null,
}) => {
  const { t } = useTranslation();
  const client = usePresenterWindowClient(deckId);
  const state = client.syncedState ?? initialSnapshot;

  return (
    <main
      role="application"
      data-theme="night"
      data-testid="presenter-console-window"
      aria-busy={!state}
      aria-label={t("presentation.presenter.consoleAria", "Dual-Screen Presenter Console")}
      className="ff-presenter-window-shell"
    >
      <PresenterCockpitView
        variant="window"
        deckTitle={state?.deckTitle || t("presentation.presenter.consoleTitle", "Presenter View")}
        slides={state?.slides ?? []}
        activeSlideIndex={state?.activeSlideIndex ?? 0}
        currentSlideHtml={state?.currentSlideHtml}
        nextSlideHtml={state?.nextSlideHtml}
        blackoutMode={state?.blackoutMode ?? "none"}
        isAudienceFullscreen={Boolean(state?.isAudienceFullscreen)}
        isAuthenticated={client.isAuthenticated || Boolean(initialSnapshot)}
        transportMode={client.transportMode}
        syncLatencyMs={client.syncLatencyMs}
        isHistoricalReadOnly={Boolean(state?.isHistoricalReadOnly)}
        snapshotTag={state?.snapshotTag ?? null}
        elapsedMs={client.elapsedMs}
        isTimerRunning={Boolean(state?.timer.isRunning)}
        notesFontPx={client.notesFontPx}
        isShortcutsOpen={client.isShortcutsOpen}
        onToggleShortcuts={() => client.setIsShortcutsOpen((v) => !v)}
        onStepNotesFont={client.stepNotesFontPx}
        onPrevSlide={() => client.sendNav("prev")}
        onNextSlide={() => client.sendNav("next")}
        onToggleBlackout={client.toggleBlackout}
        onClearBlackout={client.clearBlackout}
        onToggleFullscreen={client.toggleFullscreen}
        onTimerCommand={client.sendTimer}
        onClose={client.closeWindow}
      />
    </main>
  );
};
