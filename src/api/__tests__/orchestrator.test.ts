// @story 10.1
// @implements AC-01, AC-02, AC-03, AC-04 (Runtime Zod Validation & 429 Retry-After Backpressure Tests for cowok Orchestrator SDK)
import { describe, expect, it, vi } from "vitest";
import { OrchestratorApiClient, OrchestratorApiError } from "@/api/orchestrator";
import {
  OrchestratorAssetsResponseSchema,
  OrchestratorChatRequestSchema,
  OrchestratorChatResponseSchema,
  OrchestratorErrorCodeSchema,
  OrchestratorLivePatchRequestSchema,
  OrchestratorLivePatchResponseSchema,
  utf8ByteLength,
} from "@/lib/validations/orchestrator";

const PROJECT_UUID = "11111111-1111-4111-8111-111111111111";
const VALID_SHA256 = "a".repeat(64);
const SYNTHETIC_TOKEN = ["cwk", "sess", "test", "101"].join("_");

function createMockClient(responder: (input: RequestInfo | URL, init?: RequestInit) => Response) {
  const fetchImpl = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) =>
    responder(input, init)
  );
  return {
    fetchImpl,
    client: new OrchestratorApiClient({
      baseUrl: "http://127.0.0.1:4170",
      getSessionToken: () => SYNTHETIC_TOKEN,
      fetchImpl: fetchImpl as unknown as typeof fetch,
    }),
  };
}

describe("Story 10.1 OrchestratorApiClient & Zod Contracts", () => {
  it("validates Lane 0 chat responses at runtime with zero token cost and integer latency", async () => {
    const payload = {
      ok: true as const,
      data: {
        lane: "LANE_0_DETERMINISTIC" as const,
        latencyMs: 3,
        tokenCost: 0,
        reply: "Updated CSS token --bg-surface: #0F172A deterministically.",
        appliedPatchId: "patch-101",
        canUndo: true,
        liveSpecPath: ".cowok-design/live-spec/sess-1-doc-patch.jsonl",
        provenanceBacklinks: [
          {
            contentHash: VALID_SHA256,
            sessionId: "sess-1",
            nodeId: "node-hero",
            patchId: "patch-101",
            mimeType: "image/svg+xml",
            createdAt: 1700000000000,
          },
        ],
        actorQueueDepth: 0,
      },
      requestId: "req-chat-1",
    };

    const { client, fetchImpl } = createMockClient(
      () =>
        new Response(JSON.stringify(payload), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })
    );

    const parsed = await client.chat({
      projectId: PROJECT_UUID,
      sessionId: "sess-1",
      nodeId: "node-hero",
      message: "update --bg-surface: #0F172A",
      commandKind: "token_update",
    });

    expect(parsed.data.lane).toBe("LANE_0_DETERMINISTIC");
    expect(parsed.data.tokenCost).toBe(0);
    expect(parsed.data.latencyMs).toBeLessThan(50);
    expect(parsed.data.provenanceBacklinks).toHaveLength(1);
    expect(fetchImpl).toHaveBeenCalledTimes(1);
  });

  it("maps HTTP 429 ACTOR_BACKPRESSURE and Retry-After header into OrchestratorApiError", async () => {
    const { client } = createMockClient(
      () =>
        new Response(
          JSON.stringify({
            error: {
              code: "ACTOR_BACKPRESSURE",
              message: "SQLite write actor queue saturated; retry shortly",
              requestId: "req-bp-1",
              details: { retryAfterSecs: 1, queueCapacity: 256 },
            },
          }),
          {
            status: 429,
            headers: {
              "Content-Type": "application/json",
              "Retry-After": "1",
            },
          }
        )
    );

    let caught: unknown;
    try {
      await client.chat({
        projectId: PROJECT_UUID,
        sessionId: "sess-bp",
        message: "lint_fix",
        commandKind: "lint_fix",
      });
    } catch (err) {
      caught = err;
    }

    expect(caught).toBeInstanceOf(OrchestratorApiError);
    const apiErr = caught as OrchestratorApiError;
    expect(apiErr.status).toBe(429);
    expect(apiErr.code).toBe("ACTOR_BACKPRESSURE");
    expect(apiErr.retryAfterSecs).toBe(1);
  });

  it("validates live-patch apply/undo responses and provenance asset listings", async () => {
    const livePatchResp = {
      ok: true as const,
      data: {
        patchId: "patch-live-1",
        projectId: PROJECT_UUID,
        screenId: "checkout-screen",
        astRevision: 2,
        action: "apply" as const,
        canvasSha256: VALID_SHA256,
        htmlSnapshot: "<main data-ff-id=\"stage-root\">Updated</main>",
        canUndo: true,
        latencyMs: 4,
        appliedLines: 1,
        provenanceBacklinks: [
          {
            contentHash: VALID_SHA256,
            sessionId: "sess-live-1",
            nodeId: "node-checkout",
            patchId: "patch-live-1",
            mimeType: "image/png",
            createdAt: 1700000001000,
          },
        ],
        actorQueueDepth: 0,
      },
      requestId: "req-patch-1",
    };

    const { client: patchClient } = createMockClient(
      () =>
        new Response(JSON.stringify(livePatchResp), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })
    );

    const patchRes = await patchClient.applyLivePatch({
      projectId: PROJECT_UUID,
      sessionId: "sess-live-1",
      nodeId: "node-checkout",
      screenId: "checkout-screen",
      action: "apply",
      liveSpecPayload: JSON.stringify({
        screenId: "checkout-screen",
        ffId: "hero-title",
        opKind: "TEXT_EDIT",
        payload: { text: "Updated" },
      }),
    });
    expect(OrchestratorLivePatchResponseSchema.parse(patchRes).data.astRevision).toBe(2);

    const assetsResp = {
      ok: true as const,
      data: {
        projectId: PROJECT_UUID,
        items: livePatchResp.data.provenanceBacklinks,
        totalCount: 1,
      },
      requestId: "req-assets-1",
    };
    const { client: assetsClient } = createMockClient(
      () =>
        new Response(JSON.stringify(assetsResp), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })
    );

    const assetsRes = await assetsClient.listAssets({
      projectId: PROJECT_UUID,
      sessionId: "sess-live-1",
    });
    expect(OrchestratorAssetsResponseSchema.parse(assetsRes).data.totalCount).toBe(1);
    expect(assetsRes.data.items[0].contentHash).toBe(VALID_SHA256);
  });

  it("rejects oversized >512KB liveSpecPayload (including multi-byte UTF-8) and float leaks via strict Zod contracts", () => {
    const oversizeCheck = OrchestratorChatRequestSchema.safeParse({
      projectId: PROJECT_UUID,
      sessionId: "sess-big",
      message: "apply",
      commandKind: "apply_live_spec",
      liveSpecPayload: "x".repeat(524_289),
    });
    expect(oversizeCheck.success).toBe(false);

    // 131,073 4-byte UTF-8 emojis -> UTF-16 length 262,146 <= 524,288, but 524,292 UTF-8 bytes > 524,288
    const multibytePayload = "🚀".repeat(131_073);
    expect(multibytePayload.length).toBeLessThanOrEqual(524_288);
    expect(utf8ByteLength(multibytePayload)).toBeGreaterThan(524_288);

    const multibyteChatCheck = OrchestratorChatRequestSchema.safeParse({
      projectId: PROJECT_UUID,
      sessionId: "sess-utf8",
      message: "apply",
      commandKind: "apply_live_spec",
      liveSpecPayload: multibytePayload,
    });
    expect(multibyteChatCheck.success).toBe(false);

    const multibytePatchCheck = OrchestratorLivePatchRequestSchema.safeParse({
      projectId: PROJECT_UUID,
      sessionId: "sess-utf8",
      action: "apply",
      liveSpecPayload: multibytePayload,
    });
    expect(multibytePatchCheck.success).toBe(false);

    expect(OrchestratorErrorCodeSchema.parse("E_MXSS_BLOCKED")).toBe("E_MXSS_BLOCKED");
    expect(OrchestratorErrorCodeSchema.parse("PATH_TRAVERSAL")).toBe("PATH_TRAVERSAL");
    expect(OrchestratorErrorCodeSchema.parse("LIVE_SPEC_NOT_FOUND")).toBe("LIVE_SPEC_NOT_FOUND");

    const floatLeakCheck = OrchestratorChatResponseSchema.safeParse({
      ok: true,
      data: {
        lane: "LANE_1_DECISION_CASCADE",
        latencyMs: 12.75,
        tokenCost: 0,
        reply: "Routed",
        appliedPatchId: null,
        canUndo: false,
        liveSpecPath: null,
        provenanceBacklinks: [],
        actorQueueDepth: 0,
      },
      requestId: "req-float",
    });
    expect(floatLeakCheck.success).toBe(false);
  });
});
