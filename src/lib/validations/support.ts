// @story 14.1
// @implements AC-01, AC-02, AC-03, AC-04 (Zod Contracts for Non-Tech Support Drawer & Multi-Channel Telemetry Outbox)
import { z } from "zod";

export const DEFAULT_SUPPORT_TICKETS_LIMIT = 50;
export const MAX_SUPPORT_TICKETS_LIMIT = 200;
export const MIN_SUPPORT_TICKETS_LIMIT = 1;
export const MAX_SCREENSHOT_DATA_URL_LEN = 7_000_000;
export const MIN_TICKET_TITLE_LEN = 3;
export const MAX_TICKET_TITLE_LEN = 160;
export const MIN_TICKET_DESC_LEN = 5;
export const MAX_TICKET_DESC_LEN = 4000;
export const MIN_LOCALE_LEN = 2;
export const MAX_LOCALE_LEN = 16;
export const MAX_DIAG_TOKEN_LEN = 64;
export const MAX_RECENT_ERROR_CODES = 20;
export const MAX_ACTIVE_SKILLS = 30;
export const MAX_ADMIN_AUTHOR_LEN = 120;
export const MAX_EVENT_NAME_LEN = 128;

export const SCREENSHOT_DATA_URL_RE =
  /^data:(?:image\/(png|jpeg|webp)|video\/(mp4|webm));base64,[A-Za-z0-9+/=]+$/;

export const SupportCategorySchema = z.enum([
  "bug_report",
  "feature_request",
  "skill_template_request",
  "question",
]);
export type SupportCategory = z.infer<typeof SupportCategorySchema>;

export const DistributionChannelSchema = z.enum([
  "cli",
  "desktop",
  "claude_code",
  "codex",
  "antigravity",
  "cursor",
  "cowok_ai",
  "docker",
]);
export type DistributionChannel = z.infer<typeof DistributionChannelSchema>;

export const TelemetryEventGroupSchema = z.enum([
  "lifecycle",
  "onboarding_funnel",
  "feature_usage",
  "reliability_errors",
  "skill_ecosystem",
]);
export type TelemetryEventGroup = z.infer<typeof TelemetryEventGroupSchema>;

export const SupportTicketStatusSchema = z.enum([
  "submitted",
  "triaged",
  "in_progress",
  "resolved",
  "closed",
]);
export type SupportTicketStatus = z.infer<typeof SupportTicketStatusSchema>;

export const SupportSyncStatusSchema = z.enum([
  "queued",
  "synced",
  "local_only",
  "failed",
]);
export type SupportSyncStatus = z.infer<typeof SupportSyncStatusSchema>;

export const DiagnosticBundleSchema = z.object({
  appVersion: z.string().max(MAX_DIAG_TOKEN_LEN),
  channel: DistributionChannelSchema,
  os: z.string().max(MAX_DIAG_TOKEN_LEN),
  locale: z.string().max(MAX_LOCALE_LEN),
  recentErrorCodes: z.array(z.string().max(MAX_DIAG_TOKEN_LEN)).max(MAX_RECENT_ERROR_CODES),
  activeSkills: z.array(z.string().max(MAX_DIAG_TOKEN_LEN)).max(MAX_ACTIVE_SKILLS),
  redactionCount: z.number().int().min(0),
});
export type DiagnosticBundle = z.infer<typeof DiagnosticBundleSchema>;

export const AdminReplySchema = z.object({
  id: z.string().min(1),
  author: z.string().min(1).max(MAX_ADMIN_AUTHOR_LEN),
  message: z.string().min(1).max(MAX_TICKET_DESC_LEN),
  createdAt: z.number().int(),
});
export type AdminReply = z.infer<typeof AdminReplySchema>;

export const CreateSupportTicketRequestSchema = z.object({
  category: SupportCategorySchema,
  title: z.string().trim().min(MIN_TICKET_TITLE_LEN).max(MAX_TICKET_TITLE_LEN),
  description: z.string().trim().min(MIN_TICKET_DESC_LEN).max(MAX_TICKET_DESC_LEN),
  contactEmail: z.string().email().optional().or(z.literal("")),
  screenshotDataUrl: z
    .string()
    .max(MAX_SCREENSHOT_DATA_URL_LEN)
    .regex(
      SCREENSHOT_DATA_URL_RE,
      "Attachment must be a valid base64 PNG, JPEG, WebP, MP4, or WebM data URL"
    )
    .optional(),
  channel: DistributionChannelSchema.default("desktop"),
  locale: z.string().min(MIN_LOCALE_LEN).max(MAX_LOCALE_LEN).default("vi-VN"),
  includeDiagnostics: z.boolean().default(true),
  recentErrorCodes: z.array(z.string().max(MAX_DIAG_TOKEN_LEN)).max(MAX_RECENT_ERROR_CODES).default([]),
  activeSkills: z.array(z.string().max(MAX_DIAG_TOKEN_LEN)).max(MAX_ACTIVE_SKILLS).default([]),
});
export type CreateSupportTicketRequest = z.input<typeof CreateSupportTicketRequestSchema>;

export const LocalSupportTicketSchema = z.object({
  id: z.string().min(1),
  ticketCode: z.string().min(1),
  installId: z.string().min(1),
  channel: DistributionChannelSchema,
  category: SupportCategorySchema,
  title: z.string(),
  description: z.string(),
  contactEmail: z.string().nullable().optional(),
  screenshotSha256: z.string().nullable().optional(),
  attachmentMime: z.string().nullable().optional(),
  diagnostics: DiagnosticBundleSchema,
  status: SupportTicketStatusSchema,
  adminReplies: z.array(AdminReplySchema).default([]),
  syncStatus: SupportSyncStatusSchema,
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
});
export type LocalSupportTicket = z.infer<typeof LocalSupportTicketSchema>;

export const CreateSupportTicketResponseSchema = z.object({
  ok: z.literal(true),
  data: LocalSupportTicketSchema,
  requestId: z.string(),
});
export type CreateSupportTicketResponse = z.infer<typeof CreateSupportTicketResponseSchema>;

export const ListSupportTicketsResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    tickets: z.array(LocalSupportTicketSchema),
  }),
  requestId: z.string(),
});
export type ListSupportTicketsResponse = z.infer<typeof ListSupportTicketsResponseSchema>;

export const TelemetryEventRequestSchema = z.object({
  channel: DistributionChannelSchema.default("desktop"),
  eventGroup: TelemetryEventGroupSchema,
  eventName: z.string().trim().min(1).max(MAX_EVENT_NAME_LEN),
  payload: z.record(z.unknown()).default({}),
});
export type TelemetryEventRequest = z.input<typeof TelemetryEventRequestSchema>;

export const TelemetryEventResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    queued: z.boolean(),
    droppedByOptOut: z.boolean(),
    eventId: z.string().nullable().optional(),
    outboxDepth: z.number().int().min(0),
    redactionCount: z.number().int().min(0),
  }),
  requestId: z.string(),
});
export type TelemetryEventResponse = z.infer<typeof TelemetryEventResponseSchema>;

export const TelemetryPreferencesSchema = z.object({
  telemetryEnabled: z.boolean(),
  effectiveEnabled: z.boolean(),
  envLocked: z.boolean(),
  lockedReason: z.string().nullable().optional(),
  airgapped: z.boolean(),
  installId: z.string().min(1),
  pendingOutboxCount: z.number().int().min(0),
  updatedAt: z.number().int(),
});
export type TelemetryPreferences = z.infer<typeof TelemetryPreferencesSchema>;

export const UpdateTelemetryPreferencesRequestSchema = z.object({
  telemetryEnabled: z.boolean(),
});
export type UpdateTelemetryPreferencesRequest = z.infer<
  typeof UpdateTelemetryPreferencesRequestSchema
>;

export const TelemetryPreferencesResponseSchema = z.object({
  ok: z.literal(true),
  data: TelemetryPreferencesSchema,
  requestId: z.string(),
});
export type TelemetryPreferencesResponse = z.infer<typeof TelemetryPreferencesResponseSchema>;

export const SupportErrorCodeSchema = z.enum([
  "INVALID_REQUEST",
  "UNAUTHORIZED",
  "AUTH_FORBIDDEN",
  "PAYLOAD_TOO_LARGE",
  "STORAGE_ERROR",
  "INTERNAL_ERROR",
]);
export type SupportErrorCode = z.infer<typeof SupportErrorCodeSchema>;

export const SupportErrorEnvelopeSchema = z.object({
  error: z.object({
    code: z.string(),
    message: z.string(),
    field: z.string().optional(),
    requestId: z.string(),
  }),
});
export type SupportErrorEnvelope = z.infer<typeof SupportErrorEnvelopeSchema>;
