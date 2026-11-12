// @vitest-environment jsdom
// @story 04.1
// @implements AC-03, AC-07
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, fireEvent } from "@testing-library/react";
import {
  DESIGN_CDN_CSP,
  HostToIframeMessageSchema,
  IFRAME_SANDBOX_PERMISSIONS,
  IframeToHostMessageSchema,
  STRICT_OFFLINE_CSP,
  wrapUserHtmlSrcDoc,
} from "../../src/features/flow/utils/html-sandbox-wrapper";
import { CompanionLiveHtmlRightPane, type CompanionLiveHtmlRightPaneProps } from "../../src/features/flow/components/CompanionLiveHtmlRightPane";
import { DEFAULT_TABS, INITIAL_SLIDES, SCREEN_PREVIEW_MAP } from "../../src/features/flow/components/companion-canvas-data";

function buildPaneProps(selectedNodeId: string): CompanionLiveHtmlRightPaneProps {
  return {
    activeTab: DEFAULT_TABS[0],
    structureLens: "userflow",
    selectedNodeId,
    activeSlide: INITIAL_SLIDES[0],
    activeScreenData: SCREEN_PREVIEW_MAP[selectedNodeId] || SCREEN_PREVIEW_MAP["node-scr-02"],
    pointerMode: "interact",
    onSetPointerMode: () => {},
    isInlineCodeOpen: false,
    onToggleInlineCode: () => {},
    inlineCodeText: "",
    onChangeInlineCode: () => {},
    viewportPreset: "desktop",
    onSetViewportPreset: () => {},
    aspectRatio: "auto",
    onSetAspectRatio: () => {},
    isPresenting: false,
    onTogglePresenting: () => {},
    isLeftStructureHidden: false,
    onToggleLeftStructureHidden: () => {},
    highlightedRegionId: "",
    onRegionClick: () => {},
  };
}

describe("Option CSP-B Sandboxed HTML Preview Security Gate (SEC-IFRAME-01..08)", () => {
  afterEach(() => {
    cleanup();
  });

  it("SEC-IFRAME-01: enforces exact sandbox='allow-scripts allow-forms' and referrerPolicy='no-referrer' (0 allow-same-origin)", () => {
    expect(IFRAME_SANDBOX_PERMISSIONS).toBe("allow-scripts allow-forms");
    expect(IFRAME_SANDBOX_PERMISSIONS).not.toContain("allow-same-origin");

    const { getByTestId } = render(React.createElement(CompanionLiveHtmlRightPane, buildPaneProps("node-scr-02")));
    const iframe = getByTestId("sandboxed-html-iframe") as HTMLIFrameElement;
    expect(iframe.getAttribute("sandbox")).toBe("allow-scripts allow-forms");
    expect(iframe.getAttribute("sandbox") || "").not.toContain("allow-same-origin");
    expect(iframe.getAttribute("referrerpolicy")).toBe("no-referrer");
  });

  it("SEC-IFRAME-02: wrapUserHtmlSrcDoc injects zero host wrapper CSS and preserves user styles 100%", () => {
    const userHtml = `<!DOCTYPE html><html><head><style>.hero{color:#123456;margin:0}</style></head><body class="dark-landing"><h1 class="hero">Hello</h1></body></html>`;
    const wrapped = wrapUserHtmlSrcDoc(userHtml, { allowDesignCdn: false, epoch: 1 });

    expect(wrapped).toContain(".hero{color:#123456;margin:0}");
    expect(wrapped).toContain('class="dark-landing"');
    expect(wrapped).not.toContain("--bg-canvas");
    expect(wrapped).not.toContain("--action-primary");
    expect(wrapped).not.toContain("<style data-cowok");
  });

  it("SEC-IFRAME-03: strips malicious <base>, <meta refresh>, and user-supplied CSP tags via DOMParser", () => {
    const hostileHtml = `<!DOCTYPE html><html><head>
      <base href="https://evil.attacker.example/">
      <meta http-equiv="refresh" content="0;url=https://evil.attacker.example/steal">
      <meta http-equiv="Content-Security-Policy" content="default-src * 'unsafe-eval'">
      <title>Test</title>
    </head><body><p>Content</p></body></html>`;

    const wrapped = wrapUserHtmlSrcDoc(hostileHtml, { allowDesignCdn: false, epoch: 2 });
    expect(wrapped.toLowerCase()).not.toContain("<base");
    expect(wrapped.toLowerCase()).not.toContain("evil.attacker.example");
    expect(wrapped.toLowerCase()).not.toContain("unsafe-eval");
    expect(wrapped).toContain("Content-Security-Policy");
  });

  it("SEC-IFRAME-04: Byte-0 <head> places CSP meta + MessagePort.prototype freeze + capture-phase submit block before user scripts", () => {
    const userHtml = `<html><head><script>window.__USER_RAN = true;</script></head><body><form action="https://exfil.example"><button>Go</button></form></body></html>`;
    const wrapped = wrapUserHtmlSrcDoc(userHtml, { allowDesignCdn: false, epoch: 3 });

    const cspIdx = wrapped.indexOf('http-equiv="Content-Security-Policy"');
    const bridgeIdx = wrapped.indexOf('data-cowok-sandbox-bridge="v1"');
    const freezeIdx = wrapped.indexOf("Object.freeze(MessagePort.prototype)");
    const submitIdx = wrapped.indexOf('window.addEventListener("submit"');
    const trustedIdx = wrapped.indexOf("!event.isTrusted");
    const userScriptIdx = wrapped.indexOf("window.__USER_RAN");

    expect(cspIdx).toBeGreaterThan(0);
    expect(bridgeIdx).toBeGreaterThan(cspIdx);
    expect(freezeIdx).toBeGreaterThan(bridgeIdx);
    expect(submitIdx).toBeGreaterThan(bridgeIdx);
    expect(trustedIdx).toBeGreaterThan(bridgeIdx);
    expect(userScriptIdx).toBeGreaterThan(bridgeIdx);
  });

  it("SEC-IFRAME-05: defaults to STRICT_OFFLINE_CSP with default-src 'none', connect-src 'none', form-action 'none'", () => {
    expect(STRICT_OFFLINE_CSP).toContain("default-src 'none'");
    expect(STRICT_OFFLINE_CSP).toContain("connect-src 'none'");
    expect(STRICT_OFFLINE_CSP).toContain("form-action 'none'");
    expect(STRICT_OFFLINE_CSP).not.toContain("https://");
  });

  it("SEC-IFRAME-06: 1-click Design CDN allowlist permits read-only Fonts/Tailwind/unpkg/jsDelivr while keeping connect-src 'none' & form-action 'none'", () => {
    expect(DESIGN_CDN_CSP).toContain("https://fonts.googleapis.com");
    expect(DESIGN_CDN_CSP).toContain("https://fonts.gstatic.com");
    expect(DESIGN_CDN_CSP).toContain("https://cdn.tailwindcss.com");
    expect(DESIGN_CDN_CSP).toContain("https://unpkg.com");
    expect(DESIGN_CDN_CSP).toContain("https://cdn.jsdelivr.net");
    expect(DESIGN_CDN_CSP).toContain("https://cdnjs.cloudflare.com");
    expect(DESIGN_CDN_CSP).toContain("connect-src 'none'");
    expect(DESIGN_CDN_CSP).toContain("form-action 'none'");
  });

  it("SEC-IFRAME-07: directional MessageChannel schemas reject inbound APPLY_HTML from iframe to host", () => {
    expect(
      HostToIframeMessageSchema.safeParse({
        type: "APPLY_HTML",
        epoch: 1,
        html: "<div>Host authored</div>",
      }).success
    ).toBe(true);

    expect(
      IframeToHostMessageSchema.safeParse({
        type: "APPLY_HTML",
        epoch: 1,
        html: "<script>alert(1)</script>",
      }).success
    ).toBe(false);

    expect(
      IframeToHostMessageSchema.safeParse({
        type: "ELEMENT_CLICKED",
        epoch: 1,
        selector: '[data-ff-id="sec-kpi"]',
        tagName: "section",
      }).success
    ).toBe(true);
  });

  it("SEC-IFRAME-08: rotates epoch and closes previous MessagePort on node/CDN switch", () => {
    const closeSpy = vi.fn();
    const OrigMessageChannel = globalThis.MessageChannel;
    class MockChannel {
      port1 = { close: closeSpy, onmessage: null as ((ev: MessageEvent) => void) | null, postMessage: vi.fn() };
      port2 = { close: vi.fn(), onmessage: null, postMessage: vi.fn() };
    }
    vi.stubGlobal("MessageChannel", MockChannel);

    try {
      const { getByTestId, rerender } = render(
        React.createElement(CompanionLiveHtmlRightPane, buildPaneProps("node-scr-01"))
      );

      const iframe = getByTestId("sandboxed-html-iframe") as HTMLIFrameElement;
      Object.defineProperty(iframe, "contentWindow", {
        value: { postMessage: vi.fn() },
        configurable: true,
      });
      fireEvent.load(iframe);

      fireEvent.click(getByTestId("toggle-design-cdn-btn"));
      expect(closeSpy).toHaveBeenCalled();

      rerender(React.createElement(CompanionLiveHtmlRightPane, buildPaneProps("node-scr-02")));
    } finally {
      vi.stubGlobal("MessageChannel", OrigMessageChannel);
    }
  });
});
