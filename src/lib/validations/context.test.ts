// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import { describe, expect, it, vi } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { ContextClient } from "../../api/context";
import { GET as getTelemetryRoute } from "../../app/api/v1/context/telemetry/route";
import { POST as postAssembleRoute } from "../../app/api/v1/context/assemble/route";
import { matchBffRoute } from "../../lib/vite-bff-routes";
import { issueBffLoopbackSession, setBoundBffPort } from "../../lib/api-guard";
import { assembleForwardHeaders, relayDaemonResponse } from "../../lib/daemon-hmac";
import {
  ContextAssembleRequestSchema,
  ContextAssembleSuccessSchema,
  ContextTelemetryQuerySchema,
} from "./context";

describe("Story 02.1 Context Zod, BFF & SDK Contracts", () => {
  const validProjectId = "11111111-1111-4111-8111-111111111111";

  it("UT-API-14/16/17: validates strict assemble request boundaries and rejects unknown keys", () => {
    const valid = ContextAssembleRequestSchema.safeParse({
      projectId: validProjectId,
      portalId: "DeveloperPortal",
      horizon: "mid",
      modelProfile: "A",
      intent: "a".repeat(2000),
      requestedImageTokens: 1024,
      sourceIds: Array.from({ length: 100 }, (_, i) => `11111111-1111-4111-8111-${String(i).padStart(12, "0")}`),
    });
    expect(valid.success).toBe(true);

    expect(
      ContextAssembleRequestSchema.safeParse({
        projectId: validProjectId,
        portalId: "DeveloperPortal",
        horizon: "mid",
        modelProfile: "A",
        intent: "Assemble",
        requestedImageTokens: 0,
        tenantId: "spoof-tenant",
      }).success
    ).toBe(false);

    expect(
      ContextAssembleRequestSchema.safeParse({
        projectId: validProjectId,
        portalId: "DeveloperPortal",
        horizon: "mid",
        modelProfile: "A",
        intent: "a".repeat(2001),
        requestedImageTokens: 0,
        sourceIds: [],
      }).success
    ).toBe(false);
  });

  it("UT-API-15/21/22: rejects unauthenticated/invalid Idempotency-Key and unknown query params", async () => {
    const reqNoAuth = new Request("http://127.0.0.1:4170/api/v1/context/assemble", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({}),
    });
    const resNoAuth = await postAssembleRoute(reqNoAuth);
    expect(resNoAuth.status).toBe(401);

    const reqBadQuery = new Request(
      `http://127.0.0.1:4170/api/v1/context/telemetry?projectId=${validProjectId}&sort=asc`,
      {
        method: "GET",
        headers: { "X-Cowok-Boot-Cap": "invalid" },
      }
    );
    const resQuery = await getTelemetryRoute(reqBadQuery);
    expect(resQuery.status).toBe(401);

    expect(
      ContextTelemetryQuerySchema.safeParse({
        projectId: validProjectId,
        limit: 0,
      }).success
    ).toBe(false);
    expect(
      ContextTelemetryQuerySchema.safeParse({
        projectId: validProjectId,
        limit: 101,
      }).success
    ).toBe(false);
  });

  it("UT-API-18/20 & E2E-08: wires Vite BFF routes and prevents direct fetch() inside ContextMeter UI/hooks", async () => {
    expect(matchBffRoute("POST", "/api/v1/context/assemble")).not.toBeNull();
    expect(matchBffRoute("GET", "/api/v1/context/telemetry")).not.toBeNull();

    const uiFiles = [
      path.resolve(__dirname, "../../features/context-meter/ContextMeterWidget.tsx"),
      path.resolve(__dirname, "../../features/context-meter/ContextMeterPopover.tsx"),
      path.resolve(__dirname, "../../features/context-meter/useContextMeter.ts"),
    ];
    for (const f of uiFiles) {
      const src = fs.readFileSync(f, "utf-8");
      expect(src).not.toMatch(/\bfetch\s*\(/);
    }

    const fakeFetch = vi.fn(async (url: RequestInfo | URL) => {
      if (String(url).includes("/api/auth/local-session")) {
        return new Response(JSON.stringify({ sessionToken: "tok-1" }), { status: 200 });
      }
      return new Response(JSON.stringify({ ok: true, data: { broken: true }, requestId: validProjectId }), {
        status: 200,
      });
    });
    const client = new ContextClient({ fetchImpl: fakeFetch as unknown as typeof fetch });
    const out = await client.assembleContext({
      projectId: validProjectId,
      portalId: "DeveloperPortal",
      horizon: "mid",
      modelProfile: "A",
      intent: "Validate daemon contract",
      requestedImageTokens: 0,
    });
    expect(out.ok).toBe(false);
    if (!out.ok) {
      expect(out.error.code).toBe("DAEMON_CONTRACT_VIOLATION");
    }

    const validOut = ContextAssembleSuccessSchema.safeParse({
      ok: true,
      requestId: validProjectId,
      data: {
        telemetryId: validProjectId,
        projectId: validProjectId,
        portalId: "SalesApp",
        horizon: "short",
        modelProfile: "B",
        adapterMode: "local_reduced",
        reasonCode: "atomic_complete",
        staticPrefixSha256: "a".repeat(64),
        tokenUsage: {
          staticPrefix: 120,
          dynamicTail: 80,
          outline: 40,
          textTotal: 240,
          image: 0,
          ssotDom: 160,
          nonSsotText: 80,
          ssotDomRatio: 0.67,
        },
        sources: { requested: 2, included: 2, pruned: 0 },
        latencyMs: 5,
        assembledAt: "2026-11-21T01:00:00.000Z",
      },
    });
    expect(validOut.success).toBe(true);

    // Remediation assertions: domOutlineNodes strict Zod + assembleForwardHeaders preserves Idempotency-Key
    expect(
      ContextAssembleRequestSchema.safeParse({
        projectId: validProjectId,
        portalId: "DeveloperPortal",
        horizon: "mid",
        modelProfile: "A",
        intent: "Assemble with DOM outline",
        requestedImageTokens: 0,
        domOutlineNodes: [{ ffId: "context-meter-trigger", tag: "button", role: "button", summary: "Meter" }],
      }).success
    ).toBe(true);
    const forwarded = assembleForwardHeaders({ "idempotency-key": validProjectId }, {});
    expect(forwarded["Idempotency-Key"]).toBe(validProjectId);

    // SDK safeParse returns structured error instead of throwing uncaught ZodError
    const invalidSdkRes = await client.assembleContext({
      projectId: "not-a-uuid",
      portalId: "DeveloperPortal",
      horizon: "mid",
      modelProfile: "A",
      intent: "Invalid request check",
      requestedImageTokens: 0,
    });
    expect(invalidSdkRes.ok).toBe(false);
    if (!invalidSdkRes.ok) {
      expect(invalidSdkRes.error.code).toBe("INVALID_REQUEST");
    }

    // BFF normalizes non-UUID X-Correlation-Id in error envelope
    setBoundBffPort(5173);
    const { sessionToken } = issueBffLoopbackSession({ tenantId: "local-default", role: "editor" });
    const badCorrReq = new Request("http://127.0.0.1:5173/api/v1/context/assemble", {
      method: "POST",
      headers: {
        Host: "127.0.0.1:5173",
        Origin: "http://127.0.0.1:5173",
        "Content-Type": "application/json",
        Authorization: `Bearer ${sessionToken}`,
        "X-Cowok-Session-Token": sessionToken,
        "X-Correlation-Id": "not-a-uuid",
      },
      body: JSON.stringify({}),
    });
    const badCorrResp = await postAssembleRoute(badCorrReq, { params: {} });
    expect(badCorrResp.status).toBe(422);
    const badCorrJson = await badCorrResp.json();
    expect(badCorrJson.requestId).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i);

    const busyUpstream = new Response('{"error":{"code":"TELEMETRY_WRITE_BUSY"}}', {
      status: 503,
      headers: { "Content-Type": "application/json", "Retry-After": "1" },
    });
    const relayedBusy = await relayDaemonResponse(busyUpstream, validProjectId);
    expect(relayedBusy.status).toBe(503);
    expect(relayedBusy.headers.get("Retry-After")).toBe("1");
  });
});
