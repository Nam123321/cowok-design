// @vitest-environment jsdom
// @story 04.1
// @implements AC-03, AC-07
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, fireEvent } from "@testing-library/react";
import {
  HostToIframeMessageSchema,
  IFRAME_SANDBOX_PERMISSIONS,
  IframeToHostMessageSchema,
  STRICT_OFFLINE_CSP,
  computeArtifactSha256Sync,
  generateBootSecret,
  validateUserHtmlPayload,
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
    const wrapped = wrapUserHtmlSrcDoc(userHtml, { epoch: 1 });
    const withoutBridgeScript = wrapped.replace(/<script data-cowok-sandbox-bridge="v1"[\s\S]*?<\/script>/g, "");

    expect(wrapped).toContain(".hero{color:#123456;margin:0}");
    expect(wrapped).toContain('class="dark-landing"');
    expect(withoutBridgeScript).not.toContain("--bg-canvas");
    expect(withoutBridgeScript).not.toContain("--action-primary");
    expect(wrapped).not.toContain("<style data-cowok");
  });

  it("SEC-IFRAME-03: strips malicious <base>, <meta refresh>, and user-supplied CSP tags via DOMParser", () => {
    const hostileHtml = `<!DOCTYPE html><html><head>
      <base href="https://evil.attacker.example/">
      <meta http-equiv="refresh" content="0;url=https://evil.attacker.example/steal">
      <meta http-equiv="Content-Security-Policy" content="default-src * 'unsafe-eval'">
      <title>Test</title>
    </head><body><p>Content</p></body></html>`;

    const wrapped = wrapUserHtmlSrcDoc(hostileHtml, { epoch: 2 });
    expect(wrapped.toLowerCase()).not.toContain("<base");
    expect(wrapped.toLowerCase()).not.toContain("evil.attacker.example");
    expect(wrapped.toLowerCase()).not.toContain("unsafe-eval");
    expect(wrapped).toContain("Content-Security-Policy");
  });

  it("SEC-IFRAME-04: Byte-0 <head> places CSP meta + MessagePort.prototype freeze + capture-phase submit block before user scripts", () => {
    const userHtml = `<html><head><script>window.__USER_RAN = true;</script></head><body><form action="https://exfil.example"><button>Go</button></form></body></html>`;
    const wrapped = wrapUserHtmlSrcDoc(userHtml, { epoch: 3 });

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
    expect(userScriptIdx).toBe(-1);
  });

  it("SEC-IFRAME-05: defaults to STRICT_OFFLINE_CSP with default-src 'none', connect-src 'none', form-action 'none'", () => {
    expect(STRICT_OFFLINE_CSP).toContain("default-src 'none'");
    expect(STRICT_OFFLINE_CSP).toContain("connect-src 'none'");
    expect(STRICT_OFFLINE_CSP).toContain("form-action 'none'");
    expect(STRICT_OFFLINE_CSP).not.toContain("https://");
  });

  it("SEC-IFRAME-06: Story 04.5 zero-network convergence enforces default-src 'none' with zero external CDN script/style hosts", () => {
    const wrapped = wrapUserHtmlSrcDoc('<html><body><div data-ff-id="c1">Offline</div></body></html>');
    expect(wrapped).toContain("default-src 'none'");
    expect(wrapped).not.toContain("cdn.tailwindcss.com");
    expect(wrapped).not.toContain("unpkg.com");
    expect(wrapped).not.toContain("cdn.jsdelivr.net");
    expect(wrapped).not.toContain("https://fonts.googleapis.com");
    expect(wrapped).toContain("var BOOT_SECRET =");
    expect(generateBootSecret().length).toBe(32);
    expect(computeArtifactSha256Sync("<div>Offline</div>").length).toBe(64);
  });

  it("SEC-IFRAME-06B: DOMParser immune to HTML comment-trap fake <head> injection (no regex fallback)", () => {
    const commentTrapHtml = `<!-- <head><script>evil()</script></head> --><html><head><title>Real</title></head><body><p>OK</p></body></html>`;
    const wrapped = wrapUserHtmlSrcDoc(commentTrapHtml, { epoch: 42 });
    const cspIdx = wrapped.indexOf('http-equiv="Content-Security-Policy"');
    const headCloseIdx = wrapped.indexOf("</head>");
    expect(cspIdx).toBeGreaterThan(0);
    expect(cspIdx).toBeLessThan(headCloseIdx);
    expect(wrapped).toContain("addEventListener(\"message\"");
    expect(wrapped).toContain("}, true);");
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

  it("SEC-IFRAME-08: rotates CSPRNG epoch and closes previous MessagePort on node/substate switch", () => {
    const closeSpy = vi.fn();
    const OrigMessageChannel = globalThis.MessageChannel;
    class MockChannel {
      port1 = { close: closeSpy, onmessage: null as ((ev: MessageEvent) => void) | null, postMessage: vi.fn() };
      port2 = { close: vi.fn(), onmessage: null, postMessage: vi.fn() };
    }
    vi.stubGlobal("MessageChannel", MockChannel);

    try {
      const { getByTestId, rerender, unmount } = render(
        React.createElement(CompanionLiveHtmlRightPane, buildPaneProps("node-scr-01"))
      );

      const iframe = getByTestId("sandboxed-html-iframe") as HTMLIFrameElement;
      const postMessage = vi.fn();
      Object.defineProperty(iframe, "contentWindow", {
        value: { postMessage },
        configurable: true,
      });
      fireEvent.load(iframe);
      expect(postMessage).toHaveBeenCalledTimes(1);

      rerender(React.createElement(CompanionLiveHtmlRightPane, buildPaneProps("node-scr-02")));
      unmount();
      expect(closeSpy).toHaveBeenCalled();
    } finally {
      vi.stubGlobal("MessageChannel", OrigMessageChannel);
    }
  });

  it("SEC-IFRAME-09: strips inline on* body attributes, control-padded javascript: URLs, srcdoc, and rotates epoch", () => {
    const out1 = wrapUserHtmlSrcDoc(
      '<body onload="alert(1)" onclick="steal()" data-ok="yes"><a href="java\tscript:alert(1)">Bad</a><iframe srcdoc="<script>1</script>"></iframe><p>Hi</p></body>'
    );
    const out2 = wrapUserHtmlSrcDoc("<body><p>Hi</p></body>");
    const bodySlice = out1.slice(out1.indexOf("<body"));
    expect(bodySlice).not.toContain("onload");
    expect(bodySlice).not.toContain("onclick");
    expect(bodySlice).not.toContain("javascript:");
    expect(bodySlice).not.toContain("script:alert");
    expect(bodySlice).not.toContain("srcdoc");
    expect(bodySlice).toContain('data-ok="yes"');

    const m1 = out1.match(/var EPOCH = (\d+);/);
    const m2 = out2.match(/var EPOCH = (\d+);/);
    expect(m1?.[1]).toBeTruthy();
    expect(m2?.[1]).toBeTruthy();
    expect(m1?.[1]).not.toBe(m2?.[1]);
  });

  it("SEC-IFRAME-10: validates inline HTML edits, applies valid HTML, and blocks self-navigation port transfer", () => {
    expect(validateUserHtmlPayload("").valid).toBe(false);
    expect(validateUserHtmlPayload("plain text without tags").valid).toBe(false);
    expect(validateUserHtmlPayload("<section><h1>Valid</h1></section>").valid).toBe(true);

    const onToggle = vi.fn();
    let codeVal = "not html at all";
    const baseProps = buildPaneProps("node-scr-01");
    const { getByTestId, rerender } = render(
      React.createElement(CompanionLiveHtmlRightPane, {
        ...baseProps,
        isInlineCodeOpen: true,
        onToggleInlineCode: onToggle,
        inlineCodeText: codeVal,
        onChangeInlineCode: (v: string) => {
          codeVal = v;
        },
      })
    );

    fireEvent.click(getByTestId("apply-inline-html-btn"));
    expect(getByTestId("inline-html-error")).toBeTruthy();
    expect(onToggle).not.toHaveBeenCalled();

    rerender(
      React.createElement(CompanionLiveHtmlRightPane, {
        ...baseProps,
        isInlineCodeOpen: true,
        onToggleInlineCode: onToggle,
        inlineCodeText: "<main data-custom='yes'>Updated Screen</main>",
        onChangeInlineCode: (v: string) => {
          codeVal = v;
        },
      })
    );
    fireEvent.click(getByTestId("apply-inline-html-btn"));
    expect(onToggle).toHaveBeenCalledTimes(1);

    // Self-navigation / external href guard when iframe loads
    const postSpy = vi.fn();
    const { getByTestId: getById2 } = render(
      React.createElement(CompanionLiveHtmlRightPane, buildPaneProps("node-scr-01"))
    );
    const iframe = getById2("sandboxed-html-iframe") as HTMLIFrameElement;
    Object.defineProperty(iframe, "contentWindow", {
      value: { postMessage: postSpy, location: { href: "https://evil.example/" } },
      configurable: true,
    });
    fireEvent.load(iframe);
    expect(postSpy).not.toHaveBeenCalled();
  });
});
