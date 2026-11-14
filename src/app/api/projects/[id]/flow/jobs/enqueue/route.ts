// @story 04.4
import { makeFlowProjectPostRoute } from "@/lib/flow-bff-forward";
import { EnqueueNodeJobInputSchema } from "@/lib/validations/flow-execution";

export const POST = makeFlowProjectPostRoute("flow/jobs/enqueue", EnqueueNodeJobInputSchema);
