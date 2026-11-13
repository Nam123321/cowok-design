/** @vitest-environment jsdom */
// @story 03.2
import React from "react";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";
import {
  ConstraintsClient,
} from "../src/api/constraints";
import {
  DEFAULT_SEEDED_ICON_MANIFEST,
  FLAGSHIP_MANIFEST_SENTINEL,
  IconManifestSchema,
  ProjectDesignContractsSchema,
} from "../src/lib/validations/constraints";
import {
  ALL_REGISTERED_ICON_NAMES,
  BrandIcon,
  isRegisteredIcon,
} from "../src/components/ui/BrandIcon";
import { COWOK_ICON_REGISTRY } from "../src/components/ui/icons";
import { SPECIAL_ICON_RENDERERS } from "../src/components/ui/icons/special-renderers";

describe("Story 03.2 Remediation: Constraints SDK, Registry Integrity & BrandIcon Fallbacks", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("has zero duplicate shadowed keys between COWOK_ICON_REGISTRY and SPECIAL_ICON_RENDERERS", () => {
    const baseKeys = new Set(Object.keys(COWOK_ICON_REGISTRY));
    const shadowedKeys = Object.keys(SPECIAL_ICON_RENDERERS).filter((k) => baseKeys.has(k));
    expect(shadowedKeys).toEqual([]);
  });

  it("registers icons referenced across workbench UI components and all 143 flagship icons", () => {
    for (const required of ["play", "refresh-cw", "globe", "panel-left-close", "activity", "sparkles"]) {
      expect(isRegisteredIcon(required)).toBe(true);
      expect(ALL_REGISTERED_ICON_NAMES).toContain(required);
    }
    expect(DEFAULT_SEEDED_ICON_MANIFEST.count).toBe(143);
    for (const entry of DEFAULT_SEEDED_ICON_MANIFEST.icons) {
      expect(isRegisteredIcon(entry.name)).toBe(true);
    }
  });

  it("signals explicit DOM fallback telemetry attributes when rendering an unregistered icon", () => {
    const valid = render(<BrandIcon name="activity" aria-label="Activity" />);
    const validSvg = valid.container.querySelector("svg");
    expect(validSvg).toBeTruthy();
    expect(validSvg?.getAttribute("data-icon-fallback")).toBeNull();
    expect(validSvg?.className.baseVal).toContain("icon-activity");
    cleanup();

    const invalid = render(<BrandIcon name="nonexistent-xyz" aria-label="Missing" />);
    const fallbackSvg = invalid.container.querySelector("svg");
    expect(fallbackSvg).toBeTruthy();
    expect(fallbackSvg?.getAttribute("data-icon-fallback")).toBe("true");
    expect(fallbackSvg?.getAttribute("data-requested-icon")).toBe("nonexistent-xyz");
    expect(fallbackSvg?.className.baseVal).toContain("icon-fallback");
  });

  it("rejects schema-invalid {'icons': []} stubs and validates seeded snapshot manifest", () => {
    expect(IconManifestSchema.safeParse({ icons: [] }).success).toBe(false);
    expect(
      IconManifestSchema.safeParse({ name: "Bad", count: 99, icons: [] }).success,
    ).toBe(false);

    const parsed = IconManifestSchema.safeParse(DEFAULT_SEEDED_ICON_MANIFEST);
    expect(parsed.success).toBe(true);
  });

  it("ConstraintsClient enforces Zod schemas and calls daemon routes accurately", async () => {
    const fetchMock = vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => {
      const urlStr = String(url);
      if (urlStr.endsWith("/icon-manifest")) {
        return new Response(
          JSON.stringify({
            name: "cowok.ai Iconography v2",
            status: "flagship",
            count: 1,
            icons: [
              { name: "activity", tier: "workflow", file: "activity.svg", source: "cowok", tags: [] },
            ],
          }),
          { status: 200 },
        );
      }
      if (urlStr.endsWith("/validate-icon")) {
        return new Response(
          JSON.stringify({ requested_icon: "activty", is_valid: false, suggestions: ["activity"] }),
          { status: 200 },
        );
      }
      if (urlStr.endsWith("/validate-typography")) {
        return new Response(
          JSON.stringify({ is_valid: true, errors: [] }),
          { status: 200 },
        );
      }
      if (urlStr.endsWith("/validate-component")) {
        return new Response(
          JSON.stringify({ requested_component: "ActionButton", is_valid: true, suggestions: [] }),
          { status: 200 },
        );
      }
      if (urlStr.endsWith("/design-contracts")) {
        expect(init?.method ?? "GET").toMatch(/GET|PUT/);
        return new Response(
          JSON.stringify({
            icon_manifest_path: FLAGSHIP_MANIFEST_SENTINEL,
            allowed_icon_set: [],
            typography_contract: {
              allowed_font_families: ["Geist"],
              allowed_font_weights: [400, 600],
              allowed_font_sizes: ["14px", "16px"],
              min_wcag_contrast: 4.5,
            },
            component_registry: {
              components: [],
              forbid_unregistered: false,
            },
          }),
          { status: 200 },
        );
      }
      if (urlStr.endsWith("/lint")) {
        return new Response(
          JSON.stringify({
            violations: [
              {
                rule_code: "ENFORCE_ICON_MANIFEST",
                severity: "ERROR",
                message: "Unknown icon",
                line: 1,
                column: 10,
              },
            ],
            passed: false,
          }),
          { status: 200 },
        );
      }
      return new Response(JSON.stringify({ message: "Not Found" }), { status: 404 });
    });

    vi.stubGlobal("fetch", fetchMock);
    const client = new ConstraintsClient({
      baseUrl: "http://127.0.0.1:4096",
      getAuthToken: () => "test-token",
    });

    const manifest = await client.getIconManifest("p1");
    expect(manifest.count).toBe(1);

    const iconReport = await client.validateIcon("p1", { icon: "activty" });
    expect(iconReport.is_valid).toBe(false);
    expect(iconReport.suggestions).toEqual(["activity"]);

    const typoReport = await client.validateTypography("p1", {
      font_family: "Geist",
      font_weight: 400,
      font_size: "1rem",
      contrast_ratio: 5.0,
    });
    expect(typoReport.is_valid).toBe(true);

    const compReport = await client.validateComponent("p1", { component: "ActionButton" });
    expect(compReport.is_valid).toBe(true);

    const contracts = await client.getDesignContracts("p1");
    expect(ProjectDesignContractsSchema.safeParse(contracts).success).toBe(true);

    const saved = await client.updateDesignContracts("p1", contracts);
    expect(saved.icon_manifest_path).toBe(FLAGSHIP_MANIFEST_SENTINEL);

    const lint = await client.lintSource("p1", { source: '<BrandIcon name="activty" />' });
    expect(lint.passed).toBe(false);
    expect(lint.violations[0].rule_code).toBe("ENFORCE_ICON_MANIFEST");
  });
});
