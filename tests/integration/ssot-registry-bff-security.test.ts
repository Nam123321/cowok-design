// @story 02.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SsotRegistryApiClient } from "../../src/api/ssot-registry";
import { issueBffLoopbackSession, setBoundBffPort } from "../../src/lib/api-guard";
import * as daemonClient from "../../src/lib/daemon-client";
import { matchBffRoute } from "../../src/lib/vite-bff-routes";
import * as componentsRoute from "../../src/app/api/v1/ssot/components/route";
import * as lockRoute from "../../src/app/api/v1/ssot/appshell/lock/route";
import * as exportRoute from "../../src/app/api/v1/ssot/export/route";
import * as skillsRoute from "../../src/app/api/v1/skills/route/route";

describe("Story 02.4 BFF & Security Contracts (IT-BFF-01..06 & SEC-IDOR-03)", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setBoundBffPort(3000);
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
      host: "127.0.0.1:3000",
    };
    if (sessionToken) {
      headers["x-cowok-session-token"] = sessionToken;
    }
    if (body !== undefined) {
      headers["content-type"] = "application/json";
    }
    return new Request(`http://127.0.0.1:3000${path}`, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  }

  it("IT-BFF-01: registers all 4 Story 02.4 BFF routes in Vite SPA dispatcher", () => {
    expect(matchBffRoute("GET", "/api/v1/ssot/components")).not.toBeNull();
    expect(matchBffRoute("POST", "/api/v1/ssot/appshell/lock")).not.toBeNull();
    expect(matchBffRoute("POST", "/api/v1/ssot/export")).not.toBeNull();
    expect(matchBffRoute("POST", "/api/v1/skills/route")).not.toBeNull();
  });

  it("IT-BFF-02: blocks unauthenticated requests (401) and viewer write mutations (403)", async () => {
    const unauth = await componentsRoute.GET(
      makeReq("/api/v1/ssot/components?projectId=proj-1&portalId=DeveloperPortal", "GET"),
      { params: Promise.resolve({}) }
    );
    expect(unauth.status).toBe(401);

    const viewer = makeSession("viewer");
    const viewerLock = await lockRoute.POST(
      makeReq("/api/v1/ssot/appshell/lock", "POST", viewer.sessionToken, {
        projectId: "proj-1",
        portalId: "DeveloperPortal",
        action: "ACQUIRE",
        lockOwnerId: "viewer-user",
        expectedRevision: 0,
      }),
      { params: Promise.resolve({}) }
    );
    expect(viewerLock.status).toBe(403);

    const viewerExport = await exportRoute.POST(
      makeReq("/api/v1/ssot/export", "POST", viewer.sessionToken, {
        projectId: "proj-1",
        portalId: "DeveloperPortal",
      }),
      { params: Promise.resolve({}) }
    );
    expect(viewerExport.status).toBe(403);
  });

  it("IT-BFF-03: rejects non-admin TRANSFER or empty forceTransferReason with 403 LOCK_TRANSFER_FORBIDDEN", async () => {
    const editor = makeSession("editor");
    const nonAdminTransfer = await lockRoute.POST(
      makeReq("/api/v1/ssot/appshell/lock", "POST", editor.sessionToken, {
        projectId: "proj-1",
        portalId: "DeveloperPortal",
        action: "TRANSFER",
        lockOwnerId: "editor-user",
        expectedRevision: 1,
        forceTransferReason: "takeover",
      }),
      { params: Promise.resolve({}) }
    );
    expect(nonAdminTransfer.status).toBe(403);
    const errJson = (await nonAdminTransfer.json()) as { error: { code: string } };
    expect(errJson.error.code).toBe("LOCK_TRANSFER_FORBIDDEN");
  });

  it("IT-BFF-04: rejects malformed query/body (strict unknown keys, bad portal, oversized spine) with 422 INVALID_REQUEST", async () => {
    const editor = makeSession("editor");

    const badPortal = await componentsRoute.GET(
      makeReq(
        "/api/v1/ssot/components?projectId=proj-1&portalId=InvalidPortal",
        "GET",
        editor.sessionToken
      ),
      { params: Promise.resolve({}) }
    );
    expect(badPortal.status).toBe(422);

    const extraKey = await skillsRoute.POST(
      makeReq("/api/v1/skills/route", "POST", editor.sessionToken, {
        projectId: "proj-1",
        portalId: "DeveloperPortal",
        intentSummary: "Build KPI cards",
        unexpectedExtraField: true,
      }),
      { params: Promise.resolve({}) }
    );
    expect(extraKey.status).toBe(422);

    const badSpine = await skillsRoute.POST(
      makeReq("/api/v1/skills/route", "POST", editor.sessionToken, {
        projectId: "proj-1",
        portalId: "DeveloperPortal",
        intentSummary: "Build KPI cards",
        maxSpineTokens: 999,
      }),
      { params: Promise.resolve({}) }
    );
    expect(badSpine.status).toBe(422);
  });

  it("IT-BFF-05 & SEC-IDOR-03: preserves 409 conflict details, 503 Retry-After, and 403 cross-tenant rejection from daemon", async () => {
    const editor = makeSession("editor");
    const proxySpy = vi.spyOn(daemonClient, "forwardToDaemonFlow");

    proxySpy.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          error: {
            code: "LOCK_FENCING_CONFLICT",
            message: "Conflict",
            requestId: "req-1",
            details: {
              currentRevision: 2,
              currentFencingToken: 2,
              expiresAt: 1999999999000,
              isExpired: false,
            },
          },
        }),
        { status: 409, headers: { "Content-Type": "application/json" } }
      )
    );

    const conflictResp = await lockRoute.POST(
      makeReq("/api/v1/ssot/appshell/lock", "POST", editor.sessionToken, {
        projectId: "proj-1",
        portalId: "DeveloperPortal",
        action: "ACQUIRE",
        expectedRevision: 0,
      }),
      { params: Promise.resolve({}) }
    );
    expect(proxySpy).toHaveBeenCalledWith(
      expect.objectContaining({
        body: expect.objectContaining({ lockOwnerId: "editor-user" }),
      })
    );
    expect(conflictResp.status).toBe(409);
    const conflictBody = (await conflictResp.json()) as {
      error: { code: string; details: { currentRevision: number } };
    };
    expect(conflictBody.error.code).toBe("LOCK_FENCING_CONFLICT");
    expect(conflictBody.error.details.currentRevision).toBe(2);

    proxySpy.mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          error: {
            code: "STORAGE_BUSY",
            message: "Busy",
            requestId: "req-2",
          },
        }),
        { status: 503, headers: { "Content-Type": "application/json", "Retry-After": "1" } }
      )
    );

    const busyResp = await exportRoute.POST(
      makeReq("/api/v1/ssot/export", "POST", editor.sessionToken, {
        projectId: "proj-1",
        portalId: "DeveloperPortal",
      }),
      { params: Promise.resolve({}) }
    );
    expect(busyResp.status).toBe(503);
    expect(busyResp.headers.get("retry-after")).toBe("1");
  });

  it("IT-BFF-06: Domain SDK validates responses with Zod .parse() and propagates typed errors", async () => {
    const mockFetch = vi.fn().mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-sdk-1",
          data: {
            patterns: [],
            lock: null,
            nextCursor: null,
            quarantinedCount: 0,
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      )
    );

    const sdk = new SsotRegistryApiClient({
      baseUrl: "http://127.0.0.1:3000",
      getSessionToken: () => "tok-123",
      fetchImpl: mockFetch,
    });

    const res = await sdk.fetchComponents({
      projectId: "proj-1",
      portalId: "DeveloperPortal",
      limit: 50,
    });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.data.quarantinedCount).toBe(0);
    }

    // Invalid daemon payload must throw ZodError (never silently pass unvalidated shape)
    mockFetch.mockResolvedValueOnce(
      new Response(JSON.stringify({ ok: true, data: { malformed: true } }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    await expect(
      sdk.routeDesignSkills({
        projectId: "proj-1",
        portalId: "DeveloperPortal",
        intentSummary: "Build KPI chart",
        maxSpineTokens: 250,
      })
    ).rejects.toThrow();
  });

  it("IT-BFF-07: forwards X-Cowok-Portal via extraHeaders for cross-portal guard in daemon", async () => {
    const editor = makeSession("editor");
    const fwdSpy = vi.spyOn(daemonClient, "forwardToDaemonFlow").mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          error: {
            code: "CROSS_PORTAL_LEAK_BLOCKED",
            message: "Cross-portal boundary violation",
            field: "portalId",
            requestId: "req-portal-1",
          },
        }),
        { status: 403, headers: { "Content-Type": "application/json" } }
      )
    );

    const req = new Request("http://127.0.0.1:3000/api/v1/skills/route", {
      method: "POST",
      headers: {
        host: "127.0.0.1:3000",
        "content-type": "application/json",
        "x-cowok-session-token": editor.sessionToken,
        "x-cowok-portal": "SalesApp",
      },
      body: JSON.stringify({
        projectId: "proj-1",
        portalId: "DeveloperPortal",
        intentSummary: "Cross portal check",
      }),
    });

    const resp = await skillsRoute.POST(req, { params: Promise.resolve({}) });
    expect(resp.status).toBe(403);
    expect(fwdSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        extraHeaders: { "X-Cowok-Portal": "SalesApp" },
      })
    );
  });

  it("SEC-IDOR-03 & HIGH-01: rejects duplicate query params (422), whitespace X-Cowok-Portal (403), and cross-tenant mismatch (403)", async () => {
    const editor = makeSession("editor", "tenant-alpha");
    const dupResp = await componentsRoute.GET(
      makeReq(
        "/api/v1/ssot/components?projectId=proj-1&portalId=DeveloperPortal&portalId=SalesApp",
        "GET",
        editor.sessionToken
      ),
      { params: Promise.resolve({}) }
    );
    expect(dupResp.status).toBe(422);

    const blankPortalReq = new Request(
      "http://127.0.0.1:3000/api/v1/ssot/components?projectId=proj-1&portalId=DeveloperPortal",
      {
        method: "GET",
        headers: {
          host: "127.0.0.1:3000",
          "x-cowok-session-token": editor.sessionToken,
          "x-cowok-portal": "   ",
        },
      }
    );
    const blankPortalResp = await componentsRoute.GET(blankPortalReq, { params: Promise.resolve({}) });
    expect(blankPortalResp.status).toBe(403);

    vi.spyOn(daemonClient, "forwardToDaemonFlow").mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          error: {
            code: "PROJECT_TENANT_MISMATCH",
            message: "Cross-tenant project access forbidden",
            field: "projectId",
            requestId: "req-idor-1",
          },
        }),
        { status: 403, headers: { "Content-Type": "application/json" } }
      )
    );
    const crossTenant = await componentsRoute.GET(
      makeReq(
        "/api/v1/ssot/components?projectId=proj-tenant-beta&portalId=DeveloperPortal",
        "GET",
        editor.sessionToken
      ),
      { params: Promise.resolve({}) }
    );
    expect(crossTenant.status).toBe(403);
    const errBody = (await crossTenant.json()) as { error: { code: string } };
    expect(errBody.error.code).toBe("PROJECT_TENANT_MISMATCH");

    const admin = makeSession("admin");
    const adminEmptyReason = await lockRoute.POST(
      makeReq("/api/v1/ssot/appshell/lock", "POST", admin.sessionToken, {
        projectId: "proj-1",
        portalId: "DeveloperPortal",
        action: "TRANSFER",
        expectedRevision: 1,
        expectedFencingToken: 1,
        forceTransferReason: "   ",
      }),
      { params: Promise.resolve({}) }
    );
    expect(adminEmptyReason.status).toBe(422);
  });
});
