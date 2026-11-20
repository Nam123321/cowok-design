// @vitest-environment jsdom
// @story 04.5, 05.1, 05.2, 06.2
// @implements AC-05, AC-10, AC-08, AC-07 (Spatial Action Pill, CommentPod Envelope, Dual-Mode Edit & Inspector)
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";

afterEach(() => cleanup());
import {
  StageSpatialActionPill,
  shouldShowLinkToDagAction,
} from "../components/StageSpatialActionPill";
import { CommentPodPopover } from "../../comments/components/CommentPodPopover";
import { InlineDirectEditPopover } from "../components/InlineDirectEditPopover";
import { ElementPropertyInspectorPopover } from "../components/ElementPropertyInspectorPopover";
import { findIntersectingElementIds } from "../components/MarqueeSelectionOverlay";
import { computeFloatingPodLayout } from "../components/StageSpatialOverlaysLayer";
import { HtmlPreviewCmsActions } from "../components/HtmlPreviewCmsActions";
import { buildPreviewDocumentHtml } from "../utils/html-sandbox-wrapper";
import { StructuredCommentEnvelopeSchema } from "../../../lib/validations/element-comment";
import type { SpatialTargetSelection } from "../utils/sandbox-bridge-protocol";

const SINGLE_TARGET: SpatialTargetSelection = {
  targetType: "html_element",
  triggerSource: "interact-contextmenu",
  selector: '[data-ff-id="ff-hero_cover-headline"]',
  label: "@[Element SLD-01 • #ff-hero_cover-headline]",
  elementFfId: "ff-hero_cover-headline",
  coveredFfIds: ["ff-hero_cover-headline"],
  domHash: "abc12345",
  rect: { left: 40, top: 60, width: 320, height: 48 },
  metadata: {
    tagName: "h2",
    textContent: "Executive Deck Title",
    slotKind: "heading",
    cssVariables: { "--text-primary": "#111111", "font-size": "24px" },
    overflowPx: 0,
  },
};

const REGION_TARGET: SpatialTargetSelection = {
  targetType: "spatial_region",
  triggerSource: "select-marquee",
  selector: "box-20-20-240x160",
  label: "@[Region 240×160px @ (20,20)]",
  elementFfId: "ff-hero_cover-headline",
  coveredFfIds: ["ff-hero_cover-headline", "ff-hero_cover-body"],
  rect: { left: 20, top: 20, width: 240, height: 160 },
};

describe("StageSpatialActionPill context-aware rules (EC-E05-04)", () => {
  it("hides Link to DAG on Presentation (slide-cms) and when FE Left DAG is already focused", () => {
    expect(
      shouldShowLinkToDagAction({
        structureLens: "slide-cms",
        isLeftStructureHidden: true,
        isLeftDagFocusedOnNode: false,
      })
    ).toBe(false);

    expect(
      shouldShowLinkToDagAction({
        structureLens: "userflow",
        isLeftStructureHidden: false,
        isLeftDagFocusedOnNode: true,
      })
    ).toBe(false);

    expect(
      shouldShowLinkToDagAction({
        structureLens: "userflow",
        isLeftStructureHidden: true,
        isLeftDagFocusedOnNode: true,
      })
    ).toBe(true);
  });

  it("disables Inspect Property on spatial_region drag and hides Link to DAG on Presentation", () => {
    const onSelect = vi.fn();
    render(
      <StageSpatialActionPill
        target={REGION_TARGET}
        structureLens="slide-cms"
        isLeftStructureHidden={false}
        isLeftDagFocusedOnNode={false}
        onSelectAction={onSelect}
        onClose={vi.fn()}
      />
    );

    const inspectBtn = screen.getByTestId("spatial-action-inspect") as HTMLButtonElement;
    expect(inspectBtn.disabled).toBe(true);
    expect(inspectBtn.getAttribute("data-tooltip")).toBeTruthy();
    expect(inspectBtn.getAttribute("title")).toBeNull();
    expect((inspectBtn.textContent || "").trim()).toBe("");
    expect(screen.queryByTestId("spatial-action-link-dag")).toBeNull();

    // Verify clean single tooltips (no title attr, no noisy raw shortcut letters) & responsive bottom placement by default
    const pill = screen.getByTestId("stage-spatial-action-pill");
    expect(pill.getAttribute("data-tooltip-placement")).toBe("bottom");
    for (const btnTestId of ["spatial-action-comment", "spatial-action-edit-draft", "spatial-action-inspect", "spatial-action-close"]) {
      const btn = screen.getByTestId(btnTestId);
      expect(btn.getAttribute("title")).toBeNull();
      expect(btn.getAttribute("data-tooltip")).not.toMatch(/Alt\+Click|Shift\+Click|Ctrl\+Click/i);
    }

    fireEvent.click(screen.getByTestId("spatial-action-edit-draft"));
    expect(onSelect).toHaveBeenCalledWith("edit-draft");
  });

  it("flips tooltip placement to top near bottom edge and aligns start/end near horizontal edges", () => {
    const bottomRightTarget: SpatialTargetSelection = {
      ...SINGLE_TARGET,
      rect: { left: 490, top: 470, width: 120, height: 40 },
    };
    render(
      <StageSpatialActionPill
        target={bottomRightTarget}
        structureLens="userflow"
        isLeftStructureHidden={true}
        isLeftDagFocusedOnNode={false}
        stageWidth={560}
        stageHeight={500}
        onSelectAction={vi.fn()}
        onClose={vi.fn()}
      />
    );
    const pill = screen.getByTestId("stage-spatial-action-pill");
    expect(pill.getAttribute("data-tooltip-placement")).toBe("top");
    expect(pill.getAttribute("data-tooltip-align")).toBe("end");
  });

  it("shows all 4 actions in FE mode when Left Pane is hidden and supports C/E/I/L shortcuts", () => {
    const onSelect = vi.fn();
    render(
      <StageSpatialActionPill
        target={SINGLE_TARGET}
        structureLens="userflow"
        isLeftStructureHidden={true}
        isLeftDagFocusedOnNode={true}
        onSelectAction={onSelect}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByTestId("spatial-action-link-dag")).toBeTruthy();
    fireEvent.keyDown(window, { key: "i" });
    expect(onSelect).toHaveBeenCalledWith("inspect");
    fireEvent.keyDown(window, { key: "l" });
    expect(onSelect).toHaveBeenCalledWith("link-dag");
  });
});

describe("Marquee multi-element intersection & CommentPodPopover (EC-E04-10)", () => {
  it("intersects marquee rectangle with all enclosed data-ff-id elements ordered top-to-bottom", () => {
    const ids = findIntersectingElementIds(10, 10, 260, 140, {
      "ff-footer": { left: 20, top: 300, width: 200, height: 30 },
      "ff-body": { left: 20, top: 80, width: 200, height: 40 },
      "ff-headline": { left: 20, top: 20, width: 200, height: 32 },
    });
    expect(ids).toEqual(["ff-headline", "ff-body"]);
  });

  it("submits structured comment with sendToAgent=true vs pin-only", () => {
    const onSubmit = vi.fn();
    render(
      <CommentPodPopover
        target={SINGLE_TARGET}
        onSubmitComment={onSubmit}
        onClose={vi.fn()}
      />
    );

    const input = screen.getByTestId("comment-pod-input");
    fireEvent.change(input, { target: { value: "Tăng độ tương phản tiêu đề chính" } });
    fireEvent.click(screen.getByTestId("comment-pod-send-agent"));
    expect(onSubmit).toHaveBeenCalledWith({
      body: "Tăng độ tương phản tiêu đề chính",
      sendToAgent: true,
    });

    const parsed = StructuredCommentEnvelopeSchema.safeParse({
      commentId: "cmt-001",
      targetType: "html_element",
      elementFfId: "ff-hero_cover-headline",
      coveredFfIds: ["ff-hero_cover-headline"],
      domHash: "abc12345",
      artifactSha256: "sha256-test",
      commentBody: "Tăng độ tương phản tiêu đề chính",
    });
    expect(parsed.success).toBe(true);
  });
});

describe("InlineDirectEditPopover & ElementPropertyInspectorPopover", () => {
  it("emits live changes for single-component direct inline edit", () => {
    const onLiveChange = vi.fn();
    render(
      <InlineDirectEditPopover
        target={SINGLE_TARGET}
        initialValue="Executive Deck Title"
        slotLabel="Headline"
        onLiveChange={onLiveChange}
        onClose={vi.fn()}
      />
    );

    const input = screen.getByTestId("inline-direct-edit-input");
    fireEvent.change(input, { target: { value: "Updated Live Title" } });
    expect(onLiveChange).toHaveBeenCalledWith("Updated Live Title", "Executive Deck Title");
  });

  it("renders read-only metadata, dimensions, CSS vars and E3 overflow status", () => {
    render(
      <ElementPropertyInspectorPopover
        target={SINGLE_TARGET}
        slotKind="heading"
        slotSyncState="SYNCED"
        onClose={vi.fn()}
      />
    );
    expect(screen.getByTestId("element-property-inspector").textContent).toContain("#ff-hero_cover-headline");
    expect(screen.getByTestId("element-property-inspector").textContent).toContain("--text-primary");
  });

  it("clamps floating edit/comment pods inside narrow preview frames beside comments drawer", () => {
    const layout = computeFloatingPodLayout({
      rect: { left: 260, top: 460, width: 120, height: 34 },
      stageWidth: 280,
      stageHeight: 500,
      preferredWidth: 310,
      estimatedHeight: 185,
    });
    expect(layout.width).toBeLessThanOrEqual(264);
    expect(layout.left + layout.width).toBeLessThanOrEqual(272);
    expect(layout.top).toBeLessThan(460);
  });

  it("exposes selectable data-ff-id hooks on KPI labels and values for individual & region editing", () => {
    const html = buildPreviewDocumentHtml({
      structureLens: "userflow",
      activeSlide: { id: "s1", code: "S1", title: "T", headline: "H", subtitle: "S", kpi1: 1, kpi2: "2", kpi3: "3" },
      activeScreenData: {
        eyebrow: "SCR-01",
        headline: "Headline",
        subtitle: "Subtitle",
        amount: 1000,
        statusText: "SAFE",
        latency: "10 ms",
        actionTitle: "Action",
        ctaLabel: "Submit",
      },
      formattedAmount: "$1,000",
      labels: { lang: "vi", kpi1Label: "Hạn mức Giải ngân", kpi2Label: "Trạng thái Bảo mật", kpi3Label: "Độ trễ Đồng bộ" },
    });
    expect(html).toContain('data-ff-id="ff-kpi-1-label"');
    expect(html).toContain('data-ff-id="ff-kpi-2-label"');
    expect(html).toContain('data-ff-id="ff-kpi-3-label"');
  });

  it("renders Save button in toolbar uniformly as icon-only with tooltip & aria-label", () => {
    render(
      <HtmlPreviewCmsActions
        density="wide"
        actions={{
          revision: 2,
          syncState: "DIRTY_FAST_PATCHED",
          hasPendingSave: true,
          canUndo: true,
          canRedo: false,
          onUndo: vi.fn(),
          onRedo: vi.fn(),
          onSave: vi.fn(),
        }}
      />
    );
    const saveBtn = screen.getByTestId("cms-save-btn");
    expect(saveBtn.textContent?.trim()).toBe("");
    expect(saveBtn.getAttribute("title")).toBeTruthy();
    expect(saveBtn.getAttribute("aria-label")).toBeTruthy();
  });
});
