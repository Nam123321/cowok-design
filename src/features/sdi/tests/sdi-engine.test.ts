import { describe, expect, it } from "vitest";
import {
  AMBIGUITY_THRESHOLD,
  ClarificationSelectSchema,
  CompileIntentSchema,
  DesignIntentResponseSchema,
  toRustDesignIntentV1,
} from "../../../lib/validations/sdi";
import {
  computeAmbiguityAndRisk,
  generateClarificationVariants,
} from "../utils/ambiguity-calculator";
import {
  GET as getIntentsRoute,
  POST as compileIntentRoute,
} from "../../../app/api/projects/[id]/sdi/intent/route";
import { POST as selectClarificationRoute } from "../../../app/api/projects/[id]/sdi/clarification/route";
import { issueBffLoopbackSession } from "../../../lib/api-guard";
import { buildCanonicalHmacHeaders, ensureRunningDaemon } from "../../../lib/daemon-client";

const HTTP_OK = 200;
const HTTP_CREATED = 201;
const HTTP_BAD_REQUEST = 400;
const HTTP_FORBIDDEN = 403;
const EXPECTED_VARIANTS_COUNT = 3;

async function seedSdiTestProject(tenantId: string): Promise<{
  projectId: string;
  sessionToken: string;
}> {
  const lock = await ensureRunningDaemon();
  const projectId = crypto.randomUUID();
  const rawBody = JSON.stringify({
    id: projectId,
    slug: `sdi-${projectId.slice(0, 8)}`,
    name: "SDI Integration Project",
    product_brief: "SQLite WAL test project",
    workspace_root: null,
    inherit_brand_from_project_id: null,
  });
  const headers = buildCanonicalHmacHeaders({
    daemonNonce: lock.daemon_nonce,
    hmacKeyHex: lock.hmac_key_hex,
    method: "POST",
    canonicalPath: "/api/v1/projects",
    rawQuery: "",
    principal: { tenantId, userId: "sdi-tester", role: "editor" },
    rawBody,
    correlationId: crypto.randomUUID(),
  });
  const seedRes = await fetch(`http://${lock.host}:${lock.port}/api/v1/projects`, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...headers },
    body: rawBody,
  });
  if (!seedRes.ok) {
    throw new Error(`Project seed failed: ${seedRes.status} ${await seedRes.text()}`);
  }
  const { sessionToken } = issueBffLoopbackSession({ tenantId, role: "editor" });
  return { projectId, sessionToken };
}

function bffHeaders(sessionToken: string, hasBody: boolean = false): Record<string, string> {
  return {
    Host: "127.0.0.1:5173",
    Origin: "http://127.0.0.1:5173",
    "X-Cowok-Session-Token": sessionToken,
    ...(hasBody ? { "Content-Type": "application/json" } : {}),
  };
}

describe("Story 05.1: Structured Design Intent (SDI) & Visual Clarification", () => {
  describe("AC-01: Intent Compilation & Ambiguity Scoring", () => {
    it("should calculate low ambiguity when explicit node and specific terms are provided", () => {
      const result = computeAmbiguityAndRisk("Đổi màu button primary trên header", "node-123");
      expect(result.ambiguityScore).toBeLessThan(AMBIGUITY_THRESHOLD);
      expect(result.isAmbiguous).toBe(false);
    });

    it("should calculate high ambiguity when vague request without node is provided", () => {
      const result = computeAmbiguityAndRisk("Làm đẹp lại giao diện này");
      expect(result.ambiguityScore).toBeGreaterThanOrEqual(AMBIGUITY_THRESHOLD);
      expect(result.isAmbiguous).toBe(true);
    });

    it("should generate 3 structured clarification variants with pros and cons", () => {
      const variants = generateClarificationVariants("Cải tiến bố cục giỏ hàng");
      expect(variants).toHaveLength(EXPECTED_VARIANTS_COUNT);
      expect(variants[0].pros.length).toBeGreaterThan(0);
      expect(variants[0].cons.length).toBeGreaterThan(0);
      expect(variants[0].id).toBe("var-1-minimal");
      expect(variants[1].id).toBe("var-2-balanced");
      expect(variants[2].id).toBe("var-3-modular");
    });

    it("should validate schemas strictly via Zod", () => {
      const validCompile = CompileIntentSchema.safeParse({
        rawPrompt: "Thêm nút Back vào màn hình xác nhận",
        targetNodeId: "node-confirm",
      });
      expect(validCompile.success).toBe(true);

      const invalidCompile = CompileIntentSchema.safeParse({
        rawPrompt: "",
      });
      expect(invalidCompile.success).toBe(false);

      const validSelect = ClarificationSelectSchema.safeParse({
        intentId: "123e4567-e89b-12d3-a456-426614174000",
        selectedVariantId: "var-2-balanced",
      });
      expect(validSelect.success).toBe(true);
    });

    it("should map to Rust DesignIntentV1 schema accurately", () => {
      const rustIntent = toRustDesignIntentV1({
        id: "123e4567-e89b-12d3-a456-426614174000",
        tenantId: "tenant-1",
        projectId: "proj-1",
        rawPrompt: "Thêm giỏ hàng",
        intentType: "create_screen",
        targetScope: { nodeIds: [], screenKeys: [], components: [] },
        ambiguityScore: 0.1,
        riskScore: 0.2,
        status: "ready",
        createdAt: "2026-11-07T00:00:00Z",
        updatedAt: "2026-11-07T00:00:00Z",
      });
      expect(rustIntent.id).toBe("123e4567-e89b-12d3-a456-426614174000");
      expect(rustIntent.project_id).toBe("proj-1");
      expect(rustIntent.color_palette.length).toBeGreaterThan(0);
    });
  });

  describe("AC-02: Fastify REST API Routes, Tenant Isolation & State Transition", () => {
    it("should compile intent via POST /api/v1/projects/:id/sdi/intent", async () => {
      const { projectId, sessionToken } = await seedSdiTestProject("tenant-1");
      const req = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/intent`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          rawPrompt: "Sửa lại giao diện đẹp hơn",
        }),
      });

      const res = await compileIntentRoute(req, {
        params: { id: projectId },
      });

      expect(res.status).toBe(HTTP_CREATED);
      const data = await res.json();
      expect(data.status).toBe("clarifying");
      expect(data.ambiguityScore).toBeGreaterThanOrEqual(AMBIGUITY_THRESHOLD);
      expect(data.clarificationVariants).toHaveLength(EXPECTED_VARIANTS_COUNT);
      expect(DesignIntentResponseSchema.safeParse(data).success).toBe(true);
    });

    it("should enforce tenant isolation in GET /api/v1/projects/:id/sdi/intent", async () => {
      const { projectId, sessionToken } = await seedSdiTestProject("tenant-1");
      const compileRes = await compileIntentRoute(
        new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/intent`, {
          method: "POST",
          headers: bffHeaders(sessionToken, true),
          body: JSON.stringify({ rawPrompt: "Sửa lại giao diện đẹp hơn" }),
        }),
        { params: { id: projectId } }
      );
      expect(compileRes.status).toBe(HTTP_CREATED);

      const { sessionToken: otherToken } = issueBffLoopbackSession({
        tenantId: "different-tenant",
        role: "editor",
      });
      const getReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/intent`, {
        headers: bffHeaders(otherToken),
      });
      const resOtherTenant = await getIntentsRoute(getReq, {
        params: { id: projectId },
      });
      expect(resOtherTenant.status).toBe(HTTP_FORBIDDEN);

      const resSameTenant = await getIntentsRoute(
        new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/intent`, {
          headers: bffHeaders(sessionToken),
        }),
        {
          params: { id: projectId },
        }
      );
      expect(resSameTenant.status).toBe(HTTP_OK);
      const dataSame = await resSameTenant.json();
      expect(dataSame.length).toBeGreaterThan(0);
    });

    it("should transition status to ready on valid clarification selection", async () => {
      const { projectId, sessionToken } = await seedSdiTestProject("tenant-1");
      const compileReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/intent`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          rawPrompt: "Tối ưu hóa luồng thanh toán",
        }),
      });

      const compileRes = await compileIntentRoute(compileReq, {
        params: { id: projectId },
      });
      const intent = await compileRes.json();

      const selectReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/clarification`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          intentId: intent.id,
          selectedVariantId: "var-2-balanced",
        }),
      });

      const selectRes = await selectClarificationRoute(selectReq, {
        params: { id: projectId },
      });

      expect(selectRes.status).toBe(HTTP_OK);
      const updated = await selectRes.json();
      expect(updated.status).toBe("ready");
    });

    it("should reject forged selectedVariantId with HTTP 400", async () => {
      const { projectId, sessionToken } = await seedSdiTestProject("tenant-1");
      const compileReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/intent`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          rawPrompt: "Thay đổi màu sắc",
        }),
      });

      const compileRes = await compileIntentRoute(compileReq, {
        params: { id: projectId },
      });
      const intent = await compileRes.json();

      const selectReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/clarification`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          intentId: intent.id,
          selectedVariantId: "forged-variant-id",
        }),
      });

      const selectRes = await selectClarificationRoute(selectReq, {
        params: { id: projectId },
      });

      expect(selectRes.status).toBe(HTTP_BAD_REQUEST);
    });

    it("should reject cross-tenant clarification attempt with HTTP 403/404", async () => {
      const { projectId, sessionToken } = await seedSdiTestProject("tenant-1");
      const compileReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/intent`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          rawPrompt: "Cải tiến trang chủ",
        }),
      });

      const compileRes = await compileIntentRoute(compileReq, {
        params: { id: projectId },
      });
      const intent = await compileRes.json();

      const { sessionToken: attackerToken } = issueBffLoopbackSession({
        tenantId: "attacker-tenant",
        role: "editor",
      });
      const selectReq = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/clarification`, {
        method: "POST",
        headers: bffHeaders(attackerToken, true),
        body: JSON.stringify({
          intentId: intent.id,
          selectedVariantId: "var-1-minimal",
        }),
      });

      const selectRes = await selectClarificationRoute(selectReq, {
        params: { id: projectId },
      });

      expect([HTTP_FORBIDDEN, 404]).toContain(selectRes.status);
    });
  });

  describe("AC-03: [EDGE-CASE] Boundary Guard Against Premature Code Generation", () => {
    it("should strictly maintain status='clarifying' when ambiguityScore >= 0.45", async () => {
      const { projectId, sessionToken } = await seedSdiTestProject("tenant-1");
      const req = new Request(`http://127.0.0.1:5173/api/v1/projects/${projectId}/sdi/intent`, {
        method: "POST",
        headers: bffHeaders(sessionToken, true),
        body: JSON.stringify({
          rawPrompt: "Cải tiến toàn bộ giao diện",
        }),
      });

      const res = await compileIntentRoute(req, {
        params: { id: projectId },
      });

      expect(res.status).toBe(HTTP_CREATED);
      const data = await res.json();
      expect(data.status).toBe("clarifying");
      expect(data.status).not.toBe("ready");
      expect(data.status).not.toBe("executed");
    });
  });
});
