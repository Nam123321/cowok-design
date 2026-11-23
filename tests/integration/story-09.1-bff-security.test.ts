// @story 09.1
// @implements AC-01, AC-02, AC-03, AC-04 (BFF Auth, Role Guard, Fail-Closed Zod & Firewall Pass-through Tests)
import { beforeEach, describe, expect, it, vi } from "vitest";
import { issueBffLoopbackSession, setBoundBffPort } from "../../src/lib/api-guard";
import * as daemonClient from "../../src/lib/daemon-client";
import * as ingestRoute from "../../src/app/api/v1/rea/ingest/route";
import * as searchRoute from "../../src/app/api/v1/rea/search/route";
import * as exportProofRoute from "../../src/app/api/v1/rea/export-proof/route";

const TEST_PORT = 3000;
const HTTP_OK = 200;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_UNPROCESSABLE = 422;
const HTTP_SERVICE_UNAVAILABLE = 503;
const SAMPLE_SHA = "c".repeat(64);
const SAMPLE_TS = 1_730_600_000_000;

function makeSession(role: "viewer" | "editor" | "admin", tenantId = "tenant-091") {
  return issueBffLoopbackSession({
    userId: `${role}-user`,
    tenantId,
    role,
    isHumanWorkbench: true,
  });
}

function makeReq(path: string, method: string, sessionToken?: string, body?: unknown): Request {
  const headers: Record<string, string> = { host: `127.0.0.1:${TEST_PORT}` };
  if (sessionToken) headers["x-cowok-session-token"] = sessionToken;
  if (body !== undefined) headers["content-type"] = "application/json";
  return new Request(`http://127.0.0.1:${TEST_PORT}${path}`, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
}

describe("Story 09.1 Two-Store /rea BFF Security & Contract Tests", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setBoundBffPort(TEST_PORT);
  });

  it("IT-09.1-BFF-01: blocks unauthenticated callers (401) and viewer mutations (403)", async () => {
    const unauthSearch = await searchRoute.GET(
      makeReq("/api/v1/rea/search?q=fintech&tier=1a", "GET"),
      { params: {} }
    );
    expect(unauthSearch.status).toBe(HTTP_UNAUTHORIZED);

    const viewer = makeSession("viewer");
    const viewerIngest = await ingestRoute.POST(
      makeReq("/api/v1/rea/ingest", "POST", viewer.sessionToken, {
        slug: "demo-kit",
        sourceUrl: "https://github.com/open-design/demo",
        sourceFormat: "github",
        licenseSpdx: "MIT",
      }),
      { params: {} }
    );
    expect(viewerIngest.status).toBe(HTTP_FORBIDDEN);

    const viewerProof = await exportProofRoute.POST(
      makeReq("/api/v1/rea/export-proof", "POST", viewer.sessionToken, {
        referenceId: "ref-1",
      }),
      { params: {} }
    );
    expect(viewerProof.status).toBe(HTTP_FORBIDDEN);
  });

  it("IT-09.1-BFF-02: rejects malformed Zod payloads & unknown fields fail-closed (422)", async () => {
    const editor = makeSession("editor");

    const badIngest = await ingestRoute.POST(
      makeReq("/api/v1/rea/ingest", "POST", editor.sessionToken, {
        slug: "valid-slug",
        sourceUrl: "https://github.com/open-design/demo",
        sourceFormat: "github",
        licenseSpdx: "MIT",
        injectedTenantId: "evil-tenant",
      }),
      { params: {} }
    );
    expect(badIngest.status).toBe(HTTP_UNPROCESSABLE);

    const badTier = await searchRoute.GET(
      makeReq("/api/v1/rea/search?q=fintech&tier=9z", "GET", editor.sessionToken),
      { params: {} }
    );
    expect(badTier.status).toBe(HTTP_UNPROCESSABLE);

    const badChecksum = await exportProofRoute.POST(
      makeReq("/api/v1/rea/export-proof", "POST", editor.sessionToken, {
        referenceId: "ref-1",
        roundtripChecksum: "not-a-sha256",
      }),
      { params: {} }
    );
    expect(badChecksum.status).toBe(HTTP_UNPROCESSABLE);
  });

  it("IT-09.1-BFF-03: forwards valid ingest, search & proof requests and enforces response schema", async () => {
    const editor = makeSession("editor", "tenant-alpha");
    const forwardSpy = vi
      .spyOn(daemonClient, "forwardToDaemonFlow")
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            requestId: "req-1",
            data: {
              tier: "1a",
              query: "fintech",
              sanitizedFtsQuery: '"fintech"*',
              maxTier1aTokens: 250,
              total: 1,
              items: [
                {
                  id: "ref-1",
                  tenantId: "tenant-alpha",
                  slug: "fintech-kit",
                  sourceUrl: "https://github.com/open-design/fintech-kit",
                  sourceFormat: "github",
                  licenseSpdx: "MIT",
                  firewallVerdict: "ADOPT_PRIOR_ART",
                  firewallReason: "Cleared",
                  tier1aSummary: "Summary",
                  tier1aTokens: 1,
                  proofStatus: "awaiting-verification",
                  sanitizedSecretsCount: 0,
                  createdAt: SAMPLE_TS,
                },
              ],
            },
          }),
          { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            error: {
              code: "FIREWALL_REJECTED",
              message: "GPL rejected",
              field: "licenseSpdx",
              requestId: "req-2",
              details: { verdict: "REJECT", quarantineId: "q-1" },
            },
          }),
          { status: HTTP_UNPROCESSABLE, headers: { "Content-Type": "application/json" } }
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            ok: true,
            requestId: "req-3",
            data: {
              referenceId: "ref-1",
              slug: "fintech-kit",
              proofStatus: "proof-verified",
              verified: true,
              checksumMatched: true,
              computedSha256: SAMPLE_SHA,
              proofPayloadJson: "{}",
              verificationNote: "Verified",
            },
          }),
          { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
        )
      );

    const searchResp = await searchRoute.GET(
      makeReq("/api/v1/rea/search?q=fintech&tier=1a", "GET", editor.sessionToken),
      { params: {} }
    );
    expect(searchResp.status).toBe(HTTP_OK);
    expect(forwardSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        principal: expect.objectContaining({ tenantId: "tenant-alpha" }),
      })
    );

    const rejectResp = await ingestRoute.POST(
      makeReq("/api/v1/rea/ingest", "POST", editor.sessionToken, {
        slug: "bad-gpl",
        sourceUrl: "https://github.com/open-design/gpl",
        sourceFormat: "github",
        licenseSpdx: "GPL-3.0",
      }),
      { params: {} }
    );
    expect(rejectResp.status).toBe(HTTP_UNPROCESSABLE);
    const rejectJson = (await rejectResp.json()) as { error: { code: string } };
    expect(rejectJson.error.code).toBe("FIREWALL_REJECTED");

    const proofResp = await exportProofRoute.POST(
      makeReq("/api/v1/rea/export-proof", "POST", editor.sessionToken, {
        referenceId: "ref-1",
        roundtripChecksum: SAMPLE_SHA,
      }),
      { params: {} }
    );
    expect(proofResp.status).toBe(HTTP_OK);
    const proofJson = (await proofResp.json()) as { data: { proofStatus: string; verified: boolean } };
    expect(proofJson.data.proofStatus).toBe("proof-verified");
    expect(proofJson.data.verified).toBe(true);
  });

  it("IT-09.1-BFF-04: fails closed (503) if daemon returns malformed contract", async () => {
    const editor = makeSession("editor");
    vi.spyOn(daemonClient, "forwardToDaemonFlow").mockResolvedValue(
      new Response(JSON.stringify({ unexpected: true }), {
        status: HTTP_OK,
        headers: { "Content-Type": "application/json" },
      })
    );

    const resp = await searchRoute.GET(
      makeReq("/api/v1/rea/search?q=test", "GET", editor.sessionToken),
      { params: {} }
    );
    expect(resp.status).toBe(HTTP_SERVICE_UNAVAILABLE);
  });
});
