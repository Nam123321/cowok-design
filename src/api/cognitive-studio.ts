// @story 05.6
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
import type { z } from "zod";
import {
  type BrandProposalsResponse,
  BrandProposalsResponseSchema,
  type CreateHeritagePackRequest,
  CreateHeritagePackRequestSchema,
  type CreateHeritagePackResponse,
  CreateHeritagePackResponseSchema,
  type FeDataContractResponse,
  FeDataContractResponseSchema,
  type GenerateBrandProposalsRequest,
  GenerateBrandProposalsRequestSchema,
  type HeritagePacksListResponse,
  HeritagePacksListResponseSchema,
  type HiddenSpecsListResponse,
  HiddenSpecsListResponseSchema,
  type PutHiddenSpecRequest,
  PutHiddenSpecRequestSchema,
  type PutHiddenSpecResponse,
  PutHiddenSpecResponseSchema,
  type RevealSpecsFolderRequest,
  RevealSpecsFolderRequestSchema,
  type RevealSpecsFolderResponse,
  RevealSpecsFolderResponseSchema,
  type ScanComponentsRequest,
  ScanComponentsRequestSchema,
  type ScanComponentsResponse,
  ScanComponentsResponseSchema,
} from "@/lib/validations/cognitive-studio";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class CognitiveStudioApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly details?: Record<string, unknown>;

  constructor(status: number, code: string, message: string, details?: Record<string, unknown>) {
    super(message);
    this.name = "CognitiveStudioApiError";
    this.status = status;
    this.code = code;
    this.details = details;
  }
}

async function parseStudioError(res: Response): Promise<CognitiveStudioApiError> {
  try {
    const body = (await res.json()) as {
      error?: { code?: string; message?: string; details?: Record<string, unknown> };
    };
    return new CognitiveStudioApiError(
      res.status,
      body?.error?.code ?? "REQUEST_FAILED",
      body?.error?.message ?? `Request failed (${res.status})`,
      body?.error?.details
    );
  } catch {
    return new CognitiveStudioApiError(res.status, "REQUEST_FAILED", `Request failed (${res.status})`);
  }
}

export class CognitiveStudioApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseStudioError(res);
    return schema.parse(await res.json());
  }

  generateBrandProposals(
    payload: GenerateBrandProposalsRequest,
    signal?: AbortSignal
  ): Promise<BrandProposalsResponse> {
    const validated = GenerateBrandProposalsRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/cognitive/brand-proposals",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
        signal,
      },
      BrandProposalsResponseSchema
    );
  }

  scanSketchComponents(
    payload: ScanComponentsRequest,
    signal?: AbortSignal
  ): Promise<ScanComponentsResponse> {
    const validated = ScanComponentsRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/cognitive/scan-components",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
        signal,
      },
      ScanComponentsResponseSchema
    );
  }

  getFeDataContract(projectId: string, screenId = "default"): Promise<FeDataContractResponse> {
    const qs = new URLSearchParams({ projectId, screenId }).toString();
    return this.requestValidated(
      `/api/v1/cognitive/fe-data-contract?${qs}`,
      { method: "GET", headers: { Accept: "application/json" } },
      FeDataContractResponseSchema
    );
  }

  getHiddenSpecs(projectId: string): Promise<HiddenSpecsListResponse> {
    const qs = new URLSearchParams({ projectId }).toString();
    return this.requestValidated(
      `/api/v1/cognitive/hidden-specs?${qs}`,
      { method: "GET", headers: { Accept: "application/json" } },
      HiddenSpecsListResponseSchema
    );
  }

  putHiddenSpec(payload: PutHiddenSpecRequest): Promise<PutHiddenSpecResponse> {
    const validated = PutHiddenSpecRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/cognitive/hidden-specs",
      {
        method: "PUT",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      PutHiddenSpecResponseSchema
    );
  }

  listHeritagePacks(): Promise<HeritagePacksListResponse> {
    return this.requestValidated(
      "/api/v1/cognitive/heritage-packs",
      { method: "GET", headers: { Accept: "application/json" } },
      HeritagePacksListResponseSchema
    );
  }

  saveHeritagePack(payload: CreateHeritagePackRequest): Promise<CreateHeritagePackResponse> {
    const validated = CreateHeritagePackRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/cognitive/heritage-packs",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      CreateHeritagePackResponseSchema
    );
  }

  revealSpecsFolder(payload: RevealSpecsFolderRequest): Promise<RevealSpecsFolderResponse> {
    const validated = RevealSpecsFolderRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/cognitive/specs/reveal",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      RevealSpecsFolderResponseSchema
    );
  }
}

const defaultCognitiveStudioClient = new CognitiveStudioApiClient();

export function generateBrandProposals(
  payload: GenerateBrandProposalsRequest,
  signal?: AbortSignal
): Promise<BrandProposalsResponse> {
  return defaultCognitiveStudioClient.generateBrandProposals(payload, signal);
}

export function scanSketchComponentsApi(
  payload: ScanComponentsRequest,
  signal?: AbortSignal
): Promise<ScanComponentsResponse> {
  return defaultCognitiveStudioClient.scanSketchComponents(payload, signal);
}

export function getFeDataContract(
  projectId: string,
  screenId = "default"
): Promise<FeDataContractResponse> {
  return defaultCognitiveStudioClient.getFeDataContract(projectId, screenId);
}

export function getHiddenSpecs(projectId: string): Promise<HiddenSpecsListResponse> {
  return defaultCognitiveStudioClient.getHiddenSpecs(projectId);
}

export function putHiddenSpec(payload: PutHiddenSpecRequest): Promise<PutHiddenSpecResponse> {
  return defaultCognitiveStudioClient.putHiddenSpec(payload);
}

export function listHeritagePacks(): Promise<HeritagePacksListResponse> {
  return defaultCognitiveStudioClient.listHeritagePacks();
}

export function saveHeritagePack(
  payload: CreateHeritagePackRequest
): Promise<CreateHeritagePackResponse> {
  return defaultCognitiveStudioClient.saveHeritagePack(payload);
}

export function revealSpecsFolder(
  payload: RevealSpecsFolderRequest
): Promise<RevealSpecsFolderResponse> {
  return defaultCognitiveStudioClient.revealSpecsFolder(payload);
}
