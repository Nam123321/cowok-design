// @story 07.2
// @implements AC-01, AC-02, AC-03, AC-04
import type { z } from "zod";
import {
  type DeveloperHandoffRequest,
  DeveloperHandoffRequestSchema,
  type DeveloperHandoffResponse,
  DeveloperHandoffResponseSchema,
  type ExportHistoryResponse,
  ExportHistoryResponseSchema,
  type HtmlBundleExportRequest,
  HtmlBundleExportRequestSchema,
  type HtmlBundleExportResponse,
  HtmlBundleExportResponseSchema,
  type PptxExportRequest,
  PptxExportRequestSchema,
  type PptxExportResponse,
  PptxExportResponseSchema,
  SplitExportErrorEnvelopeSchema,
  type VectorPdfExportRequest,
  VectorPdfExportRequestSchema,
  type VectorPdfExportResponse,
  VectorPdfExportResponseSchema,
} from "../lib/validations/split-export";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class SplitExportApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly p0Count?: number;
  readonly p1Count?: number;
  readonly limitBytes?: number;
  readonly actualBytes?: number;

  constructor(params: {
    status: number;
    code: string;
    message: string;
    p0Count?: number;
    p1Count?: number;
    limitBytes?: number;
    actualBytes?: number;
  }) {
    super(params.message);
    this.name = "SplitExportApiError";
    this.status = params.status;
    this.code = params.code;
    this.p0Count = params.p0Count;
    this.p1Count = params.p1Count;
    this.limitBytes = params.limitBytes;
    this.actualBytes = params.actualBytes;
  }
}

async function parseSplitExportError(res: Response): Promise<SplitExportApiError> {
  try {
    const raw = await res.json();
    const parsed = SplitExportErrorEnvelopeSchema.safeParse(raw);
    if (parsed.success) {
      return new SplitExportApiError({
        status: res.status,
        code: parsed.data.error,
        message: parsed.data.message,
        p0Count: parsed.data.p0Count,
        p1Count: parsed.data.p1Count,
        limitBytes: parsed.data.limitBytes,
        actualBytes: parsed.data.actualBytes,
      });
    }
    const fallback = raw as {
      error?: string | { code?: string; message?: string };
      message?: string;
    };
    const code =
      typeof fallback?.error === "string"
        ? fallback.error
        : fallback?.error?.code ?? "EXPORT_REQUEST_FAILED";
    const message =
      (typeof fallback?.error === "object" ? fallback.error?.message : undefined) ??
      fallback?.message ??
      `Export request failed (${res.status})`;
    return new SplitExportApiError({ status: res.status, code, message });
  } catch {
    return new SplitExportApiError({
      status: res.status,
      code: "EXPORT_REQUEST_FAILED",
      message: `Export request failed (${res.status})`,
    });
  }
}

export class SplitExportApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseSplitExportError(res);
    return schema.parse(await res.json());
  }

  exportOfflineHtml(
    payload: HtmlBundleExportRequest,
    signal?: AbortSignal
  ): Promise<HtmlBundleExportResponse> {
    const validated = HtmlBundleExportRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/export/html-bundle",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
        signal,
      },
      HtmlBundleExportResponseSchema
    );
  }

  exportVectorPdf(
    payload: VectorPdfExportRequest,
    signal?: AbortSignal
  ): Promise<VectorPdfExportResponse> {
    const validated = VectorPdfExportRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/export/vector-pdf",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
        signal,
      },
      VectorPdfExportResponseSchema
    );
  }

  exportPptx(
    payload: PptxExportRequest,
    signal?: AbortSignal
  ): Promise<PptxExportResponse> {
    const validated = PptxExportRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/export/pptx",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
        signal,
      },
      PptxExportResponseSchema
    );
  }

  exportDeveloperHandoff(
    payload: DeveloperHandoffRequest,
    signal?: AbortSignal
  ): Promise<DeveloperHandoffResponse> {
    const validated = DeveloperHandoffRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/export/developer-handoff",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
        signal,
      },
      DeveloperHandoffResponseSchema
    );
  }

  getExportHistory(
    projectId: string,
    limit = 20,
    signal?: AbortSignal
  ): Promise<ExportHistoryResponse> {
    const qs = new URLSearchParams({
      projectId,
      limit: String(limit),
    });
    return this.requestValidated(
      `/api/v1/export/history?${qs.toString()}`,
      {
        method: "GET",
        headers: { Accept: "application/json" },
        signal,
      },
      ExportHistoryResponseSchema
    );
  }
}

export const apiSplitExport = new SplitExportApiClient();
export default apiSplitExport;
