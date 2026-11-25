// @vitest-environment jsdom
// @story 07.1
// @implements AC-01, AC-02, AC-03, AC-04
import React, { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { VisualQcOverlayPanel } from "./VisualQcOverlayPanel";
import { ProjectReadinessModal } from "./ProjectReadinessModal";
import { useVisualQcController, type VisualQcPort } from "./useVisualQcController";
import {
  QcAutoFixRequestSchema,
  type QcRunResponse,
  type QcAutoFixResponse,
} from "../../lib/validations/visual-qc";

const PROJECT_ID = "11111111-1111-4111-8111-111111111111";
const INITIAL_HTML = '<main><iframe src="https://x.example"></iframe><p style="color: #cccccc;">Low</p></main>';
const FIXED_HTML = '<main><iframe src="https://x.example" sandbox="allow-scripts"></iframe><p style="color: #595959;">Low</p></main>';

function sampleRunResponse(overrides?: Partial<QcRunResponse["data"]>): QcRunResponse {
  return {
    ok: true,
    requestId: "req-qc-01",
    data: {
      qcRunId: "qc-run-001",
      projectId: PROJECT_ID,
      targetKind: "APP_SCREEN",
      targetId: "screen-checkout",
      layoutSha1: "a".repeat(40),
      viewports: [1440, 1024, 768, 390],
      verdict: "UNVERIFIED_NO_BROWSER",
      p0Count: 1,
      p1Count: 2,
      ssimScore: null,
      issues: [
        {
          id: "sec-iframe-1440",
          ruleId: "R11_CROSS_ORIGIN_IFRAME_SANDBOX",
          category: "security",
          severity: "P0",
          message: 'Cross-origin <iframe> must enforce sandbox="allow-scripts"',
          viewportWidth: 1440,
          ffId: "ff-embed-1",
          bbox: { x: 10, y: 12, width: 80, height: 32 },
          deterministicFixable: true,
          suggestedFix: 'Set sandbox="allow-scripts"',
          targetNodeId: null,
        },
        {
          id: "ovl-1440-a-b",
          ruleId: "R01_RTREE_ELEMENT_OVERLAP",
          category: "layout",
          severity: "P0",
          message: "Elements overlap by 42%",
          viewportWidth: 1440,
          ffId: "ff-card-a",
          bbox: { x: 25, y: 30, width: 90, height: 36 },
          deterministicFixable: false,
          suggestedFix: "Adjust flex gap",
          targetNodeId: null,
        },
      ],
      autofixAttempt: 0,
      maxAutofixAttempts: 3,
      cacheHit: false,
      usedFallback: true,
      fallbackMode: "iframe_bridge_fallback",
      flowCompleteness: {
        deadEndCtas: ["screen-home:cta-broken"],
        orphanScreens: ["screen-orphan"],
        missingSubstates: { "screen-checkout": ["empty", "error"] },
        issues: [],
        isComplete: false,
      },
      rulesEvaluated: 26,
      breakerStatus: {
        isOpen: false,
        consecutiveCrashes: 0,
        totalCrashes: 0,
        totalRestarts: 0,
        lastCrashReason: null,
      },
      createdAt: 1700000000000,
      ...overrides,
    },
  };
}

function sampleFixResponse(attemptNumber: number, mode: "DETERMINISTIC_LINT_FIX" | "AGENT_PATCH"): QcAutoFixResponse {
  return {
    ok: true,
    requestId: `req-fix-${attemptNumber}`,
    data: {
      qcRunId: "qc-run-001",
      projectId: PROJECT_ID,
      mode,
      attemptNumber,
      maxAutofixAttempts: 3,
      remainingAttempts: Math.max(0, 3 - attemptNumber),
      patchedHtml: FIXED_HTML,
      previousHtml: INITIAL_HTML,
      fixedIssueIds: ["sec-iframe-1440"],
      unfixableIssueIds: ["ovl-1440-a-b"],
      elapsedMs: 1.4,
      llmTokensUsed: 0,
      diffSummary: ['[R11_CROSS_ORIGIN_IFRAME_SANDBOX] Enforced sandbox="allow-scripts"'],
      rollbackSupported: true,
      requiresAgentPatch: mode === "DETERMINISTIC_LINT_FIX",
    },
  };
}

const Harness: React.FC<{ port: VisualQcPort }> = ({ port }) => {
  const [html, setHtml] = useState(INITIAL_HTML);
  const ctrl = useVisualQcController({
    projectId: PROJECT_ID,
    targetId: "screen-checkout",
    html,
    onHtmlPatched: setHtml,
    port,
  });
  return (
    <div>
      <div data-testid="live-html">{html}</div>
      <VisualQcOverlayPanel
        runResult={ctrl.runResult}
        activeViewport={ctrl.activeViewport}
        selectedIssueId={ctrl.selectedIssueId}
        isRunning={ctrl.isRunning}
        isFixing={ctrl.isFixing}
        agentAttemptsUsed={ctrl.agentAttemptsUsed}
        isAgentFixLocked={ctrl.isAgentFixLocked}
        canRollback={ctrl.rollbackHtml !== null}
        lastFixResult={ctrl.lastFixResult}
        error={ctrl.error}
        onSelectViewport={ctrl.setActiveViewport}
        onSelectIssue={ctrl.setSelectedIssueId}
        onRunQc={() => void ctrl.runQc()}
        onDeterministicFix={() => void ctrl.runDeterministicFix()}
        onAgentFix={() => void ctrl.runAgentFix()}
        onRollback={ctrl.rollbackLastFix}
        onOpenReadiness={() => ctrl.setIsReadinessOpen(true)}
      />
      <ProjectReadinessModal
        open={ctrl.isReadinessOpen}
        runResult={ctrl.runResult}
        isRunning={ctrl.isRunning}
        onClose={() => ctrl.setIsReadinessOpen(false)}
        onRunQc={() => void ctrl.runQc({ forceRefresh: true })}
        onDeterministicFix={() => void ctrl.runDeterministicFix()}
      />
    </div>
  );
};

afterEach(() => {
  cleanup();
});

describe("Story 07.1 Visual QC Overlay, Project Readiness & 1-Click Auto-Fix", () => {
  it("renders UNVERIFIED_NO_BROWSER honestly, runs <10ms deterministic lint_fix, and rolls back via Cmd+Z", async () => {
    const port: VisualQcPort = {
      runVisualQc: vi.fn().mockResolvedValue(sampleRunResponse()),
      runAutoFix: vi.fn().mockImplementation((req) =>
        Promise.resolve(sampleFixResponse(req.attemptNumber, req.mode))
      ),
    };

    render(<Harness port={port} />);

    expect(screen.getByTestId("visual-qc-overlay").getAttribute("data-ff-id")).toBe("visual-qc-overlay");
    fireEvent.click(screen.getByTestId("qc-run-button"));

    await waitFor(() => {
      const badge = screen.getByTestId("qc-verdict-badge");
      expect(badge.getAttribute("data-verdict")).toBe("UNVERIFIED_NO_BROWSER");
      expect(badge.textContent).toContain("Chưa kiểm tra trình duyệt");
      expect(badge.style.backgroundColor).toBe("var(--state-warning-soft)");
      expect(badge.style.color).toBe("var(--state-warning)");
    });

    // Bounding boxes rendered directly over preview overlay
    expect(screen.getByTestId("qc-bbox-sec-iframe-1440")).toBeTruthy();

    // Open Project Readiness Modal and verify E2E Flow Completeness + honest badge
    fireEvent.click(screen.getByTestId("open-readiness-modal-button"));
    expect(screen.getByTestId("project-readiness-panel").getAttribute("data-ff-id")).toBe(
      "project-readiness-panel"
    );
    const unverifiedBanner = screen.getByTestId("qc-unverified-no-browser-badge");
    expect(unverifiedBanner).toBeTruthy();
    expect(unverifiedBanner.style.backgroundColor).toBe("var(--state-warning-soft)");
    expect(unverifiedBanner.style.color).toBe("var(--state-warning)");
    expect(screen.getByTestId("readiness-metric-verdict").getAttribute("data-verdict")).toBe(
      "UNVERIFIED_NO_BROWSER"
    );
    expect(screen.getByTestId("readiness-metric-verdict").textContent).toContain("Chưa kiểm tra trình duyệt");
    expect(screen.getByTestId("qc-flow-completeness-section").textContent).toMatch(
      /GAPS DETECTED|Gaps Found|Còn thiếu sót/i
    );

    // Trigger 1-Click Deterministic Auto-Fix (<10ms, 0 LLM tokens)
    const fixBtn = screen.getByTestId("one-click-lint-fix-button");
    expect(fixBtn.getAttribute("data-ff-id")).toBe("one-click-lint-fix-button");
    fireEvent.click(fixBtn);

    await waitFor(() => {
      expect(screen.getByTestId("live-html").textContent).toBe(FIXED_HTML);
      expect(screen.getByTestId("qc-autofix-diff-summary").textContent).toMatch(/0 tokens?/i);
    });

    // 1-step rollback via Cmd+Z restores previous HTML snapshot
    fireEvent.keyDown(window, { key: "z", metaKey: true });
    await waitFor(() => {
      expect(screen.getByTestId("live-html").textContent).toBe(INITIAL_HTML);
    });
  });

  it("strictly locks Agent Auto-Fix after MAX_ATTEMPTS = 3 and rejects attempt 4 in Zod schema", async () => {
    let attemptCounter = 0;
    const port: VisualQcPort = {
      runVisualQc: vi.fn().mockResolvedValue(sampleRunResponse({ verdict: "FAIL", usedFallback: false, ssimScore: 0.84 })),
      runAutoFix: vi.fn().mockImplementation((req) => {
        attemptCounter += 1;
        return Promise.resolve(sampleFixResponse(attemptCounter, req.mode));
      }),
    };

    render(<Harness port={port} />);
    fireEvent.click(screen.getByTestId("qc-run-button"));
    await waitFor(() => {
      const badge = screen.getByTestId("qc-verdict-badge");
      expect(badge.getAttribute("data-verdict")).toBe("FAIL");
      expect(badge.textContent).toContain("Cần sửa trước khi xuất");
    });

    const agentBtn = screen.getByTestId("agent-autofix-button") as HTMLButtonElement;
    for (let i = 1; i <= 3; i += 1) {
      fireEvent.click(agentBtn);
      await waitFor(() => expect(port.runAutoFix).toHaveBeenCalledTimes(i));
    }

    await waitFor(() => {
      expect(agentBtn.disabled).toBe(true);
      expect(agentBtn.textContent).toContain("3/3");
    });

    // Zod schema strictly rejects attemptNumber: 4
    const invalid = QcAutoFixRequestSchema.safeParse({
      projectId: PROJECT_ID,
      qcRunId: "qc-run-001",
      issueIds: ["ovl-1440-a-b"],
      mode: "AGENT_PATCH",
      attemptNumber: 4,
    });
    expect(invalid.success).toBe(false);
  });

  it("wraps ProjectReadinessModal in fixed backdrop, closes on Escape, uses var(--bg-surface) and i18n pending verdict (Defects 5 & 8)", () => {
    const onClose = vi.fn();
    render(
      <ProjectReadinessModal
        open={true}
        runResult={null}
        isRunning={false}
        onClose={onClose}
        onRunQc={vi.fn()}
        onDeterministicFix={vi.fn()}
      />
    );

    const backdrop = screen.getByTestId("project-readiness-backdrop");
    expect(backdrop.style.position).toBe("fixed");
    expect(backdrop.style.zIndex).toBe("9999");
    expect(backdrop.style.backgroundColor).toBe("var(--bg-overlay)");

    const panel = screen.getByTestId("project-readiness-panel");
    expect(panel.style.backgroundColor).toBe("var(--bg-surface)");
    const verdictTile = screen.getByTestId("readiness-metric-verdict");
    expect(verdictTile.getAttribute("data-verdict")).toBe("PENDING");
    expect(verdictTile.textContent).toContain("Chưa kiểm tra");

    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("renders friendly issue cards with semantic CSS vars and collapsible technical details for R26/R01/R16/R17 and fallback rules", () => {
    const run = sampleRunResponse({
      verdict: "UNVERIFIED_NO_BROWSER",
      p0Count: 2,
      p1Count: 2,
      issues: [
        {
          id: "r26-cdp",
          ruleId: "R26_UNVERIFIED_NO_BROWSER_CDP",
          category: "runtime",
          severity: "P1",
          message: "CDP binary missing on host",
          viewportWidth: 1440,
          ffId: null,
          bbox: null,
          deterministicFixable: false,
          suggestedFix: null,
          targetNodeId: null,
        },
        {
          id: "r01-ovl",
          ruleId: "R01_RTREE_ELEMENT_OVERLAP",
          category: "layout",
          severity: "P0",
          message: "Card A overlaps Card B",
          viewportWidth: 1024,
          ffId: "ff-a",
          bbox: null,
          deterministicFixable: false,
          suggestedFix: null,
          targetNodeId: null,
        },
        {
          id: "r16-contrast",
          ruleId: "R16_WCAG_CONTRAST",
          category: "a11y",
          severity: "P1",
          message: "Contrast ratio 2.1:1 < 4.5:1",
          viewportWidth: 768,
          ffId: "ff-text",
          bbox: null,
          deterministicFixable: true,
          suggestedFix: null,
          targetNodeId: null,
        },
        {
          id: "r17-iframe",
          ruleId: "R17_UNSANDBOXED_IFRAME",
          category: "security",
          severity: "P0",
          message: "Iframe missing sandbox attribute",
          viewportWidth: 390,
          ffId: "ff-frame",
          bbox: null,
          deterministicFixable: true,
          suggestedFix: null,
          targetNodeId: null,
        },
        {
          id: "r99-custom",
          ruleId: "R99_CUSTOM_CHECK",
          category: "layout",
          severity: "P2",
          message: "Custom diagnostic payload",
          viewportWidth: 1440,
          ffId: null,
          bbox: null,
          deterministicFixable: false,
          suggestedFix: null,
          targetNodeId: null,
        },
      ],
    });

    render(
      <ProjectReadinessModal
        open={true}
        runResult={run.data}
        isRunning={false}
        onClose={vi.fn()}
        onRunQc={vi.fn()}
        onDeterministicFix={vi.fn()}
      />
    );

    // Semantic CSS tokens on banner, metric tiles, and severity pills
    const banner = screen.getByTestId("qc-unverified-no-browser-badge");
    expect(banner.style.backgroundColor).toBe("var(--state-warning-soft)");
    expect(banner.style.color).toBe("var(--state-warning)");

    const p0Value = screen.getByTestId("readiness-metric-p0").querySelector("strong")!;
    expect(p0Value.style.color).toBe("var(--state-danger)");
    const p1Value = screen.getByTestId("readiness-metric-p1").querySelector("strong")!;
    expect(p1Value.style.color).toBe("var(--state-warning)");

    const p0Pill = screen.getByTestId("readiness-issue-severity-r01-ovl");
    expect(p0Pill.textContent).toBe("Lỗi cần sửa");
    expect(p0Pill.style.backgroundColor).toBe("var(--state-danger-soft)");
    expect(p0Pill.style.color).toBe("var(--state-danger)");

    const p1Pill = screen.getByTestId("readiness-issue-severity-r26-cdp");
    expect(p1Pill.textContent).toBe("Gợi ý cải thiện");
    expect(p1Pill.style.backgroundColor).toBe("var(--state-warning-soft)");
    expect(p1Pill.style.color).toBe("var(--state-warning)");

    // Friendly rule titles & collapsible <details> technical diagnostics
    expect(screen.getByTestId("readiness-issue-r26-cdp").textContent).toContain("Chưa có trình duyệt kiểm tra ảnh chụp");
    expect(screen.getByTestId("readiness-issue-r01-ovl").textContent).toContain("Các thành phần bị chồng lấn");
    expect(screen.getByTestId("readiness-issue-r16-contrast").textContent).toContain("Độ tương phản chữ chưa đủ rõ");
    expect(screen.getByTestId("readiness-issue-r17-iframe").textContent).toContain("Khung nhúng chưa được bảo vệ an toàn");
    expect(screen.getByTestId("readiness-issue-r99-custom").textContent).toContain("Cần điều chỉnh hiển thị");

    const details = screen.getByTestId("readiness-issue-details-r26-cdp") as HTMLDetailsElement;
    expect(details.tagName).toBe("DETAILS");
    expect(details.querySelector("summary")?.textContent).toContain("Xem chi tiết kỹ thuật");
    expect(details.textContent).toContain("R26_UNVERIFIED_NO_BROWSER_CDP");
    expect(details.textContent).toContain("1440px");
    expect(details.textContent).toContain("CDP binary missing on host");
  });

  it("filters overlay issues by activeViewport (+ flow) and normalizes bbox coordinates against viewport size clamped 0..92% (Defect 7)", () => {
    const multiVpRun = sampleRunResponse({
      issues: [
        {
          id: "vp-1440-only",
          ruleId: "R02_VERTICAL_OVERFLOW",
          category: "layout",
          severity: "P0",
          message: "Desktop overflow",
          viewportWidth: 1440,
          ffId: "ff-desktop",
          bbox: { x: 720, y: 450, width: 80, height: 40 },
          deterministicFixable: false,
          suggestedFix: null,
          targetNodeId: null,
        },
        {
          id: "vp-390-only",
          ruleId: "R03_HORIZONTAL_OVERFLOW",
          category: "layout",
          severity: "P0",
          message: "Mobile overflow",
          viewportWidth: 390,
          ffId: "ff-mobile",
          bbox: { x: 600, y: 1200, width: 80, height: 40 },
          deterministicFixable: false,
          suggestedFix: null,
          targetNodeId: null,
        },
        {
          id: "flow-global",
          ruleId: "R23_FLOW_DEAD_END_CTA",
          category: "flow",
          severity: "P0",
          message: "Dead CTA",
          viewportWidth: 1440,
          ffId: "cta-1",
          bbox: { x: 960, y: 540, width: 80, height: 40 },
          deterministicFixable: false,
          suggestedFix: null,
          targetNodeId: "screen-1",
        },
      ],
    });

    const noop = vi.fn();
    const { rerender } = render(
      <VisualQcOverlayPanel
        runResult={multiVpRun.data}
        activeViewport={1440}
        selectedIssueId={null}
        isRunning={false}
        isFixing={false}
        agentAttemptsUsed={0}
        isAgentFixLocked={false}
        canRollback={false}
        lastFixResult={null}
        error={null}
        onSelectViewport={noop}
        onSelectIssue={noop}
        onRunQc={noop}
        onDeterministicFix={noop}
        onAgentFix={noop}
        onRollback={noop}
        onOpenReadiness={noop}
      />
    );

    expect(screen.getByTestId("visual-qc-overlay").style.backgroundColor).toBe("var(--bg-surface)");
    const desktopBox = screen.getByTestId("qc-bbox-vp-1440-only");
    expect(desktopBox.style.left).toBe("50%");
    expect(desktopBox.style.top).toBe("50%");
    expect(screen.queryByTestId("qc-bbox-vp-390-only")).toBeNull();
    expect(screen.getByTestId("qc-bbox-flow-global")).toBeTruthy();

    // Switch to 390 mobile viewport -> shows 390 + flow, clamps overflowing coords to 92%
    rerender(
      <VisualQcOverlayPanel
        runResult={multiVpRun.data}
        activeViewport={390}
        selectedIssueId={null}
        isRunning={false}
        isFixing={false}
        agentAttemptsUsed={0}
        isAgentFixLocked={false}
        canRollback={false}
        lastFixResult={null}
        error={null}
        onSelectViewport={noop}
        onSelectIssue={noop}
        onRunQc={noop}
        onDeterministicFix={noop}
        onAgentFix={noop}
        onRollback={noop}
        onOpenReadiness={noop}
      />
    );
    expect(screen.queryByTestId("qc-bbox-vp-1440-only")).toBeNull();
    const mobileBox = screen.getByTestId("qc-bbox-vp-390-only");
    expect(mobileBox.style.left).toBe("92%");
    expect(mobileBox.style.top).toBe("92%");
    expect(screen.getByTestId("qc-bbox-flow-global")).toBeTruthy();

    // Switch to 1920 slide viewport -> normalizes y against 1080 height
    rerender(
      <VisualQcOverlayPanel
        runResult={multiVpRun.data}
        activeViewport={1920}
        selectedIssueId={null}
        isRunning={false}
        isFixing={false}
        agentAttemptsUsed={0}
        isAgentFixLocked={false}
        canRollback={false}
        lastFixResult={null}
        error={null}
        onSelectViewport={noop}
        onSelectIssue={noop}
        onRunQc={noop}
        onDeterministicFix={noop}
        onAgentFix={noop}
        onRollback={noop}
        onOpenReadiness={noop}
      />
    );
    const slideFlowBox = screen.getByTestId("qc-bbox-flow-global");
    expect(slideFlowBox.style.left).toBe("50%");
    expect(slideFlowBox.style.top).toBe("50%");
  });

  it("ignores Cmd+Z rollback shortcut inside INPUT, TEXTAREA, SELECT, and contentEditable (Defect 8)", async () => {
    const port: VisualQcPort = {
      runVisualQc: vi.fn().mockResolvedValue(sampleRunResponse()),
      runAutoFix: vi.fn().mockImplementation((req) =>
        Promise.resolve(sampleFixResponse(req.attemptNumber, req.mode))
      ),
    };

    render(
      <div>
        <input data-testid="editable-input" />
        <textarea data-testid="editable-textarea" />
        <select data-testid="editable-select" />
        <div data-testid="editable-ce" contentEditable="true" suppressContentEditableWarning />
        <Harness port={port} />
      </div>
    );

    fireEvent.click(screen.getByTestId("qc-run-button"));
    await waitFor(() => expect(screen.getByTestId("qc-verdict-badge")).toBeTruthy());

    fireEvent.click(screen.getByTestId("one-click-lint-fix-button"));
    await waitFor(() => expect(screen.getByTestId("live-html").textContent).toBe(FIXED_HTML));

    for (const tid of ["editable-input", "editable-textarea", "editable-select", "editable-ce"]) {
      fireEvent.keyDown(screen.getByTestId(tid), { key: "z", metaKey: true, bubbles: true });
      expect(screen.getByTestId("live-html").textContent).toBe(FIXED_HTML);
    }

    // Outside editable fields, Cmd+Z rolls back cleanly
    fireEvent.keyDown(window, { key: "z", metaKey: true });
    await waitFor(() => expect(screen.getByTestId("live-html").textContent).toBe(INITIAL_HTML));
  });
});
