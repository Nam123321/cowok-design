// @story 06.3
// @implements AC-01, AC-02, AC-03, AC-04
import { z } from "zod";
import {
  ValidateChartRequestSchema,
  ValidateChartResponseSchema,
  type ValidateChartRequest,
  type ValidateChartResponse,
} from "../lib/validations/presentation-chart-specs";
import {
  SlideCopilotCancelRequestSchema,
  SlideCopilotDispatchRequestSchema,
  SlideCopilotLeaseDtoSchema,
  type SlideCopilotCancelRequest,
  type SlideCopilotDispatchRequest,
  type SlideCopilotLeaseDto,
} from "../lib/validations/presentation-charts";
import { SlideUidSchema } from "../lib/validations/presentation";
import {
  HarnessDaemonTransport,
  type HarnessDaemonClientConfig,
} from "./harness-daemon-transport";

export interface PresentationChartsApiClientLike {
  request<T>(
    path: string,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>,
    options?: RequestInit & { timeoutMs?: number }
  ): Promise<T>;
}

export interface PresentationChartsRequestOptions {
  timeoutMs?: number;
  idempotencyKey?: string;
}

const DEFAULT_TIMEOUT_MS = 10_000;

const defaultTransport = new (class PresentationChartsTransport
  extends HarnessDaemonTransport
  implements PresentationChartsApiClientLike
{
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }
})();

function buildHeadersWithIdempotency(idempotencyKey?: string): Record<string, string> {
  const headers: Record<string, string> = {};
  const trimmed = idempotencyKey?.trim();
  if (trimmed) {
    headers["Idempotency-Key"] = trimmed;
  }
  return headers;
}

export class PresentationChartsClient {
  private readonly client: PresentationChartsApiClientLike;

  constructor(client: PresentationChartsApiClientLike = defaultTransport) {
    this.client = client;
  }

  dispatchSlideCopilot(
    target: { projectId: string; deckId: string; slideUid: string },
    input: SlideCopilotDispatchRequest,
    opts: PresentationChartsRequestOptions = {}
  ): Promise<SlideCopilotLeaseDto> {
    const validatedUid = SlideUidSchema.parse(target.slideUid);
    const validatedBody = SlideCopilotDispatchRequestSchema.parse(input);
    const headers = buildHeadersWithIdempotency(opts.idempotencyKey);
    const path = `/api/projects/${encodeURIComponent(target.projectId)}/decks/${encodeURIComponent(target.deckId)}/slides/${encodeURIComponent(validatedUid)}/copilot/dispatch`;
    return this.client.request(path, SlideCopilotLeaseDtoSchema, {
      method: "POST",
      headers,
      body: JSON.stringify(validatedBody),
      timeoutMs: opts.timeoutMs ?? DEFAULT_TIMEOUT_MS,
    });
  }

  cancelSlideCopilot(
    target: { projectId: string; deckId: string; slideUid: string },
    input: SlideCopilotCancelRequest,
    opts: PresentationChartsRequestOptions = {}
  ): Promise<SlideCopilotLeaseDto> {
    const validatedUid = SlideUidSchema.parse(target.slideUid);
    const validatedBody = SlideCopilotCancelRequestSchema.parse(input);
    const headers = buildHeadersWithIdempotency(opts.idempotencyKey);
    const path = `/api/projects/${encodeURIComponent(target.projectId)}/decks/${encodeURIComponent(target.deckId)}/slides/${encodeURIComponent(validatedUid)}/copilot/cancel`;
    return this.client.request(path, SlideCopilotLeaseDtoSchema, {
      method: "POST",
      headers,
      body: JSON.stringify(validatedBody),
      timeoutMs: opts.timeoutMs ?? DEFAULT_TIMEOUT_MS,
    });
  }

  validateChartSpec(
    target: { projectId: string; deckId: string; slideUid: string },
    input: ValidateChartRequest,
    opts: PresentationChartsRequestOptions = {}
  ): Promise<ValidateChartResponse> {
    const validatedUid = SlideUidSchema.parse(target.slideUid);
    const validatedBody = ValidateChartRequestSchema.parse(input);
    const path = `/api/projects/${encodeURIComponent(target.projectId)}/decks/${encodeURIComponent(target.deckId)}/slides/${encodeURIComponent(validatedUid)}/charts/validate`;
    return this.client
      .request(path, ValidateChartResponseSchema, {
        method: "POST",
        body: JSON.stringify(validatedBody),
        timeoutMs: opts.timeoutMs ?? DEFAULT_TIMEOUT_MS,
      })
      .then((res) => ValidateChartResponseSchema.parse(res));
  }
}

export const apiPresentationCharts = new PresentationChartsClient();
export default apiPresentationCharts;
