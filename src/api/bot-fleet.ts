// @story 11.1
// @implements AC-01, AC-02, AC-04 (Typed Bot Fleet Domain SDK extending HarnessDaemonTransport)
import { z } from "zod";
import {
  type BotFleetExecuteRequest,
  type BotFleetExecuteResponse,
  BotFleetExecuteRequestSchema,
  BotFleetExecuteResponseSchema,
  type BotFleetProvidersStatusResponse,
  BotFleetProvidersStatusResponseSchema,
} from "@/lib/validations/bot-fleet";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class BotFleetApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details?: Record<string, unknown>;

  constructor(status: number, code: string, message: string, details?: Record<string, unknown>) {
    super(message);
    this.name = "BotFleetApiError";
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

async function parseBotFleetError(res: Response): Promise<BotFleetApiError> {
  try {
    const body = (await res.json()) as {
      error?: { code?: string; message?: string; details?: Record<string, unknown> };
    };
    const code = body?.error?.code ?? "REQUEST_FAILED";
    const message = body?.error?.message ?? `Request failed (${res.status})`;
    return new BotFleetApiError(res.status, code, message, body?.error?.details);
  } catch {
    return new BotFleetApiError(res.status, "REQUEST_FAILED", `Request failed (${res.status})`);
  }
}

export class BotFleetApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseBotFleetError(res);
    return schema.parse(await res.json());
  }

  executePrompt(
    payload: Omit<BotFleetExecuteRequest, "mode" | "timeoutMs"> &
      Partial<Pick<BotFleetExecuteRequest, "mode" | "timeoutMs">>
  ): Promise<BotFleetExecuteResponse> {
    const validated = BotFleetExecuteRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/bot-fleet/execute",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      BotFleetExecuteResponseSchema
    );
  }

  getProvidersStatus(): Promise<BotFleetProvidersStatusResponse> {
    return this.requestValidated(
      "/api/v1/bot-fleet/providers/status",
      { method: "GET", headers: { Accept: "application/json" } },
      BotFleetProvidersStatusResponseSchema
    );
  }
}

export const apiBotFleet = new BotFleetApiClient();

export function executeBotFleetPrompt(
  payload: Omit<BotFleetExecuteRequest, "mode" | "timeoutMs"> &
    Partial<Pick<BotFleetExecuteRequest, "mode" | "timeoutMs">>
): Promise<BotFleetExecuteResponse> {
  return apiBotFleet.executePrompt(payload);
}

export function getBotFleetProvidersStatus(): Promise<BotFleetProvidersStatusResponse> {
  return apiBotFleet.getProvidersStatus();
}
