// @story 01.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07
import { Readable } from "node:stream";
import type { IncomingMessage, ServerResponse } from "node:http";
import { describe, expect, it, vi } from "vitest";
import { logger } from "../../src/lib/logger";
import { issueBffLoopbackSession } from "../../src/lib/api-guard";
import { handleBootstrapSession } from "../../src/lib/vite-bff-plugin";
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

  it("blocks admin role escalation & malformed JSON on /api/auth/local-session while issuing viewer/editor tokens", async () => {
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

    const editorReq = makeMockIncomingMessage({ body: JSON.stringify({ role: "editor" }) });
    const editorRes = makeMockServerResponse();
    await handleBootstrapSession(editorReq, editorRes.res);
    expect(editorRes.result().status).toBe(200);
    const issued = JSON.parse(editorRes.result().body) as { sessionToken: string; role: string };
    expect(issued.role).toBe("editor");
    expect(typeof issued.sessionToken).toBe("string");
    expect(issued.sessionToken.length).toBeGreaterThan(10);
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
});
