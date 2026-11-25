// @story 07.1
// @implements AC-01, AC-02, AC-03, AC-04
import type { z } from "zod";
import {
  type QcAutoFixRequest,
  QcAutoFixRequestSchema,
  type QcAutoFixResponse,
  QcAutoFixResponseSchema,
  type QcRunRequest,
  QcRunRequestSchema,
  type QcRunResponse,
  QcRunResponseSchema,
  VisualQcErrorResponseSchema,
} from "@/lib/validations/visual-qc";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class VisualQcApiError extends Error {
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
    this.name = "VisualQcApiError";
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

async function parseVisualQcError(res: Response): Promise<VisualQcApiError> {
  try {
    const raw = await res.json();
    const parsed = VisualQcErrorResponseSchema.safeParse(raw);
    if (parsed.success) {
      return new VisualQcApiError(
        res.status,
        parsed.data.error.code,
        parsed.data.error.message,
        parsed.data.error.details
      );
    }
    const fallback = raw as {
      error?: { code?: string; message?: string; details?: Record<string, unknown> };
    };
    return new VisualQcApiError(
      res.status,
      fallback?.error?.code ?? "REQUEST_FAILED",
      fallback?.error?.message ?? `Visual QC request failed (${res.status})`,
      fallback?.error?.details
    );
  } catch {
    return new VisualQcApiError(
      res.status,
      "REQUEST_FAILED",
      `Visual QC request failed (${res.status})`
    );
  }
}

export class VisualQcApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseVisualQcError(res);
    return schema.parse(await res.json());
  }

  runVisualQc(payload: QcRunRequest, signal?: AbortSignal): Promise<QcRunResponse> {
    const validated = QcRunRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/qc/run",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
        signal,
      },
      QcRunResponseSchema
    );
  }

  runAutoFix(payload: QcAutoFixRequest, signal?: AbortSignal): Promise<QcAutoFixResponse> {
    const validated = QcAutoFixRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/qc/auto-fix",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
        signal,
      },
      QcAutoFixResponseSchema
    );
  }
}

const defaultVisualQcClient = new VisualQcApiClient();

export function runVisualQc(
  payload: QcRunRequest,
  signal?: AbortSignal
): Promise<QcRunResponse> {
  return defaultVisualQcClient.runVisualQc(payload, signal);
}

export function runVisualQcAutoFix(
  payload: QcAutoFixRequest,
  signal?: AbortSignal
): Promise<QcAutoFixResponse> {
  return defaultVisualQcClient.runAutoFix(payload, signal);
}
