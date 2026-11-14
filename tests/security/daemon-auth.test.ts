// @story 04.1
// @implements AC-06
import { describe, expect, it } from "vitest";
import {
  getBffSessionRegistrySize,
  issueBffLoopbackSession,
  pruneExpiredBffSessions,
  setBoundBffPort,
} from "../../src/lib/api-guard";
import { DEFAULT_FLOW_PROJECT_ID, stripSpoofedHeaders } from "../../src/lib/daemon-client";
import * as snapshotRoute from "../../src/app/api/projects/[id]/flow/snapshot/route";
import * as nodesRoute from "../../src/app/api/projects/[id]/flow/nodes/route";

describe("Scoped Dual-Ingress Daemon & BFF Security Gate", () => {
  it("SEC-HEADER-01: rejects missing X-Cowok-Session-Token even if RFC 6265 Cookie is present", async () => {
    const req = new Request(`http://127.0.0.1:5173/api/projects/${DEFAULT_FLOW_PROJECT_ID}/flow/snapshot`, {
      method: "GET",
      headers: {
        Host: "127.0.0.1:5173",
        Cookie: "cowok_session=port_shared_cookie_attack",
      },
    });
    const res = await snapshotRoute.GET(req, { params: { id: DEFAULT_FLOW_PROJECT_ID } });
    expect(res.status).toBe(401);
    const body = await res.json();
    expect(body.code).toBe("UNAUTHENTICATED");
  });

  it("SEC-HEADER-02: rejects DNS Rebinding Host and cross-origin Origin / Sec-Fetch-Site headers", async () => {
    const session = issueBffLoopbackSession({ tenantId: "local-default", role: "editor" });

    const rebindReq = new Request(`http://127.0.0.1:5173/api/projects/${DEFAULT_FLOW_PROJECT_ID}/flow/snapshot`, {
      method: "GET",
      headers: {
        Host: "evil-rebind.attacker.com",
        "X-Cowok-Session-Token": session.sessionToken,
      },
    });
    const rebindRes = await snapshotRoute.GET(rebindReq, { params: { id: DEFAULT_FLOW_PROJECT_ID } });
    expect(rebindRes.status).toBe(403);

    const crossSiteReq = new Request(`http://127.0.0.1:5173/api/projects/${DEFAULT_FLOW_PROJECT_ID}/flow/snapshot`, {
      method: "GET",
      headers: {
        Host: "127.0.0.1:5173",
        Origin: "https://evil.example",
        "Sec-Fetch-Site": "cross-site",
        "X-Cowok-Session-Token": session.sessionToken,
      },
    });
    const crossSiteRes = await snapshotRoute.GET(crossSiteReq, { params: { id: DEFAULT_FLOW_PROJECT_ID } });
    expect(crossSiteRes.status).toBe(403);
  });

  it("SEC-HEADER-03: strips spoofed X-Cowok-*, X-Tenant-*, X-User-*, and X-Forwarded-* headers and blocks viewer mutations", async () => {
    const inbound = new Headers({
      "X-Cowok-Principal": "spoofed-principal",
      "X-Tenant-Id": "attacker-tenant",
      "X-User-Roles": "admin",
      "X-Forwarded-For": "1.2.3.4",
      Accept: "application/json",
    });
    const sanitized = stripSpoofedHeaders(inbound);
    expect(sanitized["x-cowok-principal"]).toBeUndefined();
    expect(sanitized["x-tenant-id"]).toBeUndefined();
    expect(sanitized["x-user-roles"]).toBeUndefined();
    expect(sanitized["x-forwarded-for"]).toBeUndefined();
    expect(sanitized.accept).toBe("application/json");

    const viewer = issueBffLoopbackSession({ tenantId: "local-default", role: "viewer" });
    const mutReq = new Request(`http://127.0.0.1:5173/api/projects/${DEFAULT_FLOW_PROJECT_ID}/flow/nodes`, {
      method: "POST",
      headers: {
        Host: "127.0.0.1:5173",
        "Content-Type": "application/json",
        "X-Cowok-Session-Token": viewer.sessionToken,
      },
      body: JSON.stringify({ screenKey: "blocked-view", title: "Blocked" }),
    });
    const mutRes = await nodesRoute.POST(mutReq, { params: { id: DEFAULT_FLOW_PROJECT_ID } });
    expect(mutRes.status).toBe(403);
    const body = await mutRes.json();
    expect(body.code).toBe("INSUFFICIENT_ROLE");
  });

  it("SEC-IDOR-01: cross-tenant session cannot read or mutate another tenant's project graph", async () => {
    const attackerSession = issueBffLoopbackSession({ tenantId: "tenant-intruder", role: "editor" });
    const req = new Request(`http://127.0.0.1:5173/api/projects/${DEFAULT_FLOW_PROJECT_ID}/flow/snapshot`, {
      method: "GET",
      headers: {
        Host: "127.0.0.1:5173",
        "X-Cowok-Session-Token": attackerSession.sessionToken,
      },
    });
    const res = await snapshotRoute.GET(req, { params: { id: DEFAULT_FLOW_PROJECT_ID } });
    expect(res.status).toBe(404);
  });

  it("SEC-ORIGIN-PORT: pins bound BFF port when configured and rejects cross-port callers", async () => {
    const session = issueBffLoopbackSession({ tenantId: "local-default", role: "editor" });
    setBoundBffPort(5173);
    try {
      const wrongPortReq = new Request(`http://127.0.0.1:9999/api/projects/${DEFAULT_FLOW_PROJECT_ID}/flow/snapshot`, {
        method: "GET",
        headers: {
          Host: "127.0.0.1:9999",
          "X-Cowok-Session-Token": session.sessionToken,
        },
      });
      const wrongPortRes = await snapshotRoute.GET(wrongPortReq, { params: { id: DEFAULT_FLOW_PROJECT_ID } });
      expect(wrongPortRes.status).toBe(403);
      const wrongPortBody = await wrongPortRes.json();
      expect(wrongPortBody.code).toBe("ORIGIN_REJECTED");

      const rightPortReq = new Request(`http://127.0.0.1:5173/api/projects/${DEFAULT_FLOW_PROJECT_ID}/flow/snapshot`, {
        method: "GET",
        headers: {
          Host: "127.0.0.1:5173",
          "X-Cowok-Session-Token": session.sessionToken,
        },
      });
      const rightPortRes = await snapshotRoute.GET(rightPortReq, { params: { id: DEFAULT_FLOW_PROJECT_ID } });
      expect(rightPortRes.status).toBe(200);
    } finally {
      setBoundBffPort(null);
    }
  });

  it("SEC-SESSION-CAP: evicts expired sessions and enforces registry capacity bounds", async () => {
    const expired = issueBffLoopbackSession({ tenantId: "local-default", role: "viewer", ttlMs: -1 });
    const expiredReq = new Request(`http://127.0.0.1:5173/api/projects/${DEFAULT_FLOW_PROJECT_ID}/flow/snapshot`, {
      method: "GET",
      headers: {
        Host: "127.0.0.1:5173",
        "X-Cowok-Session-Token": expired.sessionToken,
      },
    });
    const expiredRes = await snapshotRoute.GET(expiredReq, { params: { id: DEFAULT_FLOW_PROJECT_ID } });
    expect(expiredRes.status).toBe(401);

    const first = issueBffLoopbackSession({ tenantId: "local-default", role: "viewer", maxCapacity: 2 });
    const second = issueBffLoopbackSession({ tenantId: "local-default", role: "viewer", maxCapacity: 2 });
    const third = issueBffLoopbackSession({ tenantId: "local-default", role: "viewer", maxCapacity: 2 });
    expect(getBffSessionRegistrySize()).toBeLessThanOrEqual(2);
    expect(third.sessionToken).not.toBe(first.sessionToken);
    expect(second.sessionToken).not.toBe(third.sessionToken);

    const evictedReq = new Request(`http://127.0.0.1:5173/api/projects/${DEFAULT_FLOW_PROJECT_ID}/flow/snapshot`, {
      method: "GET",
      headers: {
        Host: "127.0.0.1:5173",
        "X-Cowok-Session-Token": first.sessionToken,
      },
    });
    const evictedRes = await snapshotRoute.GET(evictedReq, { params: { id: DEFAULT_FLOW_PROJECT_ID } });
    expect(evictedRes.status).toBe(401);
    expect(pruneExpiredBffSessions()).toBeGreaterThanOrEqual(0);
  });
});
