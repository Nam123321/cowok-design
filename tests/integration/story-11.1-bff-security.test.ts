// @story 11.1
// @implements AC-01, AC-02, AC-03, AC-04 (BFF Route Security, RBAC, Zod Fail-Closed & 429 Passthrough Tests)
import { beforeEach, describe, expect, it, vi } from "vitest";
import { issueBffLoopbackSession, setBoundBffPort } from "../../src/lib/api-guard";
import * as daemonClient from "../../src/lib/daemon-client";
import * as executeRoute from "../../src/app/api/v1/bot-fleet/execute/route";
import * as providersStatusRoute from "../../src/app/api/v1/bot-fleet/providers/status/route";

const BFF_PORT = 3000;
const HTTP_OK = 200;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_UNPROCESSABLE = 422;
const HTTP_SERVICE_UNAVAILABLE = 503;
const PROJECT_UUID = "11111111-1111-4111-8111-111111111111";
const ADAPTER_COUNT_27 = 27;

describe("Story 11.1 Bot Fleet BFF Security & Contract Integration", () => {
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

  it("IT-11.1-BFF-01: blocks unauthenticated callers (401) and viewer execution mutations (403)", async () => {
    const unauthStatus = await providersStatusRoute.GET(
      makeReq("/api/v1/bot-fleet/providers/status", "GET"),
      { params: {} }
    );
    expect(unauthStatus.status).toBe(HTTP_UNAUTHORIZED);

    const unauthExec = await executeRoute.POST(
      makeReq("/api/v1/bot-fleet/execute", "POST", undefined, {
        projectId: PROJECT_UUID,
        sessionId: "sess-1",
        prompt: "hello",
      }),
      { params: {} }
    );
    expect(unauthExec.status).toBe(HTTP_UNAUTHORIZED);

    const viewer = makeSession("viewer");
    const viewerExec = await executeRoute.POST(
      makeReq("/api/v1/bot-fleet/execute", "POST", viewer.sessionToken, {
        projectId: PROJECT_UUID,
        sessionId: "sess-1",
        prompt: "hello",
      }),
      { params: {} }
    );
    expect(viewerExec.status).toBe(HTTP_FORBIDDEN);
  });

  it("IT-11.1-BFF-02: rejects malformed Zod payloads and unknown fields fail-closed (422)", async () => {
    const editor = makeSession("editor");
    const badRes = await executeRoute.POST(
      makeReq("/api/v1/bot-fleet/execute", "POST", editor.sessionToken, {
        projectId: "not-a-uuid",
        sessionId: "sess-1",
        prompt: "hello",
        injectedArg: "--danger",
      }),
      { params: {} }
    );
    expect(badRes.status).toBe(HTTP_UNPROCESSABLE);
  });

  it("IT-11.1-BFF-03: validates daemon execute & providers/status responses and preserves 503 Retry-After", async () => {
    const editor = makeSession("editor");
    const viewer = makeSession("viewer");

    vi.spyOn(daemonClient, "forwardToDaemonFlow").mockImplementation(async (args) => {
      if (args.daemonPath === "/api/v1/bot-fleet/providers/status") {
        return new Response(
          JSON.stringify({
            ok: true,
            requestId: "req-status-1",
            data: {
              sandboxProtocol: "posix_0600_fd_pgid",
              sessionDirIsolated: true,
              adapterCount: ADAPTER_COUNT_27,
              readyAdapterCount: 1,
              adapters: [],
              byokProviders: [],
              checkedAt: "2026-11-22T18:00:00Z",
            },
          }),
          { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
        );
      }

      const bodyObj = args.body as { prompt?: string } | undefined;
      if (bodyObj?.prompt === "TRIGGER_429") {
        return new Response(
          JSON.stringify({
            error: {
              code: "STORAGE_BUSY",
              message: "All providers cooling down",
              field: "providerId",
              requestId: "req-429",
              details: { cooldownApplied: true, failoverProviders: ["gemini"] },
            },
          }),
          {
            status: HTTP_SERVICE_UNAVAILABLE,
            headers: { "Content-Type": "application/json", "Retry-After": "12" },
          }
        );
      }

      return new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-exec-1",
          data: {
            executionId: "exec-1",
            mode: "byos",
            adapterId: "claude",
            providerId: null,
            modelId: "claude-sonnet-4-5",
            normalizedContent: "Clean output",
            extractedJson: null,
            ansiSequencesStripped: 0,
            jsonBlocksExtracted: 0,
            svgViewboxesInjected: 0,
            cooldownApplied: false,
            failoverProviders: [],
            pgidReaped: true,
            timedOut: false,
            durationMs: 42,
            executedAt: 1730500123,
          },
        }),
        { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
      );
    });

    const statusRes = await providersStatusRoute.GET(
      makeReq("/api/v1/bot-fleet/providers/status", "GET", viewer.sessionToken),
      { params: {} }
    );
    expect(statusRes.status).toBe(HTTP_OK);
    const statusJson = (await statusRes.json()) as { data: { adapterCount: number } };
    expect(statusJson.data.adapterCount).toBe(ADAPTER_COUNT_27);

    const execRes = await executeRoute.POST(
      makeReq("/api/v1/bot-fleet/execute", "POST", editor.sessionToken, {
        projectId: PROJECT_UUID,
        sessionId: "sess-1",
        mode: "byos",
        adapterId: "claude",
        prompt: "Build hero",
      }),
      { params: {} }
    );
    expect(execRes.status).toBe(HTTP_OK);
    const execJson = (await execRes.json()) as { data: { pgidReaped: boolean } };
    expect(execJson.data.pgidReaped).toBe(true);

    const rateRes = await executeRoute.POST(
      makeReq("/api/v1/bot-fleet/execute", "POST", editor.sessionToken, {
        projectId: PROJECT_UUID,
        sessionId: "sess-1",
        mode: "byok",
        prompt: "TRIGGER_429",
      }),
      { params: {} }
    );
    expect(rateRes.status).toBe(HTTP_SERVICE_UNAVAILABLE);
    expect(rateRes.headers.get("retry-after")).toBe("12");
  });
});
