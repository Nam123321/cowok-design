// @story 06.1, 06.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import { z } from "zod";
import {
  ApplySlideHtmlRequest,
  ApplySlideHtmlRequestSchema,
  ApplySlideHtmlResponse,
  ApplySlideHtmlResponseSchema,
  CreateDeckRequest,
  CreateDeckRequestSchema,
  DuplicateSlideRequest,
  DuplicateSlideRequestSchema,
  DuplicateSlideResponse,
  DuplicateSlideResponseSchema,
  PatchSlotsBatchRequest,
  PatchSlotsBatchRequestSchema,
  PatchSlotsBatchResponse,
  PatchSlotsBatchResponseSchema,
  PresentationDeckDto,
  PresentationDeckDtoSchema,
  PresentationDeckListResponseSchema,
  ReorderSlidesRequest,
  ReorderSlidesRequestSchema,
  SlideSlotsSnapshotResponse,
  SlideSlotsSnapshotResponseSchema,
  SlideUidSchema,
} from "../lib/validations/presentation";
import {
  HarnessDaemonTransport,
  type HarnessDaemonClientConfig,
} from "./harness-daemon-transport";
import { PresentationChartsClient } from "./presentation-charts";
import { PresentationPresenterClient } from "./presentation-presenter";
import type {
  PresenterHandshakeRequest,
  PresenterHandshakeResponse,
} from "../lib/validations/presentation-presenter";
import type {
  ValidateChartRequest,
  ValidateChartResponse,
} from "../lib/validations/presentation-chart-specs";
import type {
  SlideCopilotCancelRequest,
  SlideCopilotDispatchRequest,
  SlideCopilotLeaseDto,
} from "../lib/validations/presentation-charts";

export interface PresentationRequestOptions {
  timeoutMs?: number;
  idempotencyKey?: string;
}

export interface PresentationApiClientLike {
  request<T>(
    path: string,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>,
    options?: RequestInit & { timeoutMs?: number }
  ): Promise<T>;
}

const DEFAULT_PRESENTATION_TIMEOUT_MS = 10_000;

export const apiClient = new (class PresentationTransportAdapter
  extends HarnessDaemonTransport
  implements PresentationApiClientLike
{
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  override async request<T>(
    path: string,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>,
    options: RequestInit & { timeoutMs?: number } = {}
  ): Promise<T> {
    const { timeoutMs, signal: outerSignal, ...init } = options;
    const effectiveTimeout =
      typeof timeoutMs === "number" && timeoutMs > 0
        ? timeoutMs
        : DEFAULT_PRESENTATION_TIMEOUT_MS;

    const controller = new AbortController();
    const timer = globalThis.setTimeout(() => controller.abort(), effectiveTimeout);

    const onOuterAbort = () => controller.abort();
    if (outerSignal) {
      if (outerSignal.aborted) {
        controller.abort();
      } else {
        outerSignal.addEventListener("abort", onOuterAbort, { once: true });
      }
    }

    try {
      return await super.request(path, schema, {
        ...init,
        signal: controller.signal,
      });
    } finally {
      globalThis.clearTimeout(timer);
      if (outerSignal) {
        outerSignal.removeEventListener("abort", onOuterAbort);
      }
    }
  }
})();

export class PresentationClient {
  private readonly client: PresentationApiClientLike;
  private readonly chartsClient: PresentationChartsClient;
  private readonly presenterClient: PresentationPresenterClient;

  constructor(client: PresentationApiClientLike = apiClient) {
    this.client = client;
    this.chartsClient = new PresentationChartsClient(client);
    this.presenterClient = new PresentationPresenterClient(client);
  }

  async listDecks(
    projectId: string,
    opts: PresentationRequestOptions = {}
  ): Promise<PresentationDeckDto[]> {
    const res = await this.client.request(
      `/api/projects/${encodeURIComponent(projectId)}/decks`,
      PresentationDeckListResponseSchema,
      {
        method: "GET",
        timeoutMs: opts.timeoutMs,
      }
    );
    return res.decks;
  }

  createDeck(
    projectId: string,
    input: CreateDeckRequest,
    opts: PresentationRequestOptions = {}
  ): Promise<PresentationDeckDto> {
    const validated = CreateDeckRequestSchema.parse(input);
    const headers: Record<string, string> = {};
    if (opts.idempotencyKey?.trim()) {
      headers["Idempotency-Key"] = opts.idempotencyKey.trim();
    }
    return this.client.request(
      `/api/projects/${encodeURIComponent(projectId)}/decks`,
      PresentationDeckDtoSchema,
      {
        method: "POST",
        headers,
        body: JSON.stringify(validated),
        timeoutMs: opts.timeoutMs,
      }
    );
  }

  reorderSlides(
    projectId: string,
    deckId: string,
    input: ReorderSlidesRequest,
    opts: PresentationRequestOptions = {}
  ): Promise<PresentationDeckDto> {
    const validated = ReorderSlidesRequestSchema.parse(input);
    return this.client.request(
      `/api/projects/${encodeURIComponent(projectId)}/decks/${encodeURIComponent(deckId)}/slides/reorder`,
      PresentationDeckDtoSchema,
      {
        method: "PUT",
        body: JSON.stringify(validated),
        timeoutMs: opts.timeoutMs,
      }
    );
  }

  duplicateSlide(
    target: { projectId: string; deckId: string; slideUid: string },
    input: DuplicateSlideRequest,
    opts: PresentationRequestOptions = {}
  ): Promise<DuplicateSlideResponse> {
    const validatedUid = SlideUidSchema.parse(target.slideUid);
    const validatedBody = DuplicateSlideRequestSchema.parse(input);
    const headers: Record<string, string> = {};
    if (opts.idempotencyKey?.trim()) {
      headers["Idempotency-Key"] = opts.idempotencyKey.trim();
    }
    return this.client.request(
      `/api/projects/${encodeURIComponent(target.projectId)}/decks/${encodeURIComponent(target.deckId)}/slides/${encodeURIComponent(validatedUid)}/duplicate`,
      DuplicateSlideResponseSchema,
      {
        method: "POST",
        headers,
        body: JSON.stringify(validatedBody),
        timeoutMs: opts.timeoutMs,
      }
    );
  }

  listSlideSlots(
    target: { projectId: string; deckId: string; slideUid: string },
    opts: PresentationRequestOptions = {}
  ): Promise<SlideSlotsSnapshotResponse> {
    const validatedUid = SlideUidSchema.parse(target.slideUid);
    return this.client.request(
      `/api/projects/${encodeURIComponent(target.projectId)}/decks/${encodeURIComponent(target.deckId)}/slides/${encodeURIComponent(validatedUid)}/slots`,
      SlideSlotsSnapshotResponseSchema,
      {
        method: "GET",
        timeoutMs: opts.timeoutMs,
      }
    );
  }

  async commitSlotBatch(
    target: { projectId: string; deckId: string; slideUid: string },
    input: PatchSlotsBatchRequest,
    opts: PresentationRequestOptions = {}
  ): Promise<PatchSlotsBatchResponse> {
    const validatedUid = SlideUidSchema.parse(target.slideUid);
    const validatedBody = PatchSlotsBatchRequestSchema.parse(input);
    const headers: Record<string, string> = {};
    if (opts.idempotencyKey?.trim()) {
      headers["Idempotency-Key"] = opts.idempotencyKey.trim();
    }
    const res = await this.client.request(
      `/api/projects/${encodeURIComponent(target.projectId)}/decks/${encodeURIComponent(target.deckId)}/slides/${encodeURIComponent(validatedUid)}/slots/batch`,
      PatchSlotsBatchResponseSchema,
      {
        method: "POST",
        headers,
        body: JSON.stringify(validatedBody),
        timeoutMs: opts.timeoutMs,
      }
    );
    return PatchSlotsBatchResponseSchema.parse(res);
  }

  async applyCustomHtml(
    target: { projectId: string; deckId: string; slideUid: string },
    input: ApplySlideHtmlRequest,
    opts: PresentationRequestOptions = {}
  ): Promise<ApplySlideHtmlResponse> {
    const validatedUid = SlideUidSchema.parse(target.slideUid);
    const validatedBody = ApplySlideHtmlRequestSchema.parse(input);
    const headers: Record<string, string> = {};
    if (opts.idempotencyKey?.trim()) {
      headers["Idempotency-Key"] = opts.idempotencyKey.trim();
    }
    const res = await this.client.request(
      `/api/projects/${encodeURIComponent(target.projectId)}/decks/${encodeURIComponent(target.deckId)}/slides/${encodeURIComponent(validatedUid)}/apply-html`,
      ApplySlideHtmlResponseSchema,
      {
        method: "POST",
        headers,
        body: JSON.stringify(validatedBody),
        timeoutMs: opts.timeoutMs,
      }
    );
    return ApplySlideHtmlResponseSchema.parse(res);
  }

  dispatchSlideCopilot(
    target: { projectId: string; deckId: string; slideUid: string },
    input: SlideCopilotDispatchRequest,
    opts: PresentationRequestOptions = {}
  ): Promise<SlideCopilotLeaseDto> {
    return this.chartsClient.dispatchSlideCopilot(target, input, opts);
  }

  cancelSlideCopilot(
    target: { projectId: string; deckId: string; slideUid: string },
    input: SlideCopilotCancelRequest,
    opts: PresentationRequestOptions = {}
  ): Promise<SlideCopilotLeaseDto> {
    return this.chartsClient.cancelSlideCopilot(target, input, opts);
  }

  validateChartSpec(
    target: { projectId: string; deckId: string; slideUid: string },
    input: ValidateChartRequest,
    opts: PresentationRequestOptions = {}
  ): Promise<ValidateChartResponse> {
    return this.chartsClient.validateChartSpec(target, input, opts);
  }

  createPresenterSession(
    target: { projectId?: string; deckId: string },
    input: Partial<PresenterHandshakeRequest> = {},
    opts: PresentationRequestOptions = {}
  ): Promise<PresenterHandshakeResponse> {
    return this.presenterClient.createPresenterSession(target, input, opts);
  }
}

export const apiPresentation = new PresentationClient();
export default apiPresentation;
