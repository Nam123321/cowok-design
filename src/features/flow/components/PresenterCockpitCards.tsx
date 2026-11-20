// @story 06.4
// @implements AC-01, AC-02, AC-03, AC-04
import React, { useMemo } from "react";
import { BrandIcon } from "../../../components/ui/BrandIcon";
import { useFormatter } from "../../../hooks/useFormatter";
import { useTranslation } from "../../../hooks/useTranslation";
import type {
  PresenterBlackoutMode,
  PresenterSlideSnapshotDto,
  PresenterTransportMode,
} from "../../../lib/validations/presentation-presenter";
import { IFRAME_SANDBOX_PERMISSIONS, wrapUserHtmlSrcDoc } from "../utils/html-sandbox-wrapper";
import { formatStopwatchMmSs } from "../utils/presenter-timer";

const ICON_SM = 12;
const ICON_MD = 14;
const ICON_STROKE = 1.85;
const NOTES_FONT_STEP = 2;

export interface PresenterHeaderProps {
  deckTitle: string;
  activeSlideIndex: number;
  slidesCount: number;
  isAuthenticated: boolean;
  transportMode: PresenterTransportMode;
  syncLatencyMs: number | null;
  isHistoricalReadOnly: boolean;
  snapshotTag: string | null;
  isShortcutsOpen: boolean;
  onToggleShortcuts: () => void;
  onReopenWindow?: () => void;
  onClose: () => void;
}

const TRANSPORT_LABEL_KEYS: Record<PresenterTransportMode, [string, string]> = {
  broadcast_channel: ["presentation.presenter.transportBroadcast", "BroadcastChannel"],
  tauri_ipc: ["presentation.presenter.transportTauri", "Tauri IPC"],
  inline_fallback: ["presentation.presenter.transportInline", "Inline Split"],
};

export const PresenterTopBar: React.FC<PresenterHeaderProps> = (p) => {
  const { t, locale } = useTranslation();
  const { formatNumber } = useFormatter({ locale });
  const [trKey, trFallback] = TRANSPORT_LABEL_KEYS[p.transportMode];
  const slideTotal = Math.max(1, p.slidesCount);
  const slidePos = Math.min(slideTotal, p.activeSlideIndex + 1);
  return (
    <header className="ff-presenter-topbar">
      <div className="ff-presenter-topbar__title-group">
        <BrandIcon name="monitor" size={ICON_MD} strokeWidth={ICON_STROKE} color="var(--human-accent)" />
        <span className="ff-presenter-topbar__title">{p.deckTitle}</span>
        <span className="ff-metric-line" data-testid="presenter-metric-line">
          <span className={`ff-presenter-dot ${p.isAuthenticated ? "ff-presenter-dot--live" : "ff-presenter-dot--sync"}`} />
          <span>{p.isAuthenticated ? t("presentation.presenter.synced", "Synced") : t("presentation.presenter.syncingState", "Syncing...")}</span>
          <span>{"•"}</span>
          <span>{`${formatNumber(slidePos)} / ${formatNumber(slideTotal)}`}</span>
          <span>{"•"}</span>
          <span>{t(trKey, trFallback)}</span>
          {typeof p.syncLatencyMs === "number" && <span>{`(${formatNumber(p.syncLatencyMs)} ms)`}</span>}
        </span>
        {p.isHistoricalReadOnly && (
          <span className="ff-presenter-snap-pill" data-testid="presenter-snap-readonly-badge">
            <BrandIcon name="lock" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
            <span>{p.snapshotTag || t("presentation.presenter.historicalReadOnlyBadge", "SNAP-* Read-Only")}</span>
          </span>
        )}
      </div>
      <div className="ff-presenter-topbar__actions">
        {p.onReopenWindow && (
          <button type="button" data-testid="btn-reopen-presenter" onClick={p.onReopenWindow} className="ff-presenter-btn ff-presenter-btn--primary">
            <BrandIcon name="external-link" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
            <span>{t("presentation.presenter.reopenWindow", "Reopen Presenter Window")}</span>
          </button>
        )}
        <button type="button" data-testid="btn-presenter-shortcuts" aria-expanded={p.isShortcutsOpen} onClick={p.onToggleShortcuts} className="ff-presenter-btn" title={t("presentation.presenter.shortcutsTitle", "Keyboard Shortcuts")}>
          <BrandIcon name="help-circle" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
        </button>
        <button type="button" data-testid="btn-close-presenter" onClick={p.onClose} className="ff-presenter-btn" aria-label={t("presentation.presenter.closeConsole", "Close Presenter")}>
          <BrandIcon name="x" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
        </button>
      </div>
    </header>
  );
};

export const PresenterShortcutsPopover: React.FC<{ isOpen: boolean }> = ({ isOpen }) => {
  const { t } = useTranslation();
  if (!isOpen) return null;
  return (
    <div role="dialog" data-testid="presenter-shortcuts-popover" className="ff-presenter-shortcuts-popover">
      <div className="ff-presenter-shortcuts-popover__row"><span>{"← / → / Space"}</span><span>{t("presentation.presenter.shortcutNav", "Prev / Next Slide")}</span></div>
      <div className="ff-presenter-shortcuts-popover__row"><span>{"B / W"}</span><span>{t("presentation.presenter.shortcutBlackout", "Blackout / Whiteout")}</span></div>
      <div className="ff-presenter-shortcuts-popover__row"><span>{"F"}</span><span>{t("presentation.presenter.shortcutFullscreen", "Audience Fullscreen")}</span></div>
      <div className="ff-presenter-shortcuts-popover__row"><span>{"P"}</span><span>{t("presentation.presenter.shortcutTimer", "Start / Pause Timer")}</span></div>
      <div className="ff-presenter-shortcuts-popover__row"><span>{"Esc"}</span><span>{t("presentation.presenter.shortcutEscape", "Close Presenter Only")}</span></div>
    </div>
  );
};

export const PresenterBlackoutBanner: React.FC<{
  blackoutMode: PresenterBlackoutMode;
  isHistoricalReadOnly?: boolean;
  onClearBlackout: () => void;
}> = ({ blackoutMode, isHistoricalReadOnly, onClearBlackout }) => {
  const { t } = useTranslation();
  if (blackoutMode === "none") return null;
  return (
    <div role="status" aria-live="assertive" data-testid="presenter-blackout-status-bar" className="ff-presenter-blackout-banner">
      <span>
        {blackoutMode === "black"
          ? t("presentation.presenter.blackoutActiveBanner", "BLACKOUT ACTIVE (B) — Audience screen hidden")
          : t("presentation.presenter.whiteoutActiveBanner", "WHITEOUT ACTIVE (W) — Audience screen white")}
      </span>
      <button type="button" data-testid="btn-resume-projection" disabled={Boolean(isHistoricalReadOnly)} onClick={onClearBlackout} className="ff-presenter-btn ff-presenter-btn--primary">
        {t("presentation.presenter.resumeProjection", "Resume Projection")}
      </button>
    </div>
  );
};

export const PresenterSlideControlsBar: React.FC<{
  activeSlideIndex: number;
  slidesCount: number;
  blackoutMode: PresenterBlackoutMode;
  isAudienceFullscreen: boolean;
  isHistoricalReadOnly?: boolean;
  onPrev: () => void;
  onNext: () => void;
  onToggleBlackout: (mode: "black" | "white", trigger: "button") => void;
  onToggleFullscreen: () => void;
}> = (p) => {
  const { t } = useTranslation();
  const hasPrev = p.activeSlideIndex > 0 && !p.isHistoricalReadOnly;
  const hasNext = p.activeSlideIndex < Math.max(0, p.slidesCount - 1) && !p.isHistoricalReadOnly;
  const locked = Boolean(p.isHistoricalReadOnly);
  return (
    <div className="ff-presenter-controls-bar">
      <div className="ff-presenter-controls-group">
        <button type="button" data-testid="btn-presenter-prev" disabled={!hasPrev} onClick={p.onPrev} className="ff-presenter-btn">
          <BrandIcon name="arrow-left" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
          <span>{t("presentation.presenter.prevSlide", "Prev")}</span>
        </button>
        <button type="button" data-testid="btn-presenter-next" disabled={!hasNext} onClick={p.onNext} className="ff-presenter-btn ff-presenter-btn--primary">
          <span>{t("presentation.presenter.nextSlide", "Next")}</span>
          <BrandIcon name="chevron-right" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
        </button>
      </div>
      <div className="ff-presenter-controls-group">
        <button type="button" data-testid="btn-presenter-blackout" disabled={locked} aria-pressed={p.blackoutMode === "black"} onClick={() => p.onToggleBlackout("black", "button")} className={`ff-presenter-btn ${p.blackoutMode === "black" ? "ff-presenter-btn--warning" : ""}`}>
          <BrandIcon name="eye-off" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
          <span>{t("presentation.presenter.blackoutBtn", "Black (B)")}</span>
        </button>
        <button type="button" data-testid="btn-presenter-whiteout" disabled={locked} aria-pressed={p.blackoutMode === "white"} onClick={() => p.onToggleBlackout("white", "button")} className={`ff-presenter-btn ${p.blackoutMode === "white" ? "ff-presenter-btn--warning" : ""}`}>
          <BrandIcon name="sun" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
          <span>{t("presentation.presenter.whiteoutBtn", "White (W)")}</span>
        </button>
        <button type="button" data-testid="btn-presenter-fullscreen" disabled={locked} aria-pressed={p.isAudienceFullscreen} onClick={p.onToggleFullscreen} className="ff-presenter-btn">
          <BrandIcon name={p.isAudienceFullscreen ? "minimize" : "maximize"} size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
          <span>{t("presentation.presenter.fullscreenBtn", "Fullscreen (F)")}</span>
        </button>
      </div>
    </div>
  );
};

export const PresenterCurrentSlideCard: React.FC<{
  slide: PresenterSlideSnapshotDto | null;
  slideHtml?: string;
  activeSlideIndex: number;
  slidesCount: number;
  blackoutMode: PresenterBlackoutMode;
  isAudienceFullscreen: boolean;
  isHistoricalReadOnly?: boolean;
  onPrev: () => void;
  onNext: () => void;
  onToggleBlackout: (mode: "black" | "white", trigger: "button") => void;
  onClearBlackout: () => void;
  onToggleFullscreen: () => void;
}> = (p) => {
  const { t } = useTranslation();
  const safeSlideSrcDoc = useMemo(() => (p.slideHtml ? wrapUserHtmlSrcDoc(p.slideHtml) : undefined), [p.slideHtml]);
  return (
    <section className="ff-presenter-stage-col">
      <PresenterBlackoutBanner blackoutMode={p.blackoutMode} isHistoricalReadOnly={p.isHistoricalReadOnly} onClearBlackout={p.onClearBlackout} />
      <div className="ff-presenter-slide-frame" data-testid="presenter-current-slide-frame">
        {safeSlideSrcDoc ? (
          <iframe title={t("presentation.presenter.currentSlideFrameTitle", "Current Slide Preview")} sandbox={IFRAME_SANDBOX_PERMISSIONS} referrerPolicy="no-referrer" srcDoc={safeSlideSrcDoc} className="ff-presenter-slide-iframe" />
        ) : (
          <div className="ff-presenter-slide-fallback">
            <span className="ff-presenter-slide-fallback__eyebrow">{p.slide?.eyebrowTag || t("presentation.presenter.currentSlideLabel", "Current Slide")}</span>
            <p className="ff-presenter-slide-fallback__arc">{p.slide?.narrativeArc || p.slide?.slideUid || ""}</p>
          </div>
        )}
      </div>
      <PresenterSlideControlsBar {...p} />
    </section>
  );
};

const PresenterNextSlideCard: React.FC<{
  nextSlide: PresenterSlideSnapshotDto | null;
  nextSlideHtml?: string | null;
}> = ({ nextSlide, nextSlideHtml }) => {
  const { t } = useTranslation();
  const safeNextSrcDoc = useMemo(() => (nextSlideHtml ? wrapUserHtmlSrcDoc(nextSlideHtml) : null), [nextSlideHtml]);
  return (
    <div className="ff-presenter-card ff-presenter-next-card" data-testid="presenter-next-slide-card">
      <div className="ff-presenter-card__header">
        <span>{t("presentation.presenter.nextSlideLabel", "Next Slide")}</span>
        {nextSlide && <span className="ff-presenter-mono-tag">{nextSlide.eyebrowTag || nextSlide.slideUid}</span>}
      </div>
      {nextSlide ? (
        <div className="ff-presenter-next-frame">
          {safeNextSrcDoc ? (
            <iframe title={t("presentation.presenter.nextSlideFrameTitle", "Next Slide Preview")} sandbox={IFRAME_SANDBOX_PERMISSIONS} referrerPolicy="no-referrer" srcDoc={safeNextSrcDoc} className="ff-presenter-slide-iframe" />
          ) : (
            <div className="ff-presenter-slide-fallback">
              <span className="ff-presenter-slide-fallback__eyebrow">{nextSlide.eyebrowTag}</span>
              <p className="ff-presenter-slide-fallback__arc">{nextSlide.narrativeArc}</p>
            </div>
          )}
        </div>
      ) : (
        <div data-testid="presenter-end-of-deck" className="ff-presenter-end-of-deck">
          {t("presentation.presenter.endOfDeck", "End of Presentation")}
        </div>
      )}
    </div>
  );
};

const PresenterTimerCard: React.FC<{
  elapsedMs: number;
  isTimerRunning: boolean;
  onTimerCommand: (action: "start" | "pause" | "reset") => void;
}> = ({ elapsedMs, isTimerRunning, onTimerCommand }) => {
  const { t } = useTranslation();
  return (
    <div className="ff-presenter-card ff-presenter-timer-card">
      <div className="ff-presenter-card__header">
        <span>{t("presentation.presenter.paceTimerLabel", "Pace Stopwatch")}</span>
        <BrandIcon name="clock" size={ICON_SM} strokeWidth={ICON_STROKE} color="var(--text-muted)" />
      </div>
      <div className="ff-presenter-timer-row">
        <div data-testid="presenter-timer" data-running={isTimerRunning ? "true" : "false"} className="ff-presenter-timer-digits">
          {formatStopwatchMmSs(elapsedMs)}
        </div>
        <div className="ff-presenter-controls-group">
          <button type="button" data-testid="btn-presenter-timer-start" disabled={isTimerRunning} onClick={() => onTimerCommand("start")} className="ff-presenter-btn ff-presenter-btn--primary">
            <BrandIcon name="play" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
            <span>{t("presentation.presenter.timerStart", "Start")}</span>
          </button>
          <button type="button" data-testid="btn-presenter-timer-pause" disabled={!isTimerRunning} onClick={() => onTimerCommand("pause")} className="ff-presenter-btn">
            <BrandIcon name="pause" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
            <span>{t("presentation.presenter.timerPause", "Pause")}</span>
          </button>
          <button type="button" data-testid="btn-presenter-timer-reset" onClick={() => onTimerCommand("reset")} className="ff-presenter-btn">
            <BrandIcon name="refresh-cw" size={ICON_SM} strokeWidth={ICON_STROKE} color="currentColor" />
            <span>{t("presentation.presenter.timerReset", "Reset")}</span>
          </button>
        </div>
      </div>
    </div>
  );
};

const PresenterSpeakerNotesCard: React.FC<{
  speakerNotes: string;
  notesFontPx: number;
  onStepNotesFont: (delta: number) => void;
}> = ({ speakerNotes, notesFontPx, onStepNotesFont }) => {
  const { t } = useTranslation();
  return (
    <div className="ff-presenter-card ff-presenter-notes-card">
      <div className="ff-presenter-card__header">
        <span>{t("presentation.presenter.speakerNotesTitle", "Speaker Notes")}</span>
        <div className="ff-presenter-controls-group">
          <button type="button" data-testid="btn-notes-font-dec" onClick={() => onStepNotesFont(-NOTES_FONT_STEP)} className="ff-presenter-btn" aria-label={t("presentation.presenter.decreaseNotesFont", "Decrease font")}>{"A-"}</button>
          <button type="button" data-testid="btn-notes-font-inc" onClick={() => onStepNotesFont(NOTES_FONT_STEP)} className="ff-presenter-btn" aria-label={t("presentation.presenter.increaseNotesFont", "Increase font")}>{"A+"}</button>
        </div>
      </div>
      <div data-testid="presenter-speaker-notes" aria-live="polite" className="ff-presenter-notes-body" style={{ fontSize: `${notesFontPx}px` }}>
        {speakerNotes.trim().length > 0 ? (
          <span className="ff-presenter-notes-text">{speakerNotes}</span>
        ) : (
          <span className="ff-presenter-notes-empty">{t("presentation.presenter.noSpeakerNotes", "No speaker notes for this slide.")}</span>
        )}
      </div>
    </div>
  );
};

export const PresenterRightCockpitColumn: React.FC<{
  nextSlide: PresenterSlideSnapshotDto | null;
  nextSlideHtml?: string | null;
  elapsedMs: number;
  isTimerRunning: boolean;
  speakerNotes: string;
  notesFontPx: number;
  onStepNotesFont: (delta: number) => void;
  onTimerCommand: (action: "start" | "pause" | "reset") => void;
}> = (p) => (
  <aside className="ff-presenter-right-col">
    <PresenterNextSlideCard nextSlide={p.nextSlide} nextSlideHtml={p.nextSlideHtml} />
    <PresenterTimerCard elapsedMs={p.elapsedMs} isTimerRunning={p.isTimerRunning} onTimerCommand={p.onTimerCommand} />
    <PresenterSpeakerNotesCard speakerNotes={p.speakerNotes} notesFontPx={p.notesFontPx} onStepNotesFont={p.onStepNotesFont} />
  </aside>
);
