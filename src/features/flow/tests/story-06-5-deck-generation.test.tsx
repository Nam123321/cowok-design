// @vitest-environment jsdom
// @story 06.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  CopilotRewriteSlideResponseSchema,
  type GenerateDeckResponse,
  GenerateDeckRequestSchema,
  type NormalizeChartSourceResponse,
  NormalizeChartSourceRequestSchema,
} from "../../../lib/validations/deck-generation";
import * as deckGenApi from "../../../api/deck-generation";
import { DeckCsvGeneratorModal } from "../components/DeckCsvGeneratorModal";
import { FloatingSlideCopilot } from "../components/FloatingSlideCopilot";
import { SlideAspectAndZoomControls } from "../components/HtmlPreviewSlideControls";
import { createEmptyCopilotLease } from "../hooks/useSlideCopilotHelpers";

const SLIDE_UID_1 = "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV";
const SLIDE_UID_2 = "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAW";
const SHA_VALID = "a".repeat(64);

describe("Story 06.5 — Option D v2 Chart & Dashboard Studio (Data Storytelling, Drill-Down & Unified Toolbar)", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("UT-01: validates Zod contracts for chart normalization, deck generation & <=30 word copilot rewrite", () => {
    const validNorm = NormalizeChartSourceRequestSchema.parse({
      projectId: "proj-1",
      casSha256: SHA_VALID,
      chartKind: "pie",
    });
    expect(validNorm.maxSeries).toBe(12);
    expect(validNorm.maxPoints).toBe(200);

    const badSeries = NormalizeChartSourceRequestSchema.safeParse({
      projectId: "proj-1",
      inlineContent: "A,B\n1,2",
      chartKind: "bar",
      maxSeries: 13,
    });
    expect(badSeries.success).toBe(false);

    const validGen = GenerateDeckRequestSchema.parse({
      projectId: "proj-1",
      title: "Q4 Review",
      brief: "Core growth drivers and retention.",
      framePreset: "Window",
    });
    expect(validGen.framePreset).toBe("Window");
    expect(validGen.aspectRatio).toBe("16:9");

    const overflowRewrite = CopilotRewriteSlideResponseSchema.safeParse({
      deckId: "deck-1",
      slideUid: SLIDE_UID_1,
      targetFfId: "ff-s1-b1",
      newRevision: 2,
      contentSha256: SHA_VALID,
      rewrittenText: "Too many words",
      wordCount: 31,
      wasCondensedOrSplit: false,
      continuationSlides: [],
      generationEngine: "stageshow-deterministic",
    });
    expect(overflowRewrite.success).toBe(false);
  });

  it("UT-02: renames studio header, removes redundant brief/word-budget & places Stage/Window/Phone in Row 2 toolbar with 1 primary CTA", () => {
    render(
      <DeckCsvGeneratorModal
        isOpen={true}
        projectId="proj-alpha"
        onClose={vi.fn()}
      />
    );

    expect(screen.getByTestId("studio-drawer-title").textContent).toBe("Studio Biểu đồ & Dashboard");
    expect(screen.queryByTestId("deck-gen-brief-input")).toBeNull();
    expect(screen.queryByTestId("deck-word-budget-indicator")).toBeNull();

    const form = screen.getByTestId("deck-launchpad-form");
    expect(form.getAttribute("data-ff-id")).toBe("deck-launchpad-form");
    expect(form.querySelectorAll('button[type="submit"]').length).toBe(1);

    const selector = screen.getByTestId("stageshow-frame-selector");
    expect(selector.getAttribute("data-ff-id")).toBe("stageshow-frame-selector");
    expect(screen.getByTestId("studio-canvas-toolbar").contains(selector)).toBe(true);

    const stageBtn = screen.getByTestId("stageshow-frame-stage");
    const windowBtn = screen.getByTestId("stageshow-frame-window");
    const phoneBtn = screen.getByTestId("stageshow-frame-phone");

    expect(stageBtn.getAttribute("aria-checked")).toBe("true");
    fireEvent.click(windowBtn);
    expect(windowBtn.getAttribute("aria-checked")).toBe("true");
    expect(stageBtn.getAttribute("aria-checked")).toBe("false");
    expect(screen.getByTestId("studio-frame-chrome-window")).toBeTruthy();
    fireEvent.click(phoneBtn);
    expect(phoneBtn.getAttribute("aria-checked")).toBe("true");
    expect(screen.getByTestId("studio-frame-chrome-phone")).toBeTruthy();
  });

  it("UT-03: displays structured CowokToast alert when backend rejects scanned/image-only PDF without double-uploading", async () => {
    const normSpy = vi.spyOn(deckGenApi, "normalizeChartSource");
    vi.spyOn(deckGenApi, "generateDeck").mockRejectedValueOnce(
      new deckGenApi.DeckGenerationApiError(
        422,
        "SCANNED_PDF_NO_TEXT_TABLES",
        "Scanned or image-only PDF detected (0 extractable text tables).",
        "This PDF appears to be an image scan. Paste CSV or TSV rows instead."
      )
    );

    render(
      <DeckCsvGeneratorModal
        isOpen={true}
        projectId="proj-alpha"
        onClose={vi.fn()}
      />
    );

    // Raw CSV starts collapsed by default; expand and paste CSV
    expect(screen.queryByTestId("deck-gen-paste-input")).toBeNull();
    fireEvent.click(screen.getByTestId("studio-raw-csv-toggle"));
    fireEvent.change(screen.getByTestId("deck-gen-paste-input"), {
      target: { value: "Channel,Revenue\nDirect,100\n" },
    });

    fireEvent.click(screen.getByTestId("deck-gen-primary-cta"));

    const toast = await screen.findByTestId("deck-gen-toast");
    expect(toast.getAttribute("role")).toBe("alert");
    expect(toast.textContent).toContain("SCANNED_PDF_NO_TEXT_TABLES");
    expect(toast.textContent).toContain("CSV");
    expect(normSpy).not.toHaveBeenCalled();
  });

  it("UT-04: normalizes CSV -> ChartSpecV12 and generates multi-slide V15 deck deriving brief from Action Title & 3-Act Arc", async () => {
    const normResp: NormalizeChartSourceResponse = {
      spec: {
        kind: "pie",
        title: "Revenue Mix",
        unit: "%",
        series: [
          {
            name: "Revenue",
            points: [
              { label: "Direct ($1,234.56)", value: 60, colorToken: "human-accent" },
              { label: "Partner (42.5M VND)", value: 40, colorToken: "agent-accent" },
            ],
          },
        ],
      },
      canonicalChartsJson: "{}",
      ariaLabel: "Revenue Mix (pie)",
      estimatedSvgPrimitives: 14,
      requestedKind: "pie",
      sourceFormat: "csv",
      proportionalSum: 100,
      detectedUnit: "$",
      rowCount: 2,
      seriesCount: 1,
      rawPoints: [
        { seriesName: "Revenue", category: "Direct", rawDisplay: "$1,234.56", rawValue: 1234.56, unit: "$", normalizedValue: 60 },
        { seriesName: "Revenue", category: "Partner", rawDisplay: "42.5M VND", rawValue: 42.5, unit: "M VND", normalizedValue: 40 },
      ],
    };

    const genResp: GenerateDeckResponse = {
      deck: {
        id: "deck-065-1",
        projectId: "proj-alpha",
        title: "Revenue Mix",
        themePreset: "night",
        aspectRatio: "16:9",
        deckProtocolVersion: 1,
        deckMode: "multi-page",
        slidesCount: 2,
        revision: 1,
        lastExportedAt: null,
        updatedAt: Date.now(),
        slides: [
          {
            id: "s1", deckId: "deck-065-1", slideUid: SLIDE_UID_1, slideIndex: 0, layoutKind: "hero_cover",
            htmlPath: "presentations/deck-065-1/slides/s1.html", contentSha256: SHA_VALID,
            speakerNotes: "", narrativeArc: "Window", eyebrowTag: "WINDOW BRIEF", revision: 1, updatedAt: Date.now(),
          },
          {
            id: "s2", deckId: "deck-065-1", slideUid: SLIDE_UID_2, slideIndex: 1, layoutKind: "data_table_report",
            htmlPath: "presentations/deck-065-1/slides/s2.html", contentSha256: SHA_VALID,
            speakerNotes: "", narrativeArc: "Window", eyebrowTag: "VERIFIED CSV", revision: 1, updatedAt: Date.now(),
          },
        ],
      },
      framePreset: "Window",
      autoSplitCount: 0,
      maxWordsPerSlide: 24,
      normalizedChart: normResp,
      generationEngine: "stageshow-deterministic",
      slidesSummary: [
        { slideUid: SLIDE_UID_1, slideIndex: 0, headline: "Revenue Mix", wordCount: 18, isContinuation: false, framePreset: "Window", hasChart: false },
        { slideUid: SLIDE_UID_2, slideIndex: 1, headline: "Revenue Mix", wordCount: 24, isContinuation: false, framePreset: "Window", hasChart: true },
      ],
    };

    const normSpy = vi.spyOn(deckGenApi, "normalizeChartSource").mockResolvedValueOnce(normResp);
    const genSpy = vi.spyOn(deckGenApi, "generateDeck").mockResolvedValueOnce(genResp);
    const onDeckGenerated = vi.fn();

    render(
      <DeckCsvGeneratorModal
        isOpen={true}
        projectId="proj-alpha"
        onClose={vi.fn()}
        onDeckGenerated={onDeckGenerated}
      />
    );

    fireEvent.click(screen.getByTestId("stageshow-frame-window"));
    fireEvent.click(screen.getByTestId("studio-raw-csv-toggle"));
    fireEvent.change(screen.getByTestId("deck-gen-paste-input"), {
      target: { value: "Channel,Revenue\nDirect,$1,234.56\nPartner,42.5M VND\n" },
    });

    fireEvent.click(screen.getByTestId("deck-gen-primary-cta"));

    await waitFor(() => expect(onDeckGenerated).toHaveBeenCalledWith(genResp));
    expect(normSpy).not.toHaveBeenCalled();
    expect(genSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        projectId: "proj-alpha",
        framePreset: "Window",
        brief: expect.stringContaining("Hồi 1: Bối cảnh"),
      })
    );

    const preview = screen.getByTestId("normalized-chart-preview");
    expect(preview.textContent).toContain("$1,234.56");
    expect(preview.textContent).toContain("42.5M VND");
    expect(screen.getByTestId("chart-proportional-sum").textContent).toContain("100");
    expect(screen.getByTestId("deck-gen-result-summary").textContent).toContain("24/30");

    fireEvent.click(screen.getByTestId("deck-gen-preview-chart-btn"));
    await waitFor(() => expect(normSpy).toHaveBeenCalledTimes(1));
  });

  it("UT-05: SlideAspectAndZoomControls enforces real projectId, opens modal, closes on Escape & wires FloatingSlideCopilot", async () => {
    const { rerender } = render(
      <SlideAspectAndZoomControls
        aspectRatio="16-9"
        zoomMode="fit"
        isCompact={false}
        onSetAspectRatio={vi.fn()}
        onSetZoomMode={vi.fn()}
      />
    );
    expect((screen.getByTestId("open-deck-csv-generator-btn") as HTMLButtonElement).disabled).toBe(true);

    rerender(
      <SlideAspectAndZoomControls
        aspectRatio="16-9"
        zoomMode="fit"
        isCompact={false}
        projectId="proj-alpha"
        onSetAspectRatio={vi.fn()}
        onSetZoomMode={vi.fn()}
      />
    );

    fireEvent.click(screen.getByTestId("open-deck-csv-generator-btn"));
    expect(screen.getByTestId("deck-csv-generator-modal")).toBeTruthy();

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByTestId("deck-csv-generator-modal")).toBeNull());

    const rewriteSpy = vi.spyOn(deckGenApi, "rewriteSlideWithCopilot").mockResolvedValueOnce({
      deckId: "deck-1",
      slideUid: SLIDE_UID_1,
      targetFfId: "ff-s1-b1",
      newRevision: 2,
      contentSha256: SHA_VALID,
      rewrittenText: "Concise executive takeaway",
      wordCount: 3,
      wasCondensedOrSplit: false,
      continuationSlides: [],
      generationEngine: "stageshow-deterministic",
    });
    const onRewriteCommitted = vi.fn();
    render(
      <FloatingSlideCopilot
        isOpen={true} isCollapsed={false} isReadOnly={false}
        projectId="proj-alpha" deckId="deck-1" slideRevision={1} slideCode="S1"
        slideUid={SLIDE_UID_1} targetFfId="ff-s1-b1" draftPrompt="Tighten executive summary"
        attachedComment={null} activeLease={createEmptyCopilotLease(SLIDE_UID_1)}
        messages={[]} activeGlobalSlots={0} maxGlobalSlots={4}
        onDraftPromptChange={vi.fn()} onSendPrompt={vi.fn()} onRewriteCommitted={onRewriteCommitted}
        onCancelLease={vi.fn()} onCollapse={vi.fn()} onExpand={vi.fn()} onClose={vi.fn()} onClearAttachedComment={vi.fn()}
      />
    );
    fireEvent.click(screen.getByTestId("slide-copilot-quick-rewrite-btn"));
    await waitFor(() => expect(rewriteSpy).toHaveBeenCalledTimes(1));
    expect(onRewriteCommitted).toHaveBeenCalledTimes(1);
  });

  it("UT-06: Option D RightDrawer enforces <=85vw width, resizable/collapsible splits & hidden file input", () => {
    render(<DeckCsvGeneratorModal isOpen={true} projectId="proj-alpha" onClose={vi.fn()} />);

    const drawer = screen.getByTestId("studio-right-drawer");
    expect(drawer.style.maxWidth).toBe("85vw");
    expect(screen.getByTestId("studio-drawer-resize-handle").getAttribute("role")).toBe("separator");
    expect(screen.getByTestId("studio-master-detail-resizer").getAttribute("role")).toBe("separator");
    expect(screen.getByTestId("studio-control-chat-resizer").getAttribute("role")).toBe("separator");

    const fileInput = screen.getByTestId("deck-file-input") as HTMLInputElement;
    expect(fileInput.style.display).toBe("none");
    expect(screen.getByTestId("studio-file-upload-btn")).toBeTruthy();

    const masterPane = screen.getByTestId("studio-master-pane");
    expect(masterPane.style.width).toBe("356px");
    fireEvent.mouseDown(screen.getByTestId("studio-master-detail-resizer"), { clientX: 356 });
    fireEvent.mouseMove(window, { clientX: 430 });
    fireEvent.mouseUp(window);
    expect(masterPane.style.width).toBe("430px");

    const subChatPanel = screen.getByTestId("studio-subchat-panel");
    fireEvent.click(screen.getByTestId("studio-toggle-subchat-btn"));
    expect(subChatPanel.style.height).toBe("34px");
    fireEvent.click(screen.getByTestId("studio-toggle-subchat-btn"));
    expect(subChatPanel.style.height).toBe("168px");

    fireEvent.click(screen.getByTestId("studio-toggle-master-btn"));
    expect(screen.queryByTestId("studio-master-pane")).toBeNull();
    fireEvent.click(screen.getByTestId("studio-toggle-master-btn"));
    expect(screen.getByTestId("studio-master-pane")).toBeTruthy();
  });

  it("UT-07: tracks source-data divergence on direct table edits, reverts cleanly & supports Undo/Redo", () => {
    render(<DeckCsvGeneratorModal isOpen={true} projectId="proj-alpha" onClose={vi.fn()} />);

    expect(screen.queryByTestId("studio-data-divergence-banner")).toBeNull();
    const undoBtn = screen.getByTestId("studio-undo-btn") as HTMLButtonElement;
    const redoBtn = screen.getByTestId("studio-redo-btn") as HTMLButtonElement;
    expect(undoBtn.disabled).toBe(true);

    fireEvent.change(screen.getByLabelText("Giá trị Q4 Direct"), { target: { value: "2000" } });
    const banner = screen.getByTestId("studio-data-divergence-banner");
    expect(banner.textContent).toContain("1 dòng thay đổi");
    expect(banner.textContent).toContain("+$580");
    expect(undoBtn.disabled).toBe(false);

    fireEvent.click(undoBtn);
    expect((screen.getByLabelText("Giá trị Q4 Direct") as HTMLInputElement).value).toBe("1420");
    expect(screen.queryByTestId("studio-data-divergence-banner")).toBeNull();
    expect(redoBtn.disabled).toBe(false);

    fireEvent.click(redoBtn);
    expect((screen.getByLabelText("Giá trị Q4 Direct") as HTMLInputElement).value).toBe("2000");

    fireEvent.click(screen.getByTestId("studio-add-row-btn"));
    expect(screen.getByTestId("studio-data-table").querySelectorAll("tbody tr").length).toBe(5);
    fireEvent.click(screen.getByTestId("studio-revert-source-btn"));
    expect(screen.getByTestId("studio-data-table").querySelectorAll("tbody tr").length).toBe(4);
    expect(screen.queryByTestId("studio-data-divergence-banner")).toBeNull();
  });

  it("UT-08: Progressive 2-Level Drill-Down Control Plane syncs with component rows & Detail Canvas clicks", () => {
    render(<DeckCsvGeneratorModal isOpen={true} projectId="proj-alpha" onClose={vi.fn()} />);

    // Starts at Level 1 (Dữ liệu chung) with component list
    expect(screen.getByTestId("studio-control-general")).toBeTruthy();
    expect(screen.getByTestId("studio-component-list")).toBeTruthy();

    // Toggle KPI strip off/on from Level 1 row switch
    expect(screen.getByTestId("studio-kpi-strip")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-widget-chip-kpiStrip"));
    expect(screen.queryByTestId("studio-kpi-strip")).toBeNull();
    fireEvent.click(screen.getByTestId("studio-widget-chip-kpiStrip"));
    expect(screen.getByTestId("studio-kpi-strip")).toBeTruthy();

    // Drill into Level 2 Chart Inspector from Level 1 row
    fireEvent.click(screen.getByTestId("studio-component-row-chart"));
    expect(screen.getByTestId("studio-control-inspector")).toBeTruthy();
    expect(screen.getByTestId("studio-inspector-badge").textContent).toBe("Cài đặt Biểu đồ");
    expect(screen.getByTestId("studio-target-input")).toBeTruthy();

    // Keyboard target drag handle syncs directly with Chart Inspector target input
    fireEvent.keyDown(screen.getByTestId("studio-target-drag-handle"), { key: "ArrowUp" });
    expect((screen.getByTestId("studio-target-input") as HTMLInputElement).value).toBe("1050");

    // Back to Level 1
    fireEvent.click(screen.getByTestId("studio-inspector-back-btn"));
    expect(screen.getByTestId("studio-control-general")).toBeTruthy();

    // Clicking KPI card on Detail Canvas automatically drills into KPI Inspector
    fireEvent.click(screen.getByTestId("studio-card-kpi-revenue-card"));
    expect(screen.getByTestId("studio-inspector-badge").textContent).toBe("Cài đặt Thẻ KPI");
    expect(screen.getAllByTestId("studio-kpi-tick-bar").length).toBe(3);
    fireEvent.click(screen.getByTestId("studio-kpi-tick-toggle"));
    expect(screen.queryAllByTestId("studio-kpi-tick-bar").length).toBe(0);

    // Clicking Share-Bar on Detail Canvas drills into Share-Bar Inspector and toggles sort order
    fireEvent.click(screen.getByTestId("studio-share-bar-panel"));
    expect(screen.getByTestId("studio-inspector-badge").textContent).toBe("Cài đặt Share-Bar");
    fireEvent.click(screen.getByTestId("studio-sharebar-sort-asc"));
    const shareRows = screen.getByTestId("studio-share-bar-panel").querySelectorAll("button");
    expect(shareRows[0].textContent).toContain("Organic");
  });

  it("UT-09: Unified 2-Row Canvas Toolbar switches all 10 chart shapes, palettes, sub-states & interaction modes", () => {
    render(<DeckCsvGeneratorModal isOpen={true} projectId="proj-alpha" onClose={vi.fn()} />);

    // Open Visual Chart Picker Popover and test all 10 shapes
    const shapes: ReadonlyArray<[string, string]> = [
      ["movement", "studio-svg-movement"],
      ["line", "studio-svg-line"],
      ["sparkline", "studio-svg-sparkline"],
      ["pie", "studio-svg-pie"],
      ["segmented", "studio-svg-segmented"],
      ["bullet", "studio-svg-bullet"],
      ["tick-bar", "studio-svg-tick-bar"],
      ["kpi", "studio-kpi-chart-view"],
      ["heatmap", "studio-svg-heatmap"],
      ["bar", "studio-svg-bar"],
    ];
    for (const [shapeId, expectedTestId] of shapes) {
      fireEvent.click(screen.getByTestId("studio-chart-picker-btn"));
      expect(screen.getByTestId("studio-chart-picker-popover")).toBeTruthy();
      fireEvent.click(screen.getByTestId(`studio-chart-type-${shapeId}`));
      expect(screen.getByTestId(expectedTestId)).toBeTruthy();
    }

    // Palette popover switches palette
    fireEvent.click(screen.getByTestId("studio-palette-picker-btn"));
    fireEvent.click(screen.getByTestId("studio-palette-emerald"));
    expect(screen.queryByTestId("studio-palette-picker-popover")).toBeNull();

    // Row 2 Sub-state pill switcher renders EMPTY / LOADING / ERROR / DEFAULT
    fireEvent.click(screen.getByTestId("studio-substate-EMPTY"));
    expect(screen.getByTestId("studio-state-empty")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-substate-LOADING"));
    expect(screen.getByTestId("studio-state-loading")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-substate-ERROR"));
    expect(screen.getByTestId("studio-state-error")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-substate-DEFAULT"));
    expect(screen.getByTestId("studio-svg-bar")).toBeTruthy();

    // Hover tooltip uses elevated contrast surface
    fireEvent.mouseEnter(screen.getByTestId("studio-chart-datum-row-1"));
    expect(screen.getByTestId("studio-chart-tooltip").textContent).toContain("Direct");
  });

  it("UT-10: executes all 5 Breakthrough Data Storytelling patterns (AI Lenses, Callout+Spotlight+Explain, QA Auto-Fix, What-If, CEO 3s Mode) & Sub-Chat/Slide Sync", () => {
    const onSyncToMainChat = vi.fn();
    const onEmbedIntoActiveSlide = vi.fn();
    render(
      <DeckCsvGeneratorModal
        isOpen={true}
        projectId="proj-alpha"
        onClose={vi.fn()}
        onSyncToMainChat={onSyncToMainChat}
        onEmbedIntoActiveSlide={onEmbedIntoActiveSlide}
      />
    );

    // 1. AI 3-Lens Strip + 3-Act Narrative Pill
    expect(screen.getByTestId("studio-ai-lens-strip")).toBeTruthy();
    expect(screen.getByTestId("studio-story-arc-pill").textContent).toContain("Hồi 1: Bối cảnh");
    fireEvent.click(screen.getByTestId("studio-lens-target"));
    expect(screen.getByTestId("studio-svg-tick-bar")).toBeTruthy();
    expect(screen.getByTestId("studio-canvas-title").textContent).toContain("vượt ngưỡng mục tiêu");
    fireEvent.click(screen.getByTestId("studio-lens-dynamics"));
    expect(screen.getByTestId("studio-svg-movement")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-lens-driver"));
    expect(screen.getByTestId("studio-svg-bar")).toBeTruthy();
    expect(screen.getByTestId("studio-svg-callout-row-1")).toBeTruthy();

    // Undo restores previous lens cleanly
    fireEvent.click(screen.getByTestId("studio-undo-btn"));
    expect(screen.getByTestId("studio-svg-movement")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-redo-btn"));
    expect(screen.getByTestId("studio-svg-bar")).toBeTruthy();

    // 2. Contextual Point Actions: SVG Callout toggle, Spotlight Grey-Out, Ask Agent Explain
    fireEvent.click(screen.getByTestId("studio-chart-datum-row-2"));
    expect(screen.getByTestId("studio-point-action-bar")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-point-add-callout-btn"));
    expect(screen.getByTestId("studio-svg-callout-row-2")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-point-spotlight-btn"));
    fireEvent.click(screen.getByTestId("studio-point-ask-ai-btn"));
    expect(screen.getByTestId("studio-subchat-messages").textContent).toContain("Phân tích Partner");

    // 3. What-If Scenario Direct Manipulation -> badge + live KPI + Divergence banner + 1-click revert
    fireEvent.click(screen.getByTestId("studio-whatif-plus20-btn"));
    expect(screen.getByTestId("studio-whatif-badge").textContent).toContain("Kịch bản giả lập");
    expect(screen.getByTestId("studio-data-divergence-banner")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-revert-source-btn"));
    expect(screen.queryByTestId("studio-whatif-badge")).toBeNull();
    expect(screen.queryByTestId("studio-data-divergence-banner")).toBeNull();

    // 4. 1-Click Honest-Data QA Audit & Auto-Fix
    fireEvent.click(screen.getByTestId("studio-mode-text"));
    fireEvent.click(screen.getByTestId("studio-canvas-title"));
    const titleEditor = screen.getByTestId("studio-canvas-title-input");
    fireEvent.change(titleEditor, { target: { value: "Doanh thu Q4" } });
    fireEvent.keyDown(titleEditor, { key: "Enter" });
    fireEvent.click(screen.getByTestId("studio-qa-audit-btn"));
    expect(screen.getByTestId("studio-qa-audit-popover")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-qa-autofix-btn"));
    expect(screen.getByTestId("studio-canvas-title").textContent).toContain("dẫn đầu");

    // 5. Audience Mode (Slide CEO 3s vs Chi tiết) hides clutter in CEO mode
    expect(screen.getByTestId("studio-share-bar-panel")).toBeTruthy();
    expect(screen.getByTestId("studio-canvas-footnote")).toBeTruthy();
    fireEvent.click(screen.getByTestId("studio-audience-toggle"));
    expect(screen.getByTestId("studio-audience-toggle").textContent).toBe("Slide CEO 3s");
    expect(screen.queryByTestId("studio-share-bar-panel")).toBeNull();
    expect(screen.queryByTestId("studio-canvas-footnote")).toBeNull();

    // Pin mode + Sub-Chat command + Embed into active slide & Main Chat sync
    fireEvent.click(screen.getByTestId("studio-mode-pin"));
    fireEvent.click(screen.getByTestId("studio-chart-stage"), { clientX: 120, clientY: 90 });
    expect(screen.getByTestId("studio-comment-pin-1")).toBeTruthy();
    expect(screen.getByTestId("studio-mode-pin-count").textContent).toBe("1");
    fireEvent.change(screen.getByTestId("studio-subchat-input"), {
      target: { value: "Chuyển biểu đồ đường và xử lý ghim" },
    });
    fireEvent.click(screen.getByTestId("studio-subchat-send-btn"));
    expect(screen.getByTestId("studio-svg-line")).toBeTruthy();
    expect(screen.queryByTestId("studio-comment-pin-1")).toBeNull();

    fireEvent.click(screen.getByTestId("studio-embed-slide-btn"));
    expect(onEmbedIntoActiveSlide).toHaveBeenCalledWith(expect.stringContaining('data-audience="ceo"'), expect.any(String));
    expect(onSyncToMainChat).toHaveBeenCalledWith(expect.stringContaining("[Dashboard Studio]"));
  });
});
