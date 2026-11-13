// @story 01.3, 04.1, 04.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import { z, type ZodSchema } from "zod";
import { guardedRoute } from "./api-guard";
import { forwardToDaemonFlow } from "./daemon-client";
import { ApplyProposalSchema, CreateProposalSchema } from "./validations/change-proposal";
import {
  CreateSnapshotSchema,
  ExportZipSchema,
  GitHubPRSchema,
  RevertSnapshotSchema,
} from "./validations/version-snapshot";
import { ClarificationSelectSchema, CompileIntentSchema } from "./validations/sdi";
import {
  LintSourceInputSchema,
  ProjectDesignContractsSchema,
  ValidateComponentInputSchema,
  ValidateIconInputSchema,
  ValidateTypographyInputSchema,
} from "./validations/constraints";
import { UpdateElementCommentSchema } from "./validations/element-comment";
import {
  UpdateScreenFixtureSchema,
  ViewportPresetSchema,
} from "./validations/screen-fixture";
import * as snapshotRoute from "../app/api/projects/[id]/flow/snapshot/route";
import * as nodesRoute from "../app/api/projects/[id]/flow/nodes/route";
import * as positionsRoute from "../app/api/projects/[id]/flow/nodes/positions/route";
import * as singleNodeRoute from "../app/api/projects/[id]/flow/nodes/[nodeId]/route";
import * as transitionsRoute from "../app/api/projects/[id]/flow/transitions/route";
import * as singleTransitionRoute from "../app/api/projects/[id]/flow/transitions/[transitionId]/route";
import * as deployDraftRoute from "../app/api/projects/[id]/flow/deploy-draft/route";
import * as projectListRoute from "../app/api/projects/route";
import * as projectSessionsRoute from "../app/api/projects/[id]/sessions/route";
import * as singleSessionRoute from "../app/api/projects/[id]/sessions/[sessionId]/route";
import * as pathTokenRoute from "../app/api/projects/[id]/path-token/route";
import * as projectSpecsRoute from "../app/api/projects/[id]/specs/route";
import * as singleSpecRoute from "../app/api/projects/[id]/specs/[spec_key]/route";
import * as synthesizeSpecsRoute from "../app/api/projects/[id]/specs/synthesize/route";
import * as contextSliceRoute from "../app/api/projects/[id]/context-slice/route";
import * as commentsRoute from "../app/api/projects/[id]/flow/nodes/[nodeId]/comments/route";
import * as fixturesRoute from "../app/api/projects/[id]/flow/nodes/[nodeId]/fixtures/route";
import * as doctorRoute from "../app/api/doctor/route";

export type RouteFn = (req: Request, ctx: { params?: Record<string, string> }) => Promise<Response>;

interface RouteEntry {
  method: string;
  pattern: RegExp;
  extractParams: (match: RegExpExecArray) => Record<string, string>;
  resolveHandler: (match: RegExpExecArray) => RouteFn;
}

function forwardGuardedDaemon<B = unknown>(
  daemonPath: string,
  options: { body?: ZodSchema<B> } = {}
): RouteFn {
  return guardedRoute(
    async (req, ctx, { body }) =>
      forwardToDaemonFlow({
        method: req.method,
        daemonPath,
        rawQuery: new URL(req.url).search.slice(1),
        body,
        inboundHeaders: req.headers,
        principal: {
          tenantId: ctx.tenantId || "local-default",
          userId: ctx.userId || "local-operator",
          role: ctx.role || "viewer",
        },
        correlationId: ctx.correlationId,
      }),
    { authMode: "strict-loopback", ...options }
  );
}

function proxyRoute<B = unknown>(
  method: string,
  pattern: RegExp,
  buildDaemonPath: (m: RegExpExecArray) => string,
  body?: ZodSchema<B>
): RouteEntry {
  return {
    method,
    pattern,
    extractParams: (m): Record<string, string> => (m[1] ? { id: m[1] } : {}),
    resolveHandler: (m) => forwardGuardedDaemon(buildDaemonPath(m), { body }),
  };
}

const ViewportPatchSchema = z.object({ viewportPreset: ViewportPresetSchema });
const ProjectPatchSchema = z.object({
  name: z.string().min(1).max(160).optional(),
  product_brief: z.string().max(4000).optional(),
});

const idOnly = (m: RegExpExecArray): Record<string, string> => ({ id: m[1] });
const noParams = (): Record<string, string> => ({});

const ROUTE_TABLE: RouteEntry[] = [
  { method: "GET", pattern: /^\/api\/projects$/, extractParams: noParams, resolveHandler: () => projectListRoute.GET },
  { method: "POST", pattern: /^\/api\/projects$/, extractParams: noParams, resolveHandler: () => projectListRoute.POST },
  proxyRoute("PATCH", /^\/api\/projects\/([^/]+)$/, (m) => `/api/v1/projects/${m[1]}`, ProjectPatchSchema),
  proxyRoute("DELETE", /^\/api\/projects\/([^/]+)$/, (m) => `/api/v1/projects/${m[1]}`),
  { method: "GET", pattern: /^\/api\/doctor$/, extractParams: noParams, resolveHandler: () => doctorRoute.GET },
  proxyRoute("POST", /^\/api\/doctor\/verify-key$/, () => "/api/v1/doctor/verify-key"),
  proxyRoute("GET", /^\/api\/agent-settings$/, () => "/api/v1/agent-settings"),
  proxyRoute("PUT", /^\/api\/agent-settings$/, () => "/api/v1/agent-settings"),
  proxyRoute("GET", /^\/api\/mcp-servers$/, () => "/api/v1/mcp-servers"),
  proxyRoute("POST", /^\/api\/mcp-servers$/, () => "/api/v1/mcp-servers"),
  proxyRoute("DELETE", /^\/api\/mcp-servers\/([^/]+)$/, (m) => `/api/v1/mcp-servers/${m[1]}`),

  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/sessions$/, extractParams: idOnly, resolveHandler: () => projectSessionsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/sessions$/, extractParams: idOnly, resolveHandler: () => projectSessionsRoute.POST },
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/sessions\/([^/]+)$/, extractParams: (m) => ({ id: m[1], sessionId: m[2] }), resolveHandler: () => singleSessionRoute.GET },
  { method: "PUT", pattern: /^\/api\/projects\/([^/]+)\/sessions\/([^/]+)$/, extractParams: (m) => ({ id: m[1], sessionId: m[2] }), resolveHandler: () => singleSessionRoute.PUT },
  { method: "DELETE", pattern: /^\/api\/projects\/([^/]+)\/sessions\/([^/]+)$/, extractParams: (m) => ({ id: m[1], sessionId: m[2] }), resolveHandler: () => singleSessionRoute.DELETE },

  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/path-token$/, extractParams: idOnly, resolveHandler: () => pathTokenRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/context-slice$/, extractParams: idOnly, resolveHandler: () => contextSliceRoute.POST },

  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/specs$/, extractParams: idOnly, resolveHandler: () => projectSpecsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/specs\/synthesize$/, extractParams: idOnly, resolveHandler: () => synthesizeSpecsRoute.POST },
  { method: "PUT", pattern: /^\/api\/projects\/([^/]+)\/specs\/([^/]+)$/, extractParams: (m) => ({ id: m[1], spec_key: decodeURIComponent(m[2]) }), resolveHandler: () => singleSpecRoute.PUT },
  { method: "DELETE", pattern: /^\/api\/projects\/([^/]+)\/specs\/([^/]+)$/, extractParams: (m) => ({ id: m[1], spec_key: decodeURIComponent(m[2]) }), resolveHandler: () => singleSpecRoute.DELETE },

  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/flow\/snapshot$/, extractParams: idOnly, resolveHandler: () => snapshotRoute.GET },
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes$/, extractParams: idOnly, resolveHandler: () => nodesRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes$/, extractParams: idOnly, resolveHandler: () => nodesRoute.POST },
  { method: "PATCH", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/positions$/, extractParams: idOnly, resolveHandler: () => positionsRoute.PATCH },
  { method: "PATCH", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)$/, extractParams: (m) => ({ id: m[1], nodeId: m[2] }), resolveHandler: () => singleNodeRoute.PATCH },
  { method: "DELETE", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)$/, extractParams: (m) => ({ id: m[1], nodeId: m[2] }), resolveHandler: () => singleNodeRoute.DELETE },
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/flow\/transitions$/, extractParams: idOnly, resolveHandler: () => transitionsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/transitions$/, extractParams: idOnly, resolveHandler: () => transitionsRoute.POST },
  { method: "PATCH", pattern: /^\/api\/projects\/([^/]+)\/flow\/transitions\/([^/]+)$/, extractParams: (m) => ({ id: m[1], transitionId: m[2] }), resolveHandler: () => singleTransitionRoute.PATCH },
  { method: "DELETE", pattern: /^\/api\/projects\/([^/]+)\/flow\/transitions\/([^/]+)$/, extractParams: (m) => ({ id: m[1], transitionId: m[2] }), resolveHandler: () => singleTransitionRoute.DELETE },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/deploy-draft$/, extractParams: idOnly, resolveHandler: () => deployDraftRoute.POST },

  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/comments$/, extractParams: (m) => ({ id: m[1], nodeId: m[2] }), resolveHandler: () => commentsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/comments$/, extractParams: (m) => ({ id: m[1], nodeId: m[2] }), resolveHandler: () => commentsRoute.POST },
  proxyRoute("PATCH", /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/comments\/([^/]+)$/, (m) => `/api/v1/projects/${m[1]}/flow/nodes/${m[2]}/comments/${m[3]}`, UpdateElementCommentSchema),
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/fixtures$/, extractParams: (m) => ({ id: m[1], nodeId: m[2] }), resolveHandler: () => fixturesRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/fixtures$/, extractParams: (m) => ({ id: m[1], nodeId: m[2] }), resolveHandler: () => fixturesRoute.POST },
  proxyRoute("PATCH", /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/fixtures\/([^/]+)$/, (m) => `/api/v1/projects/${m[1]}/flow/nodes/${m[2]}/fixtures/${m[3]}`, UpdateScreenFixtureSchema),
  proxyRoute("PATCH", /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/viewport$/, (m) => `/api/v1/projects/${m[1]}/flow/nodes/${m[2]}/viewport`, ViewportPatchSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/flow\/jobs\/events$/, (m) => `/api/v1/projects/${m[1]}/flow/jobs/events`),
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/flow\/jobs\/stream$/, (m) => `/api/v1/projects/${m[1]}/flow/jobs/stream`),

  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/icon-manifest$/, (m) => `/api/v1/projects/${m[1]}/icon-manifest`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/validate-icon$/, (m) => `/api/v1/projects/${m[1]}/validate-icon`, ValidateIconInputSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/validate-typography$/, (m) => `/api/v1/projects/${m[1]}/validate-typography`, ValidateTypographyInputSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/validate-component$/, (m) => `/api/v1/projects/${m[1]}/validate-component`, ValidateComponentInputSchema),
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/design-contracts$/, (m) => `/api/v1/projects/${m[1]}/design-contracts`),
  proxyRoute("PUT", /^\/api\/projects\/([^/]+)\/design-contracts$/, (m) => `/api/v1/projects/${m[1]}/design-contracts`, ProjectDesignContractsSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/lint$/, (m) => `/api/v1/projects/${m[1]}/lint`, LintSourceInputSchema),
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/constraint-profile$/, (m) => `/api/v1/projects/${m[1]}/constraint-profile`),
  proxyRoute("PUT", /^\/api\/projects\/([^/]+)\/constraint-profile$/, (m) => `/api/v1/projects/${m[1]}/constraint-profile`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/inject-ff-id$/, (m) => `/api/v1/projects/${m[1]}/inject-ff-id`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/surgical-patch$/, (m) => `/api/v1/projects/${m[1]}/surgical-patch`),

  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/snapshots$/, (m) => `/api/v1/projects/${m[1]}/snapshots`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/snapshots$/, (m) => `/api/v1/projects/${m[1]}/snapshots`, CreateSnapshotSchema),
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/snapshots\/export\/download$/, (m) => `/api/v1/projects/${m[1]}/snapshots/export/download`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/snapshots\/export$/, (m) => `/api/v1/projects/${m[1]}/snapshots/export`, ExportZipSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/snapshots\/pr$/, (m) => `/api/v1/projects/${m[1]}/snapshots/pr`, GitHubPRSchema),
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/snapshots\/([^/]+)$/, (m) => `/api/v1/projects/${m[1]}/snapshots/${m[2]}`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/snapshots\/([^/]+)\/revert$/, (m) => `/api/v1/projects/${m[1]}/snapshots/${m[2]}/revert`, RevertSnapshotSchema),

  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/proposals$/, (m) => `/api/v1/projects/${m[1]}/proposals`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/proposals$/, (m) => `/api/v1/projects/${m[1]}/proposals`, CreateProposalSchema),
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)$/, (m) => `/api/v1/projects/${m[1]}/proposals/${m[2]}`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)\/validate$/, (m) => `/api/v1/projects/${m[1]}/proposals/${m[2]}/validate`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)\/apply$/, (m) => `/api/v1/projects/${m[1]}/proposals/${m[2]}/apply`, ApplyProposalSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)\/reject$/, (m) => `/api/v1/projects/${m[1]}/proposals/${m[2]}/reject`),

  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/sdi\/intent$/, (m) => `/api/v1/projects/${m[1]}/sdi/intent`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/sdi\/intent$/, (m) => `/api/v1/projects/${m[1]}/sdi/intent`, CompileIntentSchema),
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/sdi\/intent\/([^/]+)$/, (m) => `/api/v1/projects/${m[1]}/sdi/intent/${m[2]}`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/sdi\/clarification$/, (m) => `/api/v1/projects/${m[1]}/sdi/clarification`, ClarificationSelectSchema),
];

export function matchBffRoute(
  method: string,
  pathname: string
): { handler: RouteFn; params: Record<string, string> } | null {
  const normalizedPath = pathname.startsWith("/api/v1/")
    ? `/api/${pathname.slice("/api/v1/".length)}`
    : pathname;
  for (const entry of ROUTE_TABLE) {
    if (entry.method !== method) continue;
    const match = entry.pattern.exec(normalizedPath);
    if (match) {
      return {
        handler: entry.resolveHandler(match),
        params: entry.extractParams(match),
      };
    }
  }
  return null;
}
