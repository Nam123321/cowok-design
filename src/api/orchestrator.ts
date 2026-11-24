// @story 10.1
// @implements AC-01, AC-02, AC-03, AC-04 (Typed cowok Orchestrator Domain SDK enforcing runtime .parse() & 429 Retry-After parsing)
import { z } from "zod";
import {
  type OrchestratorAssetsQuery,
  OrchestratorAssetsQuerySchema,
  type OrchestratorAssetsResponse,
  OrchestratorAssetsResponseSchema,
  type OrchestratorChatRequest,
  OrchestratorChatRequestSchema,
  type OrchestratorChatResponse,
  OrchestratorChatResponseSchema,
  OrchestratorErrorResponseSchema,
  type OrchestratorLivePatchRequest,
  OrchestratorLivePatchRequestSchema,
  type OrchestratorLivePatchResponse,
  OrchestratorLivePatchResponseSchema,
} from "@/lib/validations/orchestrator";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class OrchestratorApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly retryAfterSecs?: number;
  readonly details?: Record<string, unknown>;

  constructor(params: {
    status: number;
    code: string;
    message: string;
    retryAfterSecs?: number;
    details?: Record<string, unknown>;
  }) {
    super(params.message);
    this.name = "OrchestratorApiError";
    this.status = params.status;
    this.code = params.code;
    this.retryAfterSecs = params.retryAfterSecs;
    this.details = params.details;
  }
}

function parseRetryAfterHeader(res: Response): number | undefined {
  const raw = res.headers.get("retry-after");
  if (!raw) return undefined;
  const parsed = Number.parseInt(raw.trim(), 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : undefined;
}

async function parseOrchestratorError(res: Response): Promise<OrchestratorApiError> {
  const retryAfterSecs = parseRetryAfterHeader(res);
  try {
    const rawJson: unknown = await res.json();
    const checked = OrchestratorErrorResponseSchema.safeParse(rawJson);
    if (checked.success) {
      return new OrchestratorApiError({
        status: res.status,
        code: checked.data.error.code,
        message: checked.data.error.message,
        retryAfterSecs,
        details: checked.data.error.details,
      });
    }
    const fallback = rawJson as {
      error?: { code?: string; message?: string; details?: Record<string, unknown> };
    };
    return new OrchestratorApiError({
      status: res.status,
      code: fallback?.error?.code ?? "REQUEST_FAILED",
      message: fallback?.error?.message ?? `Request failed (${res.status})`,
      retryAfterSecs,
      details: fallback?.error?.details,
    });
  } catch {
    return new OrchestratorApiError({
      status: res.status,
      code: "REQUEST_FAILED",
      message: `Request failed (${res.status})`,
      retryAfterSecs,
    });
  }
}

export class OrchestratorApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseOrchestratorError(res);
    return schema.parse(await res.json());
  }

  chat(
    payload: z.input<typeof OrchestratorChatRequestSchema>
  ): Promise<OrchestratorChatResponse> {
    const validated: OrchestratorChatRequest = OrchestratorChatRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/orchestrator/chat",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      OrchestratorChatResponseSchema
    );
  }

  applyLivePatch(
    payload: z.input<typeof OrchestratorLivePatchRequestSchema>
  ): Promise<OrchestratorLivePatchResponse> {
    const validated: OrchestratorLivePatchRequest =
      OrchestratorLivePatchRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/orchestrator/live-patch",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      OrchestratorLivePatchResponseSchema
    );
  }

  listAssets(
    params: z.input<typeof OrchestratorAssetsQuerySchema>
  ): Promise<OrchestratorAssetsResponse> {
    const validated: OrchestratorAssetsQuery = OrchestratorAssetsQuerySchema.parse(params);
    const search = new URLSearchParams({ projectId: validated.projectId });
    if (validated.sessionId !== undefined) {
      search.set("sessionId", validated.sessionId);
    }
    return this.requestValidated(
      `/api/v1/orchestrator/assets?${search.toString()}`,
      { method: "GET", headers: { Accept: "application/json" } },
      OrchestratorAssetsResponseSchema
    );
  }
}

export const apiOrchestrator = new OrchestratorApiClient();

export function sendOrchestratorChat(
  payload: z.input<typeof OrchestratorChatRequestSchema>
): Promise<OrchestratorChatResponse> {
  return apiOrchestrator.chat(payload);
}

export function applyOrchestratorLivePatch(
  payload: z.input<typeof OrchestratorLivePatchRequestSchema>
): Promise<OrchestratorLivePatchResponse> {
  return apiOrchestrator.applyLivePatch(payload);
}

export function listOrchestratorAssets(
  params: z.input<typeof OrchestratorAssetsQuerySchema>
): Promise<OrchestratorAssetsResponse> {
  return apiOrchestrator.listAssets(params);
}
