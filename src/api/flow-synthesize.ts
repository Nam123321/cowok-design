// @story 05.5
// @implements AC-01, AC-02, AC-03, AC-04 (Typed 3-Tier Flow Synthesizer Domain SDK with runtime .parse())
import { z } from "zod";
import {
  type FlowSubstatesListResponse,
  FlowSubstatesListResponseSchema,
  FlowSubstatesQuerySchema,
  FlowSynthesizeErrorResponseSchema,
  type FlowSynthesizeRequest,
  FlowSynthesizeRequestSchema,
  type FlowTier,
  type SynthesizeFlowResponse,
  SynthesizeFlowResponseSchema,
} from "@/lib/validations/flow-synthesize";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class FlowSynthesizeApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly field?: string;

  constructor(status: number, code: string, message: string, field?: string) {
    super(message);
    this.name = "FlowSynthesizeApiError";
    this.status = status;
    this.code = code;
    this.field = field;
  }
}

async function parseFlowSynthesizeError(res: Response): Promise<FlowSynthesizeApiError> {
  try {
    const raw: unknown = await res.json();
    const parsed = FlowSynthesizeErrorResponseSchema.safeParse(raw);
    if (parsed.success) {
      return new FlowSynthesizeApiError(
        res.status,
        parsed.data.error.code,
        parsed.data.error.message,
        parsed.data.error.field
      );
    }
    const fallback = raw as { error?: { code?: unknown; message?: unknown; field?: unknown } } | null;
    const code =
      typeof fallback?.error?.code === "string" ? fallback.error.code : "REQUEST_FAILED";
    const message =
      typeof fallback?.error?.message === "string"
        ? fallback.error.message
        : `Request failed (${res.status})`;
    const field = typeof fallback?.error?.field === "string" ? fallback.error.field : undefined;
    return new FlowSynthesizeApiError(res.status, code, message, field);
  } catch {
    return new FlowSynthesizeApiError(
      res.status,
      "REQUEST_FAILED",
      `Request failed (${res.status})`
    );
  }
}

export class FlowSynthesizeApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  async synthesize(
    payload: z.input<typeof FlowSynthesizeRequestSchema>,
    signal?: AbortSignal
  ): Promise<SynthesizeFlowResponse> {
    const validated: FlowSynthesizeRequest = FlowSynthesizeRequestSchema.parse(payload);
    const res = await this.fetchAuthedResponse("/api/v1/flows/synthesize", {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(validated),
      signal,
    });
    if (!res.ok) throw await parseFlowSynthesizeError(res);
    return SynthesizeFlowResponseSchema.parse(await res.json());
  }

  async listSubstates(
    params: { projectId: string; flowTier?: FlowTier },
    signal?: AbortSignal
  ): Promise<FlowSubstatesListResponse> {
    const validated = FlowSubstatesQuerySchema.parse(params);
    const search = new URLSearchParams({ projectId: validated.projectId });
    if (validated.flowTier) {
      search.set("flowTier", validated.flowTier);
    }
    const res = await this.fetchAuthedResponse(`/api/v1/flows/substates?${search.toString()}`, {
      method: "GET",
      headers: { Accept: "application/json" },
      signal,
    });
    if (!res.ok) throw await parseFlowSynthesizeError(res);
    return FlowSubstatesListResponseSchema.parse(await res.json());
  }
}

export const apiFlowSynthesize = new FlowSynthesizeApiClient();

export function synthesizeThreeTierFlow(
  payload: z.input<typeof FlowSynthesizeRequestSchema>,
  signal?: AbortSignal
): Promise<SynthesizeFlowResponse> {
  return apiFlowSynthesize.synthesize(payload, signal);
}

export function listFlowSubstates(
  params: { projectId: string; flowTier?: FlowTier },
  signal?: AbortSignal
): Promise<FlowSubstatesListResponse> {
  return apiFlowSynthesize.listSubstates(params, signal);
}
