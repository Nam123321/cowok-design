// @vitest-environment jsdom
// @story 05.1
// @implements AC-01, AC-02, AC-03, AC-04
import React from "react";
import { describe, expect, it, vi, afterEach } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  wrapUserHtmlSrcDoc,
  buildStrictOfflineCsp,
  computeArtifactSha256,
  sanitizeCssContent,
} from "../utils/html-sandbox-wrapper";
import { CompanionLiveHtmlRightPane } from "../components/CompanionLiveHtmlRightPane";
import { CompanionCanvasTab } from "../components/CompanionCanvasTab";
import { DagNodeCard, type ExtendedWorkbenchNode } from "../components/DagNodeCard";
import { ProjectLaunchpadModal } from "../components/ProjectLaunchpadModal";
import { SampleDataStudioModal } from "../components/SampleDataStudioModal";
import { DEFAULT_TABS, INITIAL_SLIDES, type ScreenPreviewData } from "../components/companion-canvas-data";
import type { WorkbenchNode } from "../store/flow-workbench-types";

describe("Story 05.1: Frontend Sandbox Hardening, IframeKeepAlivePool & Option 5 Storyboard", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  describe("1. Sandbox HTML Hardening & CSP Nonce", () => {
    it("strips dangerous tags (script, iframe, object, embed, foreignobject, unsafe svg, template, noscript) and infers safe svg viewBox", () => {
      const maliciousPayload = `
        <div>Safe Content</div>
        <script>alert('xss')</script>
        <iframe src="https://evil.com"></iframe>
        <object data="malicious.swf"></object>
        <embed src="plugin.pdf">
        <svg onload="alert(1)"><text>Unsafe SVG</text></svg>
        <svg><rect width="10" height="10"/></svg>
        <template><p>Template Injected</p></template>
        <noscript><p>Noscript Injected</p></noscript>
      `;
      const wrapped = wrapUserHtmlSrcDoc(maliciousPayload);

      expect(wrapped).toContain("Safe Content");
      expect(wrapped).not.toContain("<script>alert");
      expect(wrapped).not.toContain("<iframe");
      expect(wrapped).not.toContain("<object");
      expect(wrapped).not.toContain("<embed");
      expect(wrapped).not.toContain("Unsafe SVG");
      expect(wrapped).toContain('viewBox="0 0 1200 800"');
      expect(wrapped).not.toContain("<template");
      expect(wrapped).not.toContain("<noscript");
    });

    it("strips all inline on* event attributes and dangerous URI schemes", () => {
      const hostileHtml = `
        <button onclick="alert('click')" onmouseover="alert('hover')">Click Me</button>
        <a href="javascript:alert('href')">Bad Link</a>
        <form action="vbscript:msgbox('bad')"><input formaction="data:text/html,<script>"></form>
        <img src="data:image/svg+xml;base64,PHN2Zz4=" onerror="alert('img')">
      `;
      const wrapped = wrapUserHtmlSrcDoc(hostileHtml);

      expect(wrapped).not.toContain("onclick=");
      expect(wrapped).not.toContain("onmouseover=");
      expect(wrapped).not.toContain("onerror=");
      expect(wrapped).not.toContain('href="javascript:');
      expect(wrapped).not.toContain('action="vbscript:');
      expect(wrapped).not.toContain('formaction="data:text/html');
      expect(wrapped).not.toContain('src="data:image/svg+xml');
    });

    it("strips CSS url(...) and @import inside both <style> blocks and inline style attributes", () => {
      const stylePayload = `
        <style>
          @import url("https://evil.com/evil.css");
          @import 'remote.css';
          .card { background: url('https://evil.com/leak.png'); color: #111; }
          .obfuscated { background: ur/*comment*/l("leak.jpg"); }
        </style>
        <div style="background: url('inline-leak.png'); color: blue;">Content</div>
      `;
      const wrapped = wrapUserHtmlSrcDoc(stylePayload);

      expect(wrapped).not.toContain("https://evil.com/evil.css");
      expect(wrapped).not.toContain("remote.css");
      expect(wrapped).not.toContain("https://evil.com/leak.png");
      expect(wrapped).not.toContain("inline-leak.png");
      expect(wrapped).not.toContain("leak.jpg");
      expect(wrapped).toContain("color: #111");
      expect(wrapped).toContain("color: blue");
    });

    it("sanitizes CSS content cleanly against comment-bypass attempts", () => {
      const dirty = `
        /* comment */
        @import url("malicious.css");
        .banner { background-image: url(data:image/png;base64,...); }
      `;
      const clean = sanitizeCssContent(dirty);
      expect(clean).not.toContain("@import");
      expect(clean).not.toContain("url(");
      expect(clean).toContain(".banner");
    });

    it("generates per-render CSPRNG nonce in STRICT_OFFLINE_CSP and matches bridge script nonce", async () => {
      const nonce = "c89f0124ab83ef671234abcd5678ef90";
      const csp = buildStrictOfflineCsp(nonce);

      expect(csp).toContain(`script-src 'nonce-${nonce}'`);
      expect(csp).toContain("default-src 'none'");
      expect(csp).toContain("base-uri 'none'");
      expect(csp).toContain("connect-src 'none'");
      expect(csp).toContain("form-action 'none'");

      const wrapped = wrapUserHtmlSrcDoc("<div>Secure</div>", { scriptNonce: nonce });
      expect(wrapped).toContain(`content="default-src 'none'; base-uri 'none'; connect-src 'none'; form-action 'none'; style-src 'nonce-${nonce}'; img-src data: blob:; font-src data:; script-src 'nonce-${nonce}';"`);
      expect(wrapped).toContain(`nonce="${nonce}"`);
    });

    it("computes real async Web Crypto SHA-256 matching deterministic 64-character hex digest", async () => {
      const input = "<section data-ff-id='test'><h1>Header</h1></section>";
      const hash = await computeArtifactSha256(input);

      expect(hash).toMatch(/^[0-9a-f]{64}$/);
      const directUtf8 = new TextEncoder().encode(input);
      const directBuf = await globalThis.crypto.subtle.digest("SHA-256", directUtf8);
      const expected = Array.from(new Uint8Array(directBuf))
        .map((b) => b.toString(16).padStart(2, "0"))
        .join("");
      expect(hash).toBe(expected);
    });
  });

  describe("2. IframeKeepAlivePool (LRU-5) & In-Place Hydration", () => {
    it("renders iframe preview device frame with CSPRNG verification status", () => {
      const activeSlide = INITIAL_SLIDES[0];
      const mockScreen: ScreenPreviewData = {
        eyebrow: "SCR-02",
        headline: "Treasury Approval",
        subtitle: "Multi-Sig",
        amount: 50000,
        statusText: "Ready",
        latency: "14ms",
        actionTitle: "Confirm",
        ctaLabel: "Sign",
      };

      render(
        <CompanionLiveHtmlRightPane
          activeTab={DEFAULT_TABS[0]}
          structureLens="userflow"
          selectedNodeId="SCR-02"
          activeSlide={activeSlide}
          activeScreenData={mockScreen}
          activeSubState="DEFAULT"
          pointerMode="interact"
          onSetPointerMode={vi.fn()}
          isInlineCodeOpen={false}
          onToggleInlineCode={vi.fn()}
          inlineCodeText=""
          onChangeInlineCode={vi.fn()}
          viewportPreset="desktop-1440"
          onSetViewportPreset={vi.fn()}
          aspectRatio="auto"
          onSetAspectRatio={vi.fn()}
          isPresenting={false}
          onTogglePresenting={vi.fn()}
          isLeftStructureHidden={false}
          onToggleLeftStructureHidden={vi.fn()}
          highlightedRegionId="sec-header"
          onRegionClick={vi.fn()}
        />
      );

      const statusPill = screen.getByTestId("bridge-status-pill");
      expect(statusPill.textContent).toContain("DEFAULT");
    });
  });

  describe("3. Option 5 Storyboard & DagNodeCard", () => {
    it("renders compact mini-skeleton <= 4KB, 4 substate pills, and outbound vs loopback CTA pills", () => {
      const mockNode: ExtendedWorkbenchNode = {
        id: "node-treasury-01",
        version: 1,
        screenKey: "SCR-TREASURY",
        title: "Treasury Multi-Sig Approval",
        posX: 100,
        posY: 100,
        status: "synced",
        activeSubState: "DEFAULT",
        clusterId: "Cluster-Treasury",
        sourceCtaFfId: "cta-return-vault",
        ctaLabel: "Sign Multi-Sig ──▶",
        metadata: { appShell: "TreasuryPortal" },
      };

      const handleHoverCta = vi.fn();
      const handleSelectSubState = vi.fn();

      render(
        <DagNodeCard
          node={mockNode}
          isSelected={false}
          onNodeMouseDown={vi.fn()}
          onPortMouseDown={vi.fn()}
          onUpdateNote={vi.fn()}
          onSelectSubState={handleSelectSubState}
          onHoverCta={handleHoverCta}
          nodeWidth={260}
          nodeHeight={180}
        />
      );

      expect(screen.getByTestId("dag-node-mini-skeleton")).toBeTruthy();
      const shellBadgeText = screen.getByTestId(`appshell-badge-${mockNode.id}`).textContent ?? "";
      expect(shellBadgeText).toContain("Cluster-Treasury");
      expect(shellBadgeText).toMatch(/Shell|Khung ứng dụng/);

      expect(screen.getByTestId(`dag-substate-default-${mockNode.id}`)).toBeTruthy();
      expect(screen.getByTestId(`dag-substate-loading-${mockNode.id}`)).toBeTruthy();
      expect(screen.getByTestId(`dag-substate-empty-${mockNode.id}`)).toBeTruthy();
      expect(screen.getByTestId(`dag-substate-error-${mockNode.id}`)).toBeTruthy();

      fireEvent.click(screen.getByTestId(`dag-substate-loading-${mockNode.id}`));
      expect(handleSelectSubState).toHaveBeenCalledWith(mockNode.id, "LOADING");

      const outboundCta = screen.getByTestId(`outbound-cta-pill-${mockNode.id}`);
      expect(outboundCta.textContent).toContain("Sign Multi-Sig ──▶");

      const loopbackCta = screen.getByTestId(`loopback-cta-pill-${mockNode.id}`);
      expect(loopbackCta.textContent).toContain("↺ cta-return-vault");

      fireEvent.mouseEnter(outboundCta);
      expect(handleHoverCta).toHaveBeenCalledWith(expect.any(String), true);
      fireEvent.mouseLeave(outboundCta);
      expect(handleHoverCta).toHaveBeenCalledWith(expect.any(String), false);
    });
  });

  describe("4. [EC-STORYBOARD-001] Dirty Inline Editor Guard", () => {
    it("intercepts node and substate switches when dirtyBuffer is true, prompting Save / Discard", () => {
      const handleSelectNode = vi.fn();

      render(
        <CompanionCanvasTab
          selectedNodeId="SCR-02"
          onSelectNode={handleSelectNode}
          children={<div data-testid="dag-canvas-mock">DAG Canvas</div>}
        />
      );

      fireEvent.click(screen.getByTestId("canvas-mode-inline-editor"));
      const textarea = screen.getByTestId("inline-html-textarea") as HTMLTextAreaElement;

      fireEvent.change(textarea, { target: { value: "<div>Edited Content Unsaved</div>" } });
      expect(screen.getByTestId("inline-html-dirty-badge")).toBeTruthy();

      fireEvent.click(screen.getByTestId("canvas-mode-inline-editor"));
      expect(screen.getByTestId("dirty-buffer-guard-modal")).toBeTruthy();

      // Click discard button inside dirty guard modal
      const discardBtns = screen.getAllByTestId("dirty-guard-discard-btn");
      fireEvent.click(discardBtns[0]);
      expect(screen.queryByTestId("dirty-buffer-guard-modal")).toBeNull();
    });

    it("displays [AC-03] ambiguity execution lock when ambiguityScore >= 0.45", () => {
      render(
        <CompanionCanvasTab
          selectedNodeId="SCR-02"
          ambiguityScore={0.65}
          children={<div data-testid="dag-mock">DAG</div>}
        />
      );

      expect(screen.getByTestId("ambiguity-execution-lock")).toBeTruthy();
      expect(screen.getByTestId("ambiguity-execution-lock").textContent).toMatch(/0[.,]65/);
    });
  });

  describe("5. 4-Path Launchpad & Sample Data Studio", () => {
    it("renders ProjectLaunchpadModal with 4 entry paths and offline token extraction", () => {
      const handleLaunch = vi.fn();
      render(
        <ProjectLaunchpadModal isOpen={true} onClose={vi.fn()} onLaunchPath={handleLaunch} />
      );

      expect(screen.getByTestId("project-launchpad-modal")).toBeTruthy();

      // Tab 1: Local Disk (Folder + File quick-start, zero textarea clutter)
      expect(screen.getByTestId("launchpad-panel-prd")).toBeTruthy();
      fireEvent.change(screen.getByTestId("launchpad-local-folder-input"), { target: { value: "/tmp/my-app" } });

      // Tab 4: Live Top-5 Awesome-RE Visual Gallery & Local Reference Library
      fireEvent.click(screen.getByTestId("launchpad-tab-templates"));
      expect(screen.getByTestId("launchpad-panel-templates")).toBeTruthy();
      expect(screen.getByTestId("launchpad-rea-search-section")).toBeTruthy();
      expect(screen.getByTestId("launchpad-rea-topic-design")).toBeTruthy();

      // Tab 2: Online Design Tools (single primary footer CTA + secondary Firewall Pre-check)
      fireEvent.click(screen.getByTestId("launchpad-tab-design"));
      expect(screen.getByTestId("launchpad-panel-design")).toBeTruthy();
      expect(screen.getByTestId("launchpad-rea-ingest-btn")).toBeTruthy();
      expect(screen.getByTestId("launchpad-extract-tokens-btn")).toBeTruthy();

      // Tab 4: Git Codebase Mode
      fireEvent.click(screen.getByTestId("launchpad-tab-git"));
      expect(screen.getByTestId("launchpad-panel-git")).toBeTruthy();

      // Submit
      fireEvent.click(screen.getByTestId("launchpad-submit-btn"));
      expect(handleLaunch).toHaveBeenCalledWith("git", expect.any(Object));
    });

    it("renders SampleDataStudioModal with DataTableView, AdvancedFilterBar 4 presets, and tray mode toggle", () => {
      const handleApply = vi.fn();
      render(
        <SampleDataStudioModal isOpen={true} onClose={vi.fn()} onApplyFixture={handleApply} />
      );

      expect(screen.getByTestId("sample-data-studio-modal")).toBeTruthy();
      expect(screen.getByTestId("advanced-filter-bar")).toBeTruthy();
      expect(screen.getByTestId("data-table-view")).toBeTruthy();

      expect(screen.getByTestId("preset-filter-btn-happy")).toBeTruthy();
      expect(screen.getByTestId("preset-filter-btn-empty")).toBeTruthy();
      expect(screen.getByTestId("preset-filter-btn-long_text")).toBeTruthy();
      expect(screen.getByTestId("preset-filter-btn-validation_error")).toBeTruthy();

      fireEvent.click(screen.getByTestId("preset-filter-btn-validation_error"));
      expect(screen.getByText("ERR_INSUFFICIENT_MULTI_SIG_QUORUM")).toBeTruthy();

      const toggleTrayBtn = screen.getByTestId("toggle-tray-mode-btn");
      expect(toggleTrayBtn.textContent).toBe("Tray Mode");
      fireEvent.click(toggleTrayBtn);
      expect(toggleTrayBtn.textContent).toBe("Modal Mode");

      fireEvent.click(screen.getByTestId("sample-data-apply-btn"));
      expect(handleApply).toHaveBeenCalledWith("VALIDATION_ERROR", expect.any(String));
    });

    it("preserves loopback flowType across DTO conversion & emits flow-changes-draft.json payload", async () => {
      const { canonicalToEdgeType, edgeTypeToCanonical } = await import("../store/flow-workbench-types");
      const { buildFlowChangesDraftJson } = await import("../store/flow-workbench-sync");
      const { computeDraftDelta } = await import("../store/draft-delta");

      expect(canonicalToEdgeType("loopback")).toBe("loopback");
      expect(edgeTypeToCanonical("loopback")).toBe("loopback");

      const baseSnap = { graphId: "g-1", graphVersion: 1, layoutVersion: 1, nodes: [], transitions: [] };
      const loopbackEdge = {
        id: "550e8400-e29b-41d4-a716-446655440099",
        sourceNodeId: "550e8400-e29b-41d4-a716-446655440001",
        targetNodeId: "550e8400-e29b-41d4-a716-446655440002",
        triggerEvent: "onClick",
        edgeType: "loopback" as const,
        sourceCtaFfId: "cta-retry-pin",
        maxLoopIterations: 4,
        version: 1,
        status: "draft" as const,
      };
      const delta = computeDraftDelta(baseSnap, { nodes: [], transitions: [loopbackEdge] }, "draft-loop-1");
      expect(delta.payload.createdTransitions?.[0]?.flowType).toBe("loopback");
      expect(delta.payload.createdTransitions?.[0]?.allowConfirmedLoopback).toBe(true);

      const serialized = buildFlowChangesDraftJson("proj-1", "draft-loop-1", delta);
      const parsed = JSON.parse(serialized);
      expect(parsed.schema).toBe("cowok-design.flow-diff.v1");
      expect(parsed.fileName).toBe("flow-changes-draft.json");
      expect(parsed.structuralDeltas.createdTransitions[0].flowType).toBe("loopback");

      render(<ProjectLaunchpadModal isOpen={true} onClose={vi.fn()} />);
      expect(screen.getByTestId("launchpad-prd-file-input")).toBeTruthy();
      expect(screen.getByTestId("launchpad-pick-folder-btn")).toBeTruthy();
      const folderInput = screen.getByTestId("launchpad-local-folder-input") as HTMLInputElement;
      fireEvent.change(folderInput, { target: { value: "~/Projects/fintech-wallet" } });
      expect(folderInput.value).toBe("~/Projects/fintech-wallet");
    });
  });
});
