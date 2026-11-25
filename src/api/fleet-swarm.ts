// @story 10.2
// @implements AC-01, AC-02, AC-03, AC-04 (Typed Fleet Swarm & Tier-3 Design Jury Domain SDK enforcing runtime .parse() on every boundary)
import { z } from "zod";
import {
  type AcquireWorkerLeaseRequest,
  AcquireWorkerLeaseRequestSchema,
  type AcquireWorkerLeaseResponse,
  AcquireWorkerLeaseResponseSchema,
  type DispatchFleetSwarmRequest,
  DispatchFleetSwarmRequestSchema,
  type DispatchFleetSwarmResponse,
  DispatchFleetSwarmResponseSchema,
  type EvaluateDesignJuryRequest,
  EvaluateDesignJuryRequestSchema,
  type EvaluateDesignJuryResponse,
  EvaluateDesignJuryResponseSchema,
  FleetSwarmErrorResponseSchema,
} from "@/lib/validations/fleet-swarm";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class FleetSwarmApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly field?: string;
  readonly requestId?: string;
  readonly details?: Record<string, unknown>;

  constructor(params: {
    status: number;
    code: string;
    message: string;
    field?: string;
    requestId?: string;
    details?: Record<string, unknown>;
  }) {
    super(params.message);
    this.name = "FleetSwarmApiError";
    this.status = params.status;
    this.code = params.code;
    this.field = params.field;
    this.requestId = params.requestId;
    this.details = params.details;
  }
}

async function parseFleetSwarmError(res: Response): Promise<FleetSwarmApiError> {
  try {
    const rawJson: unknown = await res.json();
    const parsed = FleetSwarmErrorResponseSchema.safeParse(rawJson);
    if (parsed.success) {
      return new FleetSwarmApiError({
        status: res.status,
        code: parsed.data.error.code,
        message: parsed.data.error.message,
        field: parsed.data.error.field,
        requestId: parsed.data.error.requestId,
        details: parsed.data.error.details,
      });
    }
    const fallback = rawJson as {
      error?: { code?: string; message?: string; details?: Record<string, unknown> };
    };
    return new FleetSwarmApiError({
      status: res.status,
      code: fallback?.error?.code ?? "REQUEST_FAILED",
      message: fallback?.error?.message ?? `Request failed (${res.status})`,
      details: fallback?.error?.details,
    });
  } catch {
    return new FleetSwarmApiError({
      status: res.status,
      code: "REQUEST_FAILED",
      message: `Request failed (${res.status})`,
    });
  }
}

export class FleetSwarmApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async postJson<T>(
    path: string,
    body: unknown,
    responseSchema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw await parseFleetSwarmError(res);
    return responseSchema.parse(await res.json());
  }

  acquireLease(
    payload: z.input<typeof AcquireWorkerLeaseRequestSchema>
  ): Promise<AcquireWorkerLeaseResponse> {
    const validated: AcquireWorkerLeaseRequest = AcquireWorkerLeaseRequestSchema.parse(payload);
    return this.postJson(
      "/api/v1/fleet/leases/acquire",
      validated,
      AcquireWorkerLeaseResponseSchema
    );
  }

  dispatchSwarm(
    payload: z.input<typeof DispatchFleetSwarmRequestSchema>
  ): Promise<DispatchFleetSwarmResponse> {
    const validated: DispatchFleetSwarmRequest = DispatchFleetSwarmRequestSchema.parse(payload);
    return this.postJson(
      "/api/v1/fleet/dispatch",
      validated,
      DispatchFleetSwarmResponseSchema
    );
  }

  evaluateDesignJury(
    payload: z.input<typeof EvaluateDesignJuryRequestSchema>
  ): Promise<EvaluateDesignJuryResponse> {
    const validated: EvaluateDesignJuryRequest = EvaluateDesignJuryRequestSchema.parse(payload);
    return this.postJson(
      "/api/v1/fleet/jury/evaluate",
      validated,
      EvaluateDesignJuryResponseSchema
    );
  }
}

export function createFleetSwarmApiClient(
  config: HarnessDaemonClientConfig = {}
): FleetSwarmApiClient {
  return new FleetSwarmApiClient(config);
}
