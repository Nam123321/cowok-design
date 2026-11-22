// @vitest-environment jsdom
// @story 05.4
// @implements AC-01, AC-02, AC-03, AC-05
import React from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  evaluateClientContrast,
  LockedRuleChangeReceiptCard,
  StageSkinToggleBar,
  SteeringChipsBar,
  WCAG_AA_MIN_RATIO,
} from "../../src/features/sketch-polish";
import { VisualClarificationModal } from "../../src/features/sdi/components/VisualClarificationModal";
import { computeEffectivePatchStack } from "../../src/lib/sketch-polish-occ-store";

describe("Story 05.4 Cognitive Sketch-to-Polish UI Contracts", () => {
  it("UT-05.4-UI-01: renders Cognitive Phase Stepper, hides sketch lens in Phase 1, shows lens & approve CTA in Phase 3, and guards OKLCH AA", () => {
    const onChangeSkin = vi.fn();
    const onAdvanceToStyle = vi.fn();
    const onToggleSketchLens = vi.fn();
    const onApproveV1 = vi.fn();

    const { rerender } = render(
      <StageSkinToggleBar
        skin="sketch-paper"
        cognitivePhase="sketch"
        onChangeSkin={onChangeSkin}
        onAdvanceToStyle={onAdvanceToStyle}
        oklchWarning="WCAG AA clamped"
        lastSwapMs={1.2}
      />
    );
    expect(document.querySelector("[data-ff-id='stage-skin-toggle']")).not.toBeNull();
    expect(screen.getByTestId("cognitive-phase-stepper")).toBeTruthy();
    expect(screen.getByTestId("oklch-contrast-warning")).toBeTruthy();
    expect(screen.queryByTestId("toggle-sketch-lens-btn")).toBeNull();

    fireEvent.click(screen.getByTestId("promote-sketch-to-style-btn"));
    expect(onAdvanceToStyle).toHaveBeenCalledTimes(1);

    rerender(
      <StageSkinToggleBar
        skin="brand-color"
        cognitivePhase="polish"
        onChangeSkin={onChangeSkin}
        onToggleSketchLens={onToggleSketchLens}
        onApproveV1={onApproveV1}
        lastSwapMs={0.4}
      />
    );
    fireEvent.click(screen.getByTestId("toggle-sketch-lens-btn"));
    expect(onToggleSketchLens).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId("approve-v1-btn"));
    expect(onApproveV1).toHaveBeenCalledTimes(1);

    rerender(
      <StageSkinToggleBar
        skin="brand-color"
        cognitivePhase="approved"
        onChangeSkin={onChangeSkin}
        onToggleSketchLens={onToggleSketchLens}
      />
    );
    expect(screen.getByTestId("approved-v1-badge")).toBeTruthy();

    const sketchCheck = evaluateClientContrast("#2E2D2B", "#FAF9F6");
    expect(sketchCheck.passesAa).toBe(true);
    expect(sketchCheck.ratio).toBeGreaterThanOrEqual(WCAG_AA_MIN_RATIO);

    const lowContrast = evaluateClientContrast("#D9E2EC", "#FAF9F6");
    expect(lowContrast.passesAa).toBe(false);
  });

  it("UT-05.4-UI-02: renders SteeringChipsBar and dispatches chip selection + instant undo", () => {
    const onSelectChip = vi.fn();
    const onUndo = vi.fn();
    render(
      <SteeringChipsBar
        activeChipId="more-whitespace"
        canUndo={true}
        onSelectChip={onSelectChip}
        onUndo={onUndo}
      />
    );
    expect(document.querySelector("[data-ff-id='steering-chips-bar']")).not.toBeNull();
    fireEvent.click(screen.getByTestId("steering-chip-softer-corners"));
    expect(onSelectChip).toHaveBeenCalledWith("softer-corners");

    fireEvent.click(screen.getByTestId("steering-undo-btn"));
    expect(onUndo).toHaveBeenCalledTimes(1);
  });

  it("UT-05.4-UI-03: renders plain-language LockedRuleChangeReceiptCard without raw ruleCode jargon", () => {
    const onConfirm = vi.fn();
    const onDismiss = vi.fn();
    render(
      <LockedRuleChangeReceiptCard
        receipt={{
          ruleCode: "APPSHELL_LOCK_BOUNDARY",
          lockOwnerId: "designer-1",
          fencingToken: 2,
          affectedFfId: "shell-nav",
          summary: "Protected AppShell slot",
          requiresReview: true,
          applied: false,
        }}
        onConfirmOverride={onConfirm}
        onDismiss={onDismiss}
      />
    );
    const card = screen.getByTestId("locked-rule-receipt-card");
    expect(card).toBeTruthy();
    expect(card.textContent).not.toContain("APPSHELL_LOCK_BOUNDARY");
    fireEvent.click(screen.getByTestId("receipt-confirm-override-btn"));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId("receipt-dismiss-btn"));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it("UT-05.4-UI-04: renders Phase 2 VisualClarificationModal with Reference Apps, user-driven themeModes, Popular Components & Live Direction Pills", () => {
    const onSwitch = vi.fn();
    const onConfirm = vi.fn();
    const onClose = vi.fn();
    const onChangeReferenceApps = vi.fn();
    const onToggleThemeMode = vi.fn();
    const onTogglePopularComponent = vi.fn();

    render(
      <VisualClarificationModal
        isOpen={true}
        variants={[]}
        liveDirections={[
          {
            id: "dir-editorial-sketch",
            label: "Direction 1",
            summary: "Monochrome sketch",
            accentHex: "#2563EB",
            surfaceHex: "#FAF9F6",
            skillSlug: "sketch-monochrome",
          },
          {
            id: "dir-polish-workbench",
            label: "Direction 2",
            summary: "Polished bento",
            accentHex: "#0F766E",
            surfaceHex: "#F8FAFC",
            skillSlug: "impeccable-polish",
          },
        ]}
        activeDirectionId="dir-editorial-sketch"
        generationEpoch={2}
        referenceApps="Linear, Notion"
        themeModes={["warm"]}
        selectedPopularComponents={["toast", "confirm-dialog"]}
        onChangeReferenceApps={onChangeReferenceApps}
        onToggleThemeMode={onToggleThemeMode}
        onTogglePopularComponent={onTogglePopularComponent}
        onSwitchLiveDirection={onSwitch}
        onConfirm={onConfirm}
        onClose={onClose}
      />
    );
    expect(document.querySelector("[data-ff-id='visual-clarification-modal']")).not.toBeNull();
    fireEvent.change(screen.getByTestId("clarify-reference-apps-input"), { target: { value: "Stripe, Airbnb" } });
    expect(onChangeReferenceApps).toHaveBeenCalledWith("Stripe, Airbnb");

    expect(screen.getByTestId("theme-mode-pill-warm").getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByTestId("theme-mode-pill-dark").getAttribute("aria-pressed")).toBe("false");
    fireEvent.click(screen.getByTestId("theme-mode-pill-dim"));
    expect(onToggleThemeMode).toHaveBeenCalledWith("dim");

    fireEvent.click(screen.getByTestId("popular-comp-tooltip"));
    expect(onTogglePopularComponent).toHaveBeenCalledWith("tooltip");

    fireEvent.click(screen.getByTestId("live-direction-pill-dir-polish-workbench"));
    expect(onSwitch).toHaveBeenCalledWith("dir-polish-workbench");
    expect(screen.getByTestId("clarify-generation-epoch").textContent).toContain("#2");

    fireEvent.click(screen.getByTestId("clarify-confirm-btn"));
    expect(onConfirm).toHaveBeenCalledWith("dir-polish-workbench");
  });

  it("UT-05.4-UI-05: supports rgb() contrast checks, fails closed on unparseable CSS, and skips undone/unapplied patches in stack replay", () => {
    const rgbPass = evaluateClientContrast("rgb(46, 45, 43)", "rgb(250, 249, 246)");
    expect(rgbPass.passesAa).toBe(true);
    expect(rgbPass.ratio).toBeGreaterThanOrEqual(WCAG_AA_MIN_RATIO);

    const invalidFail = evaluateClientContrast("not-a-color", "#FAF9F6");
    expect(invalidFail.passesAa).toBe(false);
    expect(invalidFail.ratio).toBe(0);

    const stack = computeEffectivePatchStack([
      { astRevision: 6, ffId: "ff-action-cta", opKind: "TEXT_EDIT", payloadJson: { text: "Blocked" }, lockedRuleReceipt: { applied: false } },
      { astRevision: 5, ffId: "ff-kpi-1-val", opKind: "TEXT_EDIT", payloadJson: { text: "$99k" } },
      { astRevision: 4, ffId: "ff-action-cta", opKind: "UNDO", payloadJson: {} },
      { astRevision: 3, ffId: "ff-action-cta", opKind: "TEXT_EDIT", payloadJson: { text: "Undone C" } },
      { astRevision: 2, ffId: "ff-slide-subtitle", opKind: "TEXT_EDIT", payloadJson: { text: "Keep B" } },
      { astRevision: 1, ffId: "ff-slide-headline", opKind: "TEXT_EDIT", payloadJson: { text: "Keep A" } },
    ]);
    expect(stack.map((p) => p.astRevision)).toEqual([1, 2, 5]);
  });
});
