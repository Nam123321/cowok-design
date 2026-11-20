// @story 06.3
// @implements AC-02, AC-04
import { guardedRoute } from "@/lib/api-guard";
import { forwardFlowProjectRequest } from "@/lib/flow-bff-forward";
import { StreamJobEventsQuerySchema } from "@/lib/validations/flow-execution";

export const GET = guardedRoute(
  (req, ctx) => forwardFlowProjectRequest({ method: "GET", req, ctx, suffix: "flow/jobs/stream" }),
  { authMode: "strict-loopback", roles: ["viewer", "editor", "admin"], query: StreamJobEventsQuerySchema }
);
