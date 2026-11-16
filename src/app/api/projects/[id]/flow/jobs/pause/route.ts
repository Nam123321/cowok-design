// @story 04.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFlowProjectRequest } from "@/lib/flow-bff-forward";
import { TransitionNodeJobInputSchema } from "@/lib/validations/flow-execution";

export const POST = guardedRoute(
  (req, ctx, { body }) => forwardFlowProjectRequest({ method: "POST", req, ctx, suffix: "flow/jobs/pause", body }),
  { authMode: "strict-loopback", body: TransitionNodeJobInputSchema }
);
