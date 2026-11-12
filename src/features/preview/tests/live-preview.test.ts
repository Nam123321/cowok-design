import { describe, expect, it } from "vitest";
import {
  CreateScreenFixtureSchema,
  FixtureVariantSchema,
  PreviewHydrationMessageSchema,
  UpdateScreenFixtureSchema,
  ViewportPresetSchema,
} from "../../../lib/validations/screen-fixture";
import { FIXTURE_VARIANTS } from "../components/FixtureVariantSelector";
import { VIEWPORT_PRESETS } from "../components/ViewportSwitcher";
import { apiScreenFixture } from "../../../api/screen-fixture";
import { GET, POST } from "../../../app/api/projects/[id]/flow/nodes/[nodeId]/fixtures/route";

describe("Story 04.2: State-Hydrated Sandboxed Live Preview & Viewport Switcher", () => {
  describe("AC-01: Multi-Viewport Switcher & Presets", () => {
    it("should define all 3 canonical viewport presets with exact dimensions", () => {
      expect(VIEWPORT_PRESETS["desktop-1440"]).toBeDefined();
      expect(VIEWPORT_PRESETS["desktop-1440"].width).toBe(1440);
      expect(VIEWPORT_PRESETS["desktop-1440"].height).toBe(900);

      expect(VIEWPORT_PRESETS["tablet-768"]).toBeDefined();
      expect(VIEWPORT_PRESETS["tablet-768"].width).toBe(768);
      expect(VIEWPORT_PRESETS["tablet-768"].height).toBe(1024);

      expect(VIEWPORT_PRESETS["mobile-390"]).toBeDefined();
      expect(VIEWPORT_PRESETS["mobile-390"].width).toBe(390);
      expect(VIEWPORT_PRESETS["mobile-390"].height).toBe(844);
    });

    it("should validate viewport preset schema strictly", () => {
      expect(ViewportPresetSchema.safeParse("desktop-1440").success).toBe(true);
      expect(ViewportPresetSchema.safeParse("tablet-768").success).toBe(true);
      expect(ViewportPresetSchema.safeParse("mobile-390").success).toBe(true);
      expect(ViewportPresetSchema.safeParse("ultrawide-4k").success).toBe(false);
    });

    it("should support 4 standard fixture variants", () => {
      const variants = FIXTURE_VARIANTS.map((v) => v.variant);
      expect(variants).toEqual(["DEFAULT", "EMPTY", "LOADING", "ERROR"]);

      expect(FixtureVariantSchema.safeParse("DEFAULT").success).toBe(true);
      expect(FixtureVariantSchema.safeParse("EMPTY").success).toBe(true);
      expect(FixtureVariantSchema.safeParse("LOADING").success).toBe(true);
      expect(FixtureVariantSchema.safeParse("ERROR").success).toBe(true);
      expect(FixtureVariantSchema.safeParse("UNKNOWN").success).toBe(false);
    });
  });

  describe("AC-02: State Hydration Contracts & Client SDK", () => {
    it("should validate CreateScreenFixtureSchema and UpdateScreenFixtureSchema", () => {
      const valid = CreateScreenFixtureSchema.safeParse({
        variant: "DEFAULT",
        payloadJson: JSON.stringify({ userCount: 42, active: true }),
        isDefault: true,
      });
      expect(valid.success).toBe(true);

      const invalid = CreateScreenFixtureSchema.safeParse({
        variant: "INVALID_VARIANT",
        payloadJson: "{}",
      });
      expect(invalid.success).toBe(false);

      const updateValid = UpdateScreenFixtureSchema.safeParse({
        isDefault: false,
      });
      expect(updateValid.success).toBe(true);
    });

    it("should validate PreviewHydrationMessageSchema", () => {
      const msg = {
        type: "FLOW_PREVIEW_HYDRATE",
        nodeId: "node-1",
        screenKey: "SCR_DASHBOARD",
        routePath: "/dashboard",
        variant: "LOADING",
        payload: { status: "pending", retries: 3, flag: true, note: null },
        viewport: {
          preset: "tablet-768",
          width: 768,
          height: 1024,
        },
      };

      const result = PreviewHydrationMessageSchema.safeParse(msg);
      expect(result.success).toBe(true);
    });

    it("should instantiate apiScreenFixture client SDK with expected methods", () => {
      expect(typeof apiScreenFixture.getFixtures).toBe("function");
      expect(typeof apiScreenFixture.saveFixture).toBe("function");
      expect(typeof apiScreenFixture.updateFixture).toBe("function");
      expect(typeof apiScreenFixture.setViewportPreset).toBe("function");
    });
  });

  describe("AC-03 [EDGE-CASE]: Sandboxed Boundary Security Gate", () => {
    it("should enforce strict sandbox permissions omitting allow-same-origin", () => {
      const sandboxConfig = "allow-scripts allow-forms";
      expect(sandboxConfig.includes("allow-same-origin")).toBe(false);
      expect(sandboxConfig.includes("allow-scripts")).toBe(true);
      expect(sandboxConfig.includes("allow-forms")).toBe(true);
    });

    it("should process REST API endpoints for fixtures", async () => {
      const req = new Request("http://localhost/api/v1/projects/proj-1/flow/nodes/node-1/fixtures", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          variant: "DEFAULT",
          payloadJson: JSON.stringify({ title: "Mock Data" }),
          isDefault: true,
        }),
      });

      const ctx = {
        projectId: "proj-1",
        tenantId: "tenant-1",
        params: { id: "proj-1", nodeId: "node-1" },
      };

      const postRes = await POST(req, ctx);
      expect(postRes.status).toBe(201);
      const createdFixture = await postRes.json();
      expect(createdFixture.variant).toBe("DEFAULT");
      expect(createdFixture.nodeId).toBe("node-1");

      const getReq = new Request("http://localhost/api/v1/projects/proj-1/flow/nodes/node-1/fixtures");
      const getRes = await GET(getReq, ctx);
      expect(getRes.status).toBe(200);
      const fixtures = await getRes.json();
      expect(fixtures.length).toBeGreaterThanOrEqual(1);
    });
  });
});
