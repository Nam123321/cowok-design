// @story 01.3, 04.1, 04.2, 04.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import { z } from "zod";
import { UpsertAgentSettingInputSchema, UpsertMcpServerInputSchema, VerifyProviderKeyInputSchema } from "./validations/workbench-settings";
import {
  FLOW_AND_WORKBENCH_ROUTES,
  proxyRoute,
  safeDecodeSegment,
  safeEncodeSegment,
  type RouteEntry,
  type RouteFn,
} from "./vite-bff-flow-routes";
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
import * as fsBrowseRoute from "../app/api/fs/browse/route";
import * as projectRestoreRoute from "../app/api/projects/[id]/restore/route";
import * as verifyPathTokenRoute from "../app/api/projects/[id]/verify-path-token/route";
import * as projectPromoteRoute from "../app/api/projects/[id]/promote/route";
import * as projectSpecsRoute from "../app/api/projects/[id]/specs/route";
import * as singleSpecRoute from "../app/api/projects/[id]/specs/[spec_key]/route";
import * as synthesizeSpecsRoute from "../app/api/projects/[id]/specs/synthesize/route";
import * as contextSliceRoute from "../app/api/projects/[id]/context-slice/route";
import * as doctorRoute from "../app/api/doctor/route";

export type { RouteFn };

const ProjectPatchSchema = z.object({
  name: z.string().min(1).max(160).optional(),
  product_brief: z.string().max(4000).optional(),
});
const idOnly = (m: RegExpExecArray): Record<string, string> => ({ id: m[1] });
const noParams = (): Record<string, string> => ({});

const ROUTE_TABLE: RouteEntry[] = [
  { method: "GET", pattern: /^\/api\/projects$/, extractParams: noParams, resolveHandler: () => projectListRoute.GET },
  { method: "POST", pattern: /^\/api\/projects$/, extractParams: noParams, resolveHandler: () => projectListRoute.POST },
  proxyRoute("PATCH", /^\/api\/projects\/([^/]+)$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}`, ProjectPatchSchema),
  proxyRoute("DELETE", /^\/api\/projects\/([^/]+)$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}`),
  { method: "GET", pattern: /^\/api\/doctor$/, extractParams: noParams, resolveHandler: () => doctorRoute.GET },
  proxyRoute("POST", /^\/api\/doctor\/verify-key$/, () => "/api/v1/doctor/verify-key", VerifyProviderKeyInputSchema),
  proxyRoute("GET", /^\/api\/agent-settings$/, () => "/api/v1/agent-settings"),
  proxyRoute("PUT", /^\/api\/agent-settings$/, () => "/api/v1/agent-settings", UpsertAgentSettingInputSchema),
  proxyRoute("GET", /^\/api\/mcp-servers$/, () => "/api/v1/mcp-servers"),
  proxyRoute("POST", /^\/api\/mcp-servers$/, () => "/api/v1/mcp-servers", UpsertMcpServerInputSchema),
  proxyRoute("DELETE", /^\/api\/mcp-servers\/([^/]+)$/, (m) => `/api/v1/mcp-servers/${safeEncodeSegment(m[1])}`),

  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/sessions$/, extractParams: idOnly, resolveHandler: () => projectSessionsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/sessions$/, extractParams: idOnly, resolveHandler: () => projectSessionsRoute.POST },
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/sessions\/([^/]+)$/, extractParams: (m) => ({ id: m[1], sessionId: m[2] }), resolveHandler: () => singleSessionRoute.GET },
  { method: "PUT", pattern: /^\/api\/projects\/([^/]+)\/sessions\/([^/]+)$/, extractParams: (m) => ({ id: m[1], sessionId: m[2] }), resolveHandler: () => singleSessionRoute.PUT },
  { method: "DELETE", pattern: /^\/api\/projects\/([^/]+)\/sessions\/([^/]+)$/, extractParams: (m) => ({ id: m[1], sessionId: m[2] }), resolveHandler: () => singleSessionRoute.DELETE },

  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/path-token$/, extractParams: idOnly, resolveHandler: () => pathTokenRoute.POST },
  { method: "GET", pattern: /^\/api\/fs\/browse$/, extractParams: noParams, resolveHandler: () => fsBrowseRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/restore$/, extractParams: idOnly, resolveHandler: () => projectRestoreRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/verify-path-token$/, extractParams: idOnly, resolveHandler: () => verifyPathTokenRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/promote$/, extractParams: idOnly, resolveHandler: () => projectPromoteRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/context-slice$/, extractParams: idOnly, resolveHandler: () => contextSliceRoute.POST },

  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/specs$/, extractParams: idOnly, resolveHandler: () => projectSpecsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/specs\/synthesize$/, extractParams: idOnly, resolveHandler: () => synthesizeSpecsRoute.POST },
  { method: "PUT", pattern: /^\/api\/projects\/([^/]+)\/specs\/([^/]+)$/, extractParams: (m) => ({ id: m[1], spec_key: safeDecodeSegment(m[2]) }), resolveHandler: () => singleSpecRoute.PUT },
  { method: "DELETE", pattern: /^\/api\/projects\/([^/]+)\/specs\/([^/]+)$/, extractParams: (m) => ({ id: m[1], spec_key: safeDecodeSegment(m[2]) }), resolveHandler: () => singleSpecRoute.DELETE },

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
  ...FLOW_AND_WORKBENCH_ROUTES,
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
      return { handler: entry.resolveHandler(match), params: entry.extractParams(match) };
    }
  }
  return null;
}
