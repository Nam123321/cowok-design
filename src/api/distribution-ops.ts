// @story 14.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Typed Domain SDK for 4-Channel Distribution Ops, Plugin Catalog, A/B Experiments, Quarantine, Offline Licenses & Audit Logs with Runtime Zod .parse())
import { z } from "zod";
import {
  type BulkUpdateCatalogRequest,
  BulkUpdateCatalogRequestSchema,
  type BulkUpdateCatalogResponse,
  BulkUpdateCatalogResponseSchema,
  type ClientEdgeSyncRequest,
  ClientEdgeSyncRequestSchema,
  type ClientEdgeSyncResponse,
  ClientEdgeSyncResponseSchema,
  type CowokOrgFederationRequest,
  CowokOrgFederationRequestSchema,
  type CowokOrgFederationResponse,
  CowokOrgFederationResponseSchema,
  type IssueLicenseRequest,
  IssueLicenseRequestSchema,
  type IssueLicenseResponse,
  IssueLicenseResponseSchema,
  type IssueQuarantineRequest,
  IssueQuarantineRequestSchema,
  type IssueQuarantineResponse,
  IssueQuarantineResponseSchema,
  type ListCatalogResponse,
  ListCatalogResponseSchema,
  type ListDistributionAuditLogsResponse,
  ListDistributionAuditLogsResponseSchema,
  type ListExperimentsResponse,
  ListExperimentsResponseSchema,
  type ListLicensesResponse,
  ListLicensesResponseSchema,
  type ListQuarantinesResponse,
  ListQuarantinesResponseSchema,
  type ListRolloutsResponse,
  ListRolloutsResponseSchema,
  type OfflineLicensePayload,
  OfflineLicensePayloadSchema,
  type PublishRolloutRequest,
  PublishRolloutRequestSchema,
  type PublishRolloutResponse,
  PublishRolloutResponseSchema,
  type SignedAdvisoriesManifest,
  SignedAdvisoriesManifestSchema,
  type SignedCatalogManifest,
  SignedCatalogManifestSchema,
  type SignedExperimentsManifest,
  SignedExperimentsManifestSchema,
  type SignedLatestReleaseManifest,
  SignedLatestReleaseManifestSchema,
  type UpsertCatalogPluginRequest,
  UpsertCatalogPluginRequestSchema,
  type UpsertCatalogPluginResponse,
  UpsertCatalogPluginResponseSchema,
  type UpsertExperimentRequest,
  UpsertExperimentRequestSchema,
  type UpsertExperimentResponse,
  UpsertExperimentResponseSchema,
  createSignedEnvelopeSchema,
} from "../lib/validations/distribution-ops";
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
const BASE64_PAD_MOD = 4;
const MS_PER_SECOND = 1000;
const LOOPBACK_RETRYABLE_CODES = new Set(["UNAUTHENTICATED", "INVALID_LOCAL_SESSION"]);
const SignedLicenseEnvelopeSchema = createSignedEnvelopeSchema(OfflineLicensePayloadSchema);

export class DistributionOpsApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly field?: string;

  constructor(status: number, code: string, message: string, field?: string) {
    super(message);
    this.name = "DistributionOpsApiError";
    this.status = status;
    this.code = code;
    this.field = field;
  }
}

export interface OfflineLicenseInspectionResult {
  validFormat: boolean;
  signatureVerified: boolean;
  expired: boolean;
  keyId?: string;
  publicKeyBase64?: string;
  signaturePreview?: string;
  payload?: OfflineLicensePayload;
  reason?: string;
}

const ED25519_PUBLIC_KEY_BYTES = 32;
const ED25519_SIGNATURE_BYTES = 64;
const EXPECTED_MANIFEST_SCHEMA_VERSION = 1;

function decodeBase64Bytes(raw: string): Uint8Array {
  const normalized = raw.trim().replace(/-/g, "+").replace(/_/g, "/");
  const padLen = (BASE64_PAD_MOD - (normalized.length % BASE64_PAD_MOD)) % BASE64_PAD_MOD;
  const padded = normalized + "=".repeat(padLen);
  const binary = globalThis.atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

function decodeBase64UrlUtf8(raw: string): string {
  return new TextDecoder().decode(decodeBase64Bytes(raw));
}

function toExactArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  const copy = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(copy).set(bytes);
  return copy;
}

function canonicalizeJsonValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(canonicalizeJsonValue);
  }
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([, v]) => v !== undefined)
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
    const sorted: Record<string, unknown> = {};
    for (const [k, v] of entries) {
      sorted[k] = canonicalizeJsonValue(v);
    }
    return sorted;
  }
  return value;
}

function bytesEqualConstantTime(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i += 1) {
    diff |= a[i] ^ b[i];
  }
  return diff === 0;
}

/** Browser-safe fail-closed Ed25519 offline license verifier (zero Node Buffer usage). */
export async function inspectOfflineLicenseToken(
  token: string,
  trustedPublicKeyBase64: string,
  nowUnixSec = Math.floor(Date.now() / MS_PER_SECOND)
): Promise<OfflineLicenseInspectionResult> {
  const trimmed = token.trim();
  if (!trimmed.startsWith("cwk_lic_v1.")) {
    return {
      validFormat: false,
      signatureVerified: false,
      expired: false,
      reason: "MISSING_TOKEN_PREFIX",
    };
  }
  const trustedB64 = trustedPublicKeyBase64.trim();
  if (!trustedB64) {
    return {
      validFormat: false,
      signatureVerified: false,
      expired: false,
      reason: "MISSING_TRUSTED_PUBLIC_KEY",
    };
  }
  let parsed: z.infer<typeof SignedLicenseEnvelopeSchema>;
  try {
    const encoded = trimmed.slice("cwk_lic_v1.".length);
    const jsonText = decodeBase64UrlUtf8(encoded);
    parsed = SignedLicenseEnvelopeSchema.parse(JSON.parse(jsonText));
  } catch {
    return {
      validFormat: false,
      signatureVerified: false,
      expired: false,
      reason: "MALFORMED_ENVELOPE",
    };
  }

  const baseMeta = {
    validFormat: true,
    keyId: parsed.keyId,
    publicKeyBase64: parsed.publicKeyBase64,
    signaturePreview: parsed.signatureBase64.slice(0, 20),
    payload: parsed.payload,
  };

  if (
    parsed.schemaVersion !== EXPECTED_MANIFEST_SCHEMA_VERSION ||
    parsed.manifestKind !== "license" ||
    parsed.manifestSeq < 1 ||
    !parsed.keyId.trim() ||
    parsed.issuedAt > parsed.expiresAt ||
    parsed.payload.issuedAt > parsed.payload.expiresAt
  ) {
    return {
      ...baseMeta,
      signatureVerified: false,
      expired: false,
      reason: "INVALID_ENVELOPE_CONSTRAINTS",
    };
  }

  const recomputedCanonical = JSON.stringify(canonicalizeJsonValue(parsed.payload));
  if (recomputedCanonical !== parsed.payloadCanonicalJson) {
    return {
      ...baseMeta,
      signatureVerified: false,
      expired: false,
      reason: "TAMPERED_PAYLOAD_CANONICAL_JSON",
    };
  }

  const subtle = globalThis.crypto?.subtle;
  if (!subtle) {
    return {
      ...baseMeta,
      signatureVerified: false,
      expired: false,
      reason: "WEBCRYPTO_ED25519_UNAVAILABLE",
    };
  }

  try {
    const trustedPkBytes = decodeBase64Bytes(trustedB64);
    const envelopePkBytes = decodeBase64Bytes(parsed.publicKeyBase64);
    const sigBytes = decodeBase64Bytes(parsed.signatureBase64);
    if (
      trustedPkBytes.length !== ED25519_PUBLIC_KEY_BYTES ||
      envelopePkBytes.length !== ED25519_PUBLIC_KEY_BYTES ||
      sigBytes.length !== ED25519_SIGNATURE_BYTES
    ) {
      return {
        ...baseMeta,
        signatureVerified: false,
        expired: false,
        reason: "INVALID_KEY_OR_SIGNATURE_LENGTH",
      };
    }
    if (!bytesEqualConstantTime(trustedPkBytes, envelopePkBytes)) {
      return {
        ...baseMeta,
        signatureVerified: false,
        expired: false,
        reason: "UNTRUSTED_PUBLIC_KEY",
      };
    }

    const signingInput = `v${parsed.schemaVersion}:${parsed.keyId}:${parsed.manifestKind}:${parsed.manifestSeq}:${parsed.issuedAt}:${parsed.expiresAt}:${parsed.payloadCanonicalJson}`;
    const msgBytes = new TextEncoder().encode(signingInput);
    const cryptoKey = await subtle.importKey(
      "raw",
      toExactArrayBuffer(trustedPkBytes),
      { name: "Ed25519" },
      false,
      ["verify"]
    );
    const signatureValid = await subtle.verify(
      "Ed25519",
      cryptoKey,
      toExactArrayBuffer(sigBytes),
      toExactArrayBuffer(msgBytes)
    );
    const expired = parsed.expiresAt <= nowUnixSec || parsed.payload.expiresAt <= nowUnixSec;
    return {
      ...baseMeta,
      signatureVerified: signatureValid,
      expired,
      reason: signatureValid ? (expired ? "EXPIRED_LICENSE" : undefined) : "TAMPERED_SIGNATURE",
    };
  } catch {
    return {
      ...baseMeta,
      signatureVerified: false,
      expired: false,
      reason: "WEBCRYPTO_VERIFY_FAILED",
    };
  }
}

async function parseDistOpsError(res: Response): Promise<DistributionOpsApiError> {
  try {
    const body = (await res.json()) as {
      code?: string;
      message?: string;
      error?: { code?: string; message?: string; field?: string } | string;
    };
    if (body?.error && typeof body.error === "object") {
      return new DistributionOpsApiError(
        res.status,
        body.error.code ?? "DISTRIBUTION_OPS_ERROR",
        body.error.message ?? `Distribution request failed (${res.status})`,
        body.error.field
      );
    }
    const flatCode =
      body?.code ?? (typeof body?.error === "string" ? body.error : "DISTRIBUTION_OPS_ERROR");
    return new DistributionOpsApiError(
      res.status,
      flatCode,
      body?.message ?? `Distribution request failed (${res.status})`
    );
  } catch {
    return new DistributionOpsApiError(
      res.status,
      "DISTRIBUTION_OPS_ERROR",
      `Distribution request failed (${res.status})`
    );
  }
}

function buildAdminHeaders(sessionToken?: string | null, withJson = false): Record<string, string> {
  const headers: Record<string, string> = { Accept: "application/json" };
  if (withJson) headers["Content-Type"] = "application/json";
  const trimmed = sessionToken?.trim();
  if (trimmed) {
    headers["X-Admin-Session"] = trimmed;
    headers.Authorization = `Bearer ${trimmed}`;
  }
  return headers;
}

function nextCorrelationId(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  const suffix = Math.random()
    .toString(CORRELATION_RADIX)
    .slice(CORRELATION_SLICE_START, CORRELATION_SLICE_END);
  return `dist-${Date.now()}-${suffix}`;
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

export class DistributionOpsApiClient extends HarnessDaemonTransport {
  private readonly distConfig: HarnessDaemonClientConfig;
  private distLocalSessionToken: string | null = null;
  private inflightDistBootstrap: Promise<string> | null = null;

  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
    this.distConfig = config;
  }

  override resetSessionCache(): void {
    this.distLocalSessionToken = null;
    this.inflightDistBootstrap = null;
    super.resetSessionCache();
  }

  private resolveDistFetch(): typeof fetch {
    return this.distConfig.fetchImpl || globalThis.fetch.bind(globalThis);
  }

  private async bootstrapDistLocalSession(): Promise<string> {
    const doFetch = this.resolveDistFetch();
    const baseUrl = this.distConfig.baseUrl || "";
    const res = await doFetch(`${baseUrl}/api/auth/local-session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", ...readBootCapHeaders() },
      body: JSON.stringify({
        tenantId: this.distConfig.tenantId,
        userId: this.distConfig.userId,
        role: this.distConfig.role || "editor",
      }),
    });
    if (!res.ok) {
      throw new DistributionOpsApiError(
        res.status,
        "SESSION_BOOTSTRAP_FAILED",
        `Session bootstrap failed (${res.status})`
      );
    }
    const json = (await res.json()) as { sessionToken?: string };
    const tok = typeof json.sessionToken === "string" ? json.sessionToken : "";
    if (!tok) {
      throw new DistributionOpsApiError(
        HTTP_STATUS_BAD_GATEWAY,
        "SESSION_BOOTSTRAP_EMPTY",
        "Session bootstrap missing sessionToken"
      );
    }
    this.distLocalSessionToken = tok;
    return tok;
  }

  private async ensureDistLocalSessionToken(forceRefresh = false): Promise<string> {
    if (!forceRefresh && this.distConfig.getSessionToken) {
      const injected = this.distConfig.getSessionToken();
      if (injected) return injected;
    }
    if (!forceRefresh && this.distLocalSessionToken) return this.distLocalSessionToken;
    if (this.inflightDistBootstrap) return this.inflightDistBootstrap;
    if (forceRefresh) this.distLocalSessionToken = null;
    const pending = this.bootstrapDistLocalSession().finally(() => {
      if (this.inflightDistBootstrap === pending) this.inflightDistBootstrap = null;
    });
    this.inflightDistBootstrap = pending;
    return pending;
  }

  private async fetchDistResponse(path: string, init: RequestInit, retried = false): Promise<Response> {
    const token = await this.ensureDistLocalSessionToken(retried);
    const headers = new Headers(init.headers || {});
    if (init.body !== undefined && !headers.has("Content-Type")) {
      headers.set("Content-Type", "application/json");
    }
    headers.set("X-Cowok-Session-Token", token);
    if (!headers.has("X-Correlation-Id")) {
      headers.set("X-Correlation-Id", nextCorrelationId());
    }
    const doFetch = this.resolveDistFetch();
    const res = await doFetch(`${this.distConfig.baseUrl || ""}${path}`, { ...init, headers });
    if (res.status === HTTP_STATUS_UNAUTHORIZED && !retried && (await shouldRetryLoopbackAuth(res))) {
      this.distLocalSessionToken = null;
      return this.fetchDistResponse(path, init, true);
    }
    return res;
  }

  private async requestDistParsed<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchDistResponse(path, init);
    if (!res.ok) throw await parseDistOpsError(res);
    return schema.parse(await res.json());
  }

  listRollouts(sessionToken: string): Promise<ListRolloutsResponse> {
    return this.requestDistParsed(
      "/api/v1/distribution-ops/releases/rollout",
      { method: "GET", headers: buildAdminHeaders(sessionToken) },
      ListRolloutsResponseSchema
    );
  }

  publishRollout(
    payload: PublishRolloutRequest,
    sessionToken: string
  ): Promise<PublishRolloutResponse> {
    const validated = PublishRolloutRequestSchema.parse(payload);
    return this.requestDistParsed(
      "/api/v1/distribution-ops/releases/rollout",
      {
        method: "POST",
        headers: buildAdminHeaders(sessionToken, true),
        body: JSON.stringify(validated),
      },
      PublishRolloutResponseSchema
    );
  }

  getLatestReleaseManifest(channel = "desktop", installId?: string): Promise<SignedLatestReleaseManifest> {
    const params = new URLSearchParams({ channel });
    if (installId) params.set("installId", installId);
    return this.requestDistParsed(
      `/api/v1/distribution-ops/releases/latest?${params.toString()}`,
      { method: "GET", headers: buildAdminHeaders(undefined) },
      SignedLatestReleaseManifestSchema
    );
  }

  listCatalog(sessionToken: string): Promise<ListCatalogResponse> {
    return this.requestDistParsed(
      "/api/v1/distribution-ops/skills/catalog",
      { method: "GET", headers: buildAdminHeaders(sessionToken) },
      ListCatalogResponseSchema
    );
  }

  upsertCatalogPlugin(
    payload: UpsertCatalogPluginRequest,
    sessionToken: string
  ): Promise<UpsertCatalogPluginResponse> {
    const validated = UpsertCatalogPluginRequestSchema.parse(payload);
    return this.requestDistParsed(
      "/api/v1/distribution-ops/skills/catalog",
      {
        method: "POST",
        headers: buildAdminHeaders(sessionToken, true),
        body: JSON.stringify(validated),
      },
      UpsertCatalogPluginResponseSchema
    );
  }

  bulkUpdateCatalog(
    payload: BulkUpdateCatalogRequest,
    sessionToken: string
  ): Promise<BulkUpdateCatalogResponse> {
    const validated = BulkUpdateCatalogRequestSchema.parse(payload);
    return this.requestDistParsed(
      "/api/v1/distribution-ops/skills/catalog/bulk",
      {
        method: "POST",
        headers: buildAdminHeaders(sessionToken, true),
        body: JSON.stringify(validated),
      },
      BulkUpdateCatalogResponseSchema
    );
  }

  getSignedCatalogManifest(): Promise<SignedCatalogManifest> {
    return this.requestDistParsed(
      "/api/v1/distribution-ops/skills/catalog-manifest",
      { method: "GET", headers: buildAdminHeaders(undefined) },
      SignedCatalogManifestSchema
    );
  }

  listExperiments(sessionToken: string): Promise<ListExperimentsResponse> {
    return this.requestDistParsed(
      "/api/v1/distribution-ops/experiments",
      { method: "GET", headers: buildAdminHeaders(sessionToken) },
      ListExperimentsResponseSchema
    );
  }

  upsertExperiment(
    payload: UpsertExperimentRequest,
    sessionToken: string
  ): Promise<UpsertExperimentResponse> {
    const validated = UpsertExperimentRequestSchema.parse(payload);
    return this.requestDistParsed(
      "/api/v1/distribution-ops/experiments",
      {
        method: "POST",
        headers: buildAdminHeaders(sessionToken, true),
        body: JSON.stringify(validated),
      },
      UpsertExperimentResponseSchema
    );
  }

  getSignedExperimentsManifest(): Promise<SignedExperimentsManifest> {
    return this.requestDistParsed(
      "/api/v1/distribution-ops/experiments-manifest",
      { method: "GET", headers: buildAdminHeaders(undefined) },
      SignedExperimentsManifestSchema
    );
  }

  listQuarantines(sessionToken: string): Promise<ListQuarantinesResponse> {
    return this.requestDistParsed(
      "/api/v1/distribution-ops/skills/quarantine",
      { method: "GET", headers: buildAdminHeaders(sessionToken) },
      ListQuarantinesResponseSchema
    );
  }

  issueQuarantine(
    payload: IssueQuarantineRequest,
    sessionToken: string
  ): Promise<IssueQuarantineResponse> {
    const validated = IssueQuarantineRequestSchema.parse(payload);
    return this.requestDistParsed(
      "/api/v1/distribution-ops/skills/quarantine",
      {
        method: "POST",
        headers: buildAdminHeaders(sessionToken, true),
        body: JSON.stringify(validated),
      },
      IssueQuarantineResponseSchema
    );
  }

  getSignedAdvisoriesManifest(): Promise<SignedAdvisoriesManifest> {
    return this.requestDistParsed(
      "/api/v1/distribution-ops/skills/advisories-manifest",
      { method: "GET", headers: buildAdminHeaders(undefined) },
      SignedAdvisoriesManifestSchema
    );
  }

  listLicenses(sessionToken: string): Promise<ListLicensesResponse> {
    return this.requestDistParsed(
      "/api/v1/distribution-ops/licenses",
      { method: "GET", headers: buildAdminHeaders(sessionToken) },
      ListLicensesResponseSchema
    );
  }

  issueLicense(
    payload: IssueLicenseRequest,
    sessionToken: string
  ): Promise<IssueLicenseResponse> {
    const validated = IssueLicenseRequestSchema.parse(payload);
    return this.requestDistParsed(
      "/api/v1/distribution-ops/licenses",
      {
        method: "POST",
        headers: buildAdminHeaders(sessionToken, true),
        body: JSON.stringify(validated),
      },
      IssueLicenseResponseSchema
    );
  }

  listAuditLogs(sessionToken: string): Promise<ListDistributionAuditLogsResponse> {
    return this.requestDistParsed(
      "/api/v1/distribution-ops/audit-logs",
      { method: "GET", headers: buildAdminHeaders(sessionToken) },
      ListDistributionAuditLogsResponseSchema
    );
  }

  syncClientEdge(payload: ClientEdgeSyncRequest): Promise<ClientEdgeSyncResponse> {
    const validated = ClientEdgeSyncRequestSchema.parse(payload);
    return this.requestDistParsed(
      "/api/v1/distribution-ops/edge/sync",
      {
        method: "POST",
        headers: buildAdminHeaders(undefined, true),
        body: JSON.stringify(validated),
      },
      ClientEdgeSyncResponseSchema
    );
  }

  verifyCowokOrgFederation(
    payload: CowokOrgFederationRequest
  ): Promise<CowokOrgFederationResponse> {
    const validated = CowokOrgFederationRequestSchema.parse(payload);
    return this.requestDistParsed(
      "/api/v1/distribution-ops/federation/cowok-org",
      {
        method: "POST",
        headers: buildAdminHeaders(undefined, true),
        body: JSON.stringify(validated),
      },
      CowokOrgFederationResponseSchema
    );
  }
}

export type DistributionOpsApiPort = Pick<
  DistributionOpsApiClient,
  | "listRollouts"
  | "publishRollout"
  | "listCatalog"
  | "upsertCatalogPlugin"
  | "bulkUpdateCatalog"
  | "listExperiments"
  | "upsertExperiment"
  | "listQuarantines"
  | "issueQuarantine"
  | "listLicenses"
  | "issueLicense"
  | "listAuditLogs"
>;

export const apiDistributionOps = new DistributionOpsApiClient();
