// @story 04.4
// @implements AC-01..AC-06 (Vite BFF Flow & Workbench Route Subtable <= 250 LOC)
import { type ZodSchema } from "zod";
import { guardedRoute } from "./api-guard";
import { forwardToDaemonFlow } from "./daemon-client";
import {
  LintSourceInputSchema,
  ProjectDesignContractsSchema,
  ValidateComponentInputSchema,
  ValidateIconInputSchema,
  ValidateTypographyInputSchema,
} from "./validations/constraints";
import { PostJobEventV2InputSchema } from "./validations/flow-execution";
import { UpdateElementCommentSchema } from "./validations/element-comment";
import { UpdateScreenFixtureSchema, ViewportPresetSchema } from "./validations/screen-fixture";
import { z } from "zod";
import * as commentsRoute from "../app/api/projects/[id]/flow/nodes/[nodeId]/comments/route";
import * as fixturesRoute from "../app/api/projects/[id]/flow/nodes/[nodeId]/fixtures/route";
import * as flowEnqueueRoute from "../app/api/projects/[id]/flow/jobs/enqueue/route";
import * as flowSteerRoute from "../app/api/projects/[id]/flow/jobs/steer/route";
import * as flowPauseRoute from "../app/api/projects/[id]/flow/jobs/pause/route";
import * as flowResumeRoute from "../app/api/projects/[id]/flow/jobs/resume/route";
import * as flowCancelRoute from "../app/api/projects/[id]/flow/jobs/cancel/route";
import * as flowNodeSessionsRoute from "../app/api/projects/[id]/flow/node-sessions/route";
import * as flowNodeSessionsReadRoute from "../app/api/projects/[id]/flow/node-sessions/read/route";

export type RouteFn = (req: Request, ctx: { params?: Record<string, string> }) => Promise<Response>;
export interface RouteEntry {
  method: string;
  pattern: RegExp;
  extractParams: (match: RegExpExecArray) => Record<string, string>;
  resolveHandler: (match: RegExpExecArray) => RouteFn;
}

export function safeEncodeSegment(raw: string): string {
  try {
    return encodeURIComponent(decodeURIComponent(raw));
  } catch {
    return encodeURIComponent(raw);
  }
}

export function safeDecodeSegment(raw: string): string {
  try {
    return decodeURIComponent(raw);
  } catch {
    return raw;
  }
}

export function forwardGuardedDaemon<B = unknown>(
  daemonPath: string,
  options: { body?: ZodSchema<B>; rawJsonPassthrough?: boolean } = {}
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
          role: ctx.role || (req.method === "GET" ? "viewer" : "editor"),
        },
        correlationId: ctx.correlationId,
        bffSessionToken: ctx.bffSessionToken,
        isHumanWorkbench: ctx.isHumanWorkbench,
        humanSessionToken: ctx.humanSessionToken,
      }),
    { authMode: "strict-loopback", ...options }
  );
}

export function proxyRoute<B = unknown>(
  method: string,
  pattern: RegExp,
  buildDaemonPath: (m: RegExpExecArray) => string,
  bodyOrOptions?: ZodSchema<B> | { body?: ZodSchema<B>; rawJsonPassthrough?: boolean }
): RouteEntry {
  const options =
    bodyOrOptions && "parse" in bodyOrOptions
      ? { body: bodyOrOptions }
      : (bodyOrOptions as { body?: ZodSchema<B>; rawJsonPassthrough?: boolean } | undefined) || {};
  return {
    method,
    pattern,
    extractParams: (m): Record<string, string> => (m[1] ? { id: m[1] } : {}),
    resolveHandler: (m) => forwardGuardedDaemon(buildDaemonPath(m), options),
  };
}

const idOnly = (m: RegExpExecArray): Record<string, string> => ({ id: m[1] });
const noParams = (): Record<string, string> => ({});
const ViewportPatchSchema = z.object({ viewportPreset: ViewportPresetSchema });
const ConstraintProfileInputSchema = z.object({
  id: z.string().uuid(),
  project_id: z.string().uuid(),
  max_complexity_score: z.number().int().nonnegative(),
  enforce_clean_code: z.boolean(),
  forbidden_patterns: z.array(z.string()),
  target_framework: z.string(),
});
const AstInjectionInputSchema = z.object({ source: z.string(), component_name: z.string().nullable().optional() });
const SurgicalPatchInputSchema = z.object({
  ff_id: z.string().min(1),
  patch_kind: z.enum(["text_content", "metric_value", "css_variable", "attribute_value"]),
  new_value: z.string(),
  source: z.string(),
  target_attribute: z.string().nullable().optional(),
});

export const FLOW_AND_WORKBENCH_ROUTES: RouteEntry[] = [
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/comments$/, extractParams: (m) => ({ id: m[1], nodeId: m[2] }), resolveHandler: () => commentsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/comments$/, extractParams: (m) => ({ id: m[1], nodeId: m[2] }), resolveHandler: () => commentsRoute.POST },
  proxyRoute("PATCH", /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/comments\/([^/]+)$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/flow/nodes/${safeEncodeSegment(m[2])}/comments/${safeEncodeSegment(m[3])}`, UpdateElementCommentSchema),
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/fixtures$/, extractParams: (m) => ({ id: m[1], nodeId: m[2] }), resolveHandler: () => fixturesRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/fixtures$/, extractParams: (m) => ({ id: m[1], nodeId: m[2] }), resolveHandler: () => fixturesRoute.POST },
  proxyRoute("PATCH", /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/fixtures\/([^/]+)$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/flow/nodes/${safeEncodeSegment(m[2])}/fixtures/${safeEncodeSegment(m[3])}`, UpdateScreenFixtureSchema),
  proxyRoute("PATCH", /^\/api\/projects\/([^/]+)\/flow\/nodes\/([^/]+)\/viewport$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/flow/nodes/${safeEncodeSegment(m[2])}/viewport`, ViewportPatchSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/flow\/jobs\/events$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/flow/jobs/events`, PostJobEventV2InputSchema),
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/flow\/jobs\/stream$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/flow/jobs/stream`),
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/jobs\/enqueue$/, extractParams: idOnly, resolveHandler: () => flowEnqueueRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/jobs\/steer$/, extractParams: idOnly, resolveHandler: () => flowSteerRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/jobs\/pause$/, extractParams: idOnly, resolveHandler: () => flowPauseRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/jobs\/resume$/, extractParams: idOnly, resolveHandler: () => flowResumeRoute.POST },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/jobs\/cancel$/, extractParams: idOnly, resolveHandler: () => flowCancelRoute.POST },
  { method: "GET", pattern: /^\/api\/projects\/([^/]+)\/flow\/node-sessions$/, extractParams: idOnly, resolveHandler: () => flowNodeSessionsRoute.GET },
  { method: "POST", pattern: /^\/api\/projects\/([^/]+)\/flow\/node-sessions\/read$/, extractParams: idOnly, resolveHandler: () => flowNodeSessionsReadRoute.POST },
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/icon-manifest$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/icon-manifest`),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/validate-icon$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/validate-icon`, ValidateIconInputSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/validate-typography$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/validate-typography`, ValidateTypographyInputSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/validate-component$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/validate-component`, ValidateComponentInputSchema),
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/design-contracts$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/design-contracts`),
  proxyRoute("PUT", /^\/api\/projects\/([^/]+)\/design-contracts$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/design-contracts`, ProjectDesignContractsSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/lint$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/lint`, LintSourceInputSchema),
  proxyRoute("GET", /^\/api\/projects\/([^/]+)\/constraint-profile$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/constraint-profile`),
  proxyRoute("PUT", /^\/api\/projects\/([^/]+)\/constraint-profile$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/constraint-profile`, ConstraintProfileInputSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/inject-ff-id$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/inject-ff-id`, AstInjectionInputSchema),
  proxyRoute("POST", /^\/api\/projects\/([^/]+)\/surgical-patch$/, (m) => `/api/v1/projects/${safeEncodeSegment(m[1])}/surgical-patch`, SurgicalPatchInputSchema),
];
