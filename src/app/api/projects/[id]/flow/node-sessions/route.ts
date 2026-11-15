// @story 04.4
import { guardedRoute } from "@/lib/api-guard";
import { forwardFlowProjectRequest } from "@/lib/flow-bff-forward";

export const GET = guardedRoute(
  (req, ctx) => forwardFlowProjectRequest({ method: "GET", req, ctx, suffix: "flow/node-sessions" }),
  { authMode: "strict-loopback" }
);
