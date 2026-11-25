// @vitest-environment jsdom
// @story 10.3
// @implements AC-01, AC-02, AC-03, AC-04 (Unit Tests: Anonymized CMP-07 Arena, Blind Wipe Desktop/390px, Monotonic generationEpoch Rapid Switch, Cross-Variant Borrow, /crystallize & TournamentApiClient Zod Enforcement)
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createTournamentApiClient, TournamentApiError } from "../../api/tournament";
import {
  type CrystallizeResponse,
  type ListTournamentRunsResponse,
  type SelectTournamentVariantRequest,
  type SelectTournamentVariantResponse,
  type StartTournamentData,
  type StartTournamentResponse,
} from "../../lib/validations/tournament";
import { setGlobalLocale } from "../../hooks/useTranslation";
import { extractDonorVisualZones } from "./CrossVariantZonePickerModal";
import { DesignTournamentArenaPanel } from "./DesignTournamentArenaPanel";
import { DesignTournamentStudioDrawer } from "./DesignTournamentStudioDrawer";
import {
  formatTournamentRunTimestamp,
  formatVariantDiversityBadge,
  parseSubChatDirectBorrow,
  type DesignTournamentPort,
} from "./useDesignTournament";

const PROJECT_ID = "11111111-1111-4111-8111-111111111111";
const SHA_A = "a".repeat(64);
const SHA_B = "b".repeat(64);
const SHA_C = "c".repeat(64);

function buildSampleRun(): StartTournamentData {
  return {
    runId: "run-103-001",
    projectId: PROJECT_ID,
    prompt: "Bảng điều khiển vận hành tối giản",
    axes: ["INFORMATION_DENSITY", "NAVIGATION_TOPOLOGY", "VISUAL_METAPHOR"],
    generationEpoch: 1,
    budgetMs: 15_000,
    elapsedMs: 120,
    degradedMinVariants: false,
    timedOutAxes: [],
    lowDiversityWarning: false,
    createdAt: 1_764_000_000_000,
    variants: [
      {
        id: "var-a",
        displayLabel: "Option A",
        axis: "INFORMATION_DENSITY",
        axisProfile: "editorial-kpi-strip-definition-list",
        leasedPath: ".cowok-design/tournaments/run-103-001/information_density.html",
        html: "<main data-ff-id='ff-main'><h1>Option A</h1></main>",
        artifactSha256: SHA_A,
        astSimilarity: 0.0,
        cloneDistancePassed: true,
        lowDiversityWarning: false,
        rerollAttempts: 0,
        rubricVerdict: "RECOMMENDED_BLIND_WINNER",
        rubricWins: 2,
        deScoreBps: 9_100,
        elapsedMs: 40,
        archived: false,
      },
      {
        id: "var-b",
        displayLabel: "Option B",
        axis: "NAVIGATION_TOPOLOGY",
        axisProfile: "rail-nav-split-tablist-ledger",
        leasedPath: ".cowok-design/tournaments/run-103-001/navigation_topology.html",
        html: "<style>.rail-nav{display:flex;}</style><main data-ff-id='ff-nav-main'><nav class='rail-nav' data-ff-id='ff-rail-navigation'>Nav</nav><table class='data-ledger' data-ff-id='ff-data-ledger'><tbody><tr><td>Ledger Row</td></tr></tbody></table></main>",
        artifactSha256: SHA_B,
        astSimilarity: 0.34,
        cloneDistancePassed: true,
        lowDiversityWarning: false,
        rerollAttempts: 0,
        rubricVerdict: "CONTENDER",
        rubricWins: 1,
        deScoreBps: 8_600,
        elapsedMs: 38,
        archived: false,
      },
      {
        id: "var-c",
        displayLabel: "Option C",
        axis: "VISUAL_METAPHOR",
        axisProfile: "spotlight-bento-mosaic-callout",
        leasedPath: ".cowok-design/tournaments/run-103-001/visual_metaphor.html",
        html: "<main data-ff-id='ff-bento-main'><figure data-ff-id='ff-spotlight-card'>Bento</figure></main>",
        artifactSha256: SHA_C,
        astSimilarity: 0.29,
        cloneDistancePassed: true,
        lowDiversityWarning: false,
        rerollAttempts: 0,
        rubricVerdict: "CONTENDER",
        rubricWins: 0,
        deScoreBps: 8_400,
        elapsedMs: 42,
        archived: false,
      },
    ],
    rubricSummary: {
      recommendedVariantId: "var-a",
      recommendedDisplayLabel: "Option A",
      allPairsSwapConsistent: true,
      pairwiseComparisons: [
        {
          variantAId: "var-a",
          variantALabel: "Option A",
          variantBId: "var-b",
          variantBLabel: "Option B",
          outcome: {
            winnerId: "var-a",
            winnerLabel: "Option A",
            forwardWinnerId: "var-a",
            swappedWinnerId: "var-a",
            swapConsistent: true,
            tieBreakApplied: false,
          },
        },
      ],
    },
  };
}

function createMockPort(): {
  port: DesignTournamentPort;
  startSpy: ReturnType<typeof vi.fn>;
  selectSpy: ReturnType<typeof vi.fn>;
  crystallizeSpy: ReturnType<typeof vi.fn>;
} {
  const sampleRun = buildSampleRun();
  const startSpy = vi.fn(
    async (): Promise<StartTournamentResponse> => ({
      ok: true,
      requestId: "req-start-1",
      data: sampleRun,
    })
  );
  const selectSpy = vi.fn(
    async (req: SelectTournamentVariantRequest): Promise<SelectTournamentVariantResponse> => ({
      ok: true,
      requestId: "req-select-1",
      data: {
        runId: req.runId,
        projectId: req.projectId,
        winningVariantId: req.winningVariantId,
        generationEpoch: req.generationEpoch,
        mergedHtml: "<main data-ff-id='ff-main'><h1>Merged</h1></main>",
        mergedSha256: SHA_A,
        borrowedFromVariantId: req.borrowFromVariantId ?? null,
        borrowedFfId: req.borrowFromVariantId ? "ff-rail-navigation" : null,
        extractedCssRulesCount: req.borrowFromVariantId ? 2 : 0,
        reconciledTotalFfIds: 4,
        duplicateFfIdsAfterMerge: 0,
        archivedVariantIds: ["var-b"],
        memoryEventIds: ["mem-win-1"],
      },
    })
  );
  const crystallizeSpy = vi.fn(
    async (): Promise<CrystallizeResponse> => ({
      ok: true,
      requestId: "req-cryst-1",
      data: {
        packageId: "cryst-001",
        projectId: PROJECT_ID,
        packageKind: "STARTER_TEMPLATE",
        name: "Executive Ops Kit",
        artifactRelPath: ".cowok-design/crystallized/cryst-001.json",
        artifactSha256: SHA_A,
        memoryEventId: "mem-cryst-1",
        createdAt: 1_764_000_000_500,
      },
    })
  );
  return {
    port: {
      startTournament: startSpy,
      selectWinner: selectSpy,
      crystallize: crystallizeSpy,
    },
    startSpy,
    selectSpy,
    crystallizeSpy,
  };
}

afterEach(() => {
  setGlobalLocale("vi-VN");
  cleanup();
});

describe("Story 10.3 DesignTournamentArenaPanel & SDK", () => {
  it("renders anonymized options, toggles Desktop/390px Blind Wipe slider, and hides raw model names", async () => {
    const { port, startSpy } = createMockPort();
    render(
      <DesignTournamentArenaPanel
        projectId={PROJECT_ID}
        initialPrompt="Bảng điều khiển vận hành tối giản"
        port={port}
      />
    );

    const arena = screen.getByTestId("design-tournament-arena");
    expect(arena.getAttribute("data-ff-id")).toBe("design-tournament-arena");

    fireEvent.click(screen.getByTestId("tournament-launch-btn"));
    await waitFor(() => expect(startSpy).toHaveBeenCalledTimes(1));

    expect(screen.getByTestId("tournament-option-tab-var-a").textContent).toContain("Option A");
    expect(screen.getByTestId("tournament-option-tab-var-b").textContent).toContain("Option B");
    expect(screen.getByTestId("tournament-diversity-badge-var-a").textContent).toBe("Khác biệt 100%");
    expect(screen.getByTestId("tournament-diversity-badge-var-b").textContent).toBe("Khác biệt 66%");
    expect(screen.getByTestId("tournament-diversity-badge-var-b").getAttribute("title")).toContain(
      "AST Jaccard"
    );
    expect(screen.getByTestId("tournament-diversity-badge-var-b").getAttribute("title")).toContain(
      "34%"
    );
    expect(arena.textContent).not.toMatch(/claude|gemini|gpt|Story 10\.3|ADR-006/i);
    expect(arena.textContent).not.toContain("Ẩn tên mô hình");
    const donorSelect = screen.getByTestId("cross-variant-donor-select");
    expect(donorSelect.textContent).toContain("Mượn từ Option B");

    const sliderSection = arena.querySelector("[data-ff-id='tournament-blind-wipe-slider']");
    expect(sliderSection).not.toBeNull();
    expect(sliderSection?.getAttribute("data-viewport-mode")).toBe("desktop-1440");

    // 5-icon Main Canvas responsive viewport pill group
    expect(screen.getByTestId("tournament-viewport-desktop").getAttribute("title")).toBe(
      "Màn hình Desktop (1.440px)"
    );
    expect(screen.getByTestId("tournament-viewport-tablet").getAttribute("title")).toBe(
      "Màn hình Tablet (768px)"
    );
    expect(screen.getByTestId("tournament-viewport-fold-inner").getAttribute("title")).toBe(
      "Màn hình mở gập (712px)"
    );
    expect(screen.getByTestId("tournament-viewport-mobile").getAttribute("title")).toBe(
      "Màn hình Mobile (390px)"
    );
    expect(screen.getByTestId("tournament-viewport-fold-outer").getAttribute("title")).toBe(
      "Màn hình ngoài gập (344px)"
    );

    fireEvent.click(screen.getByTestId("tournament-viewport-tablet"));
    expect(sliderSection?.getAttribute("data-viewport-mode")).toBe("tablet-768");
    fireEvent.click(screen.getByTestId("tournament-viewport-fold-inner"));
    expect(sliderSection?.getAttribute("data-viewport-mode")).toBe("fold-inner-712");
    fireEvent.click(screen.getByTestId("tournament-viewport-mobile"));
    expect(sliderSection?.getAttribute("data-viewport-mode")).toBe("mobile-390");
    fireEvent.click(screen.getByTestId("tournament-viewport-fold-outer"));
    expect(sliderSection?.getAttribute("data-viewport-mode")).toBe("fold-outer-344");
    fireEvent.click(screen.getByTestId("tournament-viewport-desktop"));
    expect(sliderSection?.getAttribute("data-viewport-mode")).toBe("desktop-1440");

    const range = screen.getByTestId("blind-wipe-range-input") as HTMLInputElement;
    fireEvent.change(range, { target: { value: "72" } });
    expect(range.value).toBe("72");
  });

  it("supports all 4 comparison modes (wipe, side-by-side, grid click-to-select, onion opacity) and low-diversity warning badge", () => {
    const runWithDuplicate = buildSampleRun();
    runWithDuplicate.variants[2] = {
      ...runWithDuplicate.variants[2],
      astSimilarity: 0.85,
      cloneDistancePassed: false,
      lowDiversityWarning: true,
    };
    const { port } = createMockPort();

    render(
      <DesignTournamentArenaPanel
        projectId={PROJECT_ID}
        initialRun={runWithDuplicate}
        port={port}
      />
    );

    // Real AST duplicate warning badge when cloneDistancePassed === false
    const dupBadge = screen.getByTestId("tournament-diversity-badge-var-c");
    expect(dupBadge.textContent).toBe("Trùng lặp 85%");
    expect(dupBadge.getAttribute("title")).toContain("85%");

    // Switch to Side-by-Side
    fireEvent.click(screen.getByTestId("compare-mode-side-by-side"));
    expect(screen.getByTestId("compare-stage-side-by-side")).toBeTruthy();
    expect(screen.getByTestId("side-by-side-left")).toBeTruthy();
    expect(screen.getByTestId("side-by-side-right")).toBeTruthy();

    // Switch to 2x2 Grid and click-to-select Option C
    fireEvent.click(screen.getByTestId("compare-mode-grid"));
    expect(screen.getByTestId("compare-stage-grid")).toBeTruthy();
    expect(screen.getByTestId("compare-grid-card-var-a")).toBeTruthy();
    expect(screen.getByTestId("compare-grid-card-var-c")).toBeTruthy();
    fireEvent.click(screen.getByTestId("compare-grid-select-var-c"));
    expect(screen.getByTestId("tournament-option-tab-var-c").getAttribute("aria-selected")).toBe("true");

    // Switch to Onion Skin and adjust opacity slider
    fireEvent.click(screen.getByTestId("compare-mode-onion"));
    expect(screen.getByTestId("compare-stage-onion")).toBeTruthy();
    const opacityInput = screen.getByTestId("onion-opacity-range-input") as HTMLInputElement;
    fireEvent.change(opacityInput, { target: { value: "30" } });
    expect(opacityInput.value).toBe("30");
    expect(screen.getByTestId("onion-opacity-value").textContent).toBe("30%");

    // Switch back to Wipe
    fireEvent.click(screen.getByTestId("compare-mode-wipe"));
    expect(screen.getByTestId("blind-wipe-range-input")).toBeTruthy();
  });

  it("enforces monotonic generationEpoch on rapid Option A -> B -> C switches so only final active option commits", async () => {
    const { port, selectSpy } = createMockPort();
    render(
      <DesignTournamentArenaPanel
        projectId={PROJECT_ID}
        initialRun={buildSampleRun()}
        port={port}
      />
    );

    // Rapid A -> B -> C clicks before confirming
    fireEvent.click(screen.getByTestId("tournament-option-tab-var-a"));
    fireEvent.click(screen.getByTestId("tournament-option-tab-var-b"));
    fireEvent.click(screen.getByTestId("tournament-option-tab-var-c"));

    // Add Cross-Variant Borrow from Option B via Visual Zone Picker Modal
    fireEvent.change(screen.getByTestId("cross-variant-donor-select"), {
      target: { value: "var-b" },
    });
    fireEvent.click(screen.getByTestId("cross-variant-open-zone-picker-btn"));
    expect(screen.getByTestId("cross-variant-zone-picker-modal")).toBeTruthy();
    fireEvent.click(screen.getByTestId("zone-picker-checkbox-ff-rail-navigation"));
    fireEvent.click(screen.getByTestId("zone-picker-apply-btn"));
    expect(screen.getByTestId("cross-variant-selected-chip-ff-rail-navigation")).toBeTruthy();

    fireEvent.click(screen.getByTestId("tournament-confirm-winner-btn"));

    await waitFor(() => expect(selectSpy).toHaveBeenCalledTimes(1));
    expect(selectSpy).toHaveBeenCalledWith({
      projectId: PROJECT_ID,
      runId: "run-103-001",
      winningVariantId: "var-c",
      generationEpoch: 4,
      borrowFromVariantId: "var-b",
      borrowSelectorOrInstruction: "ff-rail-navigation",
    });

    const receiptText = screen.getByTestId("tournament-commit-receipt").textContent ?? "";
    expect(receiptText).toContain("Option C");
    expect(receiptText).toContain("Option B");
    // Must never leak raw internal variant IDs or raw data-ff-id anchors in UI receipt
    expect(receiptText).not.toContain("var-c");
    expect(receiptText).not.toContain("ff-rail-navigation");
  });

  it("releases isCommitting immediately on rapid Option A -> B -> C switches during an in-flight commit and clears stale donorId", async () => {
    const deferred: { resolve: ((value: SelectTournamentVariantResponse) => void) | null } = {
      resolve: null,
    };
    const selectSpy = vi.fn(
      (req: SelectTournamentVariantRequest): Promise<SelectTournamentVariantResponse> => {
        if (req.winningVariantId === "var-a") {
          return new Promise<SelectTournamentVariantResponse>((resolve) => {
            deferred.resolve = resolve;
          });
        }
        return Promise.resolve({
          ok: true,
          requestId: "req-select-final",
          data: {
            runId: req.runId,
            projectId: req.projectId,
            winningVariantId: req.winningVariantId,
            generationEpoch: req.generationEpoch,
            mergedHtml: "<main data-ff-id='ff-main'><h1>Option C Final</h1></main>",
            mergedSha256: SHA_C,
            borrowedFromVariantId: null,
            borrowedFfId: null,
            extractedCssRulesCount: 0,
            reconciledTotalFfIds: 2,
            duplicateFfIdsAfterMerge: 0,
            archivedVariantIds: ["var-a", "var-b"],
            memoryEventIds: ["mem-win-final"],
          },
        });
      }
    );
    const { port } = createMockPort();
    port.selectWinner = selectSpy;
    const onCommittedHtml = vi.fn();

    render(
      <DesignTournamentArenaPanel
        projectId={PROJECT_ID}
        initialRun={buildSampleRun()}
        port={port}
        onCommittedHtml={onCommittedHtml}
      />
    );

    // Select Option B as donor while Option A is active, then start slow commit on Option A
    const donorSelect = screen.getByTestId("cross-variant-donor-select") as HTMLSelectElement;
    fireEvent.change(donorSelect, { target: { value: "var-b" } });
    expect(donorSelect.value).toBe("var-b");

    const confirmBtn = screen.getByTestId("tournament-confirm-winner-btn") as HTMLButtonElement;
    fireEvent.click(confirmBtn);
    expect(confirmBtn.disabled).toBe(true);

    // Switch rapidly Option A -> B -> C while Option A's commit is still in flight
    fireEvent.click(screen.getByTestId("tournament-option-tab-var-b"));
    // Switching active tab to var-b must auto-clear donorSelect since donorId === activeVariant.id
    expect(donorSelect.value).toBe("");
    // Confirm button MUST NOT be deadlocked!
    expect(confirmBtn.disabled).toBe(false);

    fireEvent.click(screen.getByTestId("tournament-option-tab-var-c"));
    expect(confirmBtn.disabled).toBe(false);

    // Commit Option C cleanly
    fireEvent.click(confirmBtn);
    await waitFor(() => expect(onCommittedHtml).toHaveBeenCalledTimes(1));
    expect(screen.getByTestId("tournament-commit-receipt").textContent).toContain("Option C");
    expect(confirmBtn.disabled).toBe(false);

    // Now resolve the delayed stale Option A commit -> must be discarded without overwriting Option C
    deferred.resolve?.({
      ok: true,
      requestId: "req-select-stale",
      data: {
        runId: "run-103-001",
        projectId: PROJECT_ID,
        winningVariantId: "var-a",
        generationEpoch: 1,
        mergedHtml: "<main data-ff-id='ff-main'><h1>Stale Option A</h1></main>",
        mergedSha256: SHA_A,
        borrowedFromVariantId: null,
        borrowedFfId: null,
        extractedCssRulesCount: 0,
        reconciledTotalFfIds: 2,
        duplicateFfIdsAfterMerge: 0,
        archivedVariantIds: ["var-b", "var-c"],
        memoryEventIds: ["mem-win-stale"],
      },
    });
    await Promise.resolve();
    expect(onCommittedHtml).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("tournament-commit-receipt").textContent).toContain("Option C");
  });

  it("maps TournamentApiError codes to friendly localized copy and rejects single-dot IDs", async () => {
    const { port } = createMockPort();
    port.selectWinner = vi.fn(async () => {
      throw new TournamentApiError({
        status: 422,
        code: "CSS_SECURITY_BLOCKED",
        message: "raw internal parser stacktrace",
      });
    });

    render(
      <DesignTournamentArenaPanel
        projectId={PROJECT_ID}
        initialRun={buildSampleRun()}
        port={port}
      />
    );

    fireEvent.click(screen.getByTestId("tournament-confirm-winner-btn"));
    const banner = await screen.findByTestId("tournament-error-banner");
    expect(banner.textContent).toContain("HTML/CSS không an toàn");
    expect(banner.textContent).not.toContain("raw internal parser stacktrace");

    const client = createTournamentApiClient({ baseUrl: "http://127.0.0.1:4172" });
    expect(() =>
      client.crystallize({
        projectId: PROJECT_ID,
        runId: ".",
        packageKind: "BRAND_KIT",
        name: "Bad Dot Run",
      })
    ).toThrow(/Identifier must not be '\.' or contain '\.\.'/);
  });

  it("triggers 1-Click /crystallize action and displays confirmation receipt", async () => {
    const { port, crystallizeSpy } = createMockPort();
    render(
      <DesignTournamentArenaPanel
        projectId={PROJECT_ID}
        initialRun={buildSampleRun()}
        port={port}
      />
    );

    fireEvent.change(screen.getByTestId("crystallize-name-input"), {
      target: { value: "Executive Ops Kit" },
    });
    fireEvent.click(screen.getByTestId("crystallize-action-button"));

    await waitFor(() => expect(crystallizeSpy).toHaveBeenCalledTimes(1));
    expect(screen.getByTestId("tournament-crystallize-receipt").textContent).toContain(
      "Executive Ops Kit"
    );
  });

  it("enforces Zod runtime validation and typed Kernel Guard errors in TournamentApiClient", async () => {
    const mockFetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("/api/v1/advisor/evolve")) {
        return new Response(
          JSON.stringify({
            error: {
              code: "SEALED_EVAL_TAMPER_ERROR",
              message: "Blocked advisor access to tests/eval/sealed",
              field: "targetSkillPath",
              requestId: "req-tamper-1",
            },
          }),
          { status: 403, headers: { "Content-Type": "application/json" } }
        );
      }
      return new Response(JSON.stringify({ ok: true, invalidShape: true }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    });

    const client = createTournamentApiClient({
      baseUrl: "http://127.0.0.1:4172",
      getSessionToken: () => "cwk_sess_test_token",
      fetchImpl: mockFetch as typeof fetch,
    });

    await expect(
      client.evolveAdvisor({
        projectId: PROJECT_ID,
        targetSkillPath: "tests/eval/sealed/suite.json",
      })
    ).rejects.toMatchObject({
      name: "TournamentApiError",
      status: 403,
      code: "SEALED_EVAL_TAMPER_ERROR",
    } satisfies Partial<TournamentApiError>);

    await expect(
      client.startTournament({
        projectId: PROJECT_ID,
        prompt: "Minimalist operations hub",
      })
    ).rejects.toThrow();
  });
});

function buildRecentRunsResponse(): ListTournamentRunsResponse {
  return {
    ok: true,
    requestId: "req-runs-1",
    data: {
      projectId: PROJECT_ID,
      runs: [
        {
          id: "run-hist-001",
          projectId: PROJECT_ID,
          axes: ["INFORMATION_DENSITY", "NAVIGATION_TOPOLOGY"],
          winningVariantId: "var-a",
          borrowedFromVariantId: null,
          borrowedFfId: null,
          generationEpoch: 2,
          createdAt: 1_764_000_000_000,
          variants: [
            {
              id: "var-a",
              displayLabel: "Option A",
              axisProfile: "editorial-kpi-strip",
              artifactSha256: SHA_A,
              astSimilarity: 0.0,
              cloneDistancePassed: true,
              rubricVerdict: "RECOMMENDED_BLIND_WINNER",
              archived: false,
            },
            {
              id: "var-b",
              displayLabel: "Option B",
              axisProfile: "rail-nav-split",
              artifactSha256: SHA_B,
              astSimilarity: 0.31,
              cloneDistancePassed: true,
              rubricVerdict: "CONTENDER",
              archived: true,
            },
          ],
        },
      ],
    },
  };
}

describe("Story 10.3 DesignTournamentStudioDrawer (StudioDrawerShell [5.26])", () => {
  it("renders Smart Launcher for .md & .html contexts, loads recent runs, and launches Blind Wipe on Quick Start", async () => {
    const { port, startSpy } = createMockPort();
    const listRunsSpy = vi.fn(async () => buildRecentRunsResponse());
    port.listRuns = listRunsSpy;
    const onActiveRunCountChange = vi.fn();

    const { rerender } = render(
      <DesignTournamentStudioDrawer
        isOpen
        onClose={vi.fn()}
        projectId={PROJECT_ID}
        activeTabTitle="DESIGN.md"
        activeTabKind="md"
        port={port}
        onActiveRunCountChange={onActiveRunCountChange}
      />
    );

    expect(screen.getByTestId("design-tournament-studio-drawer")).toBeTruthy();
    expect(screen.getByTestId("studio-right-drawer")).toBeTruthy();
    expect(screen.getByTestId("studio-master-pane")).toBeTruthy();
    expect(screen.getByTestId("studio-detail-pane")).toBeTruthy();
    expect(screen.getByTestId("tournament-smart-launcher")).toBeTruthy();

    // .md contextual prefill
    expect(screen.getByTestId("tournament-contextual-prompt-preview").textContent).toContain(
      "Tạo 4 phương án giao diện từ đặc tả DESIGN.md"
    );

    await waitFor(() => expect(listRunsSpy).toHaveBeenCalledWith(PROJECT_ID, 10));
    const histItem = await screen.findByTestId("tournament-recent-run-run-hist-001");
    expect(histItem.textContent).toContain("Option A");
    expect(histItem.textContent).not.toContain("var-a");
    expect(onActiveRunCountChange).toHaveBeenCalledWith(1);

    // Switch active tab to .html and verify contextual prompt updates automatically
    rerender(
      <DesignTournamentStudioDrawer
        isOpen
        onClose={vi.fn()}
        projectId={PROJECT_ID}
        activeTabTitle="checkout-app.html"
        activeTabKind="html"
        port={port}
        onActiveRunCountChange={onActiveRunCountChange}
      />
    );
    expect(screen.getByTestId("tournament-contextual-prompt-preview").textContent).toContain(
      "Tạo 4 phương án tối ưu bố cục cho checkout-app.html"
    );

    // No duplicate top prompt bar inside Studio Drawer left master pane
    expect(screen.queryByTestId("tournament-prompt-input")).toBeNull();
    expect(screen.queryByTestId("tournament-launch-btn")).toBeNull();
    // Header compact Crystallize button exists & is disabled before a run starts
    const crystallizeTrigger = screen.getByTestId(
      "tournament-crystallize-trigger-btn"
    ) as HTMLButtonElement;
    expect(crystallizeTrigger.disabled).toBe(true);
    expect(crystallizeTrigger.getAttribute("data-ff-id")).toBe("crystallize-action-button");

    // Click 1-click Quick Start CTA -> launches tournament & reveals Blind Wipe inside detail canvas
    fireEvent.click(screen.getByTestId("tournament-quick-start-btn"));
    await waitFor(() => expect(startSpy).toHaveBeenCalledTimes(1));
    expect(startSpy).toHaveBeenCalledWith({
      projectId: PROJECT_ID,
      prompt: "Tạo 4 phương án tối ưu bố cục cho checkout-app.html",
    });

    const arena = screen.getByTestId("design-tournament-arena");
    expect(arena.getAttribute("data-ff-id")).toBe("design-tournament-arena");
    expect(arena.querySelector("[data-ff-id='tournament-blind-wipe-slider']")).not.toBeNull();
    expect(screen.getByTestId("blind-wipe-range-input")).toBeTruthy();
    expect(screen.getByTestId("cross-variant-donor-select")).toBeTruthy();
    expect(screen.getByTestId("cross-variant-chat-suggest-btn")).toBeTruthy();

    // Permanent Crystallize card is removed from Left Master Pane; popover opens only on header trigger click
    expect(screen.queryByTestId("tournament-crystallize-popover")).toBeNull();
    expect(crystallizeTrigger.disabled).toBe(false);
    fireEvent.click(crystallizeTrigger);
    expect(screen.getByTestId("tournament-crystallize-popover")).toBeTruthy();
    expect(screen.getByTestId("crystallize-action-button")).toBeTruthy();
    fireEvent.click(screen.getByTestId("tournament-crystallize-popover-close"));
    expect(screen.queryByTestId("tournament-crystallize-popover")).toBeNull();
  });

  it("supports /crystallize slash command in Sub-Chat after a tournament run is active", async () => {
    const { port, startSpy, crystallizeSpy } = createMockPort();
    port.listRuns = vi.fn(async () => buildRecentRunsResponse());

    render(
      <DesignTournamentStudioDrawer
        isOpen
        onClose={vi.fn()}
        projectId={PROJECT_ID}
        activeTabTitle="checkout-app.html"
        activeTabKind="html"
        port={port}
      />
    );

    fireEvent.click(screen.getByTestId("tournament-quick-start-btn"));
    await waitFor(() => expect(startSpy).toHaveBeenCalledTimes(1));

    const chatInput = screen.getByTestId("studio-subchat-input") as HTMLTextAreaElement;
    fireEvent.change(chatInput, { target: { value: "/crystallize Executive Ops Kit" } });
    fireEvent.click(screen.getByTestId("studio-subchat-send-btn"));

    await waitFor(() => expect(crystallizeSpy).toHaveBeenCalledTimes(1));
    expect(crystallizeSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        projectId: PROJECT_ID,
        packageKind: "STARTER_TEMPLATE",
        name: "Executive Ops Kit",
      })
    );
    expect(screen.getByTestId("tournament-crystallize-receipt").textContent).toContain(
      "Executive Ops Kit"
    );
  });

  it("sends Sub-Chat refinement prompt to launch refined comparison round and syncs via CustomEvent", async () => {
    const { port, startSpy } = createMockPort();
    port.listRuns = vi.fn(async () => {
      throw new Error("offline");
    });

    const syncedEvents: CustomEvent[] = [];
    const onSync = (ev: Event) => syncedEvents.push(ev as CustomEvent);
    window.addEventListener("cowok:studio-subchat-sync", onSync);

    try {
      render(
        <DesignTournamentStudioDrawer
          isOpen
          onClose={vi.fn()}
          projectId={PROJECT_ID}
          activeTabTitle="checkout-app.html"
          activeTabKind="html"
          port={port}
        />
      );

      const chatInput = screen.getByTestId("studio-subchat-input") as HTMLTextAreaElement;
      const instruction = "Giữ thanh điều hướng trái của Option B nhưng làm gọn KPI như Option A";
      fireEvent.change(chatInput, { target: { value: instruction } });
      fireEvent.click(screen.getByTestId("studio-subchat-send-btn"));

      await waitFor(() => expect(startSpy).toHaveBeenCalledTimes(1));
      expect(startSpy.mock.calls[0]?.[0]).toMatchObject({
        projectId: PROJECT_ID,
      });
      expect(String(startSpy.mock.calls[0]?.[0]?.prompt)).toContain(instruction);

      expect(syncedEvents).toHaveLength(1);
      expect(syncedEvents[0]?.detail).toMatchObject({
        source: "design-tournament",
        projectId: PROJECT_ID,
        prompt: instruction,
      });

      const messagesBox = screen.getByTestId("studio-subchat-messages");
      await waitFor(() => {
        expect(messagesBox.textContent).toContain(instruction);
        expect(messagesBox.textContent).toContain("Đã tạo vòng so sánh mới");
        expect(messagesBox.textContent).toContain("Option A");
      });

      // Blind Wipe slider is now active in the detail canvas
      const arena = screen.getByTestId("design-tournament-arena");
      expect(arena.querySelector("[data-ff-id='tournament-blind-wipe-slider']")).not.toBeNull();
    } finally {
      window.removeEventListener("cowok:studio-subchat-sync", onSync);
    }
  });

  it("supports multi-locale formatting (vi-VN, en-US, ja-JP) and multilingual direct-borrow intent parsing", () => {
    const sample = buildSampleRun();
    expect(parseSubChatDirectBorrow("Chốt ghép ngay với Option B", sample.variants, "var-a")?.id).toBe("var-b");
    expect(parseSubChatDirectBorrow("Please apply Option C navigation", sample.variants, "var-a")?.id).toBe("var-c");
    expect(parseSubChatDirectBorrow("Option Bを採用してください", sample.variants, "var-a")?.id).toBe("var-b");
    expect(parseSubChatDirectBorrow("Option Cで確定", sample.variants, "var-a")?.id).toBe("var-c");
    expect(parseSubChatDirectBorrow("Show me a cleaner layout for Option B", sample.variants, "var-a")).toBeNull();

    setGlobalLocale("en-US");
    const enTimestamp = formatTournamentRunTimestamp(1_764_000_000_000);
    expect(enTimestamp.length).toBeGreaterThan(0);
    const enBadge = formatVariantDiversityBadge(sample.variants[1]);
    expect(enBadge.diversityPct).toBe(66);
    expect(enBadge.tooltip).toContain("34%");

    setGlobalLocale("ja-JP");
    const jaTimestamp = formatTournamentRunTimestamp(1_764_000_000_000);
    expect(jaTimestamp.length).toBeGreaterThan(0);
    const jaBadge = formatVariantDiversityBadge(sample.variants[1]);
    expect(jaBadge.passed).toBe(true);
  });

  it("extracts borrowable donor visual zones safely and commits multi-block comma-separated ffIds via CrossVariantZonePickerModal", async () => {
    const donorHtml = `<!doctype html><html><head><style>.nav{display:flex;}.kpi{display:grid;}</style></head><body><main data-ff-id="ff-root-main"><nav class="nav" data-ff-id="ff-rail-navigation">Sidebar Menu<script>alert(1)</script></nav><section class="kpi" data-ff-id="ff-kpi-summary">Velocity 98%</section><footer data-ff-id="ff-page-footer">Status OK</footer></main></body></html>`;
    const zones = extractDonorVisualZones(donorHtml);
    expect(zones.map((z) => z.ffId)).toEqual([
      "ff-rail-navigation",
      "ff-kpi-summary",
      "ff-page-footer",
    ]);
    expect(zones[0]?.labelKey).toBe("tournament.zoneLabelNav");
    expect(zones[1]?.labelKey).toBe("tournament.zoneLabelKpi");
    expect(zones[2]?.labelKey).toBe("tournament.zoneLabelFooter");
    expect(zones[0]?.previewHtml).toContain(".nav{display:flex;}");
    expect(zones[0]?.previewHtml).not.toContain("<script>");

    const { port, selectSpy } = createMockPort();
    const onFocusSubChat = vi.fn();
    render(
      <DesignTournamentArenaPanel
        projectId={PROJECT_ID}
        initialRun={buildSampleRun()}
        port={port}
        onFocusSubChat={onFocusSubChat}
      />
    );

    fireEvent.change(screen.getByTestId("cross-variant-donor-select"), {
      target: { value: "var-b" },
    });
    const openPickerBtn = screen.getByTestId("cross-variant-open-zone-picker-btn");
    fireEvent.click(openPickerBtn);
    expect(screen.getByTestId("cross-variant-zone-picker-modal")).toBeTruthy();

    fireEvent.click(screen.getByTestId("zone-picker-card-ff-rail-navigation"));
    fireEvent.click(screen.getByTestId("zone-picker-checkbox-ff-data-ledger"));
    expect(screen.getByTestId("zone-picker-selected-count").textContent).toContain("2");

    fireEvent.click(screen.getByTestId("zone-picker-apply-btn"));
    expect(screen.queryByTestId("cross-variant-zone-picker-modal")).toBeNull();
    expect(screen.getByTestId("cross-variant-open-zone-picker-btn").textContent).toContain("(2)");
    expect(screen.getByTestId("cross-variant-selected-chip-ff-rail-navigation")).toBeTruthy();
    expect(screen.getByTestId("cross-variant-selected-chip-ff-data-ledger")).toBeTruthy();

    fireEvent.click(screen.getByTestId("tournament-confirm-winner-btn"));
    await waitFor(() => expect(selectSpy).toHaveBeenCalledTimes(1));
    expect(selectSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        borrowFromVariantId: "var-b",
        borrowSelectorOrInstruction: "ff-rail-navigation,ff-data-ledger",
      })
    );

    // Remove one chip and verify shortcut button invokes onFocusSubChat
    fireEvent.click(screen.getByTestId("cross-variant-remove-chip-ff-data-ledger"));
    expect(screen.queryByTestId("cross-variant-selected-chip-ff-data-ledger")).toBeNull();
    fireEvent.click(screen.getByTestId("cross-variant-chat-suggest-btn"));
    expect(onFocusSubChat).toHaveBeenCalledTimes(1);
  });

  it("focuses the Sub-Chat textarea when clicking cross-variant-chat-suggest-btn inside DesignTournamentStudioDrawer", async () => {
    const { port, startSpy } = createMockPort();
    port.listRuns = vi.fn(async () => buildRecentRunsResponse());

    render(
      <DesignTournamentStudioDrawer
        isOpen
        onClose={vi.fn()}
        projectId={PROJECT_ID}
        activeTabTitle="checkout-app.html"
        activeTabKind="html"
        port={port}
      />
    );

    fireEvent.click(screen.getByTestId("tournament-quick-start-btn"));
    await waitFor(() => expect(startSpy).toHaveBeenCalledTimes(1));

    const suggestBtn = screen.getByTestId("cross-variant-chat-suggest-btn");
    fireEvent.click(suggestBtn);
    const subChatInput = screen.getByTestId("studio-subchat-input") as HTMLTextAreaElement;
    expect(document.activeElement).toBe(subChatInput);
  });
});
