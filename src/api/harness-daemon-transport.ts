// @story 01.3, 04.2
// @implements AC-01, AC-02, AC-03, AC-05, AC-06
import { z } from "zod";

const CORRELATION_RADIX = 16;
const CORRELATION_SLICE_START = 2;
const CORRELATION_SLICE_END = 10;
const HTTP_STATUS_NO_CONTENT = 204;
const HTTP_STATUS_UNAUTHORIZED = 401;
const HTTP_STATUS_BAD_GATEWAY = 502;

export interface HarnessDaemonClientConfig {
  baseUrl?: string;
  getSessionToken?: () => string | null;
  role?: "viewer" | "editor";
  tenantId?: string;
  userId?: string;
  fetchImpl?: typeof fetch;
}

export class HarnessDaemonApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "HarnessDaemonApiError";
    this.status = status;
    this.code = code;
  }
}

function nextCorrelationId(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  const suffix = Math.random()
    .toString(CORRELATION_RADIX)
    .slice(CORRELATION_SLICE_START, CORRELATION_SLICE_END);
  return `corr-${Date.now()}-${suffix}`;
}

async function parseErrorResponse(res: Response): Promise<HarnessDaemonApiError> {
  let code = "HTTP_ERROR";
  let message = `Request failed with HTTP ${res.status}`;
  try {
    const errBody = (await res.json()) as { code?: string; error?: string; message?: string };
    code = errBody.code || errBody.error || code;
    message = errBody.message || message;
  } catch {
    // Preserve status-based fallback when response is not valid JSON
  }
  return new HarnessDaemonApiError(res.status, code, message);
}

export class HarnessDaemonTransport {
  private readonly baseUrl: string;
  private readonly getSessionToken?: () => string | null;
  private readonly role: "viewer" | "editor";
  private readonly tenantId?: string;
  private readonly userId?: string;
  private readonly fetchImpl?: typeof fetch;
  private cachedToken: string | null = null;
  private inflightBootstrap: Promise<string> | null = null;

  constructor(config: HarnessDaemonClientConfig = {}) {
    this.baseUrl = config.baseUrl || "";
    this.getSessionToken = config.getSessionToken;
    this.role = config.role || "editor";
    this.tenantId = config.tenantId;
    this.userId = config.userId;
    this.fetchImpl = config.fetchImpl;
  }

  private resolveFetch(): typeof fetch {
    return this.fetchImpl || globalThis.fetch.bind(globalThis);
  }

  resetSessionCache(): void {
    this.cachedToken = null;
    this.inflightBootstrap = null;
  }

  private async bootstrapToken(): Promise<string> {
    const doFetch = this.resolveFetch();
    const res = await doFetch(`${this.baseUrl}/api/auth/local-session`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ tenantId: this.tenantId, userId: this.userId, role: this.role }),
    });
    if (!res.ok) {
      throw new HarnessDaemonApiError(
        res.status,
        "SESSION_BOOTSTRAP_FAILED",
        `Session bootstrap failed with HTTP ${res.status}`
      );
    }
    const json = (await res.json()) as { sessionToken?: string };
    const tok = typeof json.sessionToken === "string" ? json.sessionToken : "";
    if (!tok) {
      throw new HarnessDaemonApiError(
        HTTP_STATUS_BAD_GATEWAY,
        "SESSION_BOOTSTRAP_EMPTY",
        "Session bootstrap response missing sessionToken"
      );
    }
    this.cachedToken = tok;
    return tok;
  }

  private async ensureSessionToken(forceRefresh = false): Promise<string> {
    if (!forceRefresh && this.getSessionToken) {
      const injected = this.getSessionToken();
      if (injected) return injected;
    }
    if (!forceRefresh && this.cachedToken) return this.cachedToken;
    if (!forceRefresh && this.inflightBootstrap) return this.inflightBootstrap;

    this.inflightBootstrap = this.bootstrapToken().finally(() => {
      this.inflightBootstrap = null;
    });
    return this.inflightBootstrap;
  }

  async request<T>(
    path: string,
    schema: z.ZodType<T, z.ZodTypeDef, unknown> | null,
    options: RequestInit = {},
    retried = false
  ): Promise<T> {
    const token = await this.ensureSessionToken(retried);
    const headers = new Headers(options.headers || {});
    if (options.body !== undefined && !headers.has("Content-Type")) {
      headers.set("Content-Type", "application/json");
    }
    headers.set("X-Cowok-Session-Token", token);
    if (!headers.has("X-Correlation-Id")) {
      headers.set("X-Correlation-Id", nextCorrelationId());
    }

    const doFetch = this.resolveFetch();
    const res = await doFetch(`${this.baseUrl}${path}`, { ...options, headers });

    if (res.status === HTTP_STATUS_UNAUTHORIZED && !retried) {
      this.cachedToken = null;
      return this.request(path, schema, options, true);
    }
    if (!res.ok) {
      throw await parseErrorResponse(res);
    }
    if (schema === null || res.status === HTTP_STATUS_NO_CONTENT) {
      return undefined as T;
    }
    const rawText = await res.text();
    if (!rawText.trim()) {
      return undefined as T;
    }
    return schema.parse(JSON.parse(rawText));
  }
}
