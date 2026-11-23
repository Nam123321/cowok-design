// @story 09.1
// @implements AC-01, AC-02, AC-03, AC-04 (Domain SDK for Two-Store /rea Bridge with Runtime Zod .parse())
import { z } from "zod";
import {
  type ReaCatalogQuery,
  ReaCatalogQuerySchema,
  type ReaCatalogResponse,
  ReaCatalogResponseSchema,
  type ReaExportProofRequest,
  ReaExportProofRequestSchema,
  type ReaExportProofResponse,
  ReaExportProofResponseSchema,
  type ReaIngestRequest,
  ReaIngestRequestSchema,
  type ReaIngestResponse,
  ReaIngestResponseSchema,
  type ReaSearchQuery,
  ReaSearchQuerySchema,
  type ReaSearchResponse,
  ReaSearchResponseSchema,
} from "@/lib/validations/rea";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class ReaApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly field?: string;
  readonly details?: Record<string, unknown>;

  constructor(
    status: number,
    code: string,
    message: string,
    field?: string,
    details?: Record<string, unknown>
  ) {
    super(message);
    this.name = "ReaApiError";
    this.status = status;
    this.code = code;
    this.field = field;
    this.details = details;
  }
}

async function parseReaError(res: Response): Promise<ReaApiError> {
  try {
    const body = (await res.json()) as {
      code?: string;
      message?: string;
      error?:
        | { code?: string; message?: string; field?: string; details?: Record<string, unknown> }
        | string;
    };
    if (body?.error && typeof body.error === "object") {
      return new ReaApiError(
        res.status,
        body.error.code ?? "REQUEST_FAILED",
        body.error.message ?? `Request failed (${res.status})`,
        body.error.field,
        body.error.details
      );
    }
    const flatCode = body?.code ?? (typeof body?.error === "string" ? body.error : "REQUEST_FAILED");
    const flatMsg = body?.message ?? `Request failed (${res.status})`;
    return new ReaApiError(res.status, flatCode, flatMsg);
  } catch {
    return new ReaApiError(res.status, "REQUEST_FAILED", `Request failed (${res.status})`);
  }
}

export class ReaApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseReaError(res);
    return schema.parse(await res.json());
  }

  ingestReference(payload: ReaIngestRequest): Promise<ReaIngestResponse> {
    const validated = ReaIngestRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/rea/ingest",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      ReaIngestResponseSchema
    );
  }

  searchReferences(params: Partial<ReaSearchQuery> = {}): Promise<ReaSearchResponse> {
    const validated = ReaSearchQuerySchema.parse(params);
    const qs = new URLSearchParams({
      q: validated.q,
      limit: String(validated.limit),
      tier: validated.tier,
    }).toString();
    return this.requestValidated(
      `/api/v1/rea/search?${qs}`,
      { method: "GET", headers: { Accept: "application/json" } },
      ReaSearchResponseSchema
    );
  }

  searchCatalog(params: Partial<ReaCatalogQuery> = {}): Promise<ReaCatalogResponse> {
    const validated = ReaCatalogQuerySchema.parse(params);
    const qs = new URLSearchParams({
      q: validated.q,
      limit: String(validated.limit),
    }).toString();
    return this.requestValidated(
      `/api/v1/rea/catalog?${qs}`,
      { method: "GET", headers: { Accept: "application/json" } },
      ReaCatalogResponseSchema
    );
  }

  exportFigmaProof(payload: ReaExportProofRequest): Promise<ReaExportProofResponse> {
    const validated = ReaExportProofRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/rea/export-proof",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      ReaExportProofResponseSchema
    );
  }
}

const defaultReaClient = new ReaApiClient();

export function ingestReaReference(payload: ReaIngestRequest): Promise<ReaIngestResponse> {
  return defaultReaClient.ingestReference(payload);
}

export function searchReaReferences(
  params: Partial<ReaSearchQuery> = {}
): Promise<ReaSearchResponse> {
  return defaultReaClient.searchReferences(params);
}

export function searchAwesomeReCatalog(
  params: Partial<ReaCatalogQuery> = {}
): Promise<ReaCatalogResponse> {
  return defaultReaClient.searchCatalog(params);
}

export function exportReaFigmaProof(
  payload: ReaExportProofRequest
): Promise<ReaExportProofResponse> {
  return defaultReaClient.exportFigmaProof(payload);
}
