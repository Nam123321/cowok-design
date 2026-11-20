// @vitest-environment jsdom
// @story 06.4
// @implements AC-01, AC-02, AC-03, AC-04
// @cover UT-01, UT-02, UT-02B, UT-03, UT-03B, UT-04
import fs from "node:fs";
import path from "node:path";
import React from "react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act, cleanup } from "@testing-library/react";
import {
  PresenterBroadcastEventSchema,
  PresenterHandshakeSchema,
  TauriPresenterIpcEventSchema,
} from "../../../lib/validations/presentation-presenter";
import { matchBffRoute } from "../../../lib/vite-bff-routes";
import { usePresenterSessionController } from "../hooks/usePresenterSessionController";
import { useDeckPresentationController } from "../hooks/useDeckPresentationController";
import { PresenterConsoleWindow } from "../components/PresenterConsoleWindow";
import { PresenterInlineSplitFallback } from "../components/PresenterInlineSplitFallback";
import {
  clearPresenterTelemetryBuffer,
  getPresenterTelemetryBuffer,
  timingSafeEqualString,
  validateIncomingPresenterEvent,
} from "../utils/presenter-broadcast";
import {
  applyTimerCommand,
  computeTimerElapsedMs,
  createInitialTimerState,
  formatStopwatchMmSs,
} from "../utils/presenter-timer";
import { createKeepAliveSlot, evictOldestSlotIfFull, POOL_MAX_SLOTS } from "../utils/sandbox-bridge-protocol";
import { INITIAL_SLIDES } from "../components/companion-canvas-data";

class MockBroadcastChannel {
  static channels = new Map<string, Set<MockBroadcastChannel>>();
  readonly name: string;
  onmessage: ((ev: MessageEvent) => void) | null = null;
  closed = false;

  constructor(name: string) {
    this.name = name;
    let set = MockBroadcastChannel.channels.get(name);
    if (!set) {
      set = new Set();
      MockBroadcastChannel.channels.set(name, set);
    }
    set.add(this);
  }

  postMessage(data: unknown) {
    if (this.closed) return;
    const peers = MockBroadcastChannel.channels.get(this.name);
    if (!peers) return;
    for (const peer of peers) {
      if (peer !== this && !peer.closed && peer.onmessage) {
        peer.onmessage({ data } as MessageEvent);
      }
    }
  }

  close() {
    this.closed = true;
    MockBroadcastChannel.channels.get(this.name)?.delete(this);
  }

  static reset() {
    MockBroadcastChannel.channels.clear();
  }
}

function HostPresenterHarness(props: {
  deckId?: string;
  isHistoricalReadOnly?: boolean;
  snapshotTag?: string | null;
  speakerNotesBySlideId?: Record<string, string>;
  onPostDeckBlackout?: (m: "none" | "black" | "white") => void;
}) {
  const [activeSlideIndex, setActiveSlideIndex] = React.useState(0);
  const [isPresenting, setIsPresenting] = React.useState(false);

  const ctrl = usePresenterSessionController({
    projectId: "proj-064",
    deckId: props.deckId ?? "deck-064",
    deckTitle: "Keynote 06.4",
    aspectRatio: "16-9",
    slides: INITIAL_SLIDES,
    activeSlideIndex,
    speakerNotesBySlideId: props.speakerNotesBySlideId ?? {
      [INITIAL_SLIDES[0]!.id]: "Say hello <script>alert(1)</script> safely",
      [INITIAL_SLIDES[1]!.id]: "Slide 2 metrics cue",
    },
    isAudienceFullscreen: isPresenting,
    isHistoricalReadOnly: Boolean(props.isHistoricalReadOnly),
    snapshotTag: props.snapshotTag ?? null,
    onChangeSlide: setActiveSlideIndex,
    onPostDeckBlackout: props.onPostDeckBlackout,
  });

  const deckCtrl = useDeckPresentationController({
    slidesCount: INITIAL_SLIDES.length,
    activeSlideIndex,
    onChangeSlide: setActiveSlideIndex,
    isPresenting,
    setIsPresenting,
    isPresenterActive: ctrl.isPresenterActive,
    onToggleBlackoutKey: (mode) => ctrl.toggleBlackoutKey(mode),
  });

  return (
    <div
      data-testid="host-harness"
      data-conn-state={ctrl.connectionState}
      data-transport={ctrl.transportMode}
      data-slide-idx={activeSlideIndex}
      data-blackout={ctrl.blackoutMode}
      data-presenting={deckCtrl.isDeckPresenting ? "true" : "false"}
    >
      <button type="button" data-testid="btn-open-presenter" onClick={ctrl.openPresenterView}>
        Open Presenter
      </button>
      <button type="button" data-testid="btn-start-present" onClick={deckCtrl.enterPresent}>
        Present
      </button>
      <button type="button" data-testid="btn-start-timer" onClick={() => ctrl.dispatchTimer("start")}>
        Start Timer
      </button>
      <button
        type="button"
        data-testid="btn-guarded-mutate"
        onClick={ctrl.guardPresenterMutation(() => setActiveSlideIndex(2))}
      >
        Mutate
      </button>
      {ctrl.blackoutMode !== "none" && (
        <div data-testid="audience-blackout-overlay" data-mode={ctrl.blackoutMode} />
      )}
      {ctrl.isPopupBlockedToastVisible && (
        <div role="alert" className="ff-toast ff-toast-warning" data-testid="presenter-popup-blocked-toast">
          <button type="button" data-testid="btn-reopen-toast" onClick={ctrl.reopenPresenterWindow}>
            Mở lại Presenter Window
          </button>
        </div>
      )}
      {ctrl.isInlineFallbackOpen && (
        <PresenterInlineSplitFallback
          deckTitle="Keynote 06.4"
          slides={ctrl.presenterSlides}
          activeSlideIndex={activeSlideIndex}
          blackoutMode={ctrl.blackoutMode}
          isAudienceFullscreen={isPresenting}
          transportMode={ctrl.transportMode}
          syncLatencyMs={ctrl.syncLatencyMs}
          isHistoricalReadOnly={ctrl.isHistoricalReadOnly}
          snapshotTag={ctrl.snapshotTag}
          elapsedMs={ctrl.elapsedMs}
          isTimerRunning={ctrl.timerState.isRunning}
          onPrevSlide={() => ctrl.navigateSlide(activeSlideIndex - 1)}
          onNextSlide={() => ctrl.navigateSlide(activeSlideIndex + 1)}
          onToggleBlackout={(m) => ctrl.toggleBlackoutKey(m)}
          onClearBlackout={() => ctrl.setBlackoutMode("none")}
          onToggleFullscreen={deckCtrl.togglePresent}
          onTimerCommand={ctrl.dispatchTimer}
          onReopenWindow={ctrl.reopenPresenterWindow}
          onClose={ctrl.closePresenterSession}
        />
      )}
      <input data-testid="ime-input" type="text" />
    </div>
  );
}

describe("Story 06.4 Presenter Mode Dual-Window & Fallbacks", () => {
  const origBroadcast = globalThis.BroadcastChannel;

  beforeEach(() => {
    vi.useFakeTimers();
    clearPresenterTelemetryBuffer();
    MockBroadcastChannel.reset();
    (globalThis as unknown as { BroadcastChannel: typeof BroadcastChannel }).BroadcastChannel =
      MockBroadcastChannel as unknown as typeof BroadcastChannel;
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
    (globalThis as unknown as { BroadcastChannel: typeof BroadcastChannel }).BroadcastChannel = origBroadcast;
    delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it("UT-01: synchronous window.open in onClick omits ?secret=, completes out-of-band handshake, and activates Tauri IPC when present", () => {
    const popupWindow = { closed: false, postMessage: vi.fn(), close: vi.fn() } as unknown as Window;
    const openSpy = vi.spyOn(window, "open").mockReturnValue(popupWindow);

    render(<HostPresenterHarness deckId="deck-ut01" />);
    fireEvent.click(screen.getByTestId("btn-open-presenter"));

    expect(openSpy).toHaveBeenCalledTimes(1);
    const openedUrl = String(openSpy.mock.calls[0]![0]);
    expect(openedUrl).toMatch(/^\/presenter\/deck-ut01\?sessionId=/);
    expect(openedUrl).not.toContain("secret=");
    expect(screen.getByTestId("host-harness").getAttribute("data-conn-state")).toBe("OPENING_POPUP");

    const sessionId = new URL(openedUrl, window.location.origin).searchParams.get("sessionId")!;
    const instanceId = "11111111-1111-4111-8111-111111111111";
    const nonce = "22222222-2222-4222-8222-222222222222";

    act(() => {
      window.dispatchEvent(
        new MessageEvent("message", {
          origin: window.location.origin,
          source: popupWindow,
          data: {
            type: "PRESENTER_WINDOW_READY",
            deckId: "deck-ut01",
            sessionId,
            windowNonce: nonce,
            senderInstanceId: instanceId,
          },
        })
      );
    });

    const provisionMock = (popupWindow.postMessage as ReturnType<typeof vi.fn>).mock.calls[0]?.[0];
    const parsedProvision = PresenterHandshakeSchema.safeParse(provisionMock);
    expect(parsedProvision.success).toBe(true);
    if (!parsedProvision.success || parsedProvision.data.type !== "PRESENTER_SECRET_PROVISION") {
      throw new Error("Expected PRESENTER_SECRET_PROVISION");
    }
    const provisionedSecret = parsedProvision.data.sessionSecret;
    const provisionedEpoch = parsedProvision.data.epoch;
    expect(provisionedSecret.length).toBeGreaterThanOrEqual(32);

    act(() => {
      window.dispatchEvent(
        new MessageEvent("message", {
          origin: window.location.origin,
          source: popupWindow,
          data: {
            type: "PRESENTER_HANDSHAKE_ACK",
            deckId: "deck-ut01",
            sessionId,
            sessionSecret: provisionedSecret,
            senderInstanceId: instanceId,
            epoch: provisionedEpoch,
            rehydrated: false,
          },
        })
      );
    });

    expect(screen.getByTestId("host-harness").getAttribute("data-conn-state")).toBe("AUTHENTICATED");

    // Verify Tauri IPC mode activates when __TAURI_INTERNALS__ is present
    cleanup();
    const tauriInvoke = vi.fn().mockResolvedValue(undefined);
    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
      invoke: tauriInvoke,
    };
    render(<HostPresenterHarness deckId="deck-tauri" />);
    fireEvent.click(screen.getByTestId("btn-open-presenter"));
    expect(screen.getByTestId("host-harness").getAttribute("data-transport")).toBe("tauri_ipc");
    expect(
      TauriPresenterIpcEventSchema.safeParse({
        windowLabel: "presenter-deck-tauri",
        payload: {
          type: "REQUEST_SYNC_FULL_STATE",
          deckId: "deck-tauri",
          sessionId: "session-tauri-12345",
          sessionSecret: "a".repeat(64),
          senderOrigin: window.location.origin,
          senderRole: "presenter",
          senderInstanceId: instanceId,
          epoch: 1,
          stateRevision: 0,
          seq: 1,
          sentAtMs: Date.now(),
          reason: "initial",
        },
      }).success
    ).toBe(true);
  });

  it("UT-02 & UT-02B: rejects forged origin/secret/deckId/stale-seq/duplicate-command, handles F5 reload rebind, and calls channel.close()", () => {
    expect(timingSafeEqualString("abc123", "abc123")).toBe(true);
    expect(timingSafeEqualString("abc123", "abc124")).toBe(false);

    const seqMap = new Map<string, number>();
    const dedupe: string[] = [];
    const baseState = {
      expectedOrigin: window.location.origin,
      expectedDeckId: "deck-sec",
      expectedSessionId: "session-1234567890123456",
      expectedSecret: "s".repeat(64),
      expectedEpoch: 1,
      expectedSenderRole: "presenter" as const,
      activeSenderInstanceId: "inst-active-1111111111",
      lastAcceptedSeqByInstance: seqMap,
      processedCommandKeys: dedupe,
    };

    const validCmd = {
      type: "SLIDE_NAV",
      deckId: "deck-sec",
      sessionId: baseState.expectedSessionId,
      sessionSecret: baseState.expectedSecret,
      senderOrigin: window.location.origin,
      senderRole: "presenter",
      senderInstanceId: baseState.activeSenderInstanceId,
      epoch: 1,
      stateRevision: 0,
      seq: 1,
      commandId: "cmd-1111111111111111",
      baseRevision: 0,
      sentAtMs: Date.now(),
      direction: "next",
      targetIndex: 1,
    };

    // Origin mismatch, secret mismatch, deck mismatch, zod invalid
    expect(validateIncomingPresenterEvent({ ...validCmd, senderOrigin: "https://evil.test" }, baseState)).toEqual({
      ok: false,
      reason: "origin_mismatch",
    });
    expect(validateIncomingPresenterEvent({ ...validCmd, sessionSecret: "x".repeat(64) }, baseState)).toEqual({
      ok: false,
      reason: "secret_mismatch",
    });
    expect(validateIncomingPresenterEvent({ ...validCmd, deckId: "other-deck" }, baseState)).toEqual({
      ok: false,
      reason: "deck_mismatch",
    });
    expect(validateIncomingPresenterEvent({ ...validCmd, unknownField: 123 }, baseState)).toEqual({
      ok: false,
      reason: "zod_invalid",
    });

    // Valid command accepted, duplicate commandId dropped, stale seq dropped
    expect(validateIncomingPresenterEvent(validCmd, baseState).ok).toBe(true);
    expect(validateIncomingPresenterEvent({ ...validCmd, seq: 1 }, baseState)).toEqual({
      ok: false,
      reason: "stale_seq",
    });
    expect(validateIncomingPresenterEvent({ ...validCmd, seq: 2 }, baseState)).toEqual({
      ok: false,
      reason: "duplicate_command",
    });

    // Host live F5 reload rebind + stale baseRevision rejection + channel.close()
    const popupWindow = { closed: false, postMessage: vi.fn(), close: vi.fn() } as unknown as Window;
    vi.spyOn(window, "open").mockReturnValue(popupWindow);
    const { unmount } = render(<HostPresenterHarness deckId="deck-f5" />);
    fireEvent.click(screen.getByTestId("btn-open-presenter"));
    const openedUrl = String((window.open as ReturnType<typeof vi.fn>).mock.calls[0]![0]);
    const sessionId = new URL(openedUrl, window.location.origin).searchParams.get("sessionId")!;

    const inst1 = "inst-1111-1111-1111-111111111111";
    const inst2 = "inst-2222-2222-2222-222222222222";
    const nonce = "nonce-1111-1111-1111-111111111111";

    act(() => {
      window.dispatchEvent(new MessageEvent("message", { origin: window.location.origin, source: popupWindow, data: { type: "PRESENTER_WINDOW_READY", deckId: "deck-f5", sessionId, windowNonce: nonce, senderInstanceId: inst1 } }));
    });
    const provisionMsg = (popupWindow.postMessage as ReturnType<typeof vi.fn>).mock.calls[0]![0] as { sessionSecret: string; channelName: string };
    const secret = provisionMsg.sessionSecret;
    const ch = new MockBroadcastChannel(provisionMsg.channelName);
    const receivedSyncs: Array<Record<string, unknown>> = [];
    ch.onmessage = (ev) => receivedSyncs.push(ev.data as Record<string, unknown>);

    act(() => {
      ch.postMessage({
        type: "SLIDE_NAV", deckId: "deck-f5", sessionId, sessionSecret: secret,
        senderOrigin: window.location.origin, senderRole: "presenter", senderInstanceId: inst1,
        epoch: 1, stateRevision: 0, seq: 5, commandId: "cmd-a111111111111111", baseRevision: 0, sentAtMs: Date.now(),
        direction: "next", targetIndex: 1,
      });
    });
    expect(screen.getByTestId("host-harness").getAttribute("data-slide-idx")).toBe("1");

    // Stale baseRevision (0 < 1) must be rejected without double-advancing
    act(() => {
      ch.postMessage({
        type: "SLIDE_NAV", deckId: "deck-f5", sessionId, sessionSecret: secret,
        senderOrigin: window.location.origin, senderRole: "presenter", senderInstanceId: inst1,
        epoch: 1, stateRevision: 1, seq: 6, commandId: "cmd-b111111111111111", baseRevision: 0, sentAtMs: Date.now(),
        direction: "next", targetIndex: 2,
      });
    });
    expect(screen.getByTestId("host-harness").getAttribute("data-slide-idx")).toBe("1");

    // F5 reload fires beforeunload (with popupWindow.closed === false), generates inst2 and restarts seq = 1
    act(() => {
      ch.postMessage({
        type: "PRESENTER_WINDOW_CLOSING", deckId: "deck-f5", sessionId, sessionSecret: secret,
        senderOrigin: window.location.origin, senderRole: "presenter", senderInstanceId: inst1,
        epoch: 1, stateRevision: 1, seq: 7, sentAtMs: Date.now(), reason: "beforeunload",
      });
      window.dispatchEvent(new MessageEvent("message", { origin: window.location.origin, source: popupWindow, data: { type: "PRESENTER_WINDOW_READY", deckId: "deck-f5", sessionId, windowNonce: nonce, senderInstanceId: inst2 } }));
      ch.postMessage({
        type: "REQUEST_SYNC_FULL_STATE", deckId: "deck-f5", sessionId, sessionSecret: secret,
        senderOrigin: window.location.origin, senderRole: "presenter", senderInstanceId: inst2,
        epoch: 1, stateRevision: 0, seq: 1, sentAtMs: Date.now(), reason: "reload",
      });
    });
    expect(receivedSyncs.some((s) => s.type === "SYNC_FULL_STATE" && s.activeSlideIndex === 1)).toBe(true);
    expect(getPresenterTelemetryBuffer().some((t) => t.metric === "presenter.rehydrate.count")).toBe(true);

    // Unmount closes BroadcastChannel
    ch.close();
    unmount();
    expect(MockBroadcastChannel.channels.get(provisionMsg.channelName)?.size ?? 0).toBe(0);
  });

  it("UT-03 & UT-03B: speaker_notes escapes HTML, stopwatch <1s drift over 10m, popup-blocked inline fallback preserves timer, and POOL_MAX_SLOTS pins next slide", () => {
    // Timer drift check over 10 minutes (600,000 ms)
    let nowPerf = 1000;
    let tState = applyTimerCommand(createInitialTimerState(), "start", nowPerf);
    nowPerf += 600_000;
    const elapsed10m = computeTimerElapsedMs(tState, nowPerf);
    expect(Math.abs(elapsed10m - 600_000)).toBeLessThan(1000);
    expect(formatStopwatchMmSs(elapsed10m)).toBe("10:00");
    tState = applyTimerCommand(tState, "pause", nowPerf);
    expect(tState.isRunning).toBe(false);
    tState = applyTimerCommand(tState, "reset", nowPerf);
    expect(tState.accumulatedMs).toBe(0);

    // POOL_MAX_SLOTS = 5 pinning check
    const slots = [0, 1, 2, 3, 4].map((i) => ({
      ...createKeepAliveSlot({
        cacheKey: `k-${i}`,
        projectId: "p",
        nodeId: `SLD-${i}`,
        artifactSha256: "a".repeat(64),
        rawHtml: "<div></div>",
      }),
      lastUsedAt: 100 + i,
    }));
    const kept = evictOldestSlotIfFull(slots, ["SLD-0", "SLD-1"]);
    expect(kept.length).toBe(POOL_MAX_SLOTS - 1);
    expect(kept.some((s) => s.nodeId === "SLD-0")).toBe(true);
    expect(kept.some((s) => s.nodeId === "SLD-1")).toBe(true);

    // Popup blocked -> Night inline fallback + warning toast + escaped speaker_notes + timer continuity
    let fakePerfNow = 5000;
    vi.spyOn(globalThis.performance, "now").mockImplementation(() => fakePerfNow);
    vi.spyOn(window, "open").mockReturnValue(null);
    render(<HostPresenterHarness deckId="deck-blocked" />);

    fireEvent.click(screen.getByTestId("btn-start-timer"));
    fakePerfNow += 2200;
    act(() => { vi.advanceTimersByTime(2200); });

    fireEvent.click(screen.getByTestId("btn-open-presenter"));
    expect(screen.getByTestId("host-harness").getAttribute("data-conn-state")).toBe("POPUP_BLOCKED_INLINE_FALLBACK");
    expect(screen.getByTestId("presenter-popup-blocked-toast").className).toContain("ff-toast-warning");
    const fallback = screen.getByTestId("presenter-inline-fallback");
    expect(fallback.getAttribute("data-theme")).toBe("night");

    const notesEl = screen.getByTestId("presenter-speaker-notes");
    expect(notesEl.textContent).toContain("<script>alert(1)</script>");
    expect(notesEl.querySelector("script")).toBeNull();
    expect(screen.getByTestId("presenter-timer").textContent).toBe("00:02");
  });

  it("UT-04: single-Esc fullscreenchange vs Presenter-only Esc, IME suppression on B/W/F, SNAP-* read-only guard, and static SDK checks", () => {
    const blackoutMock = vi.fn();
    render(
      <HostPresenterHarness
        deckId="deck-ut04"
        isHistoricalReadOnly={true}
        snapshotTag="SNAP-064-LOCK"
        onPostDeckBlackout={blackoutMock}
      />
    );

    // SNAP-* read-only lock blocks mutating callback & emits readonly_snap_lock telemetry
    fireEvent.click(screen.getByTestId("btn-guarded-mutate"));
    expect(screen.getByTestId("host-harness").getAttribute("data-slide-idx")).toBe("0");
    expect(
      getPresenterTelemetryBuffer().some((t) => t.rejectReason === "readonly_snap_lock")
    ).toBe(true);

    // Enter Present -> B / W toggle blackout overlay -> IME composing blocks B/W/F -> single Esc exits
    fireEvent.click(screen.getByTestId("btn-start-present"));
    expect(screen.getByTestId("host-harness").getAttribute("data-presenting")).toBe("true");

    fireEvent.keyDown(window, { key: "b", keyCode: 66 });
    expect(screen.getByTestId("audience-blackout-overlay").getAttribute("data-mode")).toBe("black");
    expect(blackoutMock).toHaveBeenCalledWith("black");

    // IME composing ignores W
    fireEvent.keyDown(window, { key: "w", keyCode: 229, isComposing: true });
    expect(screen.getByTestId("audience-blackout-overlay").getAttribute("data-mode")).toBe("black");

    // Real W toggles whiteout
    fireEvent.keyDown(window, { key: "w", keyCode: 87 });
    expect(screen.getByTestId("audience-blackout-overlay").getAttribute("data-mode")).toBe("white");

    // Single Esc exits Present cleanly
    fireEvent.keyDown(window, { key: "Escape", keyCode: 27 });
    expect(screen.getByTestId("host-harness").getAttribute("data-presenting")).toBe("false");

    // PresenterConsoleWindow Esc closes ONLY presenter window without failing
    const closeSpy = vi.spyOn(window, "close").mockImplementation(() => {});
    render(
      <PresenterConsoleWindow
        deckId="deck-win"
        initialSnapshot={{
          deckTitle: "Snap Deck",
          aspectRatio: "16:9",
          revision: 1,
          activeSlideIndex: 0,
          slidesCount: 2,
          blackoutMode: "none",
          isAudienceFullscreen: true,
          isHistoricalReadOnly: true,
          snapshotTag: "SNAP-064-LOCK",
          timer: { accumulatedMs: 0, elapsedAtSyncMs: 0, isRunning: false, syncedAtMs: 0 },
          slides: [
            {
              slideUid: "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV",
              slideIndex: 0,
              layoutKind: "hero_cover",
              eyebrowTag: "S1",
              narrativeArc: "Intro",
              speakerNotes: "Note 1",
              contentSha256: "a".repeat(64),
              revision: 1,
            },
          ],
        }}
      />
    );
    expect(screen.getByTestId("presenter-snap-readonly-badge").textContent).toContain("SNAP-064-LOCK");
    fireEvent.keyDown(window, { key: "Escape", keyCode: 27 });
    expect(closeSpy).toHaveBeenCalled();

    // Verify BFF route registration for both direct and project-scoped endpoints
    expect(matchBffRoute("POST", "/api/decks/d1/presenter/handshake")).not.toBeNull();
    expect(matchBffRoute("POST", "/api/projects/p1/decks/d1/presenter/handshake")).not.toBeNull();
    expect(
      PresenterBroadcastEventSchema.safeParse({
        type: "BLACKOUT_TOGGLE",
        deckId: "d1",
        sessionId: "session-1234567890123456",
        sessionSecret: "s".repeat(64),
        senderOrigin: window.location.origin,
        senderRole: "host",
        senderInstanceId: "inst-1111111111111111",
        epoch: 1,
        stateRevision: 1,
        seq: 1,
        sentAtMs: Date.now(),
        mode: "black",
        keyTrigger: "B",
      }).success
    ).toBe(true);

    // Static check: 0 raw fetch() and 0 Buffer.* in Story 06.4 browser modules
    const browserFiles = [
      "src/api/presentation-presenter.ts",
      "src/features/flow/utils/presenter-broadcast.ts",
      "src/features/flow/utils/presenter-host-helpers.ts",
      "src/features/flow/utils/presenter-timer.ts",
      "src/features/flow/hooks/usePresenterSessionController.ts",
      "src/features/flow/hooks/usePresenterHostTransport.ts",
      "src/features/flow/hooks/usePresenterWindowClient.ts",
      "src/features/flow/hooks/usePresenterClientTransport.ts",
      "src/features/flow/components/PresenterCockpitCards.tsx",
      "src/features/flow/components/PresenterCockpitView.tsx",
      "src/features/flow/components/PresenterConsoleWindow.tsx",
      "src/features/flow/components/PresenterInlineSplitFallback.tsx",
    ];
    for (const rel of browserFiles) {
      const content = fs.readFileSync(path.resolve(process.cwd(), rel), "utf8");
      expect(content, `${rel} must not contain raw fetch(`).not.toMatch(/\bfetch\s*\(/);
      expect(content, `${rel} must not contain Buffer.`).not.toMatch(/\bBuffer\./);
    }
  });

  it("UT-05: running stopwatch while AUTHENTICATED does not flood SYNC_FULL_STATE, ev.source spoofing is rejected, and reopening rotates secret & preserves subscription", () => {
    const popupWindow = { closed: false, postMessage: vi.fn(), close: vi.fn() } as unknown as Window;
    const foreignWindow = { closed: false, postMessage: vi.fn(), close: vi.fn() } as unknown as Window;
    const openSpy = vi.spyOn(window, "open").mockReturnValue(popupWindow);

    render(<HostPresenterHarness deckId="deck-timer-flood" />);
    fireEvent.click(screen.getByTestId("btn-open-presenter"));
    const firstUrl = String(openSpy.mock.calls.at(-1)![0]);
    const session1 = new URL(firstUrl, window.location.origin).searchParams.get("sessionId")!;

    // Foreign same-origin ev.source must be rejected & never receive sessionSecret
    act(() => {
      window.dispatchEvent(new MessageEvent("message", { origin: window.location.origin, source: foreignWindow, data: { type: "PRESENTER_WINDOW_READY", deckId: "deck-timer-flood", sessionId: session1, windowNonce: "nonce-forged-111111", senderInstanceId: "inst-forged-111111" } }));
    });
    expect(foreignWindow.postMessage).not.toHaveBeenCalled();

    // Real popup handshake succeeds
    const inst1 = "inst-real-1111111111111111";
    act(() => {
      window.dispatchEvent(new MessageEvent("message", { origin: window.location.origin, source: popupWindow, data: { type: "PRESENTER_WINDOW_READY", deckId: "deck-timer-flood", sessionId: session1, windowNonce: "nonce-real-11111111", senderInstanceId: inst1 } }));
    });
    const provision1 = (popupWindow.postMessage as ReturnType<typeof vi.fn>).mock.calls[0]![0] as { sessionSecret: string; channelName: string; epoch: number };
    const ch1 = new MockBroadcastChannel(provision1.channelName);
    let syncCount = 0;
    let latestRev = 0;
    ch1.onmessage = (ev) => {
      const d = ev.data as { type?: string; stateRevision?: number };
      if (d.type === "SYNC_FULL_STATE") {
        syncCount += 1;
        if (typeof d.stateRevision === "number") latestRev = d.stateRevision;
      }
    };

    // Forged ACK with wrong secret or wrong ev.source must NOT authenticate
    act(() => {
      window.dispatchEvent(new MessageEvent("message", { origin: window.location.origin, source: popupWindow, data: { type: "PRESENTER_HANDSHAKE_ACK", deckId: "deck-timer-flood", sessionId: session1, sessionSecret: "f".repeat(64), senderInstanceId: inst1, epoch: provision1.epoch, rehydrated: false } }));
    });
    expect(screen.getByTestId("host-harness").getAttribute("data-conn-state")).toBe("OPENING_POPUP");

    // Valid ACK authenticates (exactly 1 SYNC_FULL_STATE)
    act(() => {
      window.dispatchEvent(new MessageEvent("message", { origin: window.location.origin, source: popupWindow, data: { type: "PRESENTER_HANDSHAKE_ACK", deckId: "deck-timer-flood", sessionId: session1, sessionSecret: provision1.sessionSecret, senderInstanceId: inst1, epoch: provision1.epoch, rehydrated: false } }));
    });
    expect(screen.getByTestId("host-harness").getAttribute("data-conn-state")).toBe("AUTHENTICATED");
    expect(syncCount).toBe(1);

    // Start stopwatch & advance 2000ms (8 ticks) -> ZERO extra SYNC_FULL_STATE storm beyond explicit dispatchTimer!
    const syncsBeforeStart = syncCount;
    fireEvent.click(screen.getByTestId("btn-start-timer"));
    const syncsAfterStart = syncCount;
    expect(syncsAfterStart).toBe(syncsBeforeStart + 1);
    act(() => { vi.advanceTimersByTime(2000); });
    expect(syncCount).toBe(syncsAfterStart);

    // Slide nav with current latestRev still succeeds (no false stale_revision from 250ms ticks)
    act(() => {
      ch1.postMessage({ type: "SLIDE_NAV", deckId: "deck-timer-flood", sessionId: session1, sessionSecret: provision1.sessionSecret, senderOrigin: window.location.origin, senderRole: "presenter", senderInstanceId: inst1, epoch: provision1.epoch, stateRevision: latestRev, seq: 2, commandId: "cmd-timer-nav-1111", baseRevision: latestRev, sentAtMs: Date.now(), direction: "next", targetIndex: 1 });
    });
    expect(screen.getByTestId("host-harness").getAttribute("data-slide-idx")).toBe("1");

    // Inline fallback renders navigation & blackout controls
    ch1.close();
    openSpy.mockReturnValue(null);
    fireEvent.click(screen.getByTestId("btn-open-presenter"));
    expect(screen.getByTestId("presenter-inline-fallback")).toBeTruthy();
    expect(screen.getByTestId("btn-presenter-next")).toBeTruthy();
    expect(screen.getByTestId("btn-presenter-blackout")).toBeTruthy();

    // Reopen popup while OPENING_POPUP rotates secret/epoch and keeps channel subscription intact
    const popup2 = { closed: false, postMessage: vi.fn(), close: vi.fn() } as unknown as Window;
    openSpy.mockReturnValue(popup2);
    fireEvent.click(screen.getByTestId("btn-open-presenter"));
    fireEvent.click(screen.getByTestId("btn-open-presenter"));
    const thirdUrl = String(openSpy.mock.calls.at(-1)![0]);
    const session3 = new URL(thirdUrl, window.location.origin).searchParams.get("sessionId")!;
    expect(session3).not.toBe(session1);
    const inst3 = "inst-reopen-333333333333";
    act(() => {
      window.dispatchEvent(new MessageEvent("message", { origin: window.location.origin, source: popup2, data: { type: "PRESENTER_WINDOW_READY", deckId: "deck-timer-flood", sessionId: session3, windowNonce: "nonce-reopen-33333", senderInstanceId: inst3 } }));
    });
    const prov3 = (popup2.postMessage as ReturnType<typeof vi.fn>).mock.calls[0]![0] as { sessionSecret: string; channelName: string; epoch: number };
    expect(prov3.sessionSecret).not.toBe(provision1.sessionSecret);
    expect(prov3.epoch).toBeGreaterThan(provision1.epoch);
    const ch3 = new MockBroadcastChannel(prov3.channelName);
    act(() => {
      ch3.postMessage({ type: "REQUEST_SYNC_FULL_STATE", deckId: "deck-timer-flood", sessionId: session3, sessionSecret: prov3.sessionSecret, senderOrigin: window.location.origin, senderRole: "presenter", senderInstanceId: inst3, epoch: prov3.epoch, stateRevision: 0, seq: 1, sentAtMs: Date.now(), reason: "initial" });
    });
    expect(screen.getByTestId("host-harness").getAttribute("data-conn-state")).toBe("AUTHENTICATED");
    ch3.close();
  });

  it("UT-06: PresenterCurrentSlideCard sanitizes slideHtml with wrapUserHtmlSrcDoc & beforeunload closes host session", async () => {
    const popupMock = { closed: false, postMessage: vi.fn(), close: vi.fn() } as unknown as Window;
    vi.spyOn(window, "open").mockReturnValue(popupMock);
    const origOpener = window.opener;
    const hostOpenerMock = { postMessage: vi.fn() };
    Object.defineProperty(window, "opener", { value: hostOpenerMock, configurable: true });

    try {
      const { unmount } = render(<PresenterConsoleWindow deckId="deck-xss" initialSnapshot={{ deckTitle: "XSS Guard", aspectRatio: "16:9", revision: 1, activeSlideIndex: 0, slidesCount: 1, blackoutMode: "none", isAudienceFullscreen: false, isHistoricalReadOnly: false, snapshotTag: null, timer: { accumulatedMs: 0, elapsedAtSyncMs: 0, isRunning: false, syncedAtMs: 0 }, slides: [{ slideUid: "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV", slideIndex: 0, layoutKind: "hero_cover", eyebrowTag: "S1", narrativeArc: "Safe", speakerNotes: "", contentSha256: "a".repeat(64), revision: 1 }], currentSlideHtml: '<div>Hello</div><script>window.__pwned = true;</script><img src="https://evil.test/x.png" onerror="alert(1)">' }} />);
      const iframe = document.querySelector('[data-testid="presenter-current-slide-frame"] iframe') as HTMLIFrameElement;
      expect(iframe).not.toBeNull();
      const srcDoc = iframe.getAttribute("srcdoc") || "";
      expect(srcDoc).toContain("Content-Security-Policy");
      expect(srcDoc).toContain("connect-src 'none'");
      expect(srcDoc).not.toContain("window.__pwned");
      expect(srcDoc).not.toContain("onerror");
      expect(iframe.getAttribute("sandbox")).not.toContain("allow-same-origin");

      // Provision client + trigger beforeunload -> must dispatch PRESENTER_WINDOW_CLOSING
      const readyCall = hostOpenerMock.postMessage.mock.calls[0]![0] as { sessionId: string; windowNonce: string; senderInstanceId: string };
      const ch = new MockBroadcastChannel(`cowok-presenter:deck-xss:${readyCall.sessionId}`);
      const closingEvents: Array<Record<string, unknown>> = [];
      ch.onmessage = (ev) => closingEvents.push(ev.data as Record<string, unknown>);
      act(() => {
        window.dispatchEvent(new MessageEvent("message", { origin: window.location.origin, source: hostOpenerMock as unknown as Window, data: { type: "PRESENTER_SECRET_PROVISION", deckId: "deck-xss", sessionId: readyCall.sessionId, windowNonce: readyCall.windowNonce, sessionSecret: "s".repeat(64), channelName: `cowok-presenter:deck-xss:${readyCall.sessionId}`, hostOrigin: window.location.origin, epoch: 1, stateRevision: 0, nextSeqFloor: 1, isHistoricalReadOnly: false, snapshotTag: null } }));
      });
      act(() => { window.dispatchEvent(new Event("beforeunload")); });
      expect(closingEvents.some((e) => e.type === "PRESENTER_WINDOW_CLOSING" && e.reason === "beforeunload")).toBe(true);
      ch.close();
      unmount();
    } finally {
      Object.defineProperty(window, "opener", { value: origOpener, configurable: true });
    }
  });
});
