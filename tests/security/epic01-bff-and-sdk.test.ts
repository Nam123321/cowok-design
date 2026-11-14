// @story 01.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07
import { Readable } from "node:stream";
import type { IncomingMessage, ServerResponse } from "node:http";
import { describe, expect, it, vi } from "vitest";
import { logger } from "../../src/lib/logger";
import { issueBffLoopbackSession, setBoundBffPort } from "../../src/lib/api-guard";
import { ensureRunningDaemon } from "../../src/lib/daemon-client";
import { handleBootstrapSession, issueBootCapToken, revokeBootCapToken } from "../../src/lib/vite-bff-plugin";
import { matchBffRoute } from "../../src/lib/vite-bff-routes";
import { HarnessDaemonApiError, HarnessDaemonClient } from "../../src/api/harness-daemon";

function makeMockIncomingMessage(opts: {
  method?: string;
  url?: string;
  headers?: Record<string, string>;
  body?: string;
}): IncomingMessage {
  const stream = Readable.from(opts.body !== undefined ? [Buffer.from(opts.body, "utf8")] : []);
  return Object.assign(stream, {
    method: opts.method || "POST",
    url: opts.url || "/api/auth/local-session",
    headers: {
      host: "127.0.0.1:5173",
      ...(opts.headers || {}),
    },
  }) as unknown as IncomingMessage;
}

function makeMockServerResponse(): {
  res: ServerResponse;
  result: () => { status: number; headers: Record<string, string>; body: string };
} {
  let status = 200;
  const headers: Record<string, string> = {};
  const chunks: Buffer[] = [];
  const res = {
    get statusCode() {
      return status;
    },
    set statusCode(code: number) {
      status = code;
    },
    setHeader(k: string, v: string | string[]) {
      headers[k.toLowerCase()] = Array.isArray(v) ? v.join(", ") : String(v);
    },
    end(chunk?: string | Buffer) {
      if (chunk !== undefined) {
        chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(String(chunk), "utf8"));
      }
    },
  } as unknown as ServerResponse;
  return {
    res,
    result: () => ({
      status,
      headers,
      body: Buffer.concat(chunks).toString("utf8"),
    }),
  };
}

describe("Epic 01 Hardening: Logger, BFF Routing & Typed HarnessDaemon SDK", () => {
  it("redacts sensitive keys inside nested arrays and emits info/warn/error streams", () => {
    const outSpy = vi.spyOn(process.stdout, "write").mockImplementation(() => true);
    const errSpy = vi.spyOn(process.stderr, "write").mockImplementation(() => true);
    try {
      logger.info("array_test", {
        entries: [{ hmac_key_hex: "deadbeef", safe: "ok" }],
      });
      logger.warn("warn_test", { token: "secret-tok" });

      expect(outSpy).toHaveBeenCalledTimes(1);
      const infoPayload = JSON.parse(String(outSpy.mock.calls[0][0]));
      expect(infoPayload.level).toBe("info");
      expect(infoPayload.entries[0].hmac_key_hex).toBe("[REDACTED]");
      expect(infoPayload.entries[0].safe).toBe("ok");

      expect(errSpy).toHaveBeenCalledTimes(1);
      const warnPayload = JSON.parse(String(errSpy.mock.calls[0][0]));
      expect(warnPayload.level).toBe("warn");
      expect(warnPayload.token).toBe("[REDACTED]");
    } finally {
      outSpy.mockRestore();
      errSpy.mockRestore();
    }
  });

  it("blocks admin escalation, unverified editor minting, cross-port origin, and malformed JSON while issuing verified tokens", async () => {
    const badJson = makeMockIncomingMessage({ body: "{not-json" });
    const badRes = makeMockServerResponse();
    await handleBootstrapSession(badJson, badRes.res);
    expect(badRes.result().status).toBe(400);
    expect(JSON.parse(badRes.result().body).code).toBe("INVALID_JSON");

    const adminReq = makeMockIncomingMessage({ body: JSON.stringify({ role: "admin" }) });
    const adminRes = makeMockServerResponse();
    await handleBootstrapSession(adminReq, adminRes.res);
    expect(adminRes.result().status).toBe(403);
    expect(JSON.parse(adminRes.result().body).code).toBe("INSUFFICIENT_ROLE");

    const unverifiedEditorReq = makeMockIncomingMessage({ body: JSON.stringify({ role: "editor" }) });
    const unverifiedEditorRes = makeMockServerResponse();
    await handleBootstrapSession(unverifiedEditorReq, unverifiedEditorRes.res);
    expect(unverifiedEditorRes.result().status).toBe(403);
    expect(JSON.parse(unverifiedEditorRes.result().body).code).toBe("INSUFFICIENT_ROLE");

    const viewerReq = makeMockIncomingMessage({ body: JSON.stringify({ role: "viewer" }) });
    const viewerRes = makeMockServerResponse();
    await handleBootstrapSession(viewerReq, viewerRes.res);
    expect(viewerRes.result().status).toBe(200);
    expect(JSON.parse(viewerRes.result().body).role).toBe("viewer");

    const capToken = issueBootCapToken();
    try {
      const capEditorReq = makeMockIncomingMessage({
        headers: { "x-cowok-boot-cap": capToken },
        body: JSON.stringify({ role: "editor" }),
      });
      const capEditorRes = makeMockServerResponse();
      await handleBootstrapSession(capEditorReq, capEditorRes.res);
      expect(capEditorRes.result().status).toBe(200);
      const issued = JSON.parse(capEditorRes.result().body) as { sessionToken: string; role: string };
      expect(issued.role).toBe("editor");
      expect(typeof issued.sessionToken).toBe("string");
      expect(issued.sessionToken.length).toBeGreaterThan(10);
    } finally {
      revokeBootCapToken(capToken);
    }

    setBoundBffPort(5173);
    try {
      const sameOriginEditorReq = makeMockIncomingMessage({
        headers: {
          host: "127.0.0.1:5173",
          origin: "http://127.0.0.1:5173",
          "sec-fetch-site": "same-origin",
        },
        body: JSON.stringify({ role: "editor" }),
      });
      const sameOriginEditorRes = makeMockServerResponse();
      await handleBootstrapSession(sameOriginEditorReq, sameOriginEditorRes.res);
      expect(sameOriginEditorRes.result().status).toBe(200);
      expect(JSON.parse(sameOriginEditorRes.result().body).role).toBe("editor");

      const crossPortEditorReq = makeMockIncomingMessage({
        headers: {
          host: "127.0.0.1:5173",
          origin: "http://127.0.0.1:9999",
          "sec-fetch-site": "same-origin",
        },
        body: JSON.stringify({ role: "editor" }),
      });
      const crossPortEditorRes = makeMockServerResponse();
      await handleBootstrapSession(crossPortEditorReq, crossPortEditorRes.res);
      expect(crossPortEditorRes.result().status).toBe(403);
      expect(JSON.parse(crossPortEditorRes.result().body).code).toBe("ORIGIN_REJECTED");
    } finally {
      setBoundBffPort(null);
    }

    const daemonLock = await ensureRunningDaemon();
    if (daemonLock.daemon_nonce) {
      const nonceReq = makeMockIncomingMessage({
        headers: { "x-cowok-daemon-nonce": daemonLock.daemon_nonce },
        body: JSON.stringify({ role: "editor" }),
      });
      const nonceRes = makeMockServerResponse();
      await handleBootstrapSession(nonceReq, nonceRes.res);
      expect(nonceRes.result().status).toBe(200);
      expect(JSON.parse(nonceRes.result().body).role).toBe("editor");
    }
  });

  it("requires authenticated strict-loopback session on matched proposals/snapshots/specs BFF routes", async () => {
    const paths = [
      ["GET", "/api/projects/00000000-0000-4000-8000-000000000001/specs"],
      ["POST", "/api/projects/00000000-0000-4000-8000-000000000001/specs/synthesize"],
      ["POST", "/api/projects/00000000-0000-4000-8000-000000000001/proposals"],
      ["GET", "/api/projects/00000000-0000-4000-8000-000000000001/snapshots"],
      ["POST", "/api/projects/00000000-0000-4000-8000-000000000001/sdi/intent"],
      ["POST", "/api/projects/00000000-0000-4000-8000-000000000001/context-slice"],
    ] as const;

    for (const [method, pathname] of paths) {
      const matched = matchBffRoute(method, pathname);
      expect(matched).not.toBeNull();
      const unauthReq = new Request(`http://127.0.0.1:5173${pathname}`, {
        method,
        headers: {
          Host: "127.0.0.1:5173",
          "Content-Type": "application/json",
          "x-tenant-id": "attacker-spoof",
        },
        body: method === "POST" ? JSON.stringify({}) : undefined,
      });
      const res = await matched!.handler(unauthReq, { params: matched!.params });
      expect(res.status).toBe(401);
    }
  });

  it("exercises full authenticated BFF -> Rust Daemon lifecycle via HarnessDaemonClient + real PKZIP download", async () => {
    const projectId = crypto.randomUUID();
    const { sessionToken } = issueBffLoopbackSession({
      tenantId: "local-default",
      userId: "sdk-e2e-user",
      role: "editor",
    });

    const bffFetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      const rawUrl = typeof input === "string" ? input : input.toString();
      const url = new URL(rawUrl, "http://127.0.0.1:5173");
      const method = (init?.method || "GET").toUpperCase();
      if (url.pathname === "/api/auth/local-session") {
        return new Response(JSON.stringify({ sessionToken, tenantId: "local-default", userId: "sdk-e2e-user", role: "editor" }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      const matched = matchBffRoute(method, url.pathname);
      if (!matched) {
        return new Response(JSON.stringify({ error: "NOT_FOUND" }), { status: 404 });
      }
      const headers = new Headers(init?.headers);
      headers.set("Host", "127.0.0.1:5173");
      const req = new Request(url.toString(), { method, headers, body: init?.body });
      return matched.handler(req, { params: matched.params });
    };

    const client = new HarnessDaemonClient({
      baseUrl: "http://127.0.0.1:5173",
      fetchImpl: bffFetch as unknown as typeof fetch,
    });

    const created = await client.createProject({
      id: projectId,
      slug: `sdk-e2e-${Date.now()}`,
      name: "SDK E2E Project",
      product_brief: "Complete enterprise design workbench with persistent SQLite WAL specs and ZIP exports.",
    });
    expect(created.id).toBe(projectId);

    const synth = await client.synthesizeSpecs(projectId, {});
    expect(synth.specs).toHaveLength(9);

    const specsList = await client.listSpecs(projectId);
    expect(specsList.specs).toHaveLength(9);
    const overview = specsList.specs.find((s) => s.spec_key === "01-product-overview.md")!;
    expect(overview.version).toBeGreaterThanOrEqual(1);

    const updated = await client.putSpec(projectId, "01-product-overview.md", {
      markdown_content: "# 01-product-overview\n- [CONFIRMED] Updated via typed SDK\n",
      expected_sha256: overview.content_sha256,
      status: "CONFIRMED",
    });
    expect(updated.version).toBe(overview.version + 1);

    // OCC stale SHA rejection -> HarnessDaemonApiError(409)
    await expect(
      client.putSpec(projectId, "01-product-overview.md", {
        markdown_content: "# Stale write",
        expected_sha256: overview.content_sha256,
      })
    ).rejects.toBeInstanceOf(HarnessDaemonApiError);

    const delRes = await client.deleteSpec(projectId, "09-data-and-copy.md");
    expect(delRes).toEqual({ deleted: true, spec_key: "09-data-and-copy.md" });

    // Real PKZIP export via BFF strict-loopback forwarder
    const exportMatched = matchBffRoute("GET", `/api/projects/${projectId}/snapshots/export/download`);
    expect(exportMatched).not.toBeNull();
    const zipReq = new Request(`http://127.0.0.1:5173/api/projects/${projectId}/snapshots/export/download`, {
      method: "GET",
      headers: {
        Host: "127.0.0.1:5173",
        "X-Cowok-Session-Token": sessionToken,
      },
    });
    const zipRes = await exportMatched!.handler(zipReq, { params: exportMatched!.params });
    expect(zipRes.status).toBe(200);
    expect(zipRes.headers.get("content-type")).toBe("application/zip");
    const zipBytes = new Uint8Array(await zipRes.arrayBuffer());
    expect(zipBytes.length).toBeGreaterThan(22);
    expect(zipBytes[0]).toBe(0x50); // 'P'
    expect(zipBytes[1]).toBe(0x4b); // 'K'
  });

  it("preserves JSON request bodies end-to-end on mutating proxyRoute endpoints", async () => {
    const projectId = crypto.randomUUID();
    const { sessionToken } = issueBffLoopbackSession({
      tenantId: "local-default",
      userId: "proxy-body-e2e",
      role: "editor",
    });

    const invokeBff = async (method: string, pathname: string, body?: unknown) => {
      const matched = matchBffRoute(method, pathname);
      expect(matched).not.toBeNull();
      const req = new Request(`http://127.0.0.1:5173${pathname}`, {
        method,
        headers: {
          Host: "127.0.0.1:5173",
          "Content-Type": "application/json",
          "X-Cowok-Session-Token": sessionToken,
        },
        body: body !== undefined ? JSON.stringify(body) : undefined,
      });
      return matched!.handler(req, { params: matched!.params });
    };

    const createRes = await invokeBff("POST", "/api/projects", {
      id: projectId,
      slug: `proxy-body-${Date.now()}`,
      name: "Proxy Body Preservation Project",
      product_brief: "Verifies mutating BFF proxy routes preserve JSON bodies.",
    });
    expect(createRes.status).toBe(201);

    const agentRes = await invokeBff("PUT", "/api/agent-settings", {
      projectId,
      providerId: "anthropic",
      modelId: "claude-sonnet-4-5",
      endpointUrl: "https://api.anthropic.com",
      credentialRef: "env:ANTHROPIC_API_KEY",
      isDefault: true,
      status: "configured",
    });
    expect(agentRes.status).toBe(200);
    const agentBody = (await agentRes.json()) as { providerId: string; modelId: string };
    expect(agentBody.providerId).toBe("anthropic");
    expect(agentBody.modelId).toBe("claude-sonnet-4-5");

    const mcpRes = await invokeBff("POST", "/api/mcp-servers", {
      projectId,
      serverSlug: `playwright-${Date.now()}`,
      displayName: "Playwright MCP",
      transportKind: "stdio",
      commandOrUrl: "npx @playwright/mcp@latest",
      permissionTier: "read_only",
      enabled: true,
    });
    expect([200, 201]).toContain(mcpRes.status);
    const mcpBody = (await mcpRes.json()) as { displayName: string; commandOrUrl: string };
    expect(mcpBody.displayName).toBe("Playwright MCP");
    expect(mcpBody.commandOrUrl).toBe("npx @playwright/mcp@latest");

    const verifyRes = await invokeBff("POST", "/api/doctor/verify-key", {
      providerId: "anthropic",
      apiKey: "sk-ant-test-12345678901234567890",
      endpointUrl: "",
    });
    expect(verifyRes.status).toBe(200);
    const verifyBody = (await verifyRes.json()) as { providerId: string; valid: boolean };
    expect(verifyBody.providerId).toBe("anthropic");
    expect(verifyBody.valid).toBe(true);

    const jobEventRes = await invokeBff("POST", `/api/projects/${projectId}/flow/jobs/events`, {
      jobId: `job-${Date.now()}`,
      nodeId: "node-login",
      eventKind: "STEP_COMPLETED",
      payloadText: "Login screen synthesized",
      jobStatus: "RUNNING",
    });
    expect([200, 201]).toContain(jobEventRes.status);
    const jobEventBody = (await jobEventRes.json()) as { nodeId: string; eventKind: string; payloadText: string };
    expect(jobEventBody.nodeId).toBe("node-login");
    expect(jobEventBody.eventKind).toBe("STEP_COMPLETED");
    expect(jobEventBody.payloadText).toBe("Login screen synthesized");
  });
});
