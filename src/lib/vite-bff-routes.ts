// @story 01.3, 04.1, 04.2, 04.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import { z } from "zod";
import {
  ProbeCliInputSchema,
  UpsertAgentSettingInputSchema,
  UpsertMcpServerInputSchema,
  UpsertVaultKeyInputSchema,
  VaultConfigInputSchema,
  VerifyProviderKeyInputSchema,
} from "./validations/workbench-settings";
import {
  FLOW_AND_WORKBENCH_ROUTES,
  proxyRoute,
  safeDecodeSegment,
  safeEncodeSegment,
  type RouteEntry,
  type RouteFn,
} from "./vite-bff-flow-routes";
import * as doctorRoute from "../app/api/doctor/route";
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
import * as versionSnapshotsRoute from "../app/api/projects/[id]/snapshots/route";
import * as exportSnapshotRoute from "../app/api/projects/[id]/snapshots/export/route";
import * as downloadSnapshotRoute from "../app/api/projects/[id]/snapshots/export/download/route";
import * as snapshotPrRoute from "../app/api/projects/[id]/snapshots/pr/route";
import * as revertSnapshotRoute from "../app/api/projects/[id]/snapshots/[snapshotId]/revert/route";
import * as proposalsRoute from "../app/api/projects/[id]/proposals/route";
import * as singleProposalRoute from "../app/api/projects/[id]/proposals/[proposalId]/route";
import * as validateProposalRoute from "../app/api/projects/[id]/proposals/[proposalId]/validate/route";
import * as applyProposalRoute from "../app/api/projects/[id]/proposals/[proposalId]/apply/route";
import * as rejectProposalRoute from "../app/api/projects/[id]/proposals/[proposalId]/reject/route";
import * as sdiIntentRoute from "../app/api/projects/[id]/sdi/intent/route";
import * as sdiClarificationRoute from "../app/api/projects/[id]/sdi/clarification/route";

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
  proxyRoute("GET", /^\/api\/doctor\/cli-matrix$/, () => "/api/v1/doctor/cli-matrix"),
  proxyRoute("GET", /^\/api\/doctor\/clis$/, () => "/api/v1/doctor/clis"),
  proxyRoute("GET", /^\/api\/doctor\/usage$/, () => "/api/v1/doctor/usage"),
  proxyRoute("POST", /^\/api\/doctor\/probe-cli$/, () => "/api/v1/doctor/probe-cli", ProbeCliInputSchema),
  proxyRoute("GET", /^\/api\/plugins$/, () => "/api/v1/plugins"),
  proxyRoute("GET", /^\/api\/plugins\/registry$/, () => "/api/v1/plugins/registry"),
  proxyRoute("POST", /^\/api\/doctor\/verify-key$/, () => "/api/v1/doctor/verify-key", VerifyProviderKeyInputSchema),
  proxyRoute("GET", /^\/api\/vault\/keys$/, () => "/api/v1/vault/keys"),
  proxyRoute("POST", /^\/api\/vault\/keys$/, () => "/api/v1/vault/keys", UpsertVaultKeyInputSchema),
  proxyRoute("DELETE", /^\/api\/vault\/keys\/([^/]+)$/, (m) => `/api/v1/vault/keys/${safeEncodeSegment(m[1])}`),
  proxyRoute("PUT", /^\/api\/settings\/vault$/, () => "/api/v1/settings/vault", VaultConfigInputSchema),
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

  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/snapshots$/, extractParams: idOnly, resolveHandler: () => versionSnapshotsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/snapshots$/, extractParams: idOnly, resolveHandler: () => versionSnapshotsRoute.POST },
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/snapshots\/export\/download$/, extractParams: idOnly, resolveHandler: () => downloadSnapshotRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/snapshots\/export$/, extractParams: idOnly, resolveHandler: () => exportSnapshotRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/snapshots\/pr$/, extractParams: idOnly, resolveHandler: () => snapshotPrRoute.POST },
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/snapshots\/([^/]+)$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/snapshots/${safeEncodeSegment(m[2])}`),
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/snapshots\/([^/]+)\/revert$/, extractParams: (m) => ({ id: m[1], snapshotId: m[2] }), resolveHandler: () => revertSnapshotRoute.POST },

  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/proposals$/, extractParams: idOnly, resolveHandler: () => proposalsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/proposals$/, extractParams: idOnly, resolveHandler: () => proposalsRoute.POST },
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)$/, extractParams: (m) => ({ id: m[1], proposalId: m[2] }), resolveHandler: () => singleProposalRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)\/validate$/, extractParams: (m) => ({ id: m[1], proposalId: m[2] }), resolveHandler: () => validateProposalRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)\/apply$/, extractParams: (m) => ({ id: m[1], proposalId: m[2] }), resolveHandler: () => applyProposalRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/proposals\/([^/]+)\/reject$/, extractParams: (m) => ({ id: m[1], proposalId: m[2] }), resolveHandler: () => rejectProposalRoute.POST },

  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/sdi\/intent$/, extractParams: idOnly, resolveHandler: () => sdiIntentRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/sdi\/intent$/, extractParams: idOnly, resolveHandler: () => sdiIntentRoute.POST },
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/sdi\/intent\/([^/]+)$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/sdi/intent/${safeEncodeSegment(m[2])}`),
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/sdi\/clarification$/, extractParams: idOnly, resolveHandler: () => sdiClarificationRoute.POST },
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
