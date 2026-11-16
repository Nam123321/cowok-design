// @story 04.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFlowProjectRequest } from "@/lib/flow-bff-forward";
import { MarkNodeSessionReadInputSchema } from "@/lib/validations/flow-execution";

export const POST = guardedRoute(
  (req, ctx, { body }) => forwardFlowProjectRequest({ method: "POST", req, ctx, suffix: "flow/node-sessions/read", body }),
  { authMode: "strict-loopback", body: MarkNodeSessionReadInputSchema }
);
