// @story 04.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFlowProjectRequest } from "@/lib/flow-bff-forward";
import { SteerNodeJobInputSchema } from "@/lib/validations/flow-execution";

export const POST = guardedRoute(
  (req, ctx, { body }) => forwardFlowProjectRequest({ method: "POST", req, ctx, suffix: "flow/jobs/steer", body }),
  { authMode: "strict-loopback", body: SteerNodeJobInputSchema }
);
