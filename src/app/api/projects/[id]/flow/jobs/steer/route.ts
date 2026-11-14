// @story 04.4
import { makeFlowProjectPostRoute } from "@/lib/flow-bff-forward";
import { SteerNodeJobInputSchema } from "@/lib/validations/flow-execution";

export const POST = makeFlowProjectPostRoute("flow/jobs/steer", SteerNodeJobInputSchema);
