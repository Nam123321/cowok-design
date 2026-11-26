// @story 13.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Domain SDK with MG-05 Runtime Zod Boundary Parsing)
import type { z } from "zod";
import {
  EvalErrorResponseSchema,
  type EvalHistoryQuery,
  EvalHistoryQuerySchema,
  type EvalHistoryResponse,
  EvalHistoryResponseSchema,
  type EvalRunRequest,
  EvalRunRequestSchema,
  type EvalRunResponse,
  EvalRunResponseSchema,
} from "@/lib/validations/eval-certification";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

const HTTP_STATUS_CONFLICT = 409;
const HTTP_STATUS_UNPROCESSABLE = 422;

export class EvalCertificationApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details?: Record<string, unknown>;

  constructor(
    status: number,
    code: string,
    message: string,
    details?: Record<string, unknown>
  ) {
    super(message);
    this.name = "EvalCertificationApiError";
    this.status = status;
    this.code = code;
    this.details = details;
  }

  get isAlreadyRunning(): boolean {
    return this.status === HTTP_STATUS_CONFLICT || this.code === "EVAL_ALREADY_RUNNING";
  }

  get isTamperError(): boolean {
    return this.status === HTTP_STATUS_UNPROCESSABLE || this.code === "SEALED_EVAL_TAMPER_ERROR";
  }
}

async function parseEvalCertificationError(res: Response): Promise<EvalCertificationApiError> {
  try {
    const raw = await res.json();
    const parsed = EvalErrorResponseSchema.safeParse(raw);
    if (parsed.success) {
      return new EvalCertificationApiError(
        res.status,
        parsed.data.error.code,
        parsed.data.error.message,
        parsed.data.error.details
      );
    }
    const fallback = raw as {
      code?: string;
      message?: string;
      error?: { code?: string; message?: string; details?: Record<string, unknown> };
    };
    return new EvalCertificationApiError(
      res.status,
      fallback?.error?.code ?? fallback?.code ?? "REQUEST_FAILED",
      fallback?.error?.message ?? fallback?.message ?? `Evaluation request failed (${res.status})`,
      fallback?.error?.details
    );
  } catch {
    return new EvalCertificationApiError(
      res.status,
      "REQUEST_FAILED",
      `Evaluation request failed (${res.status})`
    );
  }
}

function buildHistoryPath(query?: EvalHistoryQuery): string {
  if (!query) return "/api/v1/eval/history";
  const validated = EvalHistoryQuerySchema.parse(query);
  const params = new URLSearchParams();
  if (validated.limit !== undefined) params.set("limit", String(validated.limit));
  if (validated.tier) params.set("tier", validated.tier);
  if (validated.projectId) params.set("projectId", validated.projectId);
  const qs = params.toString();
  return qs ? `/api/v1/eval/history?${qs}` : "/api/v1/eval/history";
}

export class EvalCertificationApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseEvalCertificationError(res);
    return schema.parse(await res.json());
  }

  runEval(payload: EvalRunRequest, signal?: AbortSignal): Promise<EvalRunResponse> {
    const validated = EvalRunRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/eval/run",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
        signal,
      },
      EvalRunResponseSchema
    );
  }

  getEvalHistory(query?: EvalHistoryQuery, signal?: AbortSignal): Promise<EvalHistoryResponse> {
    return this.requestValidated(
      buildHistoryPath(query),
      {
        method: "GET",
        headers: { Accept: "application/json" },
        signal,
      },
      EvalHistoryResponseSchema
    );
  }
}

export const apiEvalCertification = new EvalCertificationApiClient();

export function runEvalCertification(
  payload: EvalRunRequest,
  signal?: AbortSignal
): Promise<EvalRunResponse> {
  return apiEvalCertification.runEval(payload, signal);
}

export function getEvalCertificationHistory(
  query?: EvalHistoryQuery,
  signal?: AbortSignal
): Promise<EvalHistoryResponse> {
  return apiEvalCertification.getEvalHistory(query, signal);
}
