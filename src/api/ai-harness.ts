// @story 08.7
// @implements AC-01, AC-03, AC-04 (Typed AI Harness Spine SDK with Mandatory Runtime .parse())
import { z } from "zod";
import {
  AiHarnessErrorEnvelopeSchema,
  type ModelsRegistryResponse,
  ModelsRegistryEnvelopeSchema,
  ModelsRegistryResponseSchema,
  type RecordStudioCorrectionRequest,
  RecordStudioCorrectionEnvelopeSchema,
  RecordStudioCorrectionRequestSchema,
  type RecordStudioCorrectionResponse,
  RecordStudioCorrectionResponseSchema,
  type SkillCatalogResponse,
  SkillCatalogEnvelopeSchema,
  SkillCatalogResponseSchema,
} from "../lib/validations/ai-harness";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class AiHarnessApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "AiHarnessApiError";
    this.status = status;
    this.code = code;
  }
}

async function parseAiHarnessError(res: Response): Promise<AiHarnessApiError> {
  try {
    const raw: unknown = await res.json();
    const parsed = AiHarnessErrorEnvelopeSchema.safeParse(raw);
    if (parsed.success) {
      return new AiHarnessApiError(res.status, parsed.data.error.code, parsed.data.error.message);
    }
  } catch {
    // Fallback to HTTP status message
  }
  return new AiHarnessApiError(res.status, "REQUEST_FAILED", `AI Harness request failed (${res.status})`);
}

function unwrapEnvelopeOrRaw<T>(
  raw: unknown,
  envelopeSchema: z.ZodType<{ ok: true; data: T; requestId: string }, z.ZodTypeDef, unknown>,
  dataSchema: z.ZodType<T, z.ZodTypeDef, unknown>
): T {
  const env = envelopeSchema.safeParse(raw);
  if (env.success) {
    return dataSchema.parse(env.data.data);
  }
  return dataSchema.parse(raw);
}

export class AiHarnessApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  async getSkillsCatalog(opts?: { q?: string; projectId?: string }): Promise<SkillCatalogResponse> {
    const params = new URLSearchParams();
    if (opts?.q?.trim()) params.set("q", opts.q.trim());
    if (opts?.projectId?.trim()) params.set("projectId", opts.projectId.trim());
    const qs = params.toString();
    const path = qs ? `/api/v1/skills/catalog?${qs}` : "/api/v1/skills/catalog";
    const res = await this.fetchAuthedResponse(path, {
      method: "GET",
      headers: { Accept: "application/json" },
    });
    if (!res.ok) throw await parseAiHarnessError(res);
    return unwrapEnvelopeOrRaw(await res.json(), SkillCatalogEnvelopeSchema, SkillCatalogResponseSchema);
  }

  async reloadSkillsCatalog(projectId?: string): Promise<SkillCatalogResponse> {
    const qs = projectId?.trim() ? `?${new URLSearchParams({ projectId: projectId.trim() }).toString()}` : "";
    const res = await this.fetchAuthedResponse(`/api/v1/skills/catalog/reload${qs}`, {
      method: "POST",
      headers: { Accept: "application/json" },
    });
    if (!res.ok) throw await parseAiHarnessError(res);
    return unwrapEnvelopeOrRaw(await res.json(), SkillCatalogEnvelopeSchema, SkillCatalogResponseSchema);
  }

  async getModelsRegistry(providerId?: string): Promise<ModelsRegistryResponse> {
    const qs = providerId?.trim()
      ? `?${new URLSearchParams({ providerId: providerId.trim() }).toString()}`
      : "";
    const res = await this.fetchAuthedResponse(`/api/v1/models/registry${qs}`, {
      method: "GET",
      headers: { Accept: "application/json" },
    });
    if (!res.ok) throw await parseAiHarnessError(res);
    return unwrapEnvelopeOrRaw(
      await res.json(),
      ModelsRegistryEnvelopeSchema,
      ModelsRegistryResponseSchema
    );
  }

  async recordStudioCorrection(
    payload: RecordStudioCorrectionRequest
  ): Promise<RecordStudioCorrectionResponse> {
    const validated = RecordStudioCorrectionRequestSchema.parse(payload);
    const res = await this.fetchAuthedResponse("/api/v1/learn/studio-corrections", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
      },
      body: JSON.stringify(validated),
    });
    if (!res.ok) throw await parseAiHarnessError(res);
    return unwrapEnvelopeOrRaw(
      await res.json(),
      RecordStudioCorrectionEnvelopeSchema,
      RecordStudioCorrectionResponseSchema
    );
  }
}

const defaultClient = new AiHarnessApiClient();

export const getSkillsCatalog = (opts?: { q?: string; projectId?: string }) =>
  defaultClient.getSkillsCatalog(opts);
export const reloadSkillsCatalog = (projectId?: string) =>
  defaultClient.reloadSkillsCatalog(projectId);
export const getModelsRegistry = (providerId?: string) =>
  defaultClient.getModelsRegistry(providerId);
export const recordStudioCorrection = (payload: RecordStudioCorrectionRequest) =>
  defaultClient.recordStudioCorrection(payload);
