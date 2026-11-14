// @story 04.4
import { makeFlowProjectPostRoute } from "@/lib/flow-bff-forward";
import { MarkNodeSessionReadInputSchema } from "@/lib/validations/flow-execution";

export const POST = makeFlowProjectPostRoute(
  "flow/node-sessions/read",
  MarkNodeSessionReadInputSchema
);
