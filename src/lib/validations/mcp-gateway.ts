// @story 11.2
// @implements AC-01, AC-03, AC-04 (Boundary Zod Contracts for Bidirectional Open MCP Server, Host Installer, Tool Toggle & Live Activity Audit)
import { z } from "zod";

const MAX_TOOL_NAME_LEN = 128;
const MAX_PATH_LEN = 1024;
const MAX_ID_LEN = 128;
const MAX_METHOD_LEN = 128;
const MAX_LIMIT = 200;
const DEFAULT_LIMIT = 50;

export const McpPermissionTierSchema = z.enum([
  "READ_ONLY",
  "INSPECT",
  "REVISE_PROPOSE",
  "HUMAN_ONLY",
]);
export type McpPermissionTier = z.infer<typeof McpPermissionTierSchema>;

export const McpHostIdSchema = z.enum([
  "cowok_ai",
  "antigravity",
  "claude_code",
  "codex",
  "cursor",
  "opencode",
]);
export type McpHostId = z.infer<typeof McpHostIdSchema>;

export const McpActivityStatusSchema = z.enum([
  "OK",
  "SELF_HEALING_ERROR",
  "DENIED_HUMAN_ONLY",
  "VALIDATION_ERROR",
]);
export type McpActivityStatus = z.infer<typeof McpActivityStatusSchema>;

export const McpToolPolicyDtoSchema = z.object({
  toolName: z.string().min(1).max(MAX_TOOL_NAME_LEN),
  permissionTier: McpPermissionTierSchema,
  enabled: z.boolean(),
  mcpCallable: z.boolean(),
  description: z.string(),
  lockedHumanOnly: z.boolean(),
  updatedAt: z.number().int(),
});
export type McpToolPolicyDto = z.infer<typeof McpToolPolicyDtoSchema>;

export const McpExternalActivityDtoSchema = z.object({
  id: z.string().min(1),
  projectId: z.string().nullable(),
  actorKind: z.enum(["MCP_CLIENT", "HUMAN_WORKBENCH_SESSION"]),
  hostClient: z.string(),
  methodName: z.string(),
  toolName: z.string().nullable(),
  permissionTier: z.string(),
  status: McpActivityStatusSchema,
  jsonrpcCode: z.number().int().nullable(),
  latencyMs: z.number().int().min(0),
  idempotencyKey: z.string().nullable(),
  summary: z.string(),
  createdAt: z.number().int(),
});
export type McpExternalActivityDto = z.infer<typeof McpExternalActivityDtoSchema>;

export const McpHostBackupDtoSchema = z.object({
  id: z.string().min(1),
  hostId: z.string().min(1),
  configPath: z.string().min(1),
  backupPath: z.string().min(1),
  beforeSha256: z.string().min(1),
  afterSha256: z.string().min(1),
  rolledBack: z.boolean(),
  createdAt: z.number().int(),
});
export type McpHostBackupDto = z.infer<typeof McpHostBackupDtoSchema>;

export const McpHostStatusDtoSchema = z.object({
  hostId: McpHostIdSchema,
  displayName: z.string().min(1),
  configFormat: z.enum(["json", "toml"]),
  defaultRelativePath: z.string().min(1),
  detected: z.boolean(),
  installed: z.boolean(),
  latestBackupId: z.string().nullable(),
  latestAfterSha256: z.string().nullable(),
  lastInstalledAt: z.number().int().nullable(),
});
export type McpHostStatusDto = z.infer<typeof McpHostStatusDtoSchema>;

export const McpExternalActivityQuerySchema = z
  .object({
    limit: z.coerce.number().int().min(1).max(MAX_LIMIT).default(DEFAULT_LIMIT),
    hostClient: z.string().trim().min(1).max(MAX_ID_LEN).optional(),
    status: McpActivityStatusSchema.optional(),
  })
  .strict();
export type McpExternalActivityQuery = z.infer<typeof McpExternalActivityQuerySchema>;

export const McpExternalActivityDataSchema = z.object({
  policies: z.array(McpToolPolicyDtoSchema),
  activities: z.array(McpExternalActivityDtoSchema),
  hosts: z.array(McpHostStatusDtoSchema),
  backups: z.array(McpHostBackupDtoSchema),
  serverUrl: z.string().min(1),
  generatedAt: z.string().min(1),
});
export type McpExternalActivityData = z.infer<typeof McpExternalActivityDataSchema>;

export const McpExternalActivityResponseSchema = z.object({
  ok: z.literal(true),
  data: McpExternalActivityDataSchema,
  requestId: z.string().min(1),
});
export type McpExternalActivityResponse = z.infer<typeof McpExternalActivityResponseSchema>;

export const McpHostInstallRequestSchema = z
  .object({
    hostId: McpHostIdSchema,
    action: z.enum(["install", "rollback"]).default("install"),
    backupId: z.string().trim().min(1).max(MAX_ID_LEN).optional(),
    configPathOverride: z.string().trim().min(1).max(MAX_PATH_LEN).optional(),
  })
  .strict();
export type McpHostInstallRequest = z.infer<typeof McpHostInstallRequestSchema>;

export const McpHostInstallDataSchema = z.object({
  action: z.enum(["install", "rollback"]),
  hostId: McpHostIdSchema,
  displayName: z.string().min(1),
  configPath: z.string().min(1),
  backupId: z.string().min(1),
  backupPath: z.string().min(1),
  beforeSha256: z.string().min(1),
  afterSha256: z.string().min(1),
  rolledBack: z.boolean(),
  timestamp: z.number().int(),
});
export type McpHostInstallData = z.infer<typeof McpHostInstallDataSchema>;

export const McpHostInstallResponseSchema = z.object({
  ok: z.literal(true),
  data: McpHostInstallDataSchema,
  requestId: z.string().min(1),
});
export type McpHostInstallResponse = z.infer<typeof McpHostInstallResponseSchema>;

export const McpToolToggleRequestSchema = z
  .object({
    toolName: z.string().trim().min(1).max(MAX_TOOL_NAME_LEN),
    enabled: z.boolean().optional(),
    mcpCallable: z.boolean().optional(),
  })
  .strict();
export type McpToolToggleRequest = z.infer<typeof McpToolToggleRequestSchema>;

export const McpToolToggleResponseSchema = z.object({
  ok: z.literal(true),
  data: McpToolPolicyDtoSchema,
  requestId: z.string().min(1),
});
export type McpToolToggleResponse = z.infer<typeof McpToolToggleResponseSchema>;

export const McpJsonRpcRequestSchema = z
  .object({
    jsonrpc: z.literal("2.0"),
    id: z.union([z.string(), z.number(), z.null()]).optional(),
    method: z.string().trim().min(1).max(MAX_METHOD_LEN),
    params: z.record(z.string(), z.unknown()).optional(),
  })
  .strict();
export type McpJsonRpcRequest = z.infer<typeof McpJsonRpcRequestSchema>;

export const McpJsonRpcErrorSchema = z.object({
  code: z.number().int(),
  message: z.string(),
  data: z.unknown().optional(),
});

export const McpJsonRpcResponseSchema = z.object({
  jsonrpc: z.literal("2.0"),
  id: z.union([z.string(), z.number(), z.null()]),
  result: z.unknown().optional(),
  error: McpJsonRpcErrorSchema.optional(),
});
export type McpJsonRpcResponse = z.infer<typeof McpJsonRpcResponseSchema>;

export const McpGatewayErrorResponseSchema = z.object({
  error: z.object({
    code: z.string().min(1),
    message: z.string().min(1),
    field: z.string().optional(),
    requestId: z.string().optional(),
    details: z.unknown().optional(),
  }),
});
export type McpGatewayErrorResponse = z.infer<typeof McpGatewayErrorResponseSchema>;
