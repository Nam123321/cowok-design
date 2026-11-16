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

export const DoctorModelOptionSchema = z.object({
  id: z.string(),
  label: z.string(),
  providerId: z.string().optional(),
  recommended: z.boolean().optional().default(false),
});

export const QuotaWindowSchema = z.object({
  available: z.boolean(),
  usedPercent: z.number().nullable().optional(),
  remainingPercent: z.number().nullable().optional(),
  resetsAt: z.string().nullable().optional(),
  resetsAtEpochMs: z.number().nullable().optional(),
  windowMinutes: z.number().nullable().optional(),
  resetPassed: z.boolean().optional().default(false),
});

export const ProviderUsageDtoSchema = z.object({
  id: z.string(),
  name: z.string(),
  providerGroup: z.string().optional().default("byos"),
  status: z.string(),
  accountLabel: z.string().nullable().optional(),
  plan: z.string().nullable().optional(),
  ok: z.boolean(),
  note: z.string().nullable().optional(),
  checkedAt: z.string(),
  checkedAtEpochMs: z.number(),
  fiveHour: QuotaWindowSchema.nullable().optional(),
  weekly: QuotaWindowSchema.nullable().optional(),
  models: z.array(DoctorModelOptionSchema).optional().default([]),
});

export const DoctorUsageResponseSchema = z.object({
  providers: z.array(ProviderUsageDtoSchema),
  checkedAt: z.string(),
  checkedAtEpochMs: z.number(),
});

export const CliDiagnosticItemSchema = z.object({
  id: z.string(),
  name: z.string(),
  command: z.string(),
  category: z.enum(["cli", "ide", "ide_extension", "local"]).optional(),
  scanMode: z.enum(["passive", "active", "active_probe"]).optional(),
  installed: z.boolean(),
  version: z.string().nullable().optional(),
  authenticated: z.boolean(),
  status: z.enum(["ready", "not_installed", "auth_needed", "manual_ide_setup", "timeout", "error"]),
  authReason: z.string().nullable().optional(),
  timedOut: z.boolean().optional(),
  latencyMs: z.number(),
  accountLabel: z.string().nullable().optional(),
  providerGroup: z.string().nullable().optional(),
  models: z.array(DoctorModelOptionSchema).optional(),
});

export const SkillPluginDtoSchema = z.object({
  id: z.string(),
  projectId: z.string(),
  pluginSlug: z.string(),
  version: z.string(),
  lifecycleState: z.string(),
  enabled: z.boolean(),
  sourceRepoUrl: z.string().nullable().optional(),
  manifestJson: z.record(z.string(), z.unknown()).optional().default({}),
  updatedAt: z.string(),
});

export type SkillPluginDto = z.infer<typeof SkillPluginDtoSchema>;

export const DoctorScanResponseSchema = z.object({
  clis: z.array(CliDiagnosticItemSchema).optional().default([]),
  tools: z.array(CliDiagnosticItemSchema).optional().default([]),
  scanMode: z.enum(["passive", "active"]).optional(),
  scannedAt: z.string().optional().default(""),
}).transform((data): {
  clis: z.infer<typeof CliDiagnosticItemSchema>[];
  tools: z.infer<typeof CliDiagnosticItemSchema>[];
  scanMode?: "passive" | "active";
  scannedAt: string;
} => ({
  clis: data.clis.length > 0 ? data.clis : data.tools,
  tools: data.tools.length > 0 ? data.tools : data.clis,
  ...(data.scanMode !== undefined ? { scanMode: data.scanMode } : {}),
  scannedAt: data.scannedAt,
}));

export const ProbeCliInputSchema = z.object({
  cliId: z.string().optional().default(""),
  toolId: z.string().optional().default(""),
});

export const VerifyProviderKeyInputSchema = z.object({
  providerId: z.string().min(1),
  apiKey: z.string().default(""),
  endpointUrl: z.string().default(""),
  dryRunFormatOnly: z.boolean().optional(),
});

export const VerifyProviderKeyResponseSchema = z.object({
  valid: z.boolean(),
  status: z.string(),
  providerId: z.string(),
  message: z.string(),
  credentialRef: z.string(),
  liveProbed: z.boolean().optional(),
  httpStatus: z.number().optional(),
});

export const VaultKeyDtoSchema = z.object({
  id: z.string(),
  providerId: z.string(),
  keyHint: z.string(),
  vaultRef: z.string().optional(),
  vaultMode: z.string(),
  status: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
});

export const UpsertVaultKeyInputSchema = z.object({
  providerId: z.string().min(1),
  apiKey: z.string().min(1),
  vaultMode: z.enum(["keychain", "vault_0600", "ram_only", "ephemeral"]).optional().default("vault_0600"),
});

export const DeleteVaultKeyResponseSchema = z.object({
  deleted: z.boolean(),
  id: z.string(),
});

export const VaultConfigInputSchema = z.object({
  vaultMode: z.enum(["keychain", "vault_0600", "ram_only", "ephemeral"]),
  masterPassphrase: z.string().optional(),
});

export const VaultConfigResponseSchema = z.object({
  vaultMode: z.string(),
  vaultWritable: z.boolean(),
  encryptedVaultPresent: z.boolean(),
  updatedAt: z.string(),
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
export type DoctorModelOption = z.infer<typeof DoctorModelOptionSchema>;
export type QuotaWindowDto = z.infer<typeof QuotaWindowSchema>;
export type ProviderUsageDto = z.infer<typeof ProviderUsageDtoSchema>;
export type DoctorUsageResponse = z.infer<typeof DoctorUsageResponseSchema>;
export type DoctorScanResponse = z.infer<typeof DoctorScanResponseSchema>;
export type VerifyProviderKeyInput = z.input<typeof VerifyProviderKeyInputSchema>;
export type VerifyProviderKeyResponse = z.infer<typeof VerifyProviderKeyResponseSchema>;
export type VaultKeyDto = z.infer<typeof VaultKeyDtoSchema>;
export type UpsertVaultKeyInput = z.input<typeof UpsertVaultKeyInputSchema>;
export type DeleteVaultKeyResponse = z.infer<typeof DeleteVaultKeyResponseSchema>;
export type VaultConfigInput = z.infer<typeof VaultConfigInputSchema>;
export type VaultConfigResponse = z.infer<typeof VaultConfigResponseSchema>;
export type ExecutionEventDto = z.infer<typeof ExecutionEventDtoSchema>;
export type PostJobEventInput = z.infer<typeof PostJobEventInputSchema>;
