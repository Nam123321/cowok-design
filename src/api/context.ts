// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import {
  ContextAssembleOutcome,
  ContextAssembleRequest,
  ContextAssembleRequestSchema,
  ContextAssembleSuccessSchema,
  ContextTelemetryOutcome,
  ContextTelemetryPageSchema,
  ContextTelemetryQuery,
  ContextTelemetryQuerySchema,
} from "../lib/validations/context";
import { ContextApiError, ContextApiErrorSchema } from "../lib/validations/context-errors";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

const HTTP_SERVICE_UNAVAILABLE = 503;
const UUID_BYTE_LEN = 16;
const UUID_VERSION_INDEX = 6;
const UUID_VARIANT_INDEX = 8;
const UUID_LOW_NIBBLE_MASK = 0x0f;
const UUID_VERSION4_BITS = 0x40;
const UUID_VARIANT_MASK = 0x3f;
const UUID_VARIANT_RFC4122 = 0x80;
const HEX_RADIX = 16;
const HEX_PAD_LEN = 2;
const SLICE_TIME_HI = 4;
const SLICE_CLK_HI = 6;
const SLICE_NODE_START = 8;
const SLICE_NODE_END = 10;

function nextUuidV4(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  if (typeof globalThis.crypto?.getRandomValues === "function") {
    const bytes = new Uint8Array(UUID_BYTE_LEN);
    globalThis.crypto.getRandomValues(bytes);
    bytes[UUID_VERSION_INDEX] = (bytes[UUID_VERSION_INDEX] & UUID_LOW_NIBBLE_MASK) | UUID_VERSION4_BITS;
    bytes[UUID_VARIANT_INDEX] = (bytes[UUID_VARIANT_INDEX] & UUID_VARIANT_MASK) | UUID_VARIANT_RFC4122;
    const hex = Array.from(bytes, (b) => b.toString(HEX_RADIX).padStart(HEX_PAD_LEN, "0"));
    return [
      hex.slice(0, SLICE_TIME_HI).join(""),
      hex.slice(SLICE_TIME_HI, SLICE_CLK_HI).join(""),
      hex.slice(SLICE_CLK_HI, SLICE_NODE_START).join(""),
      hex.slice(SLICE_NODE_START, SLICE_NODE_END).join(""),
      hex.slice(SLICE_NODE_END).join(""),
    ].join("-");
  }
  throw new Error("CSPRNG crypto.randomUUID/getRandomValues unavailable");
}

function makeContractError(status: number, fallbackMsg?: string): ContextApiError {
  return {
    ok: false,
    error: {
      code:
        status >= HTTP_SERVICE_UNAVAILABLE
          ? "TELEMETRY_STORAGE_UNAVAILABLE"
          : "DAEMON_CONTRACT_VIOLATION",
      message: fallbackMsg || "contextMeter.errors.daemonContractViolation",
      retryable: true,
    },
    requestId: nextUuidV4(),
  };
}

function makeInvalidRequestError(field?: string): ContextApiError {
  return {
    ok: false,
    error: {
      code: "INVALID_REQUEST",
      message: "contextMeter.errors.invalidRequest",
      ...(field ? { field } : {}),
      retryable: false,
    },
    requestId: nextUuidV4(),
  };
}

export class ContextClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  async assembleContext(
    input: ContextAssembleRequest,
    options?: { idempotencyKey?: string }
  ): Promise<ContextAssembleOutcome> {
    const parsedInput = ContextAssembleRequestSchema.safeParse(input);
    if (!parsedInput.success) {
      const firstPath = parsedInput.error.issues[0]?.path[0];
      return makeInvalidRequestError(firstPath !== undefined ? String(firstPath) : undefined);
    }
    const idempotencyKey = options?.idempotencyKey || nextUuidV4();
    try {
      const resp = await this.fetchAuthedResponse("/api/v1/context/assemble", {
        method: "POST",
        headers: {
          "Idempotency-Key": idempotencyKey,
          "X-Correlation-Id": nextUuidV4(),
        },
        body: JSON.stringify(parsedInput.data),
      });
      const raw: unknown = await resp.json();
      if (resp.ok) {
        const parsed = ContextAssembleSuccessSchema.safeParse(raw);
        return parsed.success ? parsed.data : makeContractError(resp.status);
      }
      const errParsed = ContextApiErrorSchema.safeParse(raw);
      return errParsed.success ? errParsed.data : makeContractError(resp.status);
    } catch {
      return makeContractError(
        HTTP_SERVICE_UNAVAILABLE,
        "contextMeter.errors.telemetryStorageUnavailable"
      );
    }
  }

  async getContextTelemetry(query: ContextTelemetryQuery): Promise<ContextTelemetryOutcome> {
    const parsedQuery = ContextTelemetryQuerySchema.safeParse(query);
    if (!parsedQuery.success) {
      const firstPath = parsedQuery.error.issues[0]?.path[0];
      return makeInvalidRequestError(firstPath !== undefined ? String(firstPath) : undefined);
    }
    const validated = parsedQuery.data;
    const qs = new URLSearchParams({ projectId: validated.projectId });
    if (validated.portalId) qs.set("portalId", validated.portalId);
    if (validated.modelProfile) qs.set("modelProfile", validated.modelProfile);
    if (validated.cursor) qs.set("cursor", validated.cursor);
    qs.set("limit", String(validated.limit));
    try {
      const resp = await this.fetchAuthedResponse(
        `/api/v1/context/telemetry?${qs.toString()}`,
        {
          method: "GET",
          headers: { "X-Correlation-Id": nextUuidV4() },
        }
      );
      const raw: unknown = await resp.json();
      if (resp.ok) {
        const parsed = ContextTelemetryPageSchema.safeParse(raw);
        return parsed.success ? parsed.data : makeContractError(resp.status);
      }
      const errParsed = ContextApiErrorSchema.safeParse(raw);
      return errParsed.success ? errParsed.data : makeContractError(resp.status);
    } catch {
      return makeContractError(
        HTTP_SERVICE_UNAVAILABLE,
        "contextMeter.errors.telemetryStorageUnavailable"
      );
    }
  }
}

export const apiContext = new ContextClient();
export const assembleContext = (
  input: ContextAssembleRequest,
  options?: { idempotencyKey?: string }
): Promise<ContextAssembleOutcome> => apiContext.assembleContext(input, options);
export const getContextTelemetry = (
  query: ContextTelemetryQuery
): Promise<ContextTelemetryOutcome> => apiContext.getContextTelemetry(query);
