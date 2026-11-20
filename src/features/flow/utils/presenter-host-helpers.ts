// @story 06.4
// @implements AC-01, AC-02, AC-03, AC-04
import type {
  DeckAspectRatio,
  DeckSlideDto,
} from "../../../lib/validations/presentation";
import {
  PresenterHandshakeSchema,
  type PresenterBlackoutMode,
  type PresenterBroadcastEvent,
  type PresenterSlideSnapshotDto,
  type PresenterTransportMode,
} from "../../../lib/validations/presentation-presenter";
import type { SlidePhaseItem } from "../components/companion-canvas-data";
import { DEFAULT_FALLBACK_SLIDE_UID } from "../../../lib/validations/presentation";
import { buildPreviewDocumentHtml, wrapUserHtmlSrcDoc } from "./html-sandbox-wrapper";
import {
  buildPresenterChannelName,
  emitPresenterTelemetry,
  generatePresenterSecret,
  generatePresenterUuid,
  timingSafeEqualString,
  validateIncomingPresenterEvent,
  type PresenterTransportAdapter,
} from "./presenter-broadcast";
import { buildTimerSnapshot, getMonotonicNowMs, type PresenterTimerState } from "./presenter-timer";

export const DEFAULT_SLIDE_UID_PAD = DEFAULT_FALLBACK_SLIDE_UID;
const SHA_HEX_LEN = 64;
const MAX_EYEBROW_TAG_LEN = 64;
const MAX_NARRATIVE_ARC_LEN = 120;

export interface HostSequencerState {
  sessionId: string;
  sessionSecret: string;
  epoch: number;
  stateRevision: number;
  hostInstanceId: string;
  hostSeq: number;
  activePresenterInstanceId: string | null;
  lastAcceptedSeqByInstance: Map<string, number>;
  processedCommandKeys: string[];
  transport: PresenterTransportAdapter | null;
  popupWindow: Window | null;
  openedAtPerfMs: number;
}

export function createInitialHostSequencerState(): HostSequencerState {
  return {
    sessionId: generatePresenterUuid(),
    sessionSecret: generatePresenterSecret(),
    epoch: 0,
    stateRevision: 0,
    hostInstanceId: generatePresenterUuid(),
    hostSeq: 0,
    activePresenterInstanceId: null,
    lastAcceptedSeqByInstance: new Map<string, number>(),
    processedCommandKeys: [],
    transport: null,
    popupWindow: null,
    openedAtPerfMs: getMonotonicNowMs(),
  };
}

export function rotateHostSessionIdentity(seqState: HostSequencerState): void {
  seqState.transport?.close();
  seqState.transport = null;
  seqState.sessionId = generatePresenterUuid();
  seqState.sessionSecret = generatePresenterSecret();
  seqState.epoch += 1;
  seqState.hostSeq = 0;
  seqState.activePresenterInstanceId = null;
  seqState.lastAcceptedSeqByInstance.clear();
  seqState.processedCommandKeys = [];
  seqState.popupWindow = null;
}

export function normalizeDeckAspectRatio(aspectRatio: "auto" | "16-9" | "16-10"): DeckAspectRatio {
  if (aspectRatio === "16-10") return "16:10";
  if (aspectRatio === "auto") return "Auto";
  return "16:9";
}

function normalizeSlideUid(rawId: string): string {
  if (/^SLD-[0-9A-HJKMNP-TV-Z]{26}$/.test(rawId)) return rawId;
  return DEFAULT_SLIDE_UID_PAD;
}

export function buildFallbackPresenterSlides(
  slides: SlidePhaseItem[],
  speakerNotesBySlideId?: Record<string, string>,
  persistedSlides?: DeckSlideDto[]
): PresenterSlideSnapshotDto[] {
  return slides.map((s, idx) => {
    const persisted = persistedSlides?.find((p) => p.slideUid === s.id) ?? persistedSlides?.[idx];
    return {
      slideUid: normalizeSlideUid(persisted?.slideUid ?? s.id),
      slideIndex: idx,
      layoutKind: persisted?.layoutKind ?? "hero_cover",
      eyebrowTag: persisted?.eyebrowTag ?? s.code.slice(0, MAX_EYEBROW_TAG_LEN),
      narrativeArc: persisted?.narrativeArc ?? s.subtitle.slice(0, MAX_NARRATIVE_ARC_LEN),
      speakerNotes: speakerNotesBySlideId?.[s.id] ?? persisted?.speakerNotes ?? "",
      contentSha256: persisted?.contentSha256 ?? "0".repeat(SHA_HEX_LEN),
      revision: persisted?.revision ?? 1,
    };
  });
}

export function buildPresenterSlideHtml(
  slide: SlidePhaseItem | undefined,
  customHtml?: string
): string | undefined {
  if (customHtml && customHtml.trim()) return customHtml;
  if (!slide) return undefined;
  return buildPreviewDocumentHtml({
    structureLens: "slide-cms",
    activeSlide: slide,
    activeScreenData: {
      eyebrow: slide.code,
      headline: slide.headline,
      subtitle: slide.subtitle,
      amount: slide.kpi1,
      statusText: slide.kpi2,
      latency: slide.kpi3,
      actionTitle: slide.title,
      ctaLabel: slide.code,
    },
    formattedAmount: String(slide.kpi1),
  });
}

export function buildSyncFullStateEnvelope(opts: {
  seqState: HostSequencerState;
  deckId: string;
  deckTitle: string;
  aspectRatio: DeckAspectRatio;
  revision: number;
  activeSlideIndex: number;
  slides: PresenterSlideSnapshotDto[];
  blackoutMode: PresenterBlackoutMode;
  isAudienceFullscreen: boolean;
  isHistoricalReadOnly: boolean;
  snapshotTag: string | null;
  timerState: PresenterTimerState;
  currentSlideHtml?: string;
  nextSlideHtml?: string | null;
}): PresenterBroadcastEvent {
  const { seqState } = opts;
  const now = getMonotonicNowMs();
  seqState.hostSeq += 1;
  return {
    type: "SYNC_FULL_STATE",
    deckId: opts.deckId,
    sessionId: seqState.sessionId,
    sessionSecret: seqState.sessionSecret,
    senderOrigin: typeof window !== "undefined" ? window.location.origin : "http://127.0.0.1",
    senderRole: "host",
    senderInstanceId: seqState.hostInstanceId,
    epoch: seqState.epoch,
    stateRevision: seqState.stateRevision,
    seq: seqState.hostSeq,
    sentAtMs: Date.now(),
    deckTitle: opts.deckTitle || "Presentation Deck",
    aspectRatio: opts.aspectRatio,
    revision: opts.revision,
    activeSlideIndex: opts.activeSlideIndex,
    slidesCount: opts.slides.length,
    blackoutMode: opts.blackoutMode,
    isAudienceFullscreen: opts.isAudienceFullscreen,
    isHistoricalReadOnly: opts.isHistoricalReadOnly,
    snapshotTag: opts.snapshotTag,
    timer: buildTimerSnapshot(opts.timerState, now),
    slides: opts.slides,
    currentSlideHtml: opts.currentSlideHtml,
    nextSlideHtml: opts.nextSlideHtml ?? null,
  };
}

export function handleHostWindowHandshakeMessage(opts: {
  ev: MessageEvent;
  deckId: string;
  seqState: HostSequencerState;
  isHistoricalReadOnly: boolean;
  snapshotTag: string | null;
  onAuthenticated: () => void;
}): void {
  const { ev, seqState } = opts;
  const origin = typeof window !== "undefined" ? window.location.origin : "";
  if (ev.origin !== origin) return;
  const popup = seqState.popupWindow;
  if (!popup || popup.closed || ev.source !== popup) return;
  const parsed = PresenterHandshakeSchema.safeParse(ev.data);
  if (!parsed.success) return;
  const msg = parsed.data;
  if (msg.deckId !== opts.deckId || msg.sessionId !== seqState.sessionId) return;

  if (msg.type === "PRESENTER_WINDOW_READY") {
    seqState.activePresenterInstanceId = msg.senderInstanceId;
    seqState.lastAcceptedSeqByInstance.clear();
    seqState.lastAcceptedSeqByInstance.set(msg.senderInstanceId, 0);
    seqState.processedCommandKeys = [];

    const provision = PresenterHandshakeSchema.parse({
      type: "PRESENTER_SECRET_PROVISION",
      deckId: opts.deckId,
      sessionId: seqState.sessionId,
      windowNonce: msg.windowNonce,
      sessionSecret: seqState.sessionSecret,
      channelName: buildPresenterChannelName(opts.deckId, seqState.sessionId),
      hostOrigin: origin,
      epoch: seqState.epoch,
      stateRevision: seqState.stateRevision,
      nextSeqFloor: 1,
      isHistoricalReadOnly: opts.isHistoricalReadOnly,
      snapshotTag: opts.snapshotTag,
    });
    popup.postMessage(provision, origin);
    return;
  }

  if (msg.type !== "PRESENTER_HANDSHAKE_ACK") return;
  const secretMatches = timingSafeEqualString(msg.sessionSecret, seqState.sessionSecret);
  const instanceMatches = !msg.senderInstanceId || msg.senderInstanceId === seqState.activePresenterInstanceId;
  if (msg.epoch === seqState.epoch && secretMatches && instanceMatches) {
    opts.onAuthenticated();
  }
}

export function validateHostIncomingBroadcast(opts: {
  raw: unknown;
  deckId: string;
  seqState: HostSequencerState;
  transportMode: PresenterTransportMode;
}): PresenterBroadcastEvent | null {
  const origin = typeof window !== "undefined" ? window.location.origin : "";
  const result = validateIncomingPresenterEvent(opts.raw, {
    expectedOrigin: origin,
    expectedDeckId: opts.deckId,
    expectedSessionId: opts.seqState.sessionId,
    expectedSecret: opts.seqState.sessionSecret,
    expectedEpoch: opts.seqState.epoch,
    expectedSenderRole: "presenter",
    activeSenderInstanceId: opts.seqState.activePresenterInstanceId,
    lastAcceptedSeqByInstance: opts.seqState.lastAcceptedSeqByInstance,
    processedCommandKeys: opts.seqState.processedCommandKeys,
  });
  if (!result.ok) {
    emitPresenterTelemetry({
      metric: "presenter.channel_rejected.count",
      value: 1,
      deckId: opts.deckId,
      sessionId: opts.seqState.sessionId,
      transportMode: opts.transportMode,
      rejectReason: result.reason,
    });
    return null;
  }
  return result.event;
}
