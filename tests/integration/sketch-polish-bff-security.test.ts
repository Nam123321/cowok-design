// @story 05.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { beforeEach, describe, expect, it, vi } from "vitest";
import { issueBffLoopbackSession, setBoundBffPort } from "../../src/lib/api-guard";
import * as daemonClient from "../../src/lib/daemon-client";
import { matchBffRoute } from "../../src/lib/vite-bff-routes";
import * as clarifyRoute from "../../src/app/api/v1/sdi/clarify/route";
import * as patchRoute from "../../src/app/api/v1/patch/route";

describe("Story 05.4 Sketch-to-Polish BFF & Security Contracts", () => {
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

  it("IT-05.4-BFF-01: registers GET/POST /api/v1/sdi/clarify and /api/v1/patch in Vite SPA router", () => {
    expect(matchBffRoute("GET", "/api/v1/sdi/clarify")).not.toBeNull();
    expect(matchBffRoute("POST", "/api/v1/sdi/clarify")).not.toBeNull();
    expect(matchBffRoute("GET", "/api/v1/patch")).not.toBeNull();
    expect(matchBffRoute("POST", "/api/v1/patch")).not.toBeNull();
  });

  it("IT-05.4-BFF-02: blocks unauthenticated requests (401) and viewer write mutations (403)", async () => {
    const unauth = await patchRoute.GET(
      makeReq("/api/v1/patch?projectId=proj-1&screenId=home", "GET"),
      { params: {} }
    );
    expect(unauth.status).toBe(401);

    const viewer = makeSession("viewer");
    const viewerClarify = await clarifyRoute.POST(
      makeReq("/api/v1/sdi/clarify", "POST", viewer.sessionToken, {
        projectId: "proj-1",
        goalPrompt: "Build dashboard",
        locale: "en",
      }),
      { params: {} }
    );
    expect(viewerClarify.status).toBe(403);

    const viewerPatch = await patchRoute.POST(
      makeReq("/api/v1/patch", "POST", viewer.sessionToken, {
        projectId: "proj-1",
        screenId: "home",
        baseArtifactSha256: "a".repeat(64),
        ffId: "hero-title",
        opKind: "TEXT_EDIT",
        payloadJson: { text: "Hi" },
      }),
      { params: {} }
    );
    expect(viewerPatch.status).toBe(403);
  });

  it("IT-05.4-BFF-03: rejects malformed Zod payloads & unknown keys fail-closed (422)", async () => {
    const editor = makeSession("editor");
    const badPatch = await patchRoute.POST(
      makeReq("/api/v1/patch", "POST", editor.sessionToken, {
        projectId: "proj-1",
        screenId: "home",
        baseArtifactSha256: "not-a-sha",
        ffId: "bad selector!",
        opKind: "TEXT_EDIT",
        payloadJson: { text: "x" },
        injectedKey: true,
      }),
      { params: {} }
    );
    expect(badPatch.status).toBe(422);

    const missingQuery = await patchRoute.GET(
      makeReq("/api/v1/patch?projectId=proj-1", "GET", editor.sessionToken),
      { params: {} }
    );
    expect(missingQuery.status).toBe(422);
  });

  it("IT-05.4-BFF-04: validates daemon response contract on POST /api/v1/sdi/clarify and /api/v1/patch + 409 STALE_MAPPING passthrough", async () => {
    const editor = makeSession("editor");
    vi.spyOn(daemonClient, "forwardToDaemonFlow").mockImplementation(async (args) => {
      if (args.daemonPath === "/api/v1/sdi/clarify") {
        return new Response(
          JSON.stringify({
            ok: true,
            requestId: "req-1",
            data: {
              sessionId: "sess-1",
              projectId: "11111111-1111-4111-8111-111111111111",
              status: "clarifying",
              goalPrompt: "SaaS Analytics",
              locale: "en",
              activeDirectionId: "dir-editorial-sketch",
              directions: [
                {
                  id: "dir-editorial-sketch",
                  label: "Direction 1",
                  summary: "Monochrome sketch",
                  accentHex: "#2563EB",
                  surfaceHex: "#FAF9F6",
                  skillSlug: "sketch-monochrome",
                },
                {
                  id: "dir-polish-workbench",
                  label: "Direction 2",
                  summary: "Polished bento",
                  accentHex: "#0F766E",
                  surfaceHex: "#F8FAFC",
                  skillSlug: "impeccable-polish",
                },
              ],
              generationEpoch: 1,
              preemptedWorkers: 0,
              routedSkills: ["sketch-monochrome", "impeccable-polish"],
              prefixSha256: "b".repeat(64),
              updatedAt: 1730500000,
            },
          }),
          { status: 200, headers: { "Content-Type": "application/json" } }
        );
      }
      const bodyObj = args.body as { expectedAstRevision?: number } | undefined;
      if (bodyObj?.expectedAstRevision === 999) {
        return new Response(
          JSON.stringify({
            error: {
              code: "STALE_MAPPING",
              message: "Concurrent screen edit detected",
              field: "baseArtifactSha256",
              requestId: "req-conflict",
              details: { currentAstRevision: 2 },
            },
          }),
          { status: 409, headers: { "Content-Type": "application/json" } }
        );
      }
      return new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-2",
          data: {
            patchId: "patch-1",
            projectId: "11111111-1111-4111-8111-111111111111",
            screenId: "home",
            astRevision: 1,
            baseArtifactSha256: "a".repeat(64),
            resultArtifactSha256: "c".repeat(64),
            ffId: "hero-title",
            opKind: "TEXT_EDIT",
            applyTier: "INSTANT_UNDO",
            reconciledFfIds: 0,
            htmlSnapshot: "<h1 data-ff-id=\"hero-title\">Updated</h1>",
            oklchGuard: null,
            lockedRuleReceipt: null,
            undoAvailable: true,
            executionTimeMicros: 420,
            createdAt: 1730500001,
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    });

    const clarifyRes = await clarifyRoute.POST(
      makeReq("/api/v1/sdi/clarify", "POST", editor.sessionToken, {
        projectId: "11111111-1111-4111-8111-111111111111",
        goalPrompt: "SaaS Analytics",
        locale: "en",
      }),
      { params: {} }
    );
    expect(clarifyRes.status).toBe(200);
    const clarifyBody = (await clarifyRes.json()) as { ok: boolean; data: { generationEpoch: number } };
    expect(clarifyBody.ok).toBe(true);
    expect(clarifyBody.data.generationEpoch).toBe(1);

    const patchRes = await patchRoute.POST(
      makeReq("/api/v1/patch", "POST", editor.sessionToken, {
        projectId: "11111111-1111-4111-8111-111111111111",
        screenId: "home",
        baseArtifactSha256: "a".repeat(64),
        ffId: "hero-title",
        opKind: "TEXT_EDIT",
        payloadJson: { text: "Updated" },
      }),
      { params: {} }
    );
    expect(patchRes.status).toBe(200);
    const patchBody = (await patchRes.json()) as { ok: boolean; data: { applyTier: string } };
    expect(patchBody.data.applyTier).toBe("INSTANT_UNDO");

    const staleRes = await patchRoute.POST(
      makeReq("/api/v1/patch", "POST", editor.sessionToken, {
        projectId: "11111111-1111-4111-8111-111111111111",
        screenId: "home",
        baseArtifactSha256: "a".repeat(64),
        ffId: "hero-title",
        opKind: "TEXT_EDIT",
        payloadJson: { text: "Stale" },
        expectedAstRevision: 999,
      }),
      { params: {} }
    );
    expect(staleRes.status).toBe(409);
    const staleBody = (await staleRes.json()) as { error: { code: string } };
    expect(staleBody.error.code).toBe("STALE_MAPPING");
  });

  it("IT-05.4-BFF-05: passes separate daemonPath and rawQuery on GET routes for valid HMAC-SHA256 v2 signing", async () => {
    const editor = makeSession("editor");
    const spy = vi.spyOn(daemonClient, "forwardToDaemonFlow").mockImplementation(async (args) => {
      if (args.daemonPath === "/api/v1/sdi/clarify") {
        return new Response(
          JSON.stringify({ ok: true, requestId: "req-g1", data: { session: null } }),
          { status: 200, headers: { "Content-Type": "application/json" } }
        );
      }
      return new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-g2",
          data: {
            projectId: "11111111-1111-4111-8111-111111111111",
            screenId: "home",
            astRevision: 0,
            currentArtifactSha256: "e".repeat(64),
            htmlSnapshot: "",
            patches: [],
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    });

    const getClarify = await clarifyRoute.GET(
      makeReq("/api/v1/sdi/clarify?projectId=11111111-1111-4111-8111-111111111111", "GET", editor.sessionToken),
      { params: {} }
    );
    expect(getClarify.status).toBe(200);
    expect(spy).toHaveBeenCalledWith(
      expect.objectContaining({
        daemonPath: "/api/v1/sdi/clarify",
        rawQuery: "projectId=11111111-1111-4111-8111-111111111111",
      })
    );

    const getPatch = await patchRoute.GET(
      makeReq("/api/v1/patch?projectId=11111111-1111-4111-8111-111111111111&screenId=home", "GET", editor.sessionToken),
      { params: {} }
    );
    expect(getPatch.status).toBe(200);
    expect(spy).toHaveBeenCalledWith(
      expect.objectContaining({
        daemonPath: "/api/v1/patch",
        rawQuery: "projectId=11111111-1111-4111-8111-111111111111&screenId=home",
      })
    );
  });
});
