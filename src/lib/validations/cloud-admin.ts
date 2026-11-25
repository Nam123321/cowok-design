// @story 14.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Strict Zod Contracts for Cloud Ingestion API & SuperAdmin Portal MVP)
import { z } from "zod";

export const MAX_CLOUD_ATTACHMENT_DATA_URL_LEN = 7_200_000;
export const MAX_CLOUD_TITLE_LEN = 160;
export const MIN_CLOUD_TITLE_LEN = 3;
export const MAX_CLOUD_DESC_LEN = 4000;
export const MIN_CLOUD_DESC_LEN = 5;
export const MAX_CLOUD_REPLY_LEN = 4000;
export const DEFAULT_CLOUD_PAGE_LIMIT = 50;
export const MAX_CLOUD_PAGE_LIMIT = 200;
const MAX_ID_LEN = 128;
const MAX_CHANNEL_LEN = 64;
const MAX_VERSION_LEN = 32;
const MAX_EMAIL_LEN = 160;
const MAX_MFA_CODE_LEN = 12;
const MAX_SUMMARY_LEN = 2000;

export const CloudAdminRoleSchema = z.enum([
  "SUPER_ADMIN",
  "SUPPORT_AGENT",
  "SKILL_CURATOR",
  "ANALYST_READONLY",
]);
export type CloudAdminRole = z.infer<typeof CloudAdminRoleSchema>;

export const CloudTicketCategorySchema = z.enum([
  "bug",
  "feature_request",
  "ux_friction",
  "billing",
  "other",
]);
export type CloudTicketCategory = z.infer<typeof CloudTicketCategorySchema>;

export const CloudTicketPrioritySchema = z.enum(["P0", "P1", "P2", "P3"]);
export type CloudTicketPriority = z.infer<typeof CloudTicketPrioritySchema>;

export const CloudTicketStatusSchema = z.enum([
  "open",
  "triaged",
  "replied",
  "resolved",
  "closed",
]);
export type CloudTicketStatus = z.infer<typeof CloudTicketStatusSchema>;

export const CloudClusterStatusSchema = z.enum([
  "open",
  "planned",
  "shipped",
  "declined",
]);
export type CloudClusterStatus = z.infer<typeof CloudClusterStatusSchema>;

export const CloudCrashStatusSchema = z.enum([
  "open",
  "investigating",
  "resolved",
]);
export type CloudCrashStatus = z.infer<typeof CloudCrashStatusSchema>;

export const CloudAdminErrorEnvelopeSchema = z.object({
  error: z.object({
    code: z.string().min(1),
    message: z.string().min(1),
    field: z.string().optional(),
    requestId: z.string().min(1),
  }),
});
export type CloudAdminErrorEnvelope = z.infer<typeof CloudAdminErrorEnvelopeSchema>;

export const CloudTicketReplySchema = z.object({
  id: z.string().min(1),
  ticketId: z.string().min(1),
  authorActor: z.string().min(1),
  authorRole: z.string().min(1),
  bodySanitized: z.string(),
  syncedToClient: z.boolean(),
  createdAt: z.number().int(),
});
export type CloudTicketReply = z.infer<typeof CloudTicketReplySchema>;

export const CloudSupportTicketSchema = z.object({
  id: z.string().min(1),
  installId: z.string().min(1),
  clientTicketId: z.string().nullable().optional(),
  category: CloudTicketCategorySchema,
  priority: CloudTicketPrioritySchema,
  status: CloudTicketStatusSchema,
  title: z.string(),
  descriptionSanitized: z.string(),
  diagnosticBundle: z.record(z.unknown()).default({}),
  attachmentSha256: z.string().nullable().optional(),
  attachmentMime: z.string().nullable().optional(),
  attachmentBytes: z.number().int().nullable().optional(),
  clusterId: z.string().nullable().optional(),
  channel: z.string(),
  hostClient: z.string(),
  appVersion: z.string(),
  idempotencyKey: z.string().nullable().optional(),
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
  replies: z.array(CloudTicketReplySchema).default([]),
});
export type CloudSupportTicket = z.infer<typeof CloudSupportTicketSchema>;

export const IngestTicketRequestSchema = z.object({
  installId: z.string().trim().min(1).max(MAX_ID_LEN),
  clientTicketId: z.string().trim().max(MAX_ID_LEN).optional(),
  category: z.enum([
    "bug",
    "bug_report",
    "feature_request",
    "skill_template_request",
    "ux_friction",
    "billing",
    "other",
    "question",
  ]),
  priority: CloudTicketPrioritySchema.optional(),
  title: z.string().trim().min(MIN_CLOUD_TITLE_LEN).max(MAX_CLOUD_TITLE_LEN),
  description: z.string().trim().min(MIN_CLOUD_DESC_LEN).max(MAX_CLOUD_DESC_LEN),
  diagnosticBundle: z.record(z.unknown()).optional(),
  attachmentDataUrl: z.string().max(MAX_CLOUD_ATTACHMENT_DATA_URL_LEN).optional(),
  channel: z.string().trim().min(1).max(MAX_CHANNEL_LEN).default("cli"),
  hostClient: z.string().trim().min(1).max(MAX_CHANNEL_LEN).default("standalone"),
  appVersion: z.string().trim().min(1).max(MAX_VERSION_LEN).default("0.1.0"),
  idempotencyKey: z.string().trim().min(1).max(MAX_ID_LEN).optional(),
});
export type IngestTicketRequest = z.input<typeof IngestTicketRequestSchema>;

export const IngestTicketResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    ticket: CloudSupportTicketSchema,
    deduplicated: z.boolean(),
    serverRedactionCount: z.number().int().min(0),
  }),
  requestId: z.string().min(1),
});
export type IngestTicketResponse = z.infer<typeof IngestTicketResponseSchema>;

export const IngestTelemetryRequestSchema = z.object({
  installId: z.string().trim().min(1).max(MAX_ID_LEN),
  eventName: z.string().trim().min(1).max(MAX_ID_LEN),
  funnelStage: z
    .enum(["day_0", "ai_connected", "project", "preview", "export"])
    .optional(),
  channel: z.string().trim().min(1).max(MAX_CHANNEL_LEN).default("cli"),
  hostClient: z.string().trim().min(1).max(MAX_CHANNEL_LEN).default("standalone"),
  appVersion: z.string().trim().min(1).max(MAX_VERSION_LEN).default("0.1.0"),
  properties: z.record(z.unknown()).optional(),
  occurredAt: z.number().int().positive().optional(),
});
export type IngestTelemetryRequest = z.input<typeof IngestTelemetryRequestSchema>;

export const IngestTelemetryResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    eventId: z.string().min(1),
    crashFingerprint: z.string().nullable().optional(),
    serverRedactionCount: z.number().int().min(0),
  }),
  requestId: z.string().min(1),
});
export type IngestTelemetryResponse = z.infer<typeof IngestTelemetryResponseSchema>;

export const AdminAuthSessionRequestSchema = z.object({
  oidcProvider: z.string().trim().min(1).max(MAX_CHANNEL_LEN),
  subject: z.string().trim().min(1).max(MAX_ID_LEN),
  email: z.string().trim().email().max(MAX_EMAIL_LEN),
  role: CloudAdminRoleSchema,
  mfaVerified: z.boolean(),
  mfaCode: z.string().trim().max(MAX_MFA_CODE_LEN).optional(),
  oidcAssertion: z.string().trim().min(1).max(MAX_CLOUD_DESC_LEN).optional(),
});
export type AdminAuthSessionRequest = z.input<typeof AdminAuthSessionRequestSchema>;

export const AdminOperatorSchema = z.object({
  subject: z.string().min(1),
  email: z.string().min(1),
  role: CloudAdminRoleSchema,
  mfaVerified: z.boolean(),
  oidcProvider: z.string().min(1),
});
export type AdminOperator = z.infer<typeof AdminOperatorSchema>;

export const AdminAuthSessionResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    sessionToken: z.string().min(1),
    expiresAt: z.number().int(),
    operator: AdminOperatorSchema,
  }),
  requestId: z.string().min(1),
});
export type AdminAuthSessionResponse = z.infer<typeof AdminAuthSessionResponseSchema>;

export const ListAdminTicketsResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    tickets: z.array(CloudSupportTicketSchema),
    totalCount: z.number().int().min(0),
  }),
  requestId: z.string().min(1),
});
export type ListAdminTicketsResponse = z.infer<typeof ListAdminTicketsResponseSchema>;

export const ReplyAdminTicketRequestSchema = z.object({
  body: z.string().trim().min(1).max(MAX_CLOUD_REPLY_LEN),
  priority: CloudTicketPrioritySchema.optional(),
  status: CloudTicketStatusSchema.optional(),
});
export type ReplyAdminTicketRequest = z.input<typeof ReplyAdminTicketRequestSchema>;

export const ReplyAdminTicketResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    ticket: CloudSupportTicketSchema,
    reply: CloudTicketReplySchema,
    auditLogId: z.string().min(1),
  }),
  requestId: z.string().min(1),
});
export type ReplyAdminTicketResponse = z.infer<typeof ReplyAdminTicketResponseSchema>;

export const CloudFeatureClusterSchema = z.object({
  id: z.string().min(1),
  title: z.string(),
  summary: z.string(),
  ticketCount: z.number().int().min(0),
  ticketIds: z.array(z.string()).default([]),
  githubIssueUrl: z.string().nullable().optional(),
  status: CloudClusterStatusSchema,
  createdAt: z.number().int(),
  updatedAt: z.number().int(),
});
export type CloudFeatureCluster = z.infer<typeof CloudFeatureClusterSchema>;

export const ListClustersResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    clusters: z.array(CloudFeatureClusterSchema),
  }),
  requestId: z.string().min(1),
});
export type ListClustersResponse = z.infer<typeof ListClustersResponseSchema>;

export const CreateClusterRequestSchema = z.object({
  id: z.string().trim().min(1).max(MAX_ID_LEN).optional(),
  title: z.string().trim().min(MIN_CLOUD_TITLE_LEN).max(MAX_CLOUD_TITLE_LEN),
  summary: z.string().trim().max(MAX_SUMMARY_LEN).optional(),
  ticketIds: z.array(z.string().trim().min(1)).default([]),
  githubIssueUrl: z.string().url().optional(),
  status: CloudClusterStatusSchema.optional(),
});
export type CreateClusterRequest = z.input<typeof CreateClusterRequestSchema>;

export const CreateClusterResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    cluster: CloudFeatureClusterSchema,
    auditLogId: z.string().min(1),
  }),
  requestId: z.string().min(1),
});
export type CreateClusterResponse = z.infer<typeof CreateClusterResponseSchema>;

export const BreakdownMetricSchema = z.object({
  key: z.string().min(1),
  users: z.number().int().min(0),
  events: z.number().int().min(0),
});
export type BreakdownMetric = z.infer<typeof BreakdownMetricSchema>;

export const FunnelStepMetricSchema = z.object({
  stage: z.string().min(1),
  label: z.string().min(1),
  users: z.number().int().min(0),
  conversionRatePct: z.number(),
  dropOffCount: z.number().int().min(0),
  dropOffRatePct: z.number(),
});
export type FunnelStepMetric = z.infer<typeof FunnelStepMetricSchema>;

export const FeatureAdoptionMetricSchema = z.object({
  eventName: z.string().min(1),
  totalEvents: z.number().int().min(0),
  uniqueInstalls: z.number().int().min(0),
});
export type FeatureAdoptionMetric = z.infer<typeof FeatureAdoptionMetricSchema>;

export const AnalyticsFunnelDataSchema = z.object({
  dau: z.number().int().min(0),
  wau: z.number().int().min(0),
  mau: z.number().int().min(0),
  openTickets: z.number().int().min(0),
  p0Tickets: z.number().int().min(0),
  activeCrashGroups: z.number().int().min(0),
  channelBreakdown: z.array(BreakdownMetricSchema),
  hostBreakdown: z.array(BreakdownMetricSchema),
  funnelSteps: z.array(FunnelStepMetricSchema),
  featureAdoption: z.array(FeatureAdoptionMetricSchema),
});
export type AnalyticsFunnelData = z.infer<typeof AnalyticsFunnelDataSchema>;

export const AnalyticsFunnelResponseSchema = z.object({
  ok: z.literal(true),
  data: AnalyticsFunnelDataSchema,
  requestId: z.string().min(1),
});
export type AnalyticsFunnelResponse = z.infer<typeof AnalyticsFunnelResponseSchema>;

export const CrashGroupSchema = z.object({
  fingerprint: z.string().min(1),
  appVersion: z.string().min(1),
  errorSignature: z.string(),
  sampleStackSanitized: z.string(),
  occurrenceCount: z.number().int().min(0),
  affectedInstalls: z.number().int().min(0),
  status: CloudCrashStatusSchema,
  firstSeenAt: z.number().int(),
  lastSeenAt: z.number().int(),
});
export type CrashGroup = z.infer<typeof CrashGroupSchema>;

export const ListCrashesResponseSchema = z.object({
  ok: z.literal(true),
  data: z.object({
    crashes: z.array(CrashGroupSchema),
  }),
  requestId: z.string().min(1),
});
export type ListCrashesResponse = z.infer<typeof ListCrashesResponseSchema>;
