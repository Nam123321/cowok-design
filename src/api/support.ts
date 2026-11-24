// @story 14.1
// @implements AC-01, AC-02, AC-03, AC-04 (Typed Domain SDK for Non-Tech Support Tickets & Telemetry Preferences with Runtime Zod .parse())
import { z } from "zod";
import {
  type CreateSupportTicketRequest,
  CreateSupportTicketRequestSchema,
  type CreateSupportTicketResponse,
  CreateSupportTicketResponseSchema,
  DEFAULT_SUPPORT_TICKETS_LIMIT,
  type ListSupportTicketsResponse,
  ListSupportTicketsResponseSchema,
  MAX_SUPPORT_TICKETS_LIMIT,
  MIN_SUPPORT_TICKETS_LIMIT,
  type TelemetryEventRequest,
  TelemetryEventRequestSchema,
  type TelemetryEventResponse,
  TelemetryEventResponseSchema,
  type TelemetryPreferencesResponse,
  TelemetryPreferencesResponseSchema,
  type UpdateTelemetryPreferencesRequest,
  UpdateTelemetryPreferencesRequestSchema,
} from "../lib/validations/support";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class SupportApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly field?: string;

  constructor(status: number, code: string, message: string, field?: string) {
    super(message);
    this.name = "SupportApiError";
    this.status = status;
    this.code = code;
    this.field = field;
  }
}

async function parseSupportApiError(res: Response): Promise<SupportApiError> {
  try {
    const body = (await res.json()) as {
      code?: string;
      message?: string;
      error?: { code?: string; message?: string; field?: string } | string;
    };
    if (body?.error && typeof body.error === "object") {
      return new SupportApiError(
        res.status,
        body.error.code ?? "REQUEST_FAILED",
        body.error.message ?? `Request failed (${res.status})`,
        body.error.field
      );
    }
    const flatCode = body?.code ?? (typeof body?.error === "string" ? body.error : "REQUEST_FAILED");
    const flatMsg = body?.message ?? `Request failed (${res.status})`;
    return new SupportApiError(res.status, flatCode, flatMsg);
  } catch {
    return new SupportApiError(res.status, "REQUEST_FAILED", `Request failed (${res.status})`);
  }
}

export class SupportApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseSupportApiError(res);
    return schema.parse(await res.json());
  }

  createSupportTicket(
    payload: CreateSupportTicketRequest
  ): Promise<CreateSupportTicketResponse> {
    const validated = CreateSupportTicketRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/support/tickets",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      CreateSupportTicketResponseSchema
    );
  }

  listSupportTickets(limit = DEFAULT_SUPPORT_TICKETS_LIMIT): Promise<ListSupportTicketsResponse> {
    const safeLimit = Math.max(
      MIN_SUPPORT_TICKETS_LIMIT,
      Math.min(MAX_SUPPORT_TICKETS_LIMIT, Math.floor(limit))
    );
    return this.requestValidated(
      `/api/v1/support/tickets?limit=${safeLimit}`,
      { method: "GET", headers: { Accept: "application/json" } },
      ListSupportTicketsResponseSchema
    );
  }

  async fetchSupportTicketAttachment(ticketId: string, signal?: AbortSignal): Promise<Blob> {
    const res = await this.fetchAuthedResponse(getSupportTicketAttachmentUrl(ticketId), {
      method: "GET",
      signal,
    });
    if (!res.ok) throw await parseSupportApiError(res);
    return res.blob();
  }

  trackTelemetryEvent(payload: TelemetryEventRequest): Promise<TelemetryEventResponse> {
    const validated = TelemetryEventRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/telemetry/events",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      TelemetryEventResponseSchema
    );
  }

  getTelemetryPreferences(): Promise<TelemetryPreferencesResponse> {
    return this.requestValidated(
      "/api/v1/telemetry/preferences",
      { method: "GET", headers: { Accept: "application/json" } },
      TelemetryPreferencesResponseSchema
    );
  }

  updateTelemetryPreferences(
    payload: UpdateTelemetryPreferencesRequest
  ): Promise<TelemetryPreferencesResponse> {
    const validated = UpdateTelemetryPreferencesRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/telemetry/preferences",
      {
        method: "PUT",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      TelemetryPreferencesResponseSchema
    );
  }
}

export const apiSupport = new SupportApiClient();

export function getSupportTicketAttachmentUrl(ticketId: string): string {
  return `/api/v1/support/tickets/${encodeURIComponent(ticketId)}/attachment`;
}

export function fetchSupportTicketAttachment(
  ticketId: string,
  signal?: AbortSignal
): Promise<Blob> {
  return apiSupport.fetchSupportTicketAttachment(ticketId, signal);
}

export function createSupportTicket(
  payload: CreateSupportTicketRequest
): Promise<CreateSupportTicketResponse> {
  return apiSupport.createSupportTicket(payload);
}

export function listSupportTickets(
  limit = DEFAULT_SUPPORT_TICKETS_LIMIT
): Promise<ListSupportTicketsResponse> {
  return apiSupport.listSupportTickets(limit);
}

export function trackTelemetryEvent(
  payload: TelemetryEventRequest
): Promise<TelemetryEventResponse> {
  return apiSupport.trackTelemetryEvent(payload);
}

export function getTelemetryPreferences(): Promise<TelemetryPreferencesResponse> {
  return apiSupport.getTelemetryPreferences();
}

export function updateTelemetryPreferences(
  payload: UpdateTelemetryPreferencesRequest
): Promise<TelemetryPreferencesResponse> {
  return apiSupport.updateTelemetryPreferences(payload);
}
