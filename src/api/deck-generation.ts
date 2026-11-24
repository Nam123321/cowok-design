// @story 06.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Typed Deck Generation & ChartSpecV12 SDK with Mandatory Runtime .parse())
import { z } from "zod";
import {
  type CopilotRewriteSlideResponse,
  CopilotRewriteSlideRequestSchema,
  CopilotRewriteSlideResponseSchema,
  DeckGenerationErrorResponseSchema,
  type GenerateDeckResponse,
  GenerateDeckRequestSchema,
  GenerateDeckResponseSchema,
  type NormalizeChartSourceResponse,
  NormalizeChartSourceRequestSchema,
  NormalizeChartSourceResponseSchema,
} from "../lib/validations/deck-generation";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class DeckGenerationApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly guidance?: string;
  readonly retryable: boolean;

  constructor(
    status: number,
    code: string,
    message: string,
    guidance?: string,
    retryable = false
  ) {
    super(message);
    this.name = "DeckGenerationApiError";
    this.status = status;
    this.code = code;
    this.guidance = guidance;
    this.retryable = retryable;
  }
}

async function parseDeckGenError(res: Response): Promise<DeckGenerationApiError> {
  try {
    const raw = (await res.json()) as unknown;
    const parsed = DeckGenerationErrorResponseSchema.safeParse(raw);
    if (parsed.success) {
      const { code, message, guidance, retryable } = parsed.data.error;
      return new DeckGenerationApiError(
        res.status,
        code,
        message,
        guidance,
        Boolean(retryable)
      );
    }
    const fallback = raw as {
      code?: string;
      message?: string;
      guidance?: string;
      retryable?: boolean;
      error?: { code?: string; message?: string; guidance?: string; retryable?: boolean };
    };
    const code = fallback?.error?.code ?? fallback?.code ?? "REQUEST_FAILED";
    const message =
      fallback?.error?.message ?? fallback?.message ?? `Request failed (${res.status})`;
    const guidance = fallback?.error?.guidance ?? fallback?.guidance;
    const retryable = Boolean(fallback?.error?.retryable ?? fallback?.retryable ?? false);
    return new DeckGenerationApiError(res.status, code, message, guidance, retryable);
  } catch {
    return new DeckGenerationApiError(
      res.status,
      "REQUEST_FAILED",
      `Request failed (${res.status})`
    );
  }
}

export class DeckGenerationApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async postValidated<T>(
    path: string,
    body: unknown,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      throw await parseDeckGenError(res);
    }
    return schema.parse(await res.json());
  }

  normalizeChartSource(
    payload: z.input<typeof NormalizeChartSourceRequestSchema>
  ): Promise<NormalizeChartSourceResponse> {
    const validated = NormalizeChartSourceRequestSchema.parse(payload);
    return this.postValidated(
      "/api/v1/decks/normalize-chart-source",
      validated,
      NormalizeChartSourceResponseSchema
    );
  }

  generateDeck(
    payload: z.input<typeof GenerateDeckRequestSchema>
  ): Promise<GenerateDeckResponse> {
    const validated = GenerateDeckRequestSchema.parse(payload);
    return this.postValidated(
      "/api/v1/decks/generate",
      validated,
      GenerateDeckResponseSchema
    );
  }

  rewriteSlideWithCopilot(
    payload: z.input<typeof CopilotRewriteSlideRequestSchema>
  ): Promise<CopilotRewriteSlideResponse> {
    const validated = CopilotRewriteSlideRequestSchema.parse(payload);
    return this.postValidated(
      "/api/v1/slides/copilot/rewrite",
      validated,
      CopilotRewriteSlideResponseSchema
    );
  }
}

export const apiDeckGeneration = new DeckGenerationApiClient();

export function normalizeChartSource(
  payload: z.input<typeof NormalizeChartSourceRequestSchema>
): Promise<NormalizeChartSourceResponse> {
  return apiDeckGeneration.normalizeChartSource(payload);
}

export function generateDeck(
  payload: z.input<typeof GenerateDeckRequestSchema>
): Promise<GenerateDeckResponse> {
  return apiDeckGeneration.generateDeck(payload);
}

export function rewriteSlideWithCopilot(
  payload: z.input<typeof CopilotRewriteSlideRequestSchema>
): Promise<CopilotRewriteSlideResponse> {
  return apiDeckGeneration.rewriteSlideWithCopilot(payload);
}
