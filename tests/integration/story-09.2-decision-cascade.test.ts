// @story 09.2
// @implements AC-01, AC-02, AC-03, AC-04 (BFF Route Security, RBAC, Zod Fail-Closed, 0% Float Leak, 409 STALE_FILE_OCC & Domain SDK Tests)
import { beforeEach, describe, expect, it, vi } from "vitest";
import { issueBffLoopbackSession, setBoundBffPort } from "../../src/lib/api-guard";
import * as daemonClient from "../../src/lib/daemon-client";
import * as routeEndpoint from "../../src/app/api/v1/decision/route/route";
import * as auditEndpoint from "../../src/app/api/v1/decision/audit/route";
import {
  DecisionApiClient,
  DecisionApiError,
} from "../../src/api/decision";
import {
  DecisionAuditResponseSchema,
  DecisionRouteResponseSchema,
} from "../../src/lib/validations/decision";

const BFF_PORT = 3000;
const HTTP_OK = 200;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_CONFLICT = 409;
const HTTP_PAYLOAD_TOO_LARGE = 413;
const HTTP_UNPROCESSABLE = 422;
const HTTP_BAD_GATEWAY = 502;
const PROJECT_UUID = "11111111-1111-4111-8111-111111111111";
const SHA_A = "a".repeat(64);
const SHA_B = "b".repeat(64);

describe("Story 09.2 Decision Cascade BFF Security, Zod Contracts & Domain SDK Integration", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setBoundBffPort(BFF_PORT);
  });

  function makeSession(role: "viewer" | "editor" | "admin", tenantId = "tenant-alpha") {
    return issueBffLoopbackSession({
      userId: `${role}-user`,
      tenantId,
      role,
    });
  }

  function makeReq(path: string, method: string, sessionToken?: string, body?: unknown) {
    const headers: Record<string, string> = {
      host: `127.0.0.1:${BFF_PORT}`,
    };
    if (sessionToken) {
      headers["x-cowok-session-token"] = sessionToken;
    }
    if (body !== undefined) {
      headers["content-type"] = "application/json";
    }
    return new Request(`http://127.0.0.1:${BFF_PORT}${path}`, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  }

  it("IT-09.2-BFF-01: enforces 401 unauthenticated and 403 viewer RBAC on POST /api/v1/decision/route", async () => {
    const unauthRoute = await routeEndpoint.POST(
      makeReq("/api/v1/decision/route", "POST", undefined, {
        projectId: PROJECT_UUID,
        stage: "DECISION_1",
        prompt: "Design hero page",
        untrustedArtifactSha256: SHA_A,
      }),
      { params: {} }
    );
    expect(unauthRoute.status).toBe(HTTP_UNAUTHORIZED);

    const unauthAudit = await auditEndpoint.GET(
      makeReq(`/api/v1/decision/audit?projectId=${PROJECT_UUID}`, "GET"),
      { params: {} }
    );
    expect(unauthAudit.status).toBe(HTTP_UNAUTHORIZED);

    const viewer = makeSession("viewer");
    const viewerRoute = await routeEndpoint.POST(
      makeReq("/api/v1/decision/route", "POST", viewer.sessionToken, {
        projectId: PROJECT_UUID,
        stage: "DECISION_1",
        prompt: "Design hero page",
        untrustedArtifactSha256: SHA_A,
      }),
      { params: {} }
    );
    expect(viewerRoute.status).toBe(HTTP_FORBIDDEN);
  });

  it("IT-09.2-BFF-02: rejects malformed SHA-256, attemptNumber > 3, and unknown fields fail-closed (422)", async () => {
    const editor = makeSession("editor");

    const badSha = await routeEndpoint.POST(
      makeReq("/api/v1/decision/route", "POST", editor.sessionToken, {
        projectId: PROJECT_UUID,
        stage: "DECISION_1",
        untrustedArtifactSha256: "short-hash",
      }),
      { params: {} }
    );
    expect(badSha.status).toBe(HTTP_UNPROCESSABLE);

    const badAttempt = await routeEndpoint.POST(
      makeReq("/api/v1/decision/route", "POST", editor.sessionToken, {
        projectId: PROJECT_UUID,
        stage: "DECISION_2",
        untrustedArtifactSha256: SHA_A,
        attemptNumber: 4,
      }),
      { params: {} }
    );
    expect(badAttempt.status).toBe(HTTP_UNPROCESSABLE);

    const unknownField = await routeEndpoint.POST(
      makeReq("/api/v1/decision/route", "POST", editor.sessionToken, {
        projectId: PROJECT_UUID,
        stage: "DECISION_1",
        untrustedArtifactSha256: SHA_A,
        rawLogprob: -0.05,
      }),
      { params: {} }
    );
    expect(unknownField.status).toBe(HTTP_UNPROCESSABLE);

    const viewer = makeSession("viewer");
    const badAuditQuery = await auditEndpoint.GET(
      makeReq("/api/v1/decision/audit?projectId=not-a-uuid", "GET", viewer.sessionToken),
      { params: {} }
    );
    expect(badAuditQuery.status).toBe(HTTP_UNPROCESSABLE);
  });

  it("IT-09.2-BFF-03: validates VerdictShield 0% float leak, proxies 409 STALE_FILE_OCC, and lists V21 audit records", async () => {
    const editor = makeSession("editor");
    const viewer = makeSession("viewer");

    vi.spyOn(daemonClient, "forwardToDaemonFlow").mockImplementation(async (args) => {
      if (args.daemonPath === "/api/v1/decision/audit") {
        return new Response(
          JSON.stringify({
            ok: true,
            requestId: "req-audit-1",
            data: {
              projectId: PROJECT_UUID,
              totalCount: 1,
              items: [
                {
                  id: "audit-1",
                  tenantId: "tenant-alpha",
                  projectId: PROJECT_UUID,
                  cascadeStage: "DECISION_1",
                  tierUsed: "LOCAL_FTS5_BM25",
                  confidenceKind: "ExactMatch",
                  verdictLabel: "ROUTE_CRAFT_SKILL",
                  attemptNumber: 1,
                  breakerTripped: false,
                  staleFileSha256: SHA_A,
                  latencyMs: 2,
                  createdAt: 1730500999,
                },
              ],
            },
          }),
          { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
        );
      }

      const bodyObj = args.body as { staleFileSha256?: string; currentFileSha256?: string; prompt?: string } | undefined;
      if (
        bodyObj?.staleFileSha256 &&
        bodyObj?.currentFileSha256 &&
        bodyObj.staleFileSha256 !== bodyObj.currentFileSha256
      ) {
        return new Response(
          JSON.stringify({
            error: {
              code: "STALE_FILE_OCC",
              message: "Artifact SHA-256 changed",
              field: "staleFileSha256",
              requestId: "req-occ-409",
              details: {
                expectedSha256: bodyObj.staleFileSha256,
                currentSha256: bodyObj.currentFileSha256,
              },
            },
          }),
          { status: HTTP_CONFLICT, headers: { "Content-Type": "application/json" } }
        );
      }

      if (bodyObj?.prompt === "LEAK_FLOAT_SIMULATION") {
        // Malicious upstream response attempting to inject float/logprob field -> Zod .strict() MUST block!
        return new Response(
          JSON.stringify({
            ok: true,
            requestId: "req-bad-float",
            data: {
              auditId: "audit-bad",
              output: {
                verdictLabel: "ROUTE_CRAFT_SKILL",
                confidenceKind: "ModelVerdict",
                tierUsed: "GEMINI_DECISION_1",
                cascadeStage: "DECISION_1",
                attemptNumber: 1,
                breakerTripped: false,
                escalatedToAdvisor: false,
                egressGuarded: true,
                stageTrace: [],
                latencyMs: 3,
                rawLogprob: -0.042,
              },
            },
          }),
          { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
        );
      }

      return new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-route-1",
          data: {
            auditId: "audit-1",
            output: {
              verdictLabel: "ROUTE_CRAFT_SKILL",
              confidenceKind: "ExactMatch",
              tierUsed: "LOCAL_FTS5_BM25",
              cascadeStage: "DECISION_1",
              attemptNumber: 1,
              breakerTripped: false,
              escalatedToAdvisor: false,
              egressGuarded: true,
              stageTrace: [
                {
                  stage: "DE_PRE_FILTER",
                  passed: true,
                  summary: "OCC verified",
                  durationMs: 0,
                },
                {
                  stage: "DECISION_1",
                  passed: true,
                  summary: "verdict=ROUTE_CRAFT_SKILL tier=LOCAL_FTS5_BM25",
                  durationMs: 1,
                },
              ],
              latencyMs: 2,
            },
          },
        }),
        { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
      );
    });

    // 1. Happy path decision route
    const okRes = await routeEndpoint.POST(
      makeReq("/api/v1/decision/route", "POST", editor.sessionToken, {
        projectId: PROJECT_UUID,
        stage: "DECISION_1",
        prompt: "Design hero landing page typography",
        untrustedArtifactSha256: SHA_A,
      }),
      { params: {} }
    );
    expect(okRes.status).toBe(HTTP_OK);
    const parsedRoute = DecisionRouteResponseSchema.parse(await okRes.json());
    expect(parsedRoute.data.output.verdictLabel).toBe("ROUTE_CRAFT_SKILL");
    expect(parsedRoute.data.output.confidenceKind).toBe("ExactMatch");

    // 2. StaleFile OCC 409 passthrough
    const occRes = await routeEndpoint.POST(
      makeReq("/api/v1/decision/route", "POST", editor.sessionToken, {
        projectId: PROJECT_UUID,
        stage: "DECISION_1",
        prompt: "Design hero",
        untrustedArtifactSha256: SHA_A,
        staleFileSha256: SHA_A,
        currentFileSha256: SHA_B,
      }),
      { params: {} }
    );
    expect(occRes.status).toBe(HTTP_CONFLICT);

    // 3. Upstream float/logprob leak rejected fail-closed by Zod .strict() (502)
    const leakedRes = await routeEndpoint.POST(
      makeReq("/api/v1/decision/route", "POST", editor.sessionToken, {
        projectId: PROJECT_UUID,
        stage: "DECISION_1",
        prompt: "LEAK_FLOAT_SIMULATION",
        untrustedArtifactSha256: SHA_A,
      }),
      { params: {} }
    );
    expect(leakedRes.status).toBe(HTTP_BAD_GATEWAY);

    // 4. Viewer can list V21 decision_audit_log
    const auditRes = await auditEndpoint.GET(
      makeReq(`/api/v1/decision/audit?projectId=${PROJECT_UUID}&limit=20&stage=DECISION_1`, "GET", viewer.sessionToken),
      { params: {} }
    );
    expect(auditRes.status).toBe(HTTP_OK);
    const parsedAudit = DecisionAuditResponseSchema.parse(await auditRes.json());
    expect(parsedAudit.data.items).toHaveLength(1);
    expect(parsedAudit.data.items[0].tierUsed).toBe("LOCAL_FTS5_BM25");
  });

  it("IT-09.2-BFF-05 (F-01, F-06, F-07): forwards canonical audit rawQuery, rejects unknown query params, enforces distinct RubricPair IDs, and passes through flat daemon auth errors", async () => {
    const editor = makeSession("editor");
    const viewer = makeSession("viewer");

    const forwardSpy = vi
      .spyOn(daemonClient, "forwardToDaemonFlow")
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            requestId: "req-audit-q",
            data: { projectId: PROJECT_UUID, totalCount: 0, items: [] },
          }),
          { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({ code: "UNAUTHORIZED", message: "Legacy bearer rejected" }),
          { status: HTTP_UNAUTHORIZED, headers: { "Content-Type": "application/json" } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({ code: "AUTH_FORBIDDEN", message: "Viewer cannot mutate" }),
          { status: HTTP_FORBIDDEN, headers: { "Content-Type": "application/json" } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({ code: "PAYLOAD_TOO_LARGE", message: "Body exceeded limit" }),
          { status: HTTP_PAYLOAD_TOO_LARGE, headers: { "Content-Type": "application/json" } }
        )
      );

    // F-01: Unknown query param rejected with 422 before hitting daemon
    const unknownParamRes = await auditEndpoint.GET(
      makeReq(`/api/v1/decision/audit?projectId=${PROJECT_UUID}&unknownParam=1`, "GET", viewer.sessionToken),
      { params: {} }
    );
    expect(unknownParamRes.status).toBe(HTTP_UNPROCESSABLE);
    expect(forwardSpy).not.toHaveBeenCalled();

    // F-01: Canonical rawQuery passed accurately to forwardToDaemonFlow
    const validAuditRes = await auditEndpoint.GET(
      makeReq(`/api/v1/decision/audit?projectId=${PROJECT_UUID}&limit=20&stage=DECISION_1`, "GET", viewer.sessionToken),
      { params: {} }
    );
    expect(validAuditRes.status).toBe(HTTP_OK);
    expect(forwardSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        daemonPath: "/api/v1/decision/audit",
        rawQuery: `projectId=${PROJECT_UUID}&limit=20&stage=DECISION_1`,
      })
    );

    // F-06: Duplicate candidateX.id === candidateY.id & deScoreBps > 10000 rejected with 422
    const dupRubricRes = await routeEndpoint.POST(
      makeReq("/api/v1/decision/route", "POST", editor.sessionToken, {
        projectId: PROJECT_UUID,
        stage: "RUBRIC_SHIELD",
        untrustedArtifactSha256: SHA_A,
        rubricPair: {
          candidateX: { id: "same", label: "X", text: "x", deScoreBps: 8000 },
          candidateY: { id: "same", label: "Y", text: "y", deScoreBps: 8500 },
        },
      }),
      { params: {} }
    );
    expect(dupRubricRes.status).toBe(HTTP_UNPROCESSABLE);

    const overBpsRes = await routeEndpoint.POST(
      makeReq("/api/v1/decision/route", "POST", editor.sessionToken, {
        projectId: PROJECT_UUID,
        stage: "DECISION_1",
        untrustedArtifactSha256: SHA_A,
        candidates: [{ id: "c1", label: "OVER", text: "t", deScoreBps: 10001 }],
      }),
      { params: {} }
    );
    expect(overBpsRes.status).toBe(HTTP_UNPROCESSABLE);

    // F-07: Flat daemon { code, message } for 401/403/413 preserves exact status & maps to { error: { code, message, requestId } }
    for (const [expectedStatus, expectedCode] of [
      [HTTP_UNAUTHORIZED, "UNAUTHORIZED"],
      [HTTP_FORBIDDEN, "AUTH_FORBIDDEN"],
      [HTTP_PAYLOAD_TOO_LARGE, "PAYLOAD_TOO_LARGE"],
    ] as const) {
      const res = await routeEndpoint.POST(
        makeReq("/api/v1/decision/route", "POST", editor.sessionToken, {
          projectId: PROJECT_UUID,
          stage: "DECISION_1",
          untrustedArtifactSha256: SHA_A,
        }),
        { params: {} }
      );
      expect(res.status).toBe(expectedStatus);
      const body = (await res.json()) as {
        error: { code: string; message: string; requestId: string };
      };
      expect(body.error.code).toBe(expectedCode);
      expect(body.error.requestId.length).toBeGreaterThan(0);
    }
  });

  it("IT-09.2-SDK-04: DecisionApiClient parses responses with Zod and maps 409 STALE_FILE_OCC to DecisionApiError", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            requestId: "req-sdk-1",
            data: {
              auditId: "audit-sdk-1",
              output: {
                verdictLabel: "ESCALATE_HUMAN_ADVISOR",
                confidenceKind: "DeterministicFallback",
                tierUsed: "HEURISTIC_FALLBACK",
                cascadeStage: "DECISION_2",
                attemptNumber: 3,
                breakerTripped: true,
                escalatedToAdvisor: true,
                egressGuarded: true,
                stageTrace: [],
                latencyMs: 1,
              },
            },
          }),
          { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            error: {
              code: "STALE_FILE_OCC",
              message: "Stale file conflict",
              details: { expectedSha256: SHA_A, currentSha256: SHA_B },
            },
          }),
          { status: HTTP_CONFLICT, headers: { "Content-Type": "application/json" } }
        )
      );

    const sdk = new DecisionApiClient({
      baseUrl: "http://127.0.0.1:3000",
      getSessionToken: () => "test-session-token",
      fetchImpl: fetchMock,
    });
    const res = await sdk.routeDecision({
      projectId: PROJECT_UUID,
      stage: "DECISION_2",
      untrustedArtifactSha256: SHA_A,
      attemptNumber: 3,
    });
    expect(res.data.output.breakerTripped).toBe(true);
    expect(res.data.output.verdictLabel).toBe("ESCALATE_HUMAN_ADVISOR");

    await expect(
      sdk.routeDecision({
        projectId: PROJECT_UUID,
        stage: "DECISION_1",
        untrustedArtifactSha256: SHA_A,
        staleFileSha256: SHA_A,
        currentFileSha256: SHA_B,
      })
    ).rejects.toMatchObject({
      name: "DecisionApiError",
      status: HTTP_CONFLICT,
      code: "STALE_FILE_OCC",
    } satisfies Partial<DecisionApiError>);
  });
});
