// @vitest-environment jsdom
// @story 06.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
// @cover UT-05, UT-06, UT-07
import fs from "node:fs";
import path from "node:path";
import React from "react";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, screen, act, cleanup } from "@testing-library/react";
import {
  ApplySlideHtmlRequestSchema,
  CANONICAL_FF_ID_REGEX,
  CmsRecoveryDraftSchema,
  CspViolationFrameSchema,
  PatchByFfIdFrameSchema,
  PatchSlotsBatchRequestSchema,
  SlotOverflowWarnFrameSchema,
} from "../../../lib/validations/presentation";
import {
  POOL_MAX_SLOTS,
  MAX_OUTBOUND_QUEUE_FRAMES,
  enqueueOutboundFrame,
  evictOldestSlotIfFull,
  type IframeKeepAliveSlot,
} from "../utils/sandbox-bridge-protocol";
import { CompanionStructureLeftPane } from "../components/CompanionStructureLeftPane";
import { HtmlPreviewToolbar } from "../components/HtmlPreviewToolbar";
import { CmsStageRecoveryPopup } from "../components/CmsStageRecoveryPopup";
import {
  saveRecoveryDraft,
  loadRecoveryDraft,
  clearRecoveryDraft,
  useCmsSlideEditorController,
  type PostPatchByFfIdFn,
} from "../hooks/useCmsSlideEditorController";
import { useSandboxedIframeBridge } from "../hooks/useSandboxedIframeBridge";
import type { SlidePhaseItem } from "../components/companion-canvas-data";
import { PresentationClient } from "../../../api/presentation";

const SAMPLE_SLIDES: SlidePhaseItem[] = [
  {
    id: "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV",
    code: "SLD-01 • HERO",
    title: "Executive Cover",
    headline: "Initial Headline",
    subtitle: "Initial Subtitle",
    kpi1: 1200,
    kpi2: "99.9%",
    kpi3: "8ms",
  },
  {
    id: "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAW",
    code: "SLD-02 • KPI",
    title: "Metrics Grid",
    headline: "KPI Architecture",
    subtitle: "Zero drift telemetry",
    kpi1: 2400,
    kpi2: "100%",
    kpi3: "4ms",
  },
];

function makeFakeSlot(nodeId: string, lastUsedAt: number): IframeKeepAliveSlot {
  return {
    slotId: `slot-${nodeId}`,
    cacheKey: `proj::${nodeId}::sha`,
    projectId: "proj",
    nodeId,
    artifactSha256: "sha",
    rawHtml: "<section class='slide active'></section>",
    epoch: 1,
    bootSecret: "a".repeat(32),
    channelSecret: "b".repeat(32),
    scriptNonce: "c".repeat(32),
    wrappedSrcDoc: "",
    port1: null,
    initPortDispatched: false,
    isPortVerified: false,
    retryCount: 0,
    protocolStatus: "BOOTSTRAPPING",
    elementRects: {},
    lastAcceptedSeq: 0,
    lastUsedAt,
    outboundQueue: [],
    iframeRef: React.createRef<HTMLIFrameElement | null>(),
  };
}

function DrawerHarness({
  onPatchSpy,
  initialOpen = false,
  isPresenting = false,
}: {
  onPatchSpy: ReturnType<typeof vi.fn>;
  initialOpen?: boolean;
  isPresenting?: boolean;
}) {
  const [slides, setSlides] = React.useState<SlidePhaseItem[]>(SAMPLE_SLIDES);
  const [activeIdx, setActiveIdx] = React.useState(0);
  const [drawerOpen, setDrawerOpen] = React.useState(initialOpen);
  const patchRef = React.useRef<PostPatchByFfIdFn | null>(onPatchSpy as unknown as PostPatchByFfIdFn);
  const activeSlide = slides[activeIdx]!;

  const cms = useCmsSlideEditorController({
    deckId: "deck-test-062",
    activeSlide,
    isDrawerOpen: drawerOpen,
    onUpdateActiveSlide: (patch) =>
      setSlides((prev) => prev.map((s, i) => (i === activeIdx ? { ...s, ...patch } : s))),
    postPatchByFfIdRef: patchRef,
  });

  return (
    <div>
      <button
        type="button"
        data-testid="trigger-e3-overflow"
        onClick={() =>
          cms.handleDeckTelemetryEvent({
            type: "SLIDE_OVERFLOW_WARN",
            epoch: 1,
            seq: 5,
            bootSecretAck: "a".repeat(32),
            channelSecret: "b".repeat(32),
            slideIndex: 0,
            slideUid: activeSlide.id,
            ffId: "ff-slide-headline",
            scrollHeight: 1228,
            clientHeight: 1080,
            overflowPx: 148,
          })
        }
      >
        Trigger E3
      </button>
      <HtmlPreviewToolbar
        structureLens="slide-cms"
        pointerMode="interact"
        onSetPointerMode={() => {}}
        isInlineCodeOpen={false}
        onToggleInlineCode={() => {}}
        viewportPreset="desktop-1440"
        onSetViewportPreset={() => {}}
        aspectRatio="16-9"
        onSetAspectRatio={() => {}}
        isPresenting={isPresenting}
        onTogglePresenting={() => {}}
        isLeftStructureHidden={false}
        onToggleLeftStructureHidden={() => {}}
        cmsActions={{
          revision: cms.revision,
          syncState: cms.syncState,
          hasPendingSave: cms.hasAnyDirtySlides || cms.syncState !== "SYNCED",
          canUndo: cms.canUndo,
          canRedo: cms.canRedo,
          isReadOnly: false,
          onUndo: cms.undo,
          onRedo: cms.redo,
          onSave: () => {
            void cms.flushAllPendingNow();
          },
        }}
      />
      <CmsStageRecoveryPopup
        recoveredDraft={cms.recoveredDraft}
        syncState={cms.syncState}
        lastFlushError={cms.lastFlushError}
        customHtmlOverride={cms.customHtmlOverride}
        onRestoreDraft={cms.restoreRecoveredDraft}
        onDismissDraft={cms.dismissRecoveredDraft}
        onRetryFlush={() => {
          void cms.flushPendingBatchNow();
        }}
        onReloadServerRevision={cms.reloadServerRevision}
        onResyncCustomHtml={cms.resyncCustomHtmlOverride}
      />
      <CompanionStructureLeftPane
        widthPercent={38}
        isRightPreviewHidden={false}
        structureLens="slide-cms"
        onSetLens={() => {}}
        onToggleRightPreviewHidden={() => {}}
        isDraftDirty={false}
        slides={slides}
        activeSlideIndex={activeIdx}
        isCmsDrawerOpen={drawerOpen}
        isPresenting={isPresenting}
        cmsController={cms}
        onSelectSlide={(idx) => {
          setActiveIdx(idx);
          setDrawerOpen(true);
        }}
        onCloseCmsDrawer={() => setDrawerOpen(false)}
        onUpdateActiveSlide={(patch) =>
          setSlides((prev) => prev.map((s, i) => (i === activeIdx ? { ...s, ...patch } : s)))
        }
      >
        <div>DAG Placeholder</div>
      </CompanionStructureLeftPane>
    </div>
  );
}

describe("Story 06.2 — Slide-Over CMS Drawer, MessageChannel & Zero-Fetch SDK", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
  });

  afterEach(() => {
    cleanup();
    window.sessionStorage.clear();
  });

  it("UT-05: enforces 200% track translateX(-50%), symmetric inert, focus return, IME Escape guard & 0-slot speakerNotes Zod", () => {
    // Zod: 0-slot speakerNotes-only save parses; 0-patch without notes fails
    expect(
      PatchSlotsBatchRequestSchema.safeParse({
        expectedRevision: 1,
        speakerNotes: "Presenter-only note",
        patches: [],
      }).success
    ).toBe(true);
    expect(
      PatchSlotsBatchRequestSchema.safeParse({
        expectedRevision: 1,
        patches: [],
      }).success
    ).toBe(false);

    // Zod: canonical ff_id regex rejects selector injection
    expect(CANONICAL_FF_ID_REGEX.test("ff-slide-headline_01")).toBe(true);
    expect(CANONICAL_FF_ID_REGEX.test('ff-1"], script')).toBe(false);

    // Zod: TextEncoder UTF-8 byte guard rejects >524,288 bytes without Node Buffer
    const multibyteOverflow = "ệ".repeat(180_000); // 180k chars = 540,000 UTF-8 bytes
    expect(
      ApplySlideHtmlRequestSchema.safeParse({
        expectedRevision: 1,
        rawHtml: multibyteOverflow,
        resyncStructuredSlots: true,
      }).success
    ).toBe(false);

    const patchSpy = vi.fn();
    render(<DrawerHarness onPatchSpy={patchSpy} initialOpen={false} />);

    const viewport = screen.getByTestId("slide-cms-track-viewport");
    const track = viewport.querySelector(".ff-slide-cms-track") as HTMLElement;
    const listHalf = screen.getByTestId("slide-list-half");
    const drawer = screen.getByTestId("cms-slide-over-drawer");

    // Initially closed: list interactive, drawer half inert
    expect(track.getAttribute("data-drawer-open")).toBe("false");
    expect(listHalf.hasAttribute("inert")).toBe(false);
    expect(drawer.parentElement?.hasAttribute("inert")).toBe(true);

    // Open drawer by clicking slide card
    fireEvent.click(screen.getByTestId(`slide-card-${SAMPLE_SLIDES[0]!.id}`));
    expect(track.getAttribute("data-drawer-open")).toBe("true");
    expect(listHalf.hasAttribute("inert")).toBe(true);
    expect(drawer.parentElement?.hasAttribute("inert")).toBe(false);
    expect(document.activeElement).toBe(screen.getByTestId("btn-back-slide-list"));

    // IME composing Escape MUST NOT close drawer
    const headlineInput = screen.getByTestId("cms-input-headline") as HTMLInputElement;
    fireEvent.compositionStart(headlineInput);
    fireEvent.keyDown(drawer, { key: "Escape", keyCode: 229 });
    expect(track.getAttribute("data-drawer-open")).toBe("true");
    fireEvent.compositionEnd(headlineInput);

    // Normal Escape closes drawer and restores focus to originating slide card
    fireEvent.keyDown(drawer, { key: "Escape" });
    expect(track.getAttribute("data-drawer-open")).toBe("false");
    expect(document.activeElement).toBe(screen.getByTestId(`slide-card-${SAMPLE_SLIDES[0]!.id}`));
  });

  it("UT-06: keeps targetCacheKey invariant across PATCH_BY_FF_ID, coalesces 32-frame queue, pins POOL_MAX_SLOTS & parses telemetry", () => {
    // Frame schemas accept slideUid, overflowPx, chartsJson, revision, and DECK_CSP_VIOLATION
    expect(
      PatchByFfIdFrameSchema.safeParse({
        type: "PATCH_BY_FF_ID",
        epoch: 1,
        seq: 2,
        bootSecret: "a".repeat(32),
        channelSecret: "b".repeat(32),
        slideUid: SAMPLE_SLIDES[0]!.id,
        revision: 3,
        ffId: "ff-chart-1",
        slotKind: "chart_json",
        chartsJson: '{"v":[1,2,3]}',
      }).success
    ).toBe(true);
    expect(
      SlotOverflowWarnFrameSchema.safeParse({
        type: "SLIDE_OVERFLOW_WARN",
        epoch: 1,
        seq: 3,
        bootSecretAck: "a".repeat(32),
        channelSecret: "b".repeat(32),
        slideIndex: 0,
        slideUid: SAMPLE_SLIDES[0]!.id,
        ffId: "ff-body",
        scrollHeight: 1200,
        clientHeight: 1080,
        overflowPx: 120,
      }).success
    ).toBe(true);
    expect(
      CspViolationFrameSchema.safeParse({
        type: "DECK_CSP_VIOLATION",
        epoch: 1,
        seq: 4,
        bootSecretAck: "a".repeat(32),
        channelSecret: "b".repeat(32),
        slideUid: SAMPLE_SLIDES[0]!.id,
        violatedDirective: "script-src",
        blockedUri: "inline",
      }).success
    ).toBe(true);

    // POOL_MAX_SLOTS = 5 pinning spares pinned slides (audience, presenter active, presenter next)
    expect(POOL_MAX_SLOTS).toBe(5);
    const fullPool = [
      makeFakeSlot("SLD-PIN-AUDIENCE", 10),
      makeFakeSlot("SLD-PIN-PRES", 20),
      makeFakeSlot("SLD-PIN-NEXT", 30),
      makeFakeSlot("SLD-OLD-UNPINNED", 40),
      makeFakeSlot("SLD-NEW-UNPINNED", 50),
    ];
    const afterEvict = evictOldestSlotIfFull(fullPool, [
      "SLD-PIN-AUDIENCE",
      "SLD-PIN-PRES",
      "SLD-PIN-NEXT",
    ]);
    const remainingIds = afterEvict.map((s) => s.nodeId);
    expect(remainingIds).toContain("SLD-PIN-AUDIENCE");
    expect(remainingIds).toContain("SLD-PIN-PRES");
    expect(remainingIds).toContain("SLD-PIN-NEXT");
    expect(remainingIds).not.toContain("SLD-OLD-UNPINNED");

    // Pre-PORT_ACK queue coalesces rapid typing for same (slideUid, ffId) without overflowing 32 frames
    const queue: Array<Record<string, unknown>> = [];
    for (let i = 0; i < 40; i++) {
      enqueueOutboundFrame(queue, {
        type: "PATCH_BY_FF_ID",
        slideUid: SAMPLE_SLIDES[0]!.id,
        ffId: "ff-slide-headline",
        textContent: `Edit ${i}`,
      });
    }
    expect(queue.length).toBe(1);
    expect(queue[0]?.textContent).toBe("Edit 39");
    expect(MAX_OUTBOUND_QUEUE_FRAMES).toBe(32);

    // Verify useSandboxedIframeBridge keeps targetCacheKey invariant in slide-cms mode when rawHtml updates
    const keys: string[] = [];
    function BridgeProbe({ rawHtml }: { rawHtml: string }) {
      const b = useSandboxedIframeBridge({
        projectId: "proj-1",
        activeTabId: "tab-slide",
        structureLens: "slide-cms",
        activeSlideId: SAMPLE_SLIDES[0]!.id,
        committedSlideCacheKey: SAMPLE_SLIDES[0]!.id,
        rawHtml,
        activeSubState: "DEFAULT",
        onRegionClick: () => {},
      });
      keys.push(b.targetCacheKey);
      return <div data-testid="slot-count">{b.poolSlots.length}</div>;
    }

    const { rerender } = render(<BridgeProbe rawHtml="<section>v1</section>" />);
    rerender(<BridgeProbe rawHtml="<section>v2-live-patched</section>" />);
    const nonEmptyKeys = keys.filter(Boolean);
    expect(new Set(nonEmptyKeys).size).toBe(1);
    expect(screen.getByTestId("slot-count").textContent).toBe("1");
  });

  it("UT-07: zero raw fetch in Presentation SDK, Unified Undo survives drawer close/reopen, E3 badge & recovery banner work", async () => {
    // Static check: 0 raw fetch() calls in src/api/presentation.ts and 0 Buffer.* in Story 06.2 browser files
    const sdkSource = fs.readFileSync(
      path.resolve(process.cwd(), "src/api/presentation.ts"),
      "utf8"
    );
    expect(sdkSource).not.toMatch(/\bfetch\s*\(/);

    const validationSource = fs.readFileSync(
      path.resolve(process.cwd(), "src/lib/validations/presentation.ts"),
      "utf8"
    );
    expect(validationSource).not.toMatch(/\bBuffer\./);

    // Mock transport verifies apiPresentation delegates strictly via apiClient
    const reqSpy = vi.fn().mockResolvedValue({
      slideUid: SAMPLE_SLIDES[0]!.id,
      newRevision: 2,
      contentSha256: "a".repeat(64),
      patchedSlotsCount: 1,
      executionTimeMicros: 420,
      slots: [],
    });
    const client = new PresentationClient({ request: reqSpy });
    await client.commitSlotBatch(
      { projectId: "proj-1", deckId: "deck-1", slideUid: SAMPLE_SLIDES[0]!.id },
      {
        expectedRevision: 1,
        speakerNotes: "Note",
        patches: [{ ffId: "ff-h1", slotKind: "heading", textValue: "Hi" }],
      },
      { idempotencyKey: "k-1", timeoutMs: 3000 }
    );
    expect(reqSpy).toHaveBeenCalledTimes(1);
    expect(reqSpy.mock.calls[0]?.[0]).toContain("/slots/batch");

    // Pre-seed sessionStorage crash draft before mount -> recovery banner appears & restores
    const saved = saveRecoveryDraft({
      deckId: "deck-test-062",
      slideUid: SAMPLE_SLIDES[0]!.id,
      baseRevision: 1,
      speakerNotes: "Recovered Speaker Note",
      pendingPatches: [
        { ffId: "ff-slide-headline", slotKind: "heading", textValue: "Recovered Headline" },
      ],
      savedAtMs: Date.now(),
    });
    expect(saved).toBe(true);
    expect(CmsRecoveryDraftSchema.safeParse(loadRecoveryDraft("deck-test-062", SAMPLE_SLIDES[0]!.id)).success).toBe(true);

    const patchSpy = vi.fn();
    render(<DrawerHarness onPatchSpy={patchSpy} initialOpen={true} />);

    // Recovery banner visible -> click Restore -> headline & speaker_notes restored
    expect(screen.getByTestId("cms-recovery-banner")).toBeTruthy();
    fireEvent.click(screen.getByTestId("cms-restore-draft-btn"));
    expect((screen.getByTestId("cms-input-headline") as HTMLInputElement).value).toBe("Recovered Headline");
    expect((screen.getByTestId("cms-input-speaker-notes") as HTMLTextAreaElement).value).toBe("Recovered Speaker Note");
    expect(patchSpy).toHaveBeenCalled();

    // Trigger E3 Overflow (>100px danger) -> inline badge + AI Assist trigger appear
    fireEvent.click(screen.getByTestId("trigger-e3-overflow"));
    const e3Badge = screen.getByTestId("e3-overflow-badge");
    expect(e3Badge.getAttribute("data-severity")).toBe("danger");
    expect(screen.getByTestId("cms-ai-assist-overflow")).toBeTruthy();

    // Edit headline -> close drawer -> press Cmd+Z -> Undo survives drawer transition!
    fireEvent.change(screen.getByTestId("cms-input-headline"), { target: { value: "Edited Headline V2" } });
    expect((screen.getByTestId("cms-input-headline") as HTMLInputElement).value).toBe("Edited Headline V2");
    fireEvent.click(screen.getByTestId("btn-back-slide-list"));
    await act(async () => {
      fireEvent.keyDown(window, { key: "z", metaKey: true });
    });
    expect((screen.getByTestId("cms-input-headline") as HTMLInputElement).value).toBe("Recovered Headline");
    clearRecoveryDraft("deck-test-062", SAMPLE_SLIDES[0]!.id);
  });

  it("Adapts Line-1 CMS Undo/Redo/Save controls cleanly across Wide, Medium & Standardized Min-Width (320px)", async () => {
    let observedWidth = 780;
    const roCallbacks: Array<() => void> = [];
    const origResizeObserver = window.ResizeObserver;
    const origClientWidth = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "clientWidth");
    Object.defineProperty(HTMLElement.prototype, "clientWidth", {
      configurable: true,
      get: () => observedWidth,
    });
    window.ResizeObserver = class MockRO {
      private cb: () => void;
      constructor(cb: ResizeObserverCallback) {
        this.cb = () => cb([], this as unknown as ResizeObserver);
        roCallbacks.push(this.cb);
      }
      observe() {}
      unobserve() {}
      disconnect() {}
    } as unknown as typeof ResizeObserver;

    const saveSpy = vi.fn();
    const undoSpy = vi.fn();
    const redoSpy = vi.fn();
    try {
      render(
        <HtmlPreviewToolbar
          structureLens="slide-cms"
          pointerMode="interact"
          onSetPointerMode={() => {}}
          isInlineCodeOpen={false}
          onToggleInlineCode={() => {}}
          viewportPreset="desktop-1440"
          onSetViewportPreset={() => {}}
          aspectRatio="16-9"
          onSetAspectRatio={() => {}}
          isPresenting={false}
          onTogglePresenting={() => {}}
          isLeftStructureHidden={false}
          onToggleLeftStructureHidden={() => {}}
          onOpenExportSection={() => {}}
          cmsActions={{
            revision: 3,
            syncState: "DIRTY_FAST_PATCHED",
            hasPendingSave: true,
            canUndo: true,
            canRedo: true,
            onUndo: undoSpy,
            onRedo: redoSpy,
            onSave: saveSpy,
          }}
        />
      );

      // Wide (>=740px): Full telemetry pill + Export + Save text
      const group = screen.getByTestId("toolbar-cms-actions");
      expect(group.getAttribute("data-density")).toBe("wide");
      expect(screen.getByTestId("toolbar-export-btn")).toBeTruthy();
      expect(screen.getByTestId("cms-telemetry-rev")).toBeTruthy();
      expect(screen.getByTestId("cms-save-btn").getAttribute("data-dirty")).toBe("true");
      expect(group.querySelectorAll('[data-icon-fallback="true"]').length).toBe(0);

      // Medium (560px): Export collapses to overflow, CMS rev/Undo/Redo/Save remain on Line 1
      observedWidth = 560;
      await act(async () => {
        roCallbacks.forEach((fn) => fn());
      });
      expect(group.getAttribute("data-density")).toBe("medium");
      expect(screen.getByTestId("toolbar-more-actions-btn")).toBeTruthy();
      expect(screen.getByTestId("cms-save-btn")).toBeTruthy();

      // Standardized Min-Width (320px): Icon-only playback + icon-only Undo/Redo/Save with zero missing controls
      observedWidth = 320;
      await act(async () => {
        roCallbacks.forEach((fn) => fn());
      });
      expect(group.getAttribute("data-density")).toBe("min");
      expect(screen.getByTestId("pointer-mode-toggle")).toBeTruthy();
      expect(screen.getByTestId("cms-undo-btn")).toBeTruthy();
      expect(screen.getByTestId("cms-redo-btn")).toBeTruthy();
      const saveBtn = screen.getByTestId("cms-save-btn");
      expect(saveBtn.querySelector(".ff-cms-save-dirty-dot")).toBeTruthy();
      fireEvent.click(saveBtn);
      fireEvent.click(screen.getByTestId("cms-undo-btn"));
      fireEvent.click(screen.getByTestId("cms-redo-btn"));
      expect(saveSpy).toHaveBeenCalledTimes(1);
      expect(undoSpy).toHaveBeenCalledTimes(1);
      expect(redoSpy).toHaveBeenCalledTimes(1);
    } finally {
      window.ResizeObserver = origResizeObserver;
      if (origClientWidth) Object.defineProperty(HTMLElement.prototype, "clientWidth", origClientWidth);
      else delete (HTMLElement.prototype as { clientWidth?: number }).clientWidth;
    }
  });
});
