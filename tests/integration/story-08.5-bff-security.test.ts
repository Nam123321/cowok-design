// @story 08.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { beforeEach, describe, expect, it, vi } from "vitest";
import { issueBffLoopbackSession, setBoundBffPort } from "../../src/lib/api-guard";
import * as daemonClient from "../../src/lib/daemon-client";
import * as deRunRoute from "../../src/app/api/v1/de/run/route";
import * as importRepoRoute from "../../src/app/api/v1/plugins/import-repo/route";
import * as ephemeralExecRoute from "../../src/app/api/v1/plugins/ephemeral-tools/execute/route";
import * as learnLedgerRoute from "../../src/app/api/v1/learn/ledger/route";

const TEST_PORT = 3000;
const HTTP_OK = 200;
const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;
const HTTP_UNPROCESSABLE = 422;
const SAMPLE_SHA = "b".repeat(64);
const SAMPLE_TS = 1_730_500_000_000;

function makeSession(params: {
  role: "viewer" | "editor" | "admin";
  isHumanWorkbench?: boolean;
}) {
  return issueBffLoopbackSession({
    userId: `${params.role}-user`,
    tenantId: "tenant-085",
    role: params.role,
    isHumanWorkbench: params.isHumanWorkbench ?? false,
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

describe("Story 08.5 Split DE+DR BFF Security & Contract Tests", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setBoundBffPort(TEST_PORT);
  });

  it("IT-08.5-BFF-01: blocks unauthenticated callers (401) and viewer mutations (403)", async () => {
    const unauthDe = await deRunRoute.POST(
      makeReq("/api/v1/de/run", "POST", undefined, {
        projectId: "proj-1",
        tools: ["scan-ui"],
        targetArtifactPath: "a.tsx",
      }),
      { params: {} }
    );
    expect(unauthDe.status).toBe(HTTP_UNAUTHORIZED);

    const viewer = makeSession({ role: "viewer" });
    const viewerImport = await importRepoRoute.POST(
      makeReq("/api/v1/plugins/import-repo", "POST", viewer.sessionToken, {
        projectId: "proj-1",
        skillSlug: "demo",
        repoSourceUrl: "https://github.com/cowok-ai/demo",
        licenseSpdx: "MIT",
        description: "Demo",
        bodyMarkdown: "# Demo",
      }),
      { params: {} }
    );
    expect(viewerImport.status).toBe(HTTP_FORBIDDEN);

    const viewerLedgerPost = await learnLedgerRoute.POST(
      makeReq("/api/v1/learn/ledger", "POST", viewer.sessionToken, {
        projectId: "proj-1",
        ruleId: "TELLS-001",
        failureMessage: "Slop",
        slprScore: 0.96,
      }),
      { params: {} }
    );
    expect(viewerLedgerPost.status).toBe(HTTP_FORBIDDEN);
  });

  it("IT-08.5-BFF-02: strictly enforces HUMAN_ONLY (403 HUMAN_APPROVAL_REQUIRED) on ephemeral-tools/execute", async () => {
    const nonHumanEditor = makeSession({ role: "editor", isHumanWorkbench: false });
    const denied = await ephemeralExecRoute.POST(
      makeReq("/api/v1/plugins/ephemeral-tools/execute", "POST", nonHumanEditor.sessionToken, {
        projectId: "proj-1",
        toolName: "safe-echo",
        runtime: "sh",
        scriptSource: "echo hi",
      }),
      { params: {} }
    );
    expect(denied.status).toBe(HTTP_FORBIDDEN);
    const deniedBody = (await denied.json()) as { code?: string; error?: string };
    expect(deniedBody.code || deniedBody.error).toBe("HUMAN_APPROVAL_REQUIRED");

    const humanEditor = makeSession({ role: "editor", isHumanWorkbench: true });
    vi.spyOn(daemonClient, "forwardToDaemonFlow").mockResolvedValue(
      new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-jail-1",
          data: {
            executionId: "exec-1",
            toolName: "safe-echo",
            runtime: "sh",
            actorKind: "HUMAN_WORKBENCH_SESSION",
            scriptSha256: SAMPLE_SHA,
            exitCode: 0,
            timedOut: false,
            killedPgid: null,
            durationMs: 9,
            stdout: "hi\n",
            stderr: "",
            envCleared: true,
            networkIsolated: true,
            nofollowVerified: true,
            rlimitAsBytes: 268_435_456,
            rlimitNproc: 16,
          },
        }),
        { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
      )
    );

    const allowed = await ephemeralExecRoute.POST(
      makeReq("/api/v1/plugins/ephemeral-tools/execute", "POST", humanEditor.sessionToken, {
        projectId: "proj-1",
        toolName: "safe-echo",
        runtime: "sh",
        scriptSource: "echo hi",
      }),
      { params: {} }
    );
    expect(allowed.status).toBe(HTTP_OK);
    const okJson = (await allowed.json()) as { ok: boolean; data: { envCleared: boolean } };
    expect(okJson.ok).toBe(true);
    expect(okJson.data.envCleared).toBe(true);
  });

  it("IT-08.5-BFF-03: rejects malformed Zod payloads & unknown keys fail-closed (422)", async () => {
    const editor = makeSession({ role: "editor", isHumanWorkbench: true });

    const badDe = await deRunRoute.POST(
      makeReq("/api/v1/de/run", "POST", editor.sessionToken, {
        projectId: "proj-1",
        tools: ["unknown-tool"],
        targetArtifactPath: "a.tsx",
        injected: true,
      }),
      { params: {} }
    );
    expect(badDe.status).toBe(HTTP_UNPROCESSABLE);

    const missingProjectLedger = await learnLedgerRoute.GET(
      makeReq("/api/v1/learn/ledger", "GET", editor.sessionToken),
      { params: {} }
    );
    expect(missingProjectLedger.status).toBe(HTTP_UNPROCESSABLE);
  });

  it("IT-08.5-BFF-04: validates daemon responses for de/run, import-repo, and learn/ledger + preserves security errors", async () => {
    const editor = makeSession({ role: "editor" });
    vi.spyOn(daemonClient, "forwardToDaemonFlow").mockImplementation(async (args) => {
      if (args.daemonPath === "/api/v1/de/run") {
        return new Response(
          JSON.stringify({
            ok: true,
            requestId: "req-de",
            data: {
              targetArtifactPath: "home.tsx",
              passed: true,
              costUsd: 0,
              totalDurationMs: 2,
              withinSla: true,
              results: [
                { tool: "nfa-packs", passed: true, durationMs: 1, durationMicros: 300, findings: [] },
              ],
            },
          }),
          { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
        );
      }
      if (args.daemonPath === "/api/v1/plugins/import-repo") {
        return new Response(
          JSON.stringify({
            error: {
              code: "SSRF_URL_BLOCKED",
              message: "Loopback IP blocked",
              field: "repoSourceUrl",
              requestId: "req-ssrf",
            },
          }),
          { status: HTTP_UNPROCESSABLE, headers: { "Content-Type": "application/json" } }
        );
      }
      return new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-ledger",
          data: {
            projectId: "proj-1",
            minHitCount: 3,
            promoteSlprThreshold: 0.95,
            retireSlprThreshold: 0.85,
            entries: [
              {
                id: "gfl-1",
                tenantId: "tenant-085",
                projectId: "proj-1",
                ruleId: "TELLS-001",
                fingerprintSha256: SAMPLE_SHA,
                sampleMessage: "Slop",
                hitCount: 3,
                slprScore: 0.96,
                promotedCardId: "learn-1",
                status: "PROPOSED",
                lastSeenAt: SAMPLE_TS,
                qualifiesForProposal: true,
              },
            ],
          },
        }),
        { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
      );
    });

    const deResp = await deRunRoute.POST(
      makeReq("/api/v1/de/run", "POST", editor.sessionToken, {
        projectId: "proj-1",
        tools: ["nfa-packs"],
        targetArtifactPath: "home.tsx",
      }),
      { params: {} }
    );
    expect(deResp.status).toBe(HTTP_OK);

    const ssrfResp = await importRepoRoute.POST(
      makeReq("/api/v1/plugins/import-repo", "POST", editor.sessionToken, {
        projectId: "proj-1",
        skillSlug: "ssrf-test",
        repoSourceUrl: "https://127.0.0.1/repo",
        licenseSpdx: "MIT",
        description: "Test",
        bodyMarkdown: "# Test",
      }),
      { params: {} }
    );
    expect(ssrfResp.status).toBe(HTTP_UNPROCESSABLE);
    const ssrfJson = (await ssrfResp.json()) as { error: { code: string } };
    expect(ssrfJson.error.code).toBe("SSRF_URL_BLOCKED");

    const ledgerResp = await learnLedgerRoute.GET(
      makeReq("/api/v1/learn/ledger?projectId=proj-1", "GET", editor.sessionToken),
      { params: {} }
    );
    expect(ledgerResp.status).toBe(HTTP_OK);
  });

  it("IT-08.5-BFF-05: maps flat daemon 401/403/413 auth errors cleanly instead of 503", async () => {
    const editor = makeSession({ role: "editor", isHumanWorkbench: true });
    vi.spyOn(daemonClient, "forwardToDaemonFlow").mockResolvedValueOnce(
      new Response(
        JSON.stringify({
          code: "HUMAN_APPROVAL_REQUIRED",
          message: "Human session expired",
          request_id: "req-flat-403",
        }),
        { status: HTTP_FORBIDDEN, headers: { "Content-Type": "application/json" } }
      )
    );
    const resp = await ephemeralExecRoute.POST(
      makeReq("/api/v1/plugins/ephemeral-tools/execute", "POST", editor.sessionToken, {
        projectId: "proj-1",
        toolName: "safe-echo",
        runtime: "sh",
        scriptSource: "echo hi",
      }),
      { params: {} }
    );
    expect(resp.status).toBe(HTTP_FORBIDDEN);
    const json = (await resp.json()) as { error: { code: string } };
    expect(json.error.code).toBe("HUMAN_APPROVAL_REQUIRED");
  });
});
