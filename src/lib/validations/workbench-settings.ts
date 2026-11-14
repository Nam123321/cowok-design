// @story 04.2
import { z } from "zod";

export const AgentSettingDtoSchema = z.object({
  id: z.string(),
  tenantId: z.string(),
  projectId: z.string().nullable().optional(),
  providerId: z.string().min(1),
  modelId: z.string().min(1),
  endpointUrl: z.string().default(""),
  credentialRef: z.string().default(""),
  isDefault: z.boolean().default(false),
  status: z.string().default("configured"),
  permissionMode: z.string().default("ask-every-time"),
  vaultMode: z.string().default("keychain"),
  onboardingDone: z.boolean().default(false),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export const UpsertAgentSettingInputSchema = z.object({
  projectId: z.string().nullable().optional(),
  providerId: z.string().min(1),
  modelId: z.string().min(1),
  endpointUrl: z.string().default(""),
  credentialRef: z.string().default(""),
  isDefault: z.boolean().default(false),
  status: z.string().default("configured"),
  permissionMode: z.string().optional(),
  vaultMode: z.string().optional(),
  onboardingDone: z.boolean().optional(),
});

export const McpServerDtoSchema = z.object({
  id: z.string(),
  tenantId: z.string(),
  projectId: z.string().nullable().optional(),
  serverSlug: z.string().min(1),
  displayName: z.string().min(1),
  transportKind: z.enum(["stdio", "http_sse"]),
  commandOrUrl: z.string().min(1),
  permissionTier: z.string().default("read_only"),
  enabled: z.boolean(),
  lastHealthStatus: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export const UpsertMcpServerInputSchema = z.object({
  projectId: z.string().nullable().optional(),
  serverSlug: z.string().min(1),
  displayName: z.string().min(1),
  transportKind: z.enum(["stdio", "http_sse"]).default("stdio"),
  commandOrUrl: z.string().min(1),
  permissionTier: z.string().default("read_only"),
  enabled: z.boolean().default(true),
});

export const DeleteMcpServerResponseSchema = z.object({
  deleted: z.boolean(),
  id: z.string(),
});

export const CliDiagnosticItemSchema = z.object({
  id: z.string(),
  name: z.string(),
  command: z.string(),
  installed: z.boolean(),
  version: z.string().optional(),
  authenticated: z.boolean(),
  status: z.enum(["ready", "not_installed", "auth_needed", "error"]),
  authReason: z.string().optional(),
  latencyMs: z.number(),
});

export const DoctorScanResponseSchema = z.object({
  clis: z.array(CliDiagnosticItemSchema),
  scannedAt: z.string(),
});

export const VerifyProviderKeyInputSchema = z.object({
  providerId: z.string().min(1),
  apiKey: z.string().default(""),
  endpointUrl: z.string().default(""),
});

export const VerifyProviderKeyResponseSchema = z.object({
  valid: z.boolean(),
  status: z.string(),
  providerId: z.string(),
  message: z.string(),
  credentialRef: z.string(),
});

export const ExecutionEventDtoSchema = z.object({
  id: z.string(),
  jobId: z.string(),
  projectId: z.string(),
  nodeId: z.string(),
  eventKind: z.string(),
  payloadText: z.string(),
  seqNo: z.number(),
  projectSeq: z.number().optional(),
  compositeScopeKey: z.string().optional(),
  jobStatus: z.string(),
  createdAt: z.string(),
});

export const PostJobEventInputSchema = z.object({
  jobId: z.string().min(1),
  nodeId: z.string().min(1),
  sessionId: z.string().optional(),
  eventKind: z.string().min(1),
  payloadText: z.string().default(""),
  jobStatus: z.enum(["QUEUED", "RUNNING", "PAUSED", "INTERRUPTED", "COMPLETED", "FAILED", "CANCELLED"]).optional(),
});

export type AgentSettingDto = z.infer<typeof AgentSettingDtoSchema>;
export type UpsertAgentSettingInput = z.infer<typeof UpsertAgentSettingInputSchema>;
export type McpServerDto = z.infer<typeof McpServerDtoSchema>;
export type UpsertMcpServerInput = z.infer<typeof UpsertMcpServerInputSchema>;
export type CliDiagnosticItem = z.infer<typeof CliDiagnosticItemSchema>;
export type DoctorScanResponse = z.infer<typeof DoctorScanResponseSchema>;
export type VerifyProviderKeyInput = z.infer<typeof VerifyProviderKeyInputSchema>;
export type VerifyProviderKeyResponse = z.infer<typeof VerifyProviderKeyResponseSchema>;
export type ExecutionEventDto = z.infer<typeof ExecutionEventDtoSchema>;
export type PostJobEventInput = z.infer<typeof PostJobEventInputSchema>;
