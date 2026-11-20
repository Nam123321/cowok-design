// @vitest-environment jsdom
// @story 06.1
// @implements AC-01, AC-03, AC-04, AC-05, AC-06
// @cover UT-04, UT-05, UT-06, UT-07
import fs from "node:fs";
import path from "node:path";
import React from "react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act, cleanup, waitFor } from "@testing-library/react";
import {
  IframeToHostDeckMessageSchema,
  PortReadyBootstrapSchema,
  InitPortBootstrapSchema,
  SlideUidSchema,
} from "../../../lib/validations/presentation";
import { PresentationClient, type PresentationApiClientLike } from "../../../api/presentation";
import {
  shouldIgnorePresentationKeyEvent,
  useDeckPresentationController,
} from "../hooks/useDeckPresentationController";
import {
  MAX_PROTOCOL_RETRIES,
  PROTOCOL_RETRY_DELAY_MS,
  useSandboxedIframeBridge,
} from "../hooks/useSandboxedIframeBridge";
import {
  buildPreviewDocumentHtml,
  buildStrictOfflineCsp,
  IFRAME_SANDBOX_PERMISSIONS,
  wrapUserHtmlSrcDoc,
} from "../utils/html-sandbox-wrapper";
import { CompanionCanvasTab } from "../components/CompanionCanvasTab";

function DeckControllerHarness(props: {
  slidesCount?: number;
  initialPresenting?: boolean;
  onExitWalkthrough?: () => void;
}) {
  const [activeIndex, setActiveIndex] = React.useState(0);
  const [isPresenting, setIsPresenting] = React.useState(Boolean(props.initialPresenting));
  const ctrl = useDeckPresentationController({
    slidesCount: props.slidesCount ?? 3,
    activeSlideIndex: activeIndex,
    onChangeSlide: setActiveIndex,
    onExitWalkthrough: props.onExitWalkthrough,
    isPresenting,
    setIsPresenting,
  });

  return (
    <div data-testid="harness-root" data-presenting={ctrl.isDeckPresenting ? "true" : "false"} data-slide-idx={activeIndex}>
      <button type="button" data-testid="btn-present" onClick={ctrl.enterPresent}>
        Present
      </button>
      <input data-testid="ime-input" type="text" />
    </div>
  );
}

function BridgeTestHarness(props: { rawHtml: string }) {
  const bridge = useSandboxedIframeBridge({
    projectId: "proj-ut04",
    activeTabId: "tab-ut04",
    structureLens: "slide-cms",
    activeSlideId: "sld-1",
    activeSubState: "DEFAULT",
    rawHtml: props.rawHtml,
    onRegionClick: vi.fn(),
  });
  return (
    <div
      data-testid="bridge-harness"
      data-protocol-status={bridge.protocolStatus}
      data-protocol-error={bridge.protocolError || ""}
      data-retry-count={bridge.retryCount}
      data-epoch={bridge.epoch}
      data-verified={bridge.isPortVerified ? "true" : "false"}
    >
      <iframe
        ref={bridge.iframeRef}
        title="test"
        sandbox={IFRAME_SANDBOX_PERMISSIONS}
        srcDoc={bridge.wrappedSrcDoc}
        onLoad={bridge.handleIframeLoad}
      />
    </div>
  );
}

describe("Story 06.1 Presentation Stage & HostRuntimeContract", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("UT-04: rejects malformed/stale epoch or seq and retries 500ms up to 3x -> PROTOCOL_FAILED", () => {
    expect(IFRAME_SANDBOX_PERMISSIONS).toBe("allow-scripts allow-forms");
    expect(IFRAME_SANDBOX_PERMISSIONS).not.toContain("allow-same-origin");

    const parsedInit = IframeToHostDeckMessageSchema.safeParse({
      type: "DECK_PROTOCOL_INIT",
      epoch: 1,
      seq: 1,
      bootSecretAck: "test_placeholder",
      channelSecret: "test_placeholder",
      protocolVersion: "1",
      slideCount: 4,
      activeSlideIndex: 0,
      uniqueFfIdsCount: 3,
    });
    expect(parsedInit.success).toBe(true);
    expect(
      PortReadyBootstrapSchema.safeParse({
        type: "PORT_READY",
        epoch: 2,
        bootSecret: "secret-at-least-16chars",
      }).success
    ).toBe(true);
    expect(
      InitPortBootstrapSchema.safeParse({
        type: "INIT_PORT",
        epoch: 2,
        channelSecret: "secret-at-least-16chars",
      }).success
    ).toBe(true);
    expect(SlideUidSchema.safeParse("SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV").success).toBe(true);
    expect(SlideUidSchema.safeParse("SLD-too-short").success).toBe(false);

    const html = `<section class="slide active" data-ff-id="s1-h1"><h1>Slide</h1></section>`;
    render(<BridgeTestHarness rawHtml={html} />);

    const iframe = document.querySelector("iframe") as HTMLIFrameElement;
    expect(iframe).toBeTruthy();

    // Trigger load 4 times without sending PORT_ACK to drive retries 1 -> 2 -> 3 -> PROTOCOL_FAILED
    for (let i = 0; i <= MAX_PROTOCOL_RETRIES; i++) {
      fireEvent.load(iframe);
      act(() => {
        vi.advanceTimersByTime(PROTOCOL_RETRY_DELAY_MS + 10);
      });
    }

    const root = screen.getByTestId("bridge-harness");
    expect(root.getAttribute("data-protocol-status")).toBe("PROTOCOL_FAILED");
    expect(root.getAttribute("data-protocol-error")).toBe("E_DECK_PROTOCOL_INVALID");
    expect(Number(root.getAttribute("data-retry-count"))).toBe(MAX_PROTOCOL_RETRIES);
  });

  it("UT-05: mutual exclusivity between Walkthrough & Present, single-Esc exit, and IME keyCode=229 suppression", () => {
    const exitWalkthrough = vi.fn();
    render(<DeckControllerHarness slidesCount={3} onExitWalkthrough={exitWalkthrough} />);

    const root = screen.getByTestId("harness-root");
    fireEvent.click(screen.getByTestId("btn-present"));
    expect(exitWalkthrough).toHaveBeenCalledTimes(1);
    expect(root.getAttribute("data-presenting")).toBe("true");

    // IME composing / keyCode 229 must be suppressed
    expect(shouldIgnorePresentationKeyEvent({ isComposing: true, keyCode: 39 })).toBe(true);
    expect(shouldIgnorePresentationKeyEvent({ isComposing: false, keyCode: 229 })).toBe(true);

    fireEvent.keyDown(window, { key: "ArrowRight", keyCode: 229 });
    expect(root.getAttribute("data-slide-idx")).toBe("0");

    // Real ArrowRight advances slide
    fireEvent.keyDown(window, { key: "ArrowRight", keyCode: 39 });
    expect(root.getAttribute("data-slide-idx")).toBe("1");

    // Input focus ignores arrow navigation
    const inputEl = screen.getByTestId("ime-input");
    fireEvent.keyDown(inputEl, { key: "ArrowRight", keyCode: 39 });
    expect(root.getAttribute("data-slide-idx")).toBe("1");

    // Single Esc exits cleanly
    fireEvent.keyDown(window, { key: "Escape", keyCode: 27 });
    expect(root.getAttribute("data-presenting")).toBe("false");
  });

  it("UT-05b: CompanionCanvasTab preserves active node across lens switches and separates Walkthrough vs Present", () => {
    render(
      <CompanionCanvasTab
        selectedNodeId="node-checkout"
        nodes={[
          {
            id: "node-checkout",
            screenKey: "SCR-02",
            title: "Checkout",
            posX: 100,
            posY: 100,
            version: 1,
            status: "synced",
          },
        ]}
      >
        <div data-testid="dag-canvas-mock" />
      </CompanionCanvasTab>
    );

    const slideLensBtn = screen.getByTestId("lens-slide-cms-btn");
    fireEvent.click(slideLensBtn);

    expect(document.getElementById("structureSubPane")).toBeTruthy();
    expect(document.getElementById("htmlSubPane")).toBeTruthy();
    expect(screen.getByTestId("lens-slide-cms")).toBeTruthy();
    // Inside an App HTML tab (checkout-app.html), toggling Left Lens to Slide/CMS must NOT jump Right Stage to Presentation Deck
    expect(screen.getByTestId("viewport-btn-desktop-1440")).toBeTruthy();

    // Switching to the Presentation Deck tab activates 16:9 Presentation Stage & Present Walkthrough
    const deckTab = screen.getByRole("tab", { name: /q4-architecture-deck\.html/i });
    fireEvent.click(deckTab);

    const unifiedWalkthroughBtn = screen.getByTestId("walkthrough-playback-btn");
    expect(unifiedWalkthroughBtn).toBeTruthy();
    expect(unifiedWalkthroughBtn.getAttribute("data-deck-present-trigger")).toBe("true");
    // Ensure duplicate Present button is unified into Walkthrough (zero side-by-side duplicate button)
    expect(screen.queryByTestId("btn-present-deck")).toBeNull();
    expect(screen.getByTestId("aspect-ratio-pill-group")).toBeTruthy();
    expect(screen.getByTestId("zoom-pill-scale")).toBeTruthy();
    expect(screen.getByTestId("zoom-pill-group")).toBeTruthy();
    const frame = document.querySelector(".live-html-frame") as HTMLElement | null;
    expect(frame?.getAttribute("data-slide-stage")).toBe("true");
    expect(frame?.getAttribute("data-aspect")).toBe("16-9");
    expect(frame?.style.getPropertyValue("--slide-w")).toBe("1920px");
    expect(frame?.style.getPropertyValue("--slide-h")).toBe("1080px");

    // Clicking unified Walkthrough in Slide Studio enters Present fullscreen + renders WalkthroughFloatingBar WITHOUT Dual Screen toggle
    fireEvent.click(unifiedWalkthroughBtn);
    expect(document.querySelector(".companion-canvas-tab-container")?.getAttribute("data-deck-presenting")).toBe("true");
    expect(screen.getByTestId("walkthrough-floating-bar")).toBeTruthy();
    expect(screen.queryByTestId("walkthrough-mode-dual")).toBeNull();
    expect(screen.queryByTestId("walkthrough-mode-full")).toBeNull();
    // FE-DAG viewport presets (Desktop/Tablet/Mobile) stay hidden in Presentation capability mode
    expect(screen.queryByTestId("viewport-btn-desktop-1440")).toBeNull();

    // Advance via WalkthroughFloatingBar Next button AND ArrowRight, then exit via Escape
    fireEvent.click(screen.getByTestId("walkthrough-next-btn"));
    const stageIframe = screen.getByTestId("sandboxed-html-iframe");
    expect(stageIframe.getAttribute("srcdoc") || "").toContain("SLIDE 02 • ARCHITECTURE");
    fireEvent.keyDown(window, { key: "Escape", keyCode: 27 });
    expect(document.querySelector(".companion-canvas-tab-container")?.getAttribute("data-deck-presenting")).toBe("false");
    expect(screen.queryByTestId("walkthrough-floating-bar")).toBeNull();
  });

  it("UT-04C: narrow utility toolbar collapses every multi-option pill group into dropdown selectors", async () => {
    const callbacks: ResizeObserverCallback[] = [];
    const OriginalRO = globalThis.ResizeObserver;
    class MockResizeObserver {
      constructor(cb: ResizeObserverCallback) { callbacks.push(cb); }
      observe() {}
      disconnect() {}
    }
    vi.stubGlobal("ResizeObserver", MockResizeObserver as unknown as typeof ResizeObserver);
    const { HtmlPreviewToolbar } = await import("../components/HtmlPreviewToolbar");
    const onSelectSubState = vi.fn();
    const onSetViewportPreset = vi.fn();
    const onSetAspectRatio = vi.fn();
    const onSetZoomMode = vi.fn();

    render(
      <HtmlPreviewToolbar
        structureLens="slide-cms"
        pointerMode="interact"
        onSetPointerMode={() => {}}
        activeSubState="DEFAULT"
        onSelectSubState={onSelectSubState}
        isInlineCodeOpen={false}
        onToggleInlineCode={() => {}}
        viewportPreset="desktop-1440"
        onSetViewportPreset={onSetViewportPreset}
        aspectRatio="16-9"
        onSetAspectRatio={onSetAspectRatio}
        zoomMode="fit"
        onSetZoomMode={onSetZoomMode}
        isPresenting={false}
        onTogglePresenting={() => {}}
        isLeftStructureHidden={false}
        onToggleLeftStructureHidden={() => {}}
        onToggleWalkthrough={() => {}}
      />
    );

    const toolbarEl = document.querySelector(".html-pane-toolbar") as HTMLElement;
    Object.defineProperty(toolbarEl, "clientWidth", { configurable: true, get: () => 480 });
    act(() => { callbacks.forEach((cb) => cb([], {} as ResizeObserver)); });

    expect(screen.getByTestId("fixture-substate-dropdown")).toBeTruthy();
    expect(screen.queryByTestId("viewport-preset-dropdown")).toBeNull();
    expect(screen.getByTestId("aspect-ratio-dropdown")).toBeTruthy();
    expect(screen.getByTestId("zoom-mode-dropdown")).toBeTruthy();

    fireEvent.click(screen.getByTestId("aspect-ratio-dropdown"));
    fireEvent.click(screen.getByTestId("aspect-pill-16-10"));
    expect(onSetAspectRatio).toHaveBeenCalledWith("16-10");

    fireEvent.click(screen.getByTestId("zoom-mode-dropdown"));
    fireEvent.click(screen.getByTestId("zoom-pill-pan-100"));
    expect(onSetZoomMode).toHaveBeenCalledWith("pan-100");

    vi.stubGlobal("ResizeObserver", OriginalRO);
  });

  it("UT-04D: NewConversationModal exposes FE-DAG vs Presentation 16:9 intent selector for session & project creation", async () => {
    const { NewConversationModal } = await import("../../../pages/workbench/components/NewConversationModal");
    const onCreateInProject = vi.fn();
    const onCreateNewProject = vi.fn();
    const projects = [{ id: "proj-1", name: "Checkout OS", slug: "checkout-os", provenance: "scratch" as const, isExpanded: true }];

    render(
      <NewConversationModal
        isOpen
        onClose={() => {}}
        projects={projects}
        activeProjectId="proj-1"
        onCreateInProject={onCreateInProject}
        onCreateNewProject={onCreateNewProject}
        onCreateStandalone={() => {}}
      />
    );

    fireEvent.click(screen.getByTestId("new-convo-entry-presentation"));
    fireEvent.click(document.querySelector('button[type="submit"]') as HTMLButtonElement);
    expect(onCreateInProject).toHaveBeenCalledWith("proj-1", expect.any(String), "presentation");
  });

  it("UT-04E: generateUniqueSessionTitle generates 4-5 digit project alias sequence and extractFolderBasename resolves local & GitHub names", async () => {
    const { extractFolderBasename, generateUniqueSessionTitle } = await import("../../../pages/workbench/utils/session-defaults");
    expect(extractFolderBasename("/Users/demo/workspace/wallet-v3/")).toBe("wallet-v3");
    expect(extractFolderBasename("https://github.com/acme/pitch-deck.git")).toBe("pitch-deck");

    expect(
      generateUniqueSessionTitle({
        projectName: "wallet-v3",
        existingTitles: ["wallet-v3 #0001", "WALLET-V3 #0002"],
        existingCount: 2,
      })
    ).toBe("wallet-v3 #0003");
    expect(
      generateUniqueSessionTitle({
        projectName: "pitch-deck",
        existingTitles: [],
        existingCount: 0,
      })
    ).toBe("pitch-deck #0001");
    expect(
      generateUniqueSessionTitle({
        projectName: "scale",
        existingTitles: [],
        existingCount: 9999,
      })
    ).toBe("scale #10000");
  });

  it("UT-04F: NewConversationModal 3-tab + progressive source/alias + post-create Load Material Skip + Sidebar quick + popup", async () => {
    const { NewConversationModal } = await import("../../../pages/workbench/components/NewConversationModal");
    const { ProjectLaunchpadModal } = await import("../components/ProjectLaunchpadModal");
    const { SidebarProjectGroup } = await import("../../../pages/workbench/components/SidebarProjectGroup");

    const onCreateNewProject = vi.fn();
    const onCreateStandalone = vi.fn();
    const projects = [{ id: "proj-1", name: "Cowok.ai", slug: "cowok-ai", provenance: "scratch" as const, isExpanded: true }];
    render(
      <NewConversationModal
        isOpen
        onClose={() => {}}
        projects={projects}
        activeProjectId="proj-1"
        existingSessions={[{ id: "s1", title: "Cowok.ai #0001", projectId: "proj-1", updatedAt: "Just now" }]}
        onCreateInProject={() => {}}
        onCreateNewProject={onCreateNewProject}
        onCreateStandalone={onCreateStandalone}
      />
    );

    expect(screen.getByTestId("tab-standalone-btn")).toBeTruthy();
    const projectDropdown = screen.getByTestId("project-selector-dropdown");
    expect(projectDropdown.textContent?.trim()).toBe("Cowok.ai");
    fireEvent.click(projectDropdown);
    expect(screen.getByTestId("project-selector-dropdown-option-proj-1").textContent?.trim()).toBe("Cowok.ai");
    fireEvent.click(projectDropdown);
    expect(screen.getByTestId("new-convo-session-alias-display").textContent).toBe("Cowok.ai #0002");

    fireEvent.click(document.getElementById("tab-new-project-btn")!);
    expect(screen.getByTestId("source-local-folder-card")).toBeTruthy();
    expect(screen.getByTestId("github-status-badge")).toBeTruthy();
    expect(screen.queryByTestId("new-project-alias-display")).toBeNull();

    fireEvent.click(screen.getByTestId("source-github-card"));
    fireEvent.change(screen.getByTestId("github-repo-url-input"), { target: { value: "https://github.com/acme/fintech-deck.git" } });
    expect(screen.getByTestId("new-project-alias-display").textContent).toBe("fintech-deck");
    expect(screen.getByTestId("new-convo-session-alias-display").textContent).toBe("fintech-deck #0001");

    fireEvent.click(screen.getByTestId("rename-project-alias-btn"));
    fireEvent.change(screen.getByTestId("new-project-name-input"), { target: { value: "FinTech Alias" } });
    fireEvent.click(screen.getByTestId("new-convo-entry-presentation"));
    fireEvent.click(document.querySelector('button[type="submit"]') as HTMLButtonElement);
    expect(onCreateNewProject).toHaveBeenCalledWith(
      "FinTech Alias",
      "FinTech Alias #0001",
      "folder-backed",
      { managedSlug: "fintech-alias" },
      "presentation",
      { openMaterialLaunchpad: true, githubRepoUrl: "https://github.com/acme/fintech-deck.git" }
    );
    cleanup();

    const onCloseLaunchpad = vi.fn();
    render(
      <ProjectLaunchpadModal
        isOpen
        onClose={onCloseLaunchpad}
        materialContext={{ projectId: "proj-new", projectName: "FinTech Alias", entryKind: "presentation" }}
      />
    );
    fireEvent.click(screen.getByTestId("launchpad-skip-btn"));
    expect(onCloseLaunchpad).toHaveBeenCalledTimes(1);
    cleanup();

    const onQuickCreate = vi.fn();
    render(
      <SidebarProjectGroup
        projectId="proj-1"
        projectName="Cowok.ai"
        projects={projects}
        sessions={[{ id: "s1", title: "Cowok.ai #0001", projectId: "proj-1", updatedAt: "Now" }]}
        activeSessionId="s1"
        onSelectSession={() => {}}
        onOpenNewConversationModal={() => {}}
        onCreateSessionInProject={onQuickCreate}
      />
    );
    fireEvent.click(screen.getByTestId("project-quick-add-btn-proj-1"));
    expect(screen.getByTestId("project-quick-session-popup")).toBeTruthy();
    fireEvent.click(screen.getByTestId("quick-popup-entry-presentation"));
    expect(onQuickCreate).toHaveBeenCalledWith("proj-1", "Cowok.ai #0002", "presentation");
  });

  it("UT-04G: CowokToast renders unified cowok.ai notification badge/action/dismiss & AgentChatComposer attaches local files via +", async () => {
    vi.useRealTimers();
    const { CowokToast } = await import("../../../components/ui/CowokToast");
    const { AgentChatComposer } = await import("../../../components/workbench/ChatPrimitivesBridge");

    const onUndo = vi.fn();
    const onDismiss = vi.fn();
    render(
      <CowokToast
        tone="success"
        message="Đã tạo dự án mới trong SQLite V7: Pitch Deck Demo"
        actionLabel="Hoàn tác"
        actionTestId="toast-undo-restore-btn"
        onAction={onUndo}
        onDismiss={onDismiss}
        testId="workbench-status-toast"
      />
    );
    const toastEl = screen.getByTestId("workbench-status-toast");
    expect(toastEl.className).toContain("cowok-toast--success");
    expect(toastEl.querySelector(".cowok-toast__badge")).toBeTruthy();
    fireEvent.click(screen.getByTestId("toast-undo-restore-btn"));
    expect(onUndo).toHaveBeenCalledTimes(1);
    cleanup();

    const onAttachFiles = vi.fn();
    render(
      <AgentChatComposer
        inputValue=""
        onInputChange={() => {}}
        onSend={() => {}}
        attachmentChips={["@[File: landing.html]"]}
        onAttachFiles={onAttachFiles}
      />
    );
    expect(document.getElementById("chatAttachmentTray")?.textContent).toContain("@[File: landing.html]");
    const fileInput = screen.getByTestId("chat-composer-file-input") as HTMLInputElement;
    const sampleFile = new File(["<h1>Hi</h1>"], "brief.md", { type: "text/markdown" });
    fireEvent.change(fileInput, { target: { files: [sampleFile] } });
    await waitFor(() => {
      expect(onAttachFiles).toHaveBeenCalledWith(["brief.md"], { "brief.md": "<h1>Hi</h1>" });
    });
  });

  it("UT-06: strict offline CSP nonce & pre-mount sanitizer strip adversarial scripts/event handlers/meta/base/javascript URIs", () => {
    const nonce = "nonce-0123456789abcdef";
    const csp = buildStrictOfflineCsp(nonce);
    expect(csp).toContain(`script-src 'nonce-${nonce}'`);
    expect(csp).toContain("connect-src 'none'");
    expect(csp).toContain("default-src 'none'");

    const dirtyDoc = `<!DOCTYPE html><html><head><meta http-equiv="refresh" content="0;url=https://evil.test"><base href="https://evil.test/"><script>window.stolen = true;</script></head><body><section class="slide active" data-ff-id="s1" onclick="alert(1)"><a href="javascript:alert(2)">Bad</a><iframe srcdoc="<script>alert(3)</script>"></iframe></section></body></html>`;
    const wrapped = wrapUserHtmlSrcDoc(dirtyDoc, {
      epoch: 7,
      bootSecret: "boot-secret-abcdef1234",
      scriptNonce: nonce,
    });

    // Only the engine-injected bridge script with matching nonce survives
    const scriptTags = wrapped.match(/<script\b/gi) || [];
    expect(scriptTags.length).toBe(1);
    expect(wrapped).toContain(`nonce="${nonce}"`);
    const bodyHtml = wrapped.slice(wrapped.indexOf("<body>"));
    expect(bodyHtml).not.toContain("onclick=");
    expect(bodyHtml).not.toContain("javascript:");
    expect(bodyHtml).not.toContain("<iframe");
    expect(wrapped).not.toContain("https://evil.test");
    expect(wrapped).not.toContain("window.stolen");

    const slideHtml = buildPreviewDocumentHtml({
      structureLens: "slide-cms",
      activeSlide: {
        id: "s-1",
        code: "SLIDE 1",
        title: "Cover",
        subtitle: "Sub",
        headline: "Headline",
        kpi1: 100,
        kpi2: "OK",
        kpi3: "10ms",
      },
      activeScreenData: {
        eyebrow: "DECK",
        headline: "H",
        subtitle: "S",
        amount: 100,
        statusText: "OK",
        latency: "10ms",
        actionTitle: "Action",
        ctaLabel: "Go",
      },
      formattedAmount: "$100",
    });
    expect(slideHtml).toContain('data-cowok-deck-protocol="1"');
    expect(slideHtml).toContain('class="card slide active"');
  });

  it("UT-07: apiPresentation contains 0 raw fetch() calls and routes all operations + timeoutMs through apiClient", async () => {
    const srcText = fs.readFileSync(path.resolve("src/api/presentation.ts"), "utf8");
    expect(srcText).not.toMatch(/\bfetch\s*\(/);

    const requestMock = vi.fn(async (url: string) => {
      if (url.endsWith("/duplicate")) {
        return {
          deckRevision: 2,
          slide: {
            id: "s-2",
            deckId: "d-1",
            slideUid: "SLD-01ARZ3NDEKTSV4RRFFQ69G5FB2",
            slideIndex: 1,
            layoutKind: "hero_cover",
            htmlPath: "presentations/d-1/slides/SLD-01ARZ3NDEKTSV4RRFFQ69G5FB2.html",
            contentSha256: "a".repeat(64),
            speakerNotes: "",
            narrativeArc: "",
            eyebrowTag: "",
            revision: 1,
            updatedAt: 1000,
          },
        };
      }
      if (url.endsWith("/decks")) {
        return { decks: [] };
      }
      return {
        id: "d-1",
        projectId: "p-1",
        title: "Deck",
        themePreset: "night",
        aspectRatio: "16:9",
        deckProtocolVersion: 1,
        deckMode: "multi-page",
        slidesCount: 1,
        revision: 1,
        lastExportedAt: null,
        updatedAt: 1000,
        slides: [],
      };
    });

    const mockClient: PresentationApiClientLike = {
      request: requestMock as unknown as PresentationApiClientLike["request"],
    };
    const sdk = new PresentationClient(mockClient);

    await sdk.listDecks("proj-1", { timeoutMs: 2500 });
    expect(requestMock).toHaveBeenLastCalledWith(
      "/api/projects/proj-1/decks",
      expect.anything(),
      expect.objectContaining({ method: "GET", timeoutMs: 2500 })
    );

    requestMock.mockImplementationOnce(async () => ({
      id: "d-1",
      projectId: "proj-1",
      title: "Deck",
      themePreset: "night",
      aspectRatio: "16:9",
      deckProtocolVersion: 1,
      deckMode: "multi-page",
      slidesCount: 1,
      revision: 1,
      lastExportedAt: null,
      updatedAt: 1000,
      slides: [],
    }));
    await sdk.createDeck(
      "proj-1",
      {
        title: "Deck",
        themePreset: "night",
        aspectRatio: "16:9",
        deckMode: "multi-page",
        initialSlides: [{ layoutKind: "hero_cover", headline: "Cover", eyebrowTag: "", narrativeArc: "" }],
      },
      { timeoutMs: 3500, idempotencyKey: "idem-ut07" }
    );
    expect(requestMock).toHaveBeenLastCalledWith(
      "/api/projects/proj-1/decks",
      expect.anything(),
      expect.objectContaining({
        method: "POST",
        timeoutMs: 3500,
        headers: { "Idempotency-Key": "idem-ut07" },
      })
    );

    await sdk.reorderSlides(
      "proj-1",
      "d-1",
      { expectedRevision: 1, orderedSlideUids: ["SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV"] },
      { timeoutMs: 1800 }
    );
    expect(requestMock).toHaveBeenLastCalledWith(
      "/api/projects/proj-1/decks/d-1/slides/reorder",
      expect.anything(),
      expect.objectContaining({ method: "PUT", timeoutMs: 1800 })
    );

    await sdk.duplicateSlide(
      { projectId: "proj-1", deckId: "d-1", slideUid: "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV" },
      { expectedDeckRevision: 2 },
      { timeoutMs: 4200, idempotencyKey: "idem-dup-ut07" }
    );
    expect(requestMock).toHaveBeenLastCalledWith(
      "/api/projects/proj-1/decks/d-1/slides/SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV/duplicate",
      expect.anything(),
      expect.objectContaining({
        method: "POST",
        timeoutMs: 4200,
        headers: { "Idempotency-Key": "idem-dup-ut07" },
      })
    );
  });

  it("UT-04B: enforces single INIT_PORT per epoch, rejects spoofed PORT_READY, completes PORT_READY -> INIT_PORT -> PORT_ACK, and ignores post-bind stale seq", () => {
    const closeSpy = vi.fn();
    const portPostSpy = vi.fn();
    const handlers: Array<(ev: MessageEvent) => void> = [];
    const OrigChannel = globalThis.MessageChannel;
    class MockChannel {
      port1 = {
        close: closeSpy,
        set onmessage(fn: ((ev: MessageEvent) => void) | null) {
          if (fn) handlers[0] = fn;
        },
        postMessage: portPostSpy,
      };
      port2 = {} as MessagePort;
    }
    vi.stubGlobal("MessageChannel", MockChannel);

    try {
      const html = `<section class="slide active" data-ff-id="s1-h1"><h1>Slide</h1></section>`;
      render(<BridgeTestHarness rawHtml={html} />);
      const iframe = document.querySelector("iframe") as HTMLIFrameElement;
      const postMsgSpy = vi.fn();
      const fakeContentWindow = { postMessage: postMsgSpy } as unknown as Window;
      Object.defineProperty(iframe, "contentWindow", { value: fakeContentWindow, configurable: true });

      // 1. Spoofed PORT_READY (wrong origin or wrong secret) must NOT dispatch INIT_PORT
      act(() => {
        window.dispatchEvent(new MessageEvent("message", {
          data: { type: "PORT_READY", epoch: 1, bootSecret: "bad_boot_bad_boot" },
          origin: "https://evil.example",
        }));
      });
      expect(postMsgSpy).not.toHaveBeenCalled();

      // 2. fireEvent.load sends INIT_PORT once; subsequent duplicate PORT_READY for same epoch must NOT clobber port1
      fireEvent.load(iframe);
      expect(postMsgSpy).toHaveBeenCalledTimes(1);
      const initMsg = postMsgSpy.mock.calls[0][0] as { epoch: number; bootSecret: string; channelSecret: string; artifactSha256: string };
      act(() => {
        window.dispatchEvent(new MessageEvent("message", {
          data: { type: "PORT_READY", epoch: initMsg.epoch, bootSecret: initMsg.bootSecret },
          origin: "null",
        }));
      });
      expect(postMsgSpy).toHaveBeenCalledTimes(1);
      expect(closeSpy).not.toHaveBeenCalled();

      // 3. Complete PORT_ACK -> PORT_BOUND
      act(() => {
        handlers[0]?.({
          data: {
            type: "PORT_ACK",
            epoch: initMsg.epoch,
            bootSecretAck: initMsg.bootSecret,
            channelSecret: initMsg.channelSecret,
            artifactSha256: initMsg.artifactSha256,
            domAnchorHash: "0123456789abcdef",
            seq: 5,
          },
        } as MessageEvent);
      });
      expect(screen.getByTestId("bridge-harness").getAttribute("data-protocol-status")).toBe("PORT_BOUND");

      // 4. Stale seq (<= 5) or mismatched epoch must be ignored / close port
      act(() => {
        handlers[0]?.({
          data: {
            type: "SLIDE_OVERFLOW_WARN",
            epoch: initMsg.epoch,
            seq: 4,
            bootSecretAck: initMsg.bootSecret,
            channelSecret: initMsg.channelSecret,
            slideIndex: 0,
            scrollHeight: 1400,
            clientHeight: 1080,
          },
        } as MessageEvent);
      });
      expect(closeSpy).not.toHaveBeenCalled();
    } finally {
      vi.stubGlobal("MessageChannel", OrigChannel);
    }
  });

  it("UT-09: prepareHtmlForApplyBridge strips inline style attrs and stamps script nonce onto <style> before APPLY_HTML", async () => {
    const { prepareHtmlForApplyBridge } = await import("../utils/html-sandbox-wrapper");
    const raw = `<!DOCTYPE html><html><head><style>.card{color:red}</style></head><body><div style="opacity:0.55" data-ff-id="k1">Test</div></body></html>`;
    const prepared = prepareHtmlForApplyBridge(raw, "nonce-test-xyz");
    expect(prepared).not.toContain('style="opacity:0.55"');
    expect(prepared).toContain('nonce="nonce-test-xyz"');
    expect(prepared).toContain("data-cowok-s=");
  });
});
