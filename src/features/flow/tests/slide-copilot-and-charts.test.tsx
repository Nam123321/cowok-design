// @vitest-environment jsdom
// @story 06.3
// @implements AC-01, AC-02, AC-03, AC-04
import React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { ChartSpecV12Schema } from "../../../lib/validations/presentation-chart-specs";
import { SlideCopilotLeaseDtoSchema } from "../../../lib/validations/presentation-charts";
import { processCopilotSseEvent } from "../hooks/slide-copilot-sse";
import { createEmptyCopilotLease } from "../hooks/useSlideCopilotHelpers";
import {
  parseChartSpecSafely,
  renderOffline2DChartSvg,
} from "../utils/offline-chart-2d-renderer";
import { OfflineChart2DRenderer } from "../components/OfflineChart2DRenderer";
import { FloatingSlideCopilot, SlideCopilotToastBanner } from "../components/FloatingSlideCopilot";
import { CmsBlockEditor } from "../components/CmsBlockEditor";
import { useSlideCopilotController } from "../hooks/useSlideCopilotController";
import { processValidSlotMessage } from "../utils/sandbox-bridge-protocol";
import { apiPresentationCharts } from "../../../api/presentation-charts";
import { apiFlowExecution } from "../../../api/flow-execution";
import { HarnessDaemonApiError } from "../../../api/harness-daemon-transport";

const SLIDE_UID = "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV";
const SHA_A = "a".repeat(64);
const SECRET = "s".repeat(24);

describe("Story 06.3 — Offline 2D SVG Charts & Contextual Slide Copilot", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("UT-03: validates all 7 2D chart kinds, zero-sum +-0.05 tolerance & DoS limits offline with pure SVG output", () => {
    const kinds = ["pie", "donut", "100_stacked", "funnel", "bar", "line", "area"] as const;
    for (const kind of kinds) {
      const spec = ChartSpecV12Schema.parse({
        kind,
        title: `Metric (${kind})`,
        unit: "%",
        series: [{ name: "S1", points: [{ label: "Alpha", value: 60.03 }, { label: "Beta", value: 40.0 }] }],
      });
      const res = renderOffline2DChartSvg(document, spec);
      expect(res.svg.tagName.toLowerCase()).toBe("svg");
      expect(res.svg.getAttribute("role")).toBe("img");
      expect(res.svg.getAttribute("aria-label")).toContain(`Metric (${kind})`);
      expect(res.svg.innerHTML).not.toContain("<script");
      expect(res.svg.innerHTML).not.toContain("foreignObject");
    }

    const badSum = parseChartSpecSafely(JSON.stringify({
      kind: "pie",
      title: "Bad Sum",
      series: [{ name: "S1", points: [{ label: "A", value: 40 }, { label: "B", value: 40 }] }],
    }));
    expect(badSum.ok).toBe(false);
    if (!badSum.ok) expect(badSum.code).toBe("E_CHART_SUM_NOT_100");

    const negProp = parseChartSpecSafely(JSON.stringify({
      kind: "funnel",
      title: "Neg Funnel",
      series: [{ name: "S1", points: [{ label: "A", value: -10 }, { label: "B", value: 110 }] }],
    }));
    expect(negProp.ok).toBe(false);
    if (!negProp.ok) expect(negProp.code).toBe("E_CHART_NEGATIVE_PROPORTIONAL");

    const negBar = ChartSpecV12Schema.safeParse({
      kind: "bar",
      title: "Signed Bar",
      series: [{ name: "Delta", points: [{ label: "Q1", value: -25 }, { label: "Q2", value: 75 }] }],
    });
    expect(negBar.success).toBe(true);

    const dosOverflow = parseChartSpecSafely(JSON.stringify({
      kind: "line",
      title: "DoS Huge",
      series: [{ name: "S1", points: [{ label: "P", value: 1_500_000 }] }],
    }));
    expect(dosOverflow.ok).toBe(false);
    if (!dosOverflow.ok) expect(dosOverflow.code).toBe("E_CHART_DOS_LIMIT");
  });

  it("UT-04: E2 table edit breaking zero-sum preserves last-valid SVG preview & renders recovery CTAs without auto-balance", () => {
    const validSpec = JSON.stringify({
      kind: "pie",
      title: "Revenue Share",
      series: [{ name: "S1", points: [{ label: "Core", value: 70 }, { label: "Add-on", value: 30 }] }],
    });
    const onChangeChartsJson = vi.fn();
    const onRetryWithCopilot = vi.fn();
    render(
      <OfflineChart2DRenderer
        ffId="ff-chart-rev"
        rawJson={validSpec}
        onChangeChartsJson={onChangeChartsJson}
        onRetryWithCopilot={onRetryWithCopilot}
      />
    );

    const preview = screen.getByTestId("chart-last-valid-preview");
    expect(preview.querySelector("svg[role='img']")).toBeTruthy();

    fireEvent.change(screen.getByTestId("chart-point-value-0"), { target: { value: "50" } });
    expect(screen.getByTestId("chart-validation-retry")).toBeTruthy();
    expect(screen.getByTestId("chart-stage-intact-badge")).toBeTruthy();
    expect(preview.querySelector("svg[role='img']")).toBeTruthy();
    expect(onChangeChartsJson).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId("chart-retry-copilot-btn"));
    expect(onRetryWithCopilot).toHaveBeenCalledWith("ff-chart-rev", [expect.stringContaining("E_CHART_SUM_NOT_100")]);
  });

  it("UT-05: handles Pre-Flush barrier, 409 E_PREFLUSH_CONFLICT recovery, SSE commit, lease lock & SNAP read-only guard", async () => {
    const flushSlideBeforeDispatch = vi.fn().mockResolvedValue(true);
    const reloadServerRevision = vi.fn();
    const applyCopilotBatchCommit = vi.fn();
    let capturedOnEvents: ((events: Array<Record<string, unknown>>) => void) | null = null;

    vi.spyOn(apiFlowExecution, "streamJobEvents").mockImplementation(async (_pid, opts) => {
      capturedOnEvents = opts.onEvents as unknown as typeof capturedOnEvents;
      return Promise.resolve();
    });
    const dispatchSpy = vi
      .spyOn(apiPresentationCharts, "dispatchSlideCopilot")
      .mockRejectedValueOnce(new HarnessDaemonApiError(409, "E_PREFLUSH_CONFLICT", "Slide changed concurrently"))
      .mockResolvedValueOnce({
        jobId: "JOB-COP-1",
        slideUid: SLIDE_UID,
        targetFfId: "ff-headline",
        compositeScopeKey: `p1::slide-copilot-d1::${SLIDE_UID}`,
        status: "RUNNING",
        fencingToken: 4,
        queuePosition: 0,
        leaseTtlMs: 60_000,
        leaseExpiresAtMs: Date.now() + 60_000,
        activeGlobalSlots: 1,
      });

    let ctrl!: ReturnType<typeof useSlideCopilotController>;
    const Harness: React.FC<{ isReadOnly?: boolean }> = ({ isReadOnly = false }) => {
      ctrl = useSlideCopilotController({
        enabled: true,
        projectId: "proj-1",
        deckId: "deck-1",
        slideUid: SLIDE_UID,
        isReadOnly,
        slots: [
          { id: "sl-1", slideId: SLIDE_UID, ffId: "ff-headline", slotKind: "heading", slotLabel: "Headline", textValue: "Old", safeStylesJson: "{}", chartsJson: "[]", updatedBy: "human", astRevision: 2, updatedAt: 1 },
        ],
        flushSlideBeforeDispatch,
        getSlideCommitSnapshot: () => ({ revision: 2, contentSha256: SHA_A }),
        reloadServerRevision,
        applyCopilotBatchCommit,
      });
      return (
        <>
          <CmsBlockEditor
            structureLens="slide-cms"
            activeSlide={{ id: SLIDE_UID, code: "S1", title: "Cover", headline: "Old", subtitle: "Sub", kpi1: 1, kpi2: "OK", kpi3: "10ms" }}
            slideUid={SLIDE_UID}
            slots={[{ id: "sl-1", slideId: SLIDE_UID, ffId: "ff-headline", slotKind: "heading", slotLabel: "Headline", textValue: "Old", safeStylesJson: "{}", chartsJson: "[]", updatedBy: "human", astRevision: 2, updatedAt: 1 }]}
            speakerNotes=""
            syncState="SYNCED"
            overflowAlert={null}
            customHtmlOverride={false}
            isReadOnly={isReadOnly}
            onCloseDrawer={vi.fn()}
            onComposingChange={vi.fn()}
            onEditField={vi.fn()}
            onEditSpeakerNotes={vi.fn()}
            copilotLease={ctrl.activeLease}
            onCancelCopilot={() => { void ctrl.cancelCopilot(); }}
          />
          <SlideCopilotToastBanner toast={ctrl.receiptToast} onDismiss={ctrl.dismissReceiptToast} />
          <FloatingSlideCopilot
            isOpen={ctrl.isOpen}
            isCollapsed={ctrl.isCollapsed}
            isReadOnly={isReadOnly}
            slideCode="S1"
            slideUid={SLIDE_UID}
            targetFfId={ctrl.targetFfId}
            draftPrompt={ctrl.draftPrompt}
            attachedComment={ctrl.attachedComment}
            activeLease={ctrl.activeLease}
            messages={ctrl.messages}
            activeGlobalSlots={ctrl.activeGlobalSlots}
            maxGlobalSlots={ctrl.maxGlobalSlots}
            onDraftPromptChange={ctrl.setDraftPrompt}
            onSendPrompt={() => { void ctrl.submitPrompt(ctrl.draftPrompt); }}
            onCancelLease={() => { void ctrl.cancelCopilot(); }}
            onCollapse={() => ctrl.setIsCollapsed(true)}
            onExpand={() => ctrl.setIsCollapsed(false)}
            onClose={() => ctrl.setIsOpen(false)}
            onClearAttachedComment={ctrl.clearAttachedComment}
          />
        </>
      );
    };

    const { rerender } = render(<Harness />);
    act(() => { fireEvent.keyDown(window, { key: "/", altKey: true }); });
    expect(screen.getByTestId("floating-slide-copilot")).toBeTruthy();

    await act(async () => { await ctrl.submitPrompt("Tighten headline"); });
    expect(flushSlideBeforeDispatch).toHaveBeenCalledWith(SLIDE_UID, true);
    expect(reloadServerRevision).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("copilot-error-banner").textContent).toContain("E_PREFLUSH_CONFLICT");

    await act(async () => { await ctrl.submitPrompt("Tighten headline"); });
    expect(dispatchSpy).toHaveBeenCalledTimes(2);
    expect(screen.getByTestId("slide-copilot-lease-badge")).toBeTruthy();
    expect(screen.getByTestId("cms-input-headline")).toHaveProperty("disabled", true);
    expect(screen.getByTestId("copilot-countdown-badge").textContent).toContain("60s");

    await act(async () => {
      capturedOnEvents?.([
        {
          id: "ev-1",
          jobId: "JOB-COP-1",
          projectId: "proj-1",
          nodeId: SLIDE_UID,
          eventKind: "completed",
          payloadText: JSON.stringify({
            slideUid: SLIDE_UID,
            newRevision: 3,
            contentSha256: "b".repeat(64),
            slots: [{ id: "sl-1", slideId: SLIDE_UID, ffId: "ff-headline", slotKind: "heading", slotLabel: "Headline", textValue: "Sharp New Headline", safeStylesJson: "{}", chartsJson: "[]", updatedBy: "copilot", astRevision: 3, updatedAt: 2 }],
          }),
          seqNo: 1,
          projectSeq: 10,
          jobStatus: "COMPLETED",
          createdAt: "2026-11-19T12:00:00Z",
        },
      ]);
    });

    expect(applyCopilotBatchCommit).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("copilot-commit-toast").textContent).toContain("3");
    expect(screen.getByTestId("cms-input-headline")).toHaveProperty("disabled", false);

    rerender(<Harness isReadOnly={true} />);
    expect(screen.getByTestId("slide-copilot-readonly-guard")).toBeTruthy();
    expect(screen.queryByTestId("slide-copilot-textarea")).toBeNull();
  });

  it("UT-06: validates CHART_RENDER_ACK & STAGE_MAT_CLICK frames against active epoch & channelSecret", () => {
    const onDeckProtocolEvent = vi.fn();
    const closeSpy = vi.fn();
    const slot = {
      epoch: 3,
      bootSecret: SECRET,
      channelSecret: SECRET,
      isPortVerified: true,
      lastAcceptedSeq: 0,
    } as unknown as Parameters<typeof processValidSlotMessage>[1]["slot"];
    const deps = {
      slot,
      channel: { port1: { close: closeSpy } } as unknown as MessageChannel,
      latestParamsRef: { current: { onDeckProtocolEvent } } as unknown as Parameters<typeof processValidSlotMessage>[1]["latestParamsRef"],
    } as Parameters<typeof processValidSlotMessage>[1];

    processValidSlotMessage(
      {
        data: {
          type: "CHART_RENDER_ACK",
          epoch: 3,
          seq: 1,
          bootSecretAck: SECRET,
          channelSecret: SECRET,
          slideUid: SLIDE_UID,
          ffId: "ff-chart-1",
          svgPrimitiveCount: 18,
          renderTimeMs: 4.2,
          boundsRect: { x: 0, y: 0, width: 320, height: 180 },
        },
      } as MessageEvent,
      deps
    );
    expect(onDeckProtocolEvent).toHaveBeenCalledWith(expect.objectContaining({ type: "CHART_RENDER_ACK", ffId: "ff-chart-1" }));
    expect(closeSpy).not.toHaveBeenCalled();

    processValidSlotMessage(
      { data: { type: "STAGE_MAT_CLICK", epoch: 2, seq: 2, bootSecretAck: SECRET, channelSecret: SECRET, slideUid: SLIDE_UID } } as MessageEvent,
      deps
    );
    expect(closeSpy).toHaveBeenCalledTimes(1);
  });

  it("UT-07: tolerates cancel DTO empty targetFfId, handles started/interrupted SSE, and preserves input focus across chart label edits", () => {
    const cancelDto = SlideCopilotLeaseDtoSchema.parse({
      jobId: "job-c1",
      slideUid: SLIDE_UID,
      targetFfId: "",
      compositeScopeKey: `presentation:p1:${SLIDE_UID}`,
      status: "CANCELLED",
      fencingToken: 2,
      queuePosition: 0,
      leaseTtlMs: 60000,
      leaseExpiresAtMs: null,
      activeGlobalSlots: 0,
    });
    expect(cancelDto.targetFfId).toBe("");

    let leases: Record<string, ReturnType<typeof createEmptyCopilotLease>> = {
      [SLIDE_UID]: { ...createEmptyCopilotLease(SLIDE_UID), leaseState: "queued", jobId: "job-q1", queuePosition: 1 },
    };
    const setLeases: React.Dispatch<React.SetStateAction<typeof leases>> = (up) => {
      leases = typeof up === "function" ? up(leases) : up;
    };
    const deps = {
      getSnapshot: () => ({ revision: 1, contentSha256: SHA_A }),
      applyCommit: vi.fn(),
      setLeases,
      setMessages: vi.fn(),
      setChartErrors: vi.fn(),
      setToast: vi.fn(),
      setGlobalSlots: vi.fn(),
    };
    processCopilotSseEvent(
      {
        id: "ev-start",
        jobId: "job-q1",
        projectId: "p1",
        nodeId: SLIDE_UID,
        eventKind: "started",
        payloadText: JSON.stringify({ promoted: true, fencingToken: 2, leaseExpiresAtMs: Date.now() + 60000 }),
        seqNo: 1,
        projectSeq: 1,
        compositeScopeKey: `presentation:p1:${SLIDE_UID}`,
        jobStatus: "RUNNING",
        createdAt: new Date().toISOString(),
      },
      deps
    );
    expect(leases[SLIDE_UID]?.leaseState).toBe("active");

    processCopilotSseEvent(
      {
        id: "ev-exp",
        jobId: "job-q1",
        projectId: "p1",
        nodeId: SLIDE_UID,
        eventKind: "interrupted",
        payloadText: JSON.stringify({ reason: "E_COPILOT_LEASE_TIMEOUT", fencingToken: 3, copilotLeaseExpired: true }),
        seqNo: 2,
        projectSeq: 2,
        compositeScopeKey: `presentation:p1:${SLIDE_UID}`,
        jobStatus: "INTERRUPTED",
        createdAt: new Date().toISOString(),
      },
      deps
    );
    expect(leases[SLIDE_UID]?.leaseState).toBe("unlocked");
    expect(leases[SLIDE_UID]?.errorBanner?.code).toBe("E_COPILOT_LEASE_TIMEOUT");

    const pieJson = JSON.stringify({
      kind: "pie",
      title: "Mix",
      series: [{ name: "S1", points: [{ label: "A", value: 60 }, { label: "B", value: 40 }] }],
    });
    const onChangeChartsJson = vi.fn();
    render(
      <OfflineChart2DRenderer ffId="ff-chart-1" rawJson={pieJson} onChangeChartsJson={onChangeChartsJson} />
    );
    const labelInput = screen.getByDisplayValue("A") as HTMLInputElement;
    labelInput.focus();
    fireEvent.change(labelInput, { target: { value: "Alpha" } });
    expect(document.activeElement).toBe(screen.getByDisplayValue("Alpha"));
  });
});
