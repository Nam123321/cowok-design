// @story 14.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Typed Domain SDK for Cloud Control Plane & SuperAdmin Portal with Runtime Zod .parse())
import { z } from "zod";
import {
  type AdminAuthSessionRequest,
  AdminAuthSessionRequestSchema,
  type AdminAuthSessionResponse,
  AdminAuthSessionResponseSchema,
  type AnalyticsFunnelResponse,
  AnalyticsFunnelResponseSchema,
  type CreateClusterRequest,
  CreateClusterRequestSchema,
  type CreateClusterResponse,
  CreateClusterResponseSchema,
  type IngestTelemetryRequest,
  IngestTelemetryRequestSchema,
  type IngestTelemetryResponse,
  IngestTelemetryResponseSchema,
  type IngestTicketRequest,
  IngestTicketRequestSchema,
  type IngestTicketResponse,
  IngestTicketResponseSchema,
  type ListAdminTicketsResponse,
  ListAdminTicketsResponseSchema,
  type ListClustersResponse,
  ListClustersResponseSchema,
  type ListCrashesResponse,
  ListCrashesResponseSchema,
  type ReplyAdminTicketRequest,
  ReplyAdminTicketRequestSchema,
  type ReplyAdminTicketResponse,
  ReplyAdminTicketResponseSchema,
} from "../lib/validations/cloud-admin";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
  readBootCapHeaders,
} from "./harness-daemon-transport";

const HTTP_STATUS_UNAUTHORIZED = 401;
const HTTP_STATUS_BAD_GATEWAY = 502;
const CORRELATION_RADIX = 16;
const CORRELATION_SLICE_START = 2;
const CORRELATION_SLICE_END = 10;
const LOOPBACK_RETRYABLE_CODES = new Set(["UNAUTHENTICATED", "INVALID_LOCAL_SESSION"]);

export interface CloudAdminTicketFilters {
  status?: string;
  priority?: string;
  channel?: string;
  hostClient?: string;
  category?: string;
  search?: string;
  limit?: number;
}

export interface CloudAdminCrashFilters {
  appVersion?: string;
  status?: string;
  limit?: number;
}

export class CloudAdminApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly field?: string;

  constructor(status: number, code: string, message: string, field?: string) {
    super(message);
    this.name = "CloudAdminApiError";
    this.status = status;
    this.code = code;
    this.field = field;
  }
}

async function parseCloudAdminError(res: Response): Promise<CloudAdminApiError> {
  try {
    const body = (await res.json()) as {
      code?: string;
      message?: string;
      error?: { code?: string; message?: string; field?: string } | string;
    };
    if (body?.error && typeof body.error === "object") {
      return new CloudAdminApiError(
        res.status,
        body.error.code ?? "CLOUD_ADMIN_ERROR",
        body.error.message ?? `Cloud Admin request failed (${res.status})`,
        body.error.field
      );
    }
    const flatCode =
      body?.code ?? (typeof body?.error === "string" ? body.error : "CLOUD_ADMIN_ERROR");
    return new CloudAdminApiError(
      res.status,
      flatCode,
      body?.message ?? `Cloud Admin request failed (${res.status})`
    );
  } catch {
    return new CloudAdminApiError(
      res.status,
      "CLOUD_ADMIN_ERROR",
      `Cloud Admin request failed (${res.status})`
    );
  }
}

function buildAdminHeaders(adminSessionToken?: string | null, withJson = false): Record<string, string> {
  const headers: Record<string, string> = { Accept: "application/json" };
  if (withJson) headers["Content-Type"] = "application/json";
  const trimmed = adminSessionToken?.trim();
  if (trimmed) {
    headers["X-Admin-Session"] = trimmed;
    headers.Authorization = `Bearer ${trimmed}`;
  }
  return headers;
}

function encodeQueryParams(entries: Array<[string, string | number | undefined]>): string {
  const params = new URLSearchParams();
  for (const [k, v] of entries) {
    if (v !== undefined && String(v).trim() !== "" && v !== "all" && v !== "ALL") {
      params.set(k, String(v));
    }
  }
  const qs = params.toString();
  return qs ? `?${qs}` : "";
}

function nextCloudCorrelationId(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  const suffix = Math.random().toString(CORRELATION_RADIX).slice(CORRELATION_SLICE_START, CORRELATION_SLICE_END);
  return `corr-${Date.now()}-${suffix}`;
}

async function shouldRetryLoopbackAuth(res: Response): Promise<boolean> {
  try {
    const cloned = res.clone();
    const body = (await cloned.json()) as {
      code?: string;
      error?: { code?: string } | string;
    };
    const code =
      body?.error && typeof body.error === "object"
        ? body.error.code
        : body?.code ?? (typeof body?.error === "string" ? body.error : undefined);
    return typeof code === "string" && LOOPBACK_RETRYABLE_CODES.has(code);
  } catch {
    return false;
  }
}

export class CloudAdminApiClient extends HarnessDaemonTransport {
  private readonly cloudConfig: HarnessDaemonClientConfig;
  private adminToken: string | null = null;
  private localSessionToken: string | null = null;
  private inflightLocalBootstrap: Promise<string> | null = null;

  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
    this.cloudConfig = config;
  }

  override resetSessionCache(): void {
    this.localSessionToken = null;
    this.inflightLocalBootstrap = null;
    super.resetSessionCache();
  }

  setAdminSessionToken(token: string | null): void {
    this.adminToken = token?.trim() || null;
  }

  getAdminSessionToken(): string | null {
    return this.adminToken;
  }

  private resolveCloudFetch(): typeof fetch {
    return this.cloudConfig.fetchImpl || globalThis.fetch.bind(globalThis);
  }

  private async bootstrapLocalSession(): Promise<string> {
    const doFetch = this.resolveCloudFetch();
    const baseUrl = this.cloudConfig.baseUrl || "";
    const res = await doFetch(`${baseUrl}/api/auth/local-session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", ...readBootCapHeaders() },
      body: JSON.stringify({
        tenantId: this.cloudConfig.tenantId,
        userId: this.cloudConfig.userId,
        role: this.cloudConfig.role || "editor",
      }),
    });
    if (!res.ok) {
      throw new CloudAdminApiError(res.status, "SESSION_BOOTSTRAP_FAILED", `Session bootstrap failed (${res.status})`);
    }
    const json = (await res.json()) as { sessionToken?: string };
    const tok = typeof json.sessionToken === "string" ? json.sessionToken : "";
    if (!tok) {
      throw new CloudAdminApiError(
        HTTP_STATUS_BAD_GATEWAY,
        "SESSION_BOOTSTRAP_EMPTY",
        "Session bootstrap response missing sessionToken"
      );
    }
    this.localSessionToken = tok;
    return tok;
  }

  private async ensureLocalSessionToken(forceRefresh = false): Promise<string> {
    if (!forceRefresh && this.cloudConfig.getSessionToken) {
      const injected = this.cloudConfig.getSessionToken();
      if (injected) return injected;
    }
    if (!forceRefresh && this.localSessionToken) return this.localSessionToken;
    if (this.inflightLocalBootstrap) return this.inflightLocalBootstrap;
    if (forceRefresh) this.localSessionToken = null;
    const pending = this.bootstrapLocalSession().finally(() => {
      if (this.inflightLocalBootstrap === pending) this.inflightLocalBootstrap = null;
    });
    this.inflightLocalBootstrap = pending;
    return pending;
  }

  private async fetchCloudResponse(path: string, init: RequestInit, retried = false): Promise<Response> {
    const token = await this.ensureLocalSessionToken(retried);
    const headers = new Headers(init.headers || {});
    if (init.body !== undefined && !headers.has("Content-Type")) {
      headers.set("Content-Type", "application/json");
    }
    headers.set("X-Cowok-Session-Token", token);
    if (!headers.has("X-Correlation-Id")) {
      headers.set("X-Correlation-Id", nextCloudCorrelationId());
    }
    const doFetch = this.resolveCloudFetch();
    const res = await doFetch(`${this.cloudConfig.baseUrl || ""}${path}`, { ...init, headers });
    if (res.status === HTTP_STATUS_UNAUTHORIZED && !retried && (await shouldRetryLoopbackAuth(res))) {
      this.localSessionToken = null;
      return this.fetchCloudResponse(path, init, true);
    }
    return res;
  }

  private async requestParsed<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchCloudResponse(path, init);
    if (!res.ok) throw await parseCloudAdminError(res);
    return schema.parse(await res.json());
  }

  async createAdminSession(
    payload: AdminAuthSessionRequest,
    existingSessionToken?: string | null
  ): Promise<AdminAuthSessionResponse> {
    const validated = AdminAuthSessionRequestSchema.parse(payload);
    const resp = await this.requestParsed(
      "/api/v1/cloud-admin/auth/session",
      {
        method: "POST",
        headers: buildAdminHeaders(existingSessionToken ?? undefined, true),
        body: JSON.stringify(validated),
      },
      AdminAuthSessionResponseSchema
    );
    this.setAdminSessionToken(resp.data.sessionToken);
    return resp;
  }

  async fetchTicketAttachment(ticketId: string, sessionToken?: string | null): Promise<Blob> {
    const headers = buildAdminHeaders(sessionToken ?? this.adminToken);
    headers.Accept = "image/*";
    const res = await this.fetchCloudResponse(
      `/api/v1/cloud-admin/tickets/${encodeURIComponent(ticketId)}/attachment`,
      { method: "GET", headers }
    );
    if (!res.ok) throw await parseCloudAdminError(res);
    return res.blob();
  }

  ingestTicket(payload: IngestTicketRequest): Promise<IngestTicketResponse> {
    const validated = IngestTicketRequestSchema.parse(payload);
    return this.requestParsed(
      "/api/v1/cloud-admin/ingest/tickets",
      {
        method: "POST",
        headers: buildAdminHeaders(undefined, true),
        body: JSON.stringify(validated),
      },
      IngestTicketResponseSchema
    );
  }

  ingestTelemetry(payload: IngestTelemetryRequest): Promise<IngestTelemetryResponse> {
    const validated = IngestTelemetryRequestSchema.parse(payload);
    return this.requestParsed(
      "/api/v1/cloud-admin/ingest/telemetry",
      {
        method: "POST",
        headers: buildAdminHeaders(undefined, true),
        body: JSON.stringify(validated),
      },
      IngestTelemetryResponseSchema
    );
  }

  listTickets(
    filters: CloudAdminTicketFilters = {},
    sessionToken?: string | null
  ): Promise<ListAdminTicketsResponse> {
    const qs = encodeQueryParams([
      ["status", filters.status],
      ["priority", filters.priority],
      ["channel", filters.channel],
      ["hostClient", filters.hostClient],
      ["category", filters.category],
      ["search", filters.search],
      ["limit", filters.limit],
    ]);
    return this.requestParsed(
      `/api/v1/cloud-admin/tickets${qs}`,
      { method: "GET", headers: buildAdminHeaders(sessionToken ?? this.adminToken) },
      ListAdminTicketsResponseSchema
    );
  }

  replyToTicket(
    ticketId: string,
    payload: ReplyAdminTicketRequest,
    sessionToken?: string | null
  ): Promise<ReplyAdminTicketResponse> {
    const validated = ReplyAdminTicketRequestSchema.parse(payload);
    return this.requestParsed(
      `/api/v1/cloud-admin/tickets/${encodeURIComponent(ticketId)}/reply`,
      {
        method: "POST",
        headers: buildAdminHeaders(sessionToken ?? this.adminToken, true),
        body: JSON.stringify(validated),
      },
      ReplyAdminTicketResponseSchema
    );
  }

  listClusters(sessionToken?: string | null): Promise<ListClustersResponse> {
    return this.requestParsed(
      "/api/v1/cloud-admin/clusters",
      { method: "GET", headers: buildAdminHeaders(sessionToken ?? this.adminToken) },
      ListClustersResponseSchema
    );
  }

  createCluster(
    payload: CreateClusterRequest,
    sessionToken?: string | null
  ): Promise<CreateClusterResponse> {
    const validated = CreateClusterRequestSchema.parse(payload);
    return this.requestParsed(
      "/api/v1/cloud-admin/clusters",
      {
        method: "POST",
        headers: buildAdminHeaders(sessionToken ?? this.adminToken, true),
        body: JSON.stringify(validated),
      },
      CreateClusterResponseSchema
    );
  }

  getAnalyticsFunnel(sessionToken?: string | null): Promise<AnalyticsFunnelResponse> {
    return this.requestParsed(
      "/api/v1/cloud-admin/analytics/funnel",
      { method: "GET", headers: buildAdminHeaders(sessionToken ?? this.adminToken) },
      AnalyticsFunnelResponseSchema
    );
  }

  listCrashes(
    filters: CloudAdminCrashFilters = {},
    sessionToken?: string | null
  ): Promise<ListCrashesResponse> {
    const qs = encodeQueryParams([
      ["appVersion", filters.appVersion],
      ["status", filters.status],
      ["limit", filters.limit],
    ]);
    return this.requestParsed(
      `/api/v1/cloud-admin/crashes${qs}`,
      { method: "GET", headers: buildAdminHeaders(sessionToken ?? this.adminToken) },
      ListCrashesResponseSchema
    );
  }
}

export const apiCloudAdmin = new CloudAdminApiClient();
