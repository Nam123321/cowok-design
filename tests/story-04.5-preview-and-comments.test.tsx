// @vitest-environment jsdom
// @story 04.5
// @implements AC-01, AC-02, AC-03, AC-04
import React from "react";
import fs from "node:fs";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { CompanionCanvasTab } from "../src/features/flow/components/CompanionCanvasTab";
import { CompanionLiveHtmlRightPane } from "../src/features/flow/components/CompanionLiveHtmlRightPane";
import { ElementCommentThread } from "../src/features/comments/components/ElementCommentThread";
import { DEFAULT_TABS, INITIAL_SLIDES, SCREEN_PREVIEW_MAP } from "../src/features/flow/components/companion-canvas-data";
import { FIXTURE_VARIANTS } from "../src/lib/validations/screen-fixture";
import { CreateElementCommentSchema, StaleMappingErrorPayloadSchema, UpdateElementCommentSchema } from "../src/lib/validations/element-comment";
import { wrapUserHtmlSrcDoc, STRICT_OFFLINE_CSP, generateBootSecret, computeArtifactSha256Sync } from "../src/features/flow/utils/html-sandbox-wrapper";

describe("Story 04.5: State-Hydrated Live Preview, Option CSP-B CSPRNG Bridge & 3-Way Spatial Comments", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("AC-01: renders Interact vs Inspect/Comment (C), toggles via C hotkey, switches DEFAULT|LOADING|EMPTY|ERROR, and enforces dirty_buffer lock", () => {
    const { getByTestId, queryByTestId } = render(
      React.createElement(CompanionCanvasTab, {
        selectedNodeId: "node-scr-02",
        children: React.createElement("div", { "data-testid": "dag-mock" }, "DAG"),
      })
    );

    const interactBtn = getByTestId("pointer-mode-interact");
    const inspectBtn = getByTestId("pointer-mode-inspect");
    expect(interactBtn.getAttribute("aria-pressed")).toBe("true");
    expect(inspectBtn.getAttribute("aria-pressed")).toBe("false");

    // Hotkey 'c' toggles Inspect / Comment (C)
    fireEvent.keyDown(window, { key: "c" });
    expect(inspectBtn.getAttribute("aria-pressed")).toBe("true");
    fireEvent.keyDown(window, { key: "C" });
    expect(interactBtn.getAttribute("aria-pressed")).toBe("true");

    // All 4 uppercase SubState pills present and interactive
    for (const v of ["DEFAULT", "LOADING", "EMPTY", "ERROR"]) {
      const btn = getByTestId(`fixture-variant-${v}`);
      expect(btn.textContent).toBe(v);
      fireEvent.click(btn);
      expect(btn.getAttribute("aria-pressed")).toBe("true");
    }

    // Inline editor dirty_buffer lock guard
    fireEvent.click(getByTestId("toggle-inline-code-btn"));
    const textarea = getByTestId("inline-html-textarea") as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: "<main><h1>Modified HTML</h1></main>" } });
    expect(getByTestId("inline-html-dirty-badge")).toBeTruthy();

    fireEvent.click(getByTestId("toggle-inline-code-btn"));
    expect(getByTestId("dirty-buffer-guard-banner")).toBeTruthy();
    expect(getByTestId("inline-code-editor-panel")).toBeTruthy();

    fireEvent.click(getByTestId("dirty-guard-discard-btn"));
    expect(queryByTestId("inline-code-editor-panel")).toBeNull();

    // Editing and clicking Apply closes inline editor immediately without getting trapped on dirty guard
    fireEvent.click(getByTestId("toggle-inline-code-btn"));
    fireEvent.change(getByTestId("inline-html-textarea"), { target: { value: "<main><h1>Applied HTML</h1></main>" } });
    fireEvent.click(getByTestId("apply-inline-html-btn"));
    expect(queryByTestId("inline-code-editor-panel")).toBeNull();
    expect(queryByTestId("dirty-buffer-guard-banner")).toBeNull();
  });

  it("AC-02: DOMParser-only default-src 'none' immune to fake <head> comments & rejects spoofed bootSecret", () => {
    expect(STRICT_OFFLINE_CSP).toContain("default-src 'none'");
    expect(STRICT_OFFLINE_CSP).toContain("connect-src 'none'");
    const secret = generateBootSecret();
    expect(secret).toMatch(/^[0-9a-f]{32}$/);

    const trap = `<!-- <head><script>evil()</script></head> --><html><head><style>.ok{color:#111}</style></head><body onload="alert(1)"><div>Safe</div></body></html>`;
    const wrapped = wrapUserHtmlSrcDoc(trap, { bootSecret: secret, epoch: 77 });
    expect(wrapped).toContain(`var BOOT_SECRET = "${secret}"`);
    expect(wrapped).toContain("var EPOCH = 77;");
    expect(wrapped).toContain("event.stopImmediatePropagation()");
    expect(wrapped).not.toContain("onload");

    // Host rejects PORT_ACK with wrong bootSecretAck and closes port1
    const closeSpy = vi.fn();
    const handlerRef: { fn: ((ev: MessageEvent) => void) | null } = { fn: null };
    const OrigChannel = globalThis.MessageChannel;
    class CapturingChannel {
      port1 = {
        close: closeSpy,
        set onmessage(fn: ((ev: MessageEvent) => void) | null) {
          handlerRef.fn = fn;
        },
        get onmessage() {
          return handlerRef.fn;
        },
        postMessage: vi.fn(),
      };
      port2 = { close: vi.fn(), onmessage: null, postMessage: vi.fn() };
    }
    vi.stubGlobal("MessageChannel", CapturingChannel);

    try {
      const { getByTestId } = render(
        React.createElement(CompanionLiveHtmlRightPane, {
          activeTab: DEFAULT_TABS[0],
          structureLens: "userflow",
          selectedNodeId: "node-scr-02",
          activeSlide: INITIAL_SLIDES[0],
          activeScreenData: SCREEN_PREVIEW_MAP["node-scr-02"],
          pointerMode: "click-dom",
          onSetPointerMode: () => {},
          isInlineCodeOpen: false,
          onToggleInlineCode: () => {},
          inlineCodeText: "",
          onChangeInlineCode: () => {},
          viewportPreset: "desktop-1440",
          onSetViewportPreset: () => {},
          aspectRatio: "auto",
          onSetAspectRatio: () => {},
          isPresenting: false,
          onTogglePresenting: () => {},
          isLeftStructureHidden: false,
          onToggleLeftStructureHidden: () => {},
          highlightedRegionId: "",
          onRegionClick: () => {},
        })
      );
      const iframe = getByTestId("sandboxed-html-iframe") as HTMLIFrameElement;
      const postMessage = vi.fn();
      Object.defineProperty(iframe, "contentWindow", { value: { postMessage }, configurable: true });
      fireEvent.load(iframe);
      expect(postMessage).toHaveBeenCalledTimes(1);
      const initPayload = postMessage.mock.calls[0][0] as { type: string; epoch: number; bootSecret: string; artifactSha256: string };
      expect(initPayload.type).toBe("INIT_PORT");
      expect(initPayload.bootSecret).toMatch(/^[0-9a-f]{32}$/);

      // Rejects message omitting bootSecretAck (fail-closed)
      handlerRef.fn?.({
        data: { type: "READY", epoch: initPayload.epoch },
      } as MessageEvent);
      expect(closeSpy).toHaveBeenCalledTimes(1);

      handlerRef.fn?.({
        data: {
          type: "PORT_ACK",
          epoch: initPayload.epoch,
          bootSecretAck: "00000000000000000000000000000000",
          artifactSha256: initPayload.artifactSha256,
          domAnchorHash: "1234",
        },
      } as MessageEvent);
      expect(closeSpy).toHaveBeenCalledTimes(2);

      // Sub-state change rotates epoch so reloaded iframe binds fresh INIT_PORT
      fireEvent.click(getByTestId("fixture-variant-LOADING"));
      fireEvent.load(iframe);
      expect(postMessage).toHaveBeenCalledTimes(2);
      const nextInit = postMessage.mock.calls[1][0] as { epoch: number; bootSecret: string };
      expect(nextInit.epoch).not.toBe(initPayload.epoch);
    } finally {
      vi.stubGlobal("MessageChannel", OrigChannel);
    }
  });

  it("AC-03: rejects marquee <16x16px, accepts >=16x16px, shows 0x0 Drawer fallback, and triggers STALE_MAPPING + Re-anchor", () => {
    const onRegionClick = vi.fn();
    const onQuoteText = vi.fn();

    const handlerRef: { fn: ((ev: MessageEvent) => void) | null } = { fn: null };
    const OrigChannel = globalThis.MessageChannel;
    class QuoteChannel {
      port1 = {
        close: vi.fn(),
        set onmessage(fn: ((ev: MessageEvent) => void) | null) {
          handlerRef.fn = fn;
        },
        get onmessage() {
          return handlerRef.fn;
        },
        postMessage: vi.fn(),
      };
      port2 = { close: vi.fn(), onmessage: null, postMessage: vi.fn() };
    }
    vi.stubGlobal("MessageChannel", QuoteChannel);

    try {
      const { getByTestId } = render(
        React.createElement(CompanionLiveHtmlRightPane, {
          activeTab: DEFAULT_TABS[0],
          structureLens: "userflow",
          selectedNodeId: "node-scr-02",
          activeSlide: INITIAL_SLIDES[0],
          activeScreenData: SCREEN_PREVIEW_MAP["node-scr-02"],
          pointerMode: "drag-box",
          onSetPointerMode: () => {},
          isInlineCodeOpen: false,
          onToggleInlineCode: () => {},
          inlineCodeText: "",
          onChangeInlineCode: () => {},
          viewportPreset: "desktop-1440",
          onSetViewportPreset: () => {},
          aspectRatio: "auto",
          onSetAspectRatio: () => {},
          isPresenting: false,
          onTogglePresenting: () => {},
          isLeftStructureHidden: false,
          onToggleLeftStructureHidden: () => {},
          highlightedRegionId: "",
          onRegionClick,
          onQuoteText,
        })
      );

      // Tiny drag < 16x16px ignored
      const overlay = getByTestId("marquee-drag-overlay");
      fireEvent.mouseDown(overlay, { clientX: 10, clientY: 10 });
      fireEvent.mouseUp(overlay, { clientX: 20, clientY: 20 });
      expect(onRegionClick).not.toHaveBeenCalled();

      // Valid drag >= 16x16px attached
      fireEvent.mouseDown(overlay, { clientX: 10, clientY: 10 });
      fireEvent.mouseUp(overlay, { clientX: 50, clientY: 60 });
      expect(onRegionClick).toHaveBeenCalledTimes(1);

      // TEXT_QUOTE bridged across iframe to parent quote callback
      const iframe = getByTestId("sandboxed-html-iframe") as HTMLIFrameElement;
      const postMessage = vi.fn();
      Object.defineProperty(iframe, "contentWindow", { value: { postMessage }, configurable: true });
      fireEvent.load(iframe);
      const initPayload = postMessage.mock.calls[0][0] as { epoch: number; bootSecret: string };
      handlerRef.fn?.({
        data: {
          type: "TEXT_QUOTE",
          epoch: initPayload.epoch,
          bootSecretAck: initPayload.bootSecret,
          text: "Ký & Giải ngân Ngay",
        },
      } as MessageEvent);
      expect(onQuoteText).toHaveBeenCalledWith("Ký & Giải ngân Ngay");
    } finally {
      vi.stubGlobal("MessageChannel", OrigChannel);
    }

    // Drawer fallback (0x0 Drawer badge) & Re-anchor action
    const onReanchor = vi.fn();
    const { getByTestId: getCommentEl } = render(
      React.createElement(ElementCommentThread, {
        nodeId: "node-scr-02",
        collapsedCommentIds: ["c-stale"],
        staleToastMessage: "409 STALE_MAPPING detected",
        onReanchorComment: onReanchor,
        comments: [
          {
            id: "c-stale",
            tenantId: "tenant-a",
            projectId: "proj-1",
            nodeId: "node-scr-02",
            elementFfId: "sec-missing",
            author: "Designer",
            content: "Button moved",
            domHash: "abcd1234",
            artifactSha256: computeArtifactSha256Sync("x"),
            anchorKind: "html_element",
            anchorStatus: "stale",
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
          },
        ],
      })
    );
    expect(getCommentEl("stale-mapping-toast").textContent).toContain("STALE_MAPPING");
    expect(getCommentEl("zero-rect-badge-c-stale")).toBeTruthy();
    fireEvent.click(getCommentEl("reanchor-comment-btn-c-stale"));
    expect(onReanchor).toHaveBeenCalledWith("c-stale");

    // Zod schema parity check
    expect(CreateElementCommentSchema.safeParse({
      elementFfId: "sec-header", author: "A", content: "hi", domHash: "11223344",
      artifactSha256: "abc", anchorKind: "text_quote",
    }).success).toBe(true);
    expect(UpdateElementCommentSchema.safeParse({ reanchor: true, elementFfId: "sec-kpi", domHash: "1", artifactSha256: "2" }).success).toBe(true);
    expect(StaleMappingErrorPayloadSchema.safeParse({ code: "STALE_MAPPING", message: "drift" }).success).toBe(true);
  });

  it("AC-04: enforces Zero Tailwind utility classes in screen-fixture.ts and comments components", () => {
    expect(FIXTURE_VARIANTS.every((v) => v.variant === v.variant.toUpperCase())).toBe(true);
    const filesToScan = [
      path.resolve("src/lib/validations/screen-fixture.ts"),
      path.resolve("src/components/comments/CommentPinMarker.tsx"),
      path.resolve("src/components/comments/ElementCommentItem.tsx"),
      path.resolve("src/components/comments/ElementCommentThread.tsx"),
      path.resolve("src/components/comments/NewCommentForm.tsx"),
      path.resolve("src/features/comments/components/CommentPinMarker.tsx"),
      path.resolve("src/features/comments/components/ElementCommentItem.tsx"),
      path.resolve("src/features/comments/components/ElementCommentThread.tsx"),
      path.resolve("src/features/comments/components/NewCommentForm.tsx"),
    ];
    const tailwindPattern = /\b(?:bg-(?:blue|red|purple|gray|slate|amber|green)-\d+|text-(?:blue|red|purple|gray|slate|amber)-\d+|border-(?:blue|red|purple|gray)-\d+|bg-secondary)\b/;
    for (const file of filesToScan) {
      const content = fs.readFileSync(file, "utf-8");
      expect(tailwindPattern.test(content), `Found Tailwind class in ${file}`).toBe(false);
    }
  });
});
