// @vitest-environment jsdom
// @story 04.6
// @implements AC-03, AC-05, UT-CSP-SVG-001 (Deterministic SVG viewBox Inference, Strict SVG URL Allowlist & Fail-Closed CDN CSP)
import { describe, expect, it } from "vitest";
import {
  inferSvgViewBox,
  isAllowedSvgUrl,
  parseDesignCdnCspAllowlist,
  sanitizeViaDomParser,
} from "./html-sandbox-sanitizer";

describe("UT-CSP-SVG-001: SVG ViewBox Fallback, URL Policy & Fail-Closed CDN CSP", () => {
  it("preserves SVG lacking viewBox by inferring dimensions or 0 0 1200 800 fallback", () => {
    const htmlWithDims = '<html><body><svg width="640" height="360"><rect width="10" height="10"/></svg></body></html>';
    const outDims = sanitizeViaDomParser(htmlWithDims, "", "nonce-1");
    expect(outDims).toContain("<svg");
    expect(outDims).toContain('viewBox="0 0 640 360"');

    const htmlNoDims = "<html><body><svg><circle cx='10' cy='10' r='5'/></svg></body></html>";
    const outFallback = sanitizeViaDomParser(htmlNoDims, "", "nonce-2");
    expect(outFallback).toContain("<svg");
    expect(outFallback).toContain('viewBox="0 0 1200 800"');

    const doc = new DOMParser().parseFromString('<svg width="48px" height="48px"></svg>', "text/html");
    const svg = doc.querySelector("svg")!;
    expect(inferSvgViewBox(svg)).toBe("0 0 48 48");
  });

  it("strips dangerous SVG URLs while keeping strictly allowlisted schemes", () => {
    expect(isAllowedSvgUrl("https://cdn.example.com/icon.png")).toBe(true);
    expect(isAllowedSvgUrl("http://localhost/img.png")).toBe(true);
    expect(isAllowedSvgUrl("data:image/png;base64,iVBORw0KGgo=")).toBe(true);
    expect(isAllowedSvgUrl("#grad-1")).toBe(true);
    expect(isAllowedSvgUrl("/assets/logo.png")).toBe(true);

    expect(isAllowedSvgUrl("data:image/svg+xml;base64,PHN2Zz4=")).toBe(false);
    expect(isAllowedSvgUrl("javascript:alert(1)")).toBe(false);
    expect(isAllowedSvgUrl("//evil.example/leak")).toBe(false);

    const svgHtml =
      '<html><body><svg viewBox="0 0 100 100"><a href="javascript:alert(1)"><text>X</text></a><image href="data:image/png;base64,AAAA" /><image xlink:href="data:image/svg+xml;base64,BBBB" /></svg></body></html>';
    const cleaned = sanitizeViaDomParser(svgHtml, "", "nonce-3");
    expect(cleaned).not.toContain("javascript:");
    expect(cleaned).not.toContain("image/svg+xml");
    expect(cleaned).toContain("data:image/png;base64,AAAA");
  });

  it("enforces fail-closed exact ^https://[a-z0-9.-]+$ host parsing for DESIGN_CDN_CSP", () => {
    expect(parseDesignCdnCspAllowlist("")).toEqual([]);
    expect(parseDesignCdnCspAllowlist("https://fonts.gstatic.com https://cdn.jsdelivr.net")).toEqual([
      "https://fonts.gstatic.com",
      "https://cdn.jsdelivr.net",
    ]);
    expect(parseDesignCdnCspAllowlist("https://valid.example.com https://*.wildcard.invalid")).toEqual([]);
    expect(parseDesignCdnCspAllowlist("http://insecure.example.com")).toEqual([]);
    expect(parseDesignCdnCspAllowlist("https://path.example.com/subpath")).toEqual([]);
  });

  it("wires DESIGN_CDN_CSP allowlist origins into buildStrictOfflineCspWithAllowlist fail-closed", async () => {
    const { buildStrictOfflineCspWithAllowlist } = await import("./html-sandbox-wrapper");
    const cspAllowed = buildStrictOfflineCspWithAllowlist(
      "nonce-4",
      "https://fonts.gstatic.com https://cdn.jsdelivr.net"
    );
    expect(cspAllowed).toContain("style-src 'nonce-nonce-4' https://fonts.gstatic.com https://cdn.jsdelivr.net;");
    expect(cspAllowed).toContain("font-src data: https://fonts.gstatic.com https://cdn.jsdelivr.net;");
    expect(cspAllowed).toContain("img-src data: blob: https://fonts.gstatic.com https://cdn.jsdelivr.net;");
    const cspInvalid = buildStrictOfflineCspWithAllowlist("nonce-5", "http://bad.example.com");
    expect(cspInvalid).toContain("style-src 'nonce-nonce-5';");
  });
});
