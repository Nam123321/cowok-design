// @story 05.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { z } from "zod";
import {
  type ClarifyDirectionRequest,
  type ClarifyDirectionResponse,
  ClarifyDirectionRequestSchema,
  ClarifyDirectionResponseSchema,
  type GetClarifySessionResponse,
  GetClarifySessionResponseSchema,
  type ScreenPatchHistoryResponse,
  ScreenPatchHistoryResponseSchema,
  type SurgicalPatchRequest,
  type SurgicalPatchResponse,
  SurgicalPatchRequestSchema,
  SurgicalPatchResponseSchema,
} from "@/lib/validations/sketch-polish";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class SketchPolishApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details?: Record<string, unknown>;

  constructor(status: number, code: string, message: string, details?: Record<string, unknown>) {
    super(message);
    this.name = "SketchPolishApiError";
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

async function parseErrorPayload(res: Response): Promise<SketchPolishApiError> {
  try {
    const body = (await res.json()) as {
      error?: { code?: string; message?: string; details?: Record<string, unknown> };
    };
    const code = body?.error?.code ?? "REQUEST_FAILED";
    const msg = body?.error?.message ?? `Request failed (${res.status})`;
    return new SketchPolishApiError(res.status, code, msg, body?.error?.details);
  } catch {
    return new SketchPolishApiError(res.status, "REQUEST_FAILED", `Request failed (${res.status})`);
  }
}

export class SketchPolishApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseErrorPayload(res);
    return schema.parse(await res.json());
  }

  getClarifySession(projectId: string): Promise<GetClarifySessionResponse> {
    const qs = new URLSearchParams({ projectId }).toString();
    return this.requestValidated(
      `/api/v1/sdi/clarify?${qs}`,
      { method: "GET", headers: { Accept: "application/json" } },
      GetClarifySessionResponseSchema
    );
  }

  postClarifyDirection(payload: ClarifyDirectionRequest): Promise<ClarifyDirectionResponse> {
    const validated = ClarifyDirectionRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/sdi/clarify",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      ClarifyDirectionResponseSchema
    );
  }

  getScreenPatchHistory(projectId: string, screenId: string): Promise<ScreenPatchHistoryResponse> {
    const qs = new URLSearchParams({ projectId, screenId }).toString();
    return this.requestValidated(
      `/api/v1/patch?${qs}`,
      { method: "GET", headers: { Accept: "application/json" } },
      ScreenPatchHistoryResponseSchema
    );
  }

  applySurgicalPatch(payload: SurgicalPatchRequest): Promise<SurgicalPatchResponse> {
    const validated = SurgicalPatchRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/patch",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      SurgicalPatchResponseSchema
    );
  }
}

const defaultSketchPolishClient = new SketchPolishApiClient();

export function getClarifySession(projectId: string): Promise<GetClarifySessionResponse> {
  return defaultSketchPolishClient.getClarifySession(projectId);
}

export function postClarifyDirection(
  payload: ClarifyDirectionRequest
): Promise<ClarifyDirectionResponse> {
  return defaultSketchPolishClient.postClarifyDirection(payload);
}

export function getScreenPatchHistory(
  projectId: string,
  screenId: string
): Promise<ScreenPatchHistoryResponse> {
  return defaultSketchPolishClient.getScreenPatchHistory(projectId, screenId);
}

export function applySurgicalPatch(
  payload: SurgicalPatchRequest
): Promise<SurgicalPatchResponse> {
  return defaultSketchPolishClient.applySurgicalPatch(payload);
}
