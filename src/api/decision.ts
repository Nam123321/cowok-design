// @story 09.2
// @implements AC-01, AC-02, AC-03, AC-04 (Typed Decision Cascade Domain SDK enforcing runtime .parse())
import { z } from "zod";
import {
  type CascadeStage,
  type DecisionAuditResponse,
  DecisionAuditResponseSchema,
  type DecisionRouteRequest,
  DecisionRouteRequestSchema,
  type DecisionRouteResponse,
  DecisionRouteResponseSchema,
} from "@/lib/validations/decision";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class DecisionApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details?: Record<string, unknown>;

  constructor(status: number, code: string, message: string, details?: Record<string, unknown>) {
    super(message);
    this.name = "DecisionApiError";
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

async function parseDecisionError(res: Response): Promise<DecisionApiError> {
  try {
    const body = (await res.json()) as {
      error?: { code?: string; message?: string; details?: Record<string, unknown> };
    };
    const code = body?.error?.code ?? "REQUEST_FAILED";
    const message = body?.error?.message ?? `Request failed (${res.status})`;
    return new DecisionApiError(res.status, code, message, body?.error?.details);
  } catch {
    return new DecisionApiError(res.status, "REQUEST_FAILED", `Request failed (${res.status})`);
  }
}

export class DecisionApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseDecisionError(res);
    return schema.parse(await res.json());
  }

  routeDecision(
    payload: z.input<typeof DecisionRouteRequestSchema>
  ): Promise<DecisionRouteResponse> {
    const validated: DecisionRouteRequest = DecisionRouteRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/decision/route",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      DecisionRouteResponseSchema
    );
  }

  listAuditLog(params: {
    projectId: string;
    limit?: number;
    stage?: CascadeStage;
  }): Promise<DecisionAuditResponse> {
    const search = new URLSearchParams({ projectId: params.projectId });
    if (params.limit !== undefined) {
      search.set("limit", String(params.limit));
    }
    if (params.stage !== undefined) {
      search.set("stage", params.stage);
    }
    return this.requestValidated(
      `/api/v1/decision/audit?${search.toString()}`,
      { method: "GET", headers: { Accept: "application/json" } },
      DecisionAuditResponseSchema
    );
  }
}

export const apiDecision = new DecisionApiClient();

export function routeDecisionCascade(
  payload: z.input<typeof DecisionRouteRequestSchema>
): Promise<DecisionRouteResponse> {
  return apiDecision.routeDecision(payload);
}

export function getDecisionAuditLog(params: {
  projectId: string;
  limit?: number;
  stage?: CascadeStage;
}): Promise<DecisionAuditResponse> {
  return apiDecision.listAuditLog(params);
}
