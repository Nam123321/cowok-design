// @story 02.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { z } from "zod";
import {
  type AppShellLockMutationResponse,
  AppShellLockMutationResponseSchema,
  type AppShellLockRequest,
  AppShellLockRequestSchema,
  type SkillRouteRequest,
  SkillRouteRequestSchema,
  type SkillRouteResponse,
  SkillRouteResponseSchema,
  type SsotApiError,
  SsotApiErrorSchema,
  type SsotComponentsQuery,
  SsotComponentsQuerySchema,
  type SsotComponentsResponse,
  SsotComponentsResponseSchema,
  type SsotExportRequest,
  SsotExportRequestSchema,
  type SsotExportResponse,
  SsotExportResponseSchema,
} from "../lib/validations/ssot-registry";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export type SsotSdkOutcome<T> =
  | { ok: true; data: T }
  | { ok: false; status: number; error: SsotApiError["error"]; retryAfterSeconds?: number };

const HTTP_UNAUTHORIZED = 401;
const HTTP_FORBIDDEN = 403;

function mergePortalHeaders(initHeaders: HeadersInit | undefined, portalId: string): Record<string, string> {
  const merged = new Headers(initHeaders);
  merged.set("x-cowok-portal", portalId);
  const out: Record<string, string> = {};
  merged.forEach((value, key) => {
    out[key] = value;
  });
  return out;
}

function classifyFallbackSsotError(
  status: number,
  raw: unknown
): SsotApiError["error"] {
  const fallbackMsg =
    typeof raw === "object" &&
    raw !== null &&
    typeof (raw as { message?: unknown }).message === "string"
      ? (raw as { message: string }).message
      : `HTTP ${status}`;
  let code: SsotApiError["error"]["code"] = "INTERNAL_ERROR";
  if (status === HTTP_UNAUTHORIZED) {
    code = "UNAUTHORIZED";
  } else if (status === HTTP_FORBIDDEN) {
    code = "AUTH_FORBIDDEN";
  }
  return { code, message: fallbackMsg, requestId: "client-fallback" };
}

export class SsotRegistryApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async executeValidated<T>(
    path: string,
    init: RequestInit,
    portalId: string,
    responseSchema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<SsotSdkOutcome<T>> {
    const res = await this.fetchAuthedResponse(path, {
      ...init,
      headers: mergePortalHeaders(init.headers, portalId),
    });
    const raw = await res.json().catch(() => null);
    if (res.ok) {
      const parsed = responseSchema.parse(raw);
      return { ok: true, data: parsed };
    }
    const errParsed = SsotApiErrorSchema.safeParse(raw);
    const retryAfterRaw = res.headers.get("retry-after");
    const retryAfterSeconds = retryAfterRaw ? Number(retryAfterRaw) : undefined;
    if (errParsed.success) {
      return {
        ok: false,
        status: res.status,
        error: errParsed.data.error,
        retryAfterSeconds: Number.isFinite(retryAfterSeconds) ? retryAfterSeconds : undefined,
      };
    }
    return {
      ok: false,
      status: res.status,
      error: classifyFallbackSsotError(res.status, raw),
    };
  }

  async fetchComponents(
    query: SsotComponentsQuery
  ): Promise<SsotSdkOutcome<SsotComponentsResponse>> {
    const validated = SsotComponentsQuerySchema.parse(query);
    const qs = new URLSearchParams({
      projectId: validated.projectId,
      portalId: validated.portalId,
      limit: String(validated.limit),
      ...(validated.cursor ? { cursor: validated.cursor } : {}),
    });
    return this.executeValidated(
      `/api/v1/ssot/components?${qs.toString()}`,
      { method: "GET" },
      validated.portalId,
      SsotComponentsResponseSchema
    );
  }

  async mutateAppShellLock(
    input: AppShellLockRequest
  ): Promise<SsotSdkOutcome<AppShellLockMutationResponse>> {
    const validated = AppShellLockRequestSchema.parse(input);
    return this.executeValidated(
      "/api/v1/ssot/appshell/lock",
      {
        method: "POST",
        body: JSON.stringify(validated),
      },
      validated.portalId,
      AppShellLockMutationResponseSchema
    );
  }

  async exportSsotBundle(
    input: SsotExportRequest
  ): Promise<SsotSdkOutcome<SsotExportResponse>> {
    const validated = SsotExportRequestSchema.parse(input);
    return this.executeValidated(
      "/api/v1/ssot/export",
      {
        method: "POST",
        body: JSON.stringify(validated),
      },
      validated.portalId,
      SsotExportResponseSchema
    );
  }

  async routeDesignSkills(
    input: SkillRouteRequest
  ): Promise<SsotSdkOutcome<SkillRouteResponse>> {
    const validated = SkillRouteRequestSchema.parse(input);
    return this.executeValidated(
      "/api/v1/skills/route",
      {
        method: "POST",
        body: JSON.stringify(validated),
      },
      validated.portalId,
      SkillRouteResponseSchema
    );
  }
}

const defaultSsotClient = new SsotRegistryApiClient();

export function fetchSsotComponents(
  query: SsotComponentsQuery,
  config?: HarnessDaemonClientConfig
): Promise<SsotSdkOutcome<SsotComponentsResponse>> {
  return (config ? new SsotRegistryApiClient(config) : defaultSsotClient).fetchComponents(query);
}

export function mutateAppShellLock(
  input: AppShellLockRequest,
  config?: HarnessDaemonClientConfig
): Promise<SsotSdkOutcome<AppShellLockMutationResponse>> {
  return (config ? new SsotRegistryApiClient(config) : defaultSsotClient).mutateAppShellLock(input);
}

export function exportSsotBundle(
  input: SsotExportRequest,
  config?: HarnessDaemonClientConfig
): Promise<SsotSdkOutcome<SsotExportResponse>> {
  return (config ? new SsotRegistryApiClient(config) : defaultSsotClient).exportSsotBundle(input);
}

export function routeDesignSkills(
  input: SkillRouteRequest,
  config?: HarnessDaemonClientConfig
): Promise<SsotSdkOutcome<SkillRouteResponse>> {
  return (config ? new SsotRegistryApiClient(config) : defaultSsotClient).routeDesignSkills(input);
}
