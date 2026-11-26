// @story 07.2, 10.3, 13.1, 14.3
// @implements Wave D BFF Route Subtable (Split Export, Design Tournament, Release Certification & Distribution Ops)
import { guardedRoute } from "./api-guard";
import { forwardToDaemonFlow } from "./daemon-client";
import type { RouteEntry, RouteFn } from "./vite-bff-flow-routes";

const noParams = (): Record<string, string> => ({});

function extractAdminAuthHeaders(headers: Headers): Record<string, string> {
  const extra: Record<string, string> = {};
  const auth = headers.get("authorization");
  if (auth) extra.Authorization = auth;
  const adminSess = headers.get("x-admin-session");
  if (adminSess) extra["X-Admin-Session"] = adminSess;
  return extra;
}

function forwardWaveD(daemonPath: string, forwardAdminHeaders = false): RouteFn {
  return guardedRoute(
    async (req, ctx) => {
      const url = new URL(req.url || "/", "http://127.0.0.1");
      const rawBody = req.method === "GET" || req.method === "HEAD" ? undefined : await req.text();
      let parsedBody: unknown = undefined;
      if (rawBody && rawBody.trim().length > 0) {
        try {
          parsedBody = JSON.parse(rawBody);
        } catch {
          parsedBody = rawBody;
        }
      }
      return forwardToDaemonFlow({
        method: req.method,
        daemonPath,
        rawQuery: url.search.replace(/^\?/, ""),
        body: parsedBody,
        inboundHeaders: req.headers,
        extraHeaders: forwardAdminHeaders ? extractAdminAuthHeaders(req.headers) : undefined,
        principal: {
          tenantId: ctx.tenantId || "local-default",
          userId: ctx.userId || "local-operator",
          role: ctx.role || "editor",
        },
        correlationId: ctx.correlationId,
        bffSessionToken: ctx.bffSessionToken,
        isHumanWorkbench: ctx.isHumanWorkbench,
        humanSessionToken: ctx.humanSessionToken,
        signal: req.signal,
      });
    },
    { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"] }
  );
}

function entry(method: string, pattern: RegExp, daemonPath: string, forwardAdminHeaders = false): RouteEntry {
  return {
    method,
    pattern,
    extractParams: noParams,
    resolveHandler: () => forwardWaveD(daemonPath, forwardAdminHeaders),
  };
}

export const WAVE_D_ROUTES: RouteEntry[] = [
  // Story 07.2 — Split Export & Packaging Engine
  entry("POST", /^\/api\/export\/html-bundle$/, "/api/v1/export/html-bundle"),
  entry("POST", /^\/api\/export\/vector-pdf$/, "/api/v1/export/vector-pdf"),
  entry("POST", /^\/api\/export\/pptx$/, "/api/v1/export/pptx"),
  entry("POST", /^\/api\/export\/developer-handoff$/, "/api/v1/export/developer-handoff"),
  entry("GET", /^\/api\/export\/history$/, "/api/v1/export/history"),

  // Story 10.3 — Anti-Slop Design Tournament, /crystallize & advisor 6-Step Loop
  entry("POST", /^\/api\/tournament\/start$/, "/api/v1/tournament/start"),
  entry("POST", /^\/api\/tournament\/select$/, "/api/v1/tournament/select"),
  entry("GET", /^\/api\/tournament\/runs$/, "/api/v1/tournament/runs"),
  entry("POST", /^\/api\/crystallize$/, "/api/v1/crystallize"),
  entry("POST", /^\/api\/advisor\/evolve$/, "/api/v1/advisor/evolve"),

  // Story 13.1 — Sealed Anti-Goodhart Benchmark & 25-Case M1..M5 Certification
  entry("POST", /^\/api\/eval\/run$/, "/api/v1/eval/run"),
  entry("GET", /^\/api\/eval\/history$/, "/api/v1/eval/history"),

  // Story 14.3 — Distribution Ops, Plugin Catalog Lifecycle, A/B Experiments & Offline Licenses
  entry("GET", /^\/api\/distribution-ops\/releases\/rollout$/, "/v1/admin/releases/rollout", true),
  entry("POST", /^\/api\/distribution-ops\/releases\/rollout$/, "/v1/admin/releases/rollout", true),
  entry("GET", /^\/api\/distribution-ops\/releases\/latest$/, "/v1/releases/latest.json", true),
  entry("GET", /^\/api\/distribution-ops\/skills\/catalog$/, "/v1/admin/skills/catalog", true),
  entry("POST", /^\/api\/distribution-ops\/skills\/catalog$/, "/v1/admin/skills/catalog", true),
  entry("POST", /^\/api\/distribution-ops\/skills\/catalog\/bulk$/, "/v1/admin/skills/catalog/bulk", true),
  entry("GET", /^\/api\/distribution-ops\/skills\/catalog-manifest$/, "/v1/skills/catalog.json", true),
  entry("GET", /^\/api\/distribution-ops\/experiments$/, "/v1/admin/experiments", true),
  entry("POST", /^\/api\/distribution-ops\/experiments$/, "/v1/admin/experiments", true),
  entry("GET", /^\/api\/distribution-ops\/experiments-manifest$/, "/v1/experiments/config.json", true),
  entry("GET", /^\/api\/distribution-ops\/skills\/quarantine$/, "/v1/admin/skills/quarantine-global", true),
  entry("POST", /^\/api\/distribution-ops\/skills\/quarantine$/, "/v1/admin/skills/quarantine-global", true),
  entry("GET", /^\/api\/distribution-ops\/skills\/advisories-manifest$/, "/v1/skills/advisories.json", true),
  entry("GET", /^\/api\/distribution-ops\/licenses$/, "/v1/admin/licenses/issue", true),
  entry("POST", /^\/api\/distribution-ops\/licenses$/, "/v1/admin/licenses/issue", true),
  entry("GET", /^\/api\/distribution-ops\/audit-logs$/, "/v1/admin/distribution/audit-logs", true),
  entry("POST", /^\/api\/distribution-ops\/edge\/sync$/, "/v1/client/edge/sync", true),
  entry("POST", /^\/api\/distribution-ops\/federation\/cowok-org$/, "/v1/federation/cowok-org", true),
];
