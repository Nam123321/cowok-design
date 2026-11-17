// @story 05.3
import { z } from "zod";

const GIT_REF_PATTERN = /^[a-zA-Z0-9][a-zA-Z0-9._/-]{0,120}$/;
const MAX_GIT_REF_LEN = 121;
const MAX_FORK_TITLE_LEN = 160;
const MAX_PR_TITLE_LEN = 200;
const MAX_IDE_ENTRY_PATH_LEN = 260;

export function isValidGitRefName(ref: string): boolean {
  if (!GIT_REF_PATTERN.test(ref)) return false;
  if (ref.startsWith("-")) return false;
  if (ref.includes("..") || ref.includes("@{") || ref.includes("//")) return false;
  if (ref.includes(".lock")) return false;
  if (ref.endsWith("/") || ref.endsWith(".")) return false;
  return true;
}

export const GitRefStringSchema = z
  .string()
  .min(1, "Git branch reference is required")
  .max(MAX_GIT_REF_LEN)
  .refine(isValidGitRefName, {
    message:
      "Invalid git branch reference (must match ^[a-zA-Z0-9][a-zA-Z0-9._/-]{0,120}$ without .., @{, .lock, //, or leading -)",
  });

export const SnapshotFileItemSchema = z.object({
  path: z.string().min(1),
  content: z.string(),
  sha256: z.string().optional(),
  sizeBytes: z.number().nonnegative().optional(),
});

export type SnapshotFileItem = z.infer<typeof SnapshotFileItemSchema>;

export const SnapshotTriggerSchema = z.enum([
  "manual",
  "auto_proposal_apply",
  "pre_revert_safety",
  "pre_apply_safety",
]);

export type SnapshotTrigger = z.infer<typeof SnapshotTriggerSchema>;

export const CreateSnapshotSchema = z.object({
  id: z.string().uuid().optional(),
  label: z.string().min(1, "Label is required"),
  description: z.string().default(""),
  versionTag: z.string().optional(),
  trigger: SnapshotTriggerSchema.default("manual"),
  proposalId: z.string().uuid().optional(),
  parentSnapshotId: z.string().uuid().optional(),
  chatArtifactSnapshotId: z.string().uuid().optional(),
  files: z.array(SnapshotFileItemSchema).default([]),
  metadata: z.record(z.string()).optional(),
});

export type CreateSnapshotInput = z.input<typeof CreateSnapshotSchema>;

export const VersionSnapshotDtoSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string(),
  projectId: z.string(),
  versionTag: z.string(),
  label: z.string(),
  description: z.string(),
  trigger: SnapshotTriggerSchema,
  treeSha256: z.string(),
  proposalId: z.string().nullable().optional(),
  parentSnapshotId: z.string().nullable().optional(),
  chatArtifactSnapshotId: z.string().nullable().optional(),
  files: z.array(SnapshotFileItemSchema),
  metadata: z.record(z.unknown()).nullable().optional(),
  createdAt: z.string(),
});

export type VersionSnapshotResponse = z.infer<typeof VersionSnapshotDtoSchema>;

export const ChatArtifactSnapshotDtoSchema = z.object({
  id: z.string().uuid(),
  tenantId: z.string().default("local-default"),
  projectId: z.string(),
  sessionId: z.string(),
  messageId: z.string(),
  nodeId: z.string().nullable().optional(),
  proposalId: z.string().nullable().optional(),
  parentSnapshotId: z.string().nullable().optional(),
  artifactType: z.string(),
  contentHash: z.string(),
  manifestJson: z.string().default("{}"),
  nodeSnapshotJson: z.string().default("{}"),
  label: z.string().optional(),
  createdAt: z.union([z.string(), z.number()]),
});

export type ChatArtifactSnapshotResponse = z.infer<typeof ChatArtifactSnapshotDtoSchema>;

export const RevertSnapshotSchema = z.object({
  safetyLabel: z.string().optional(),
  restoreMissingNodes: z.boolean().default(false),
});

export type RevertSnapshotInput = z.input<typeof RevertSnapshotSchema>;

export const RevertResultSchema = z.object({
  success: z.boolean(),
  revertedToSnapshotId: z.string(),
  safetySnapshotId: z.string(),
  restoredFilesCount: z.number().int().nonnegative(),
  restoredNodeIds: z.array(z.string()).optional(),
  timestamp: z.string(),
});

export type RevertResultResponse = z.infer<typeof RevertResultSchema>;

export const ForkTurnSnapshotSchema = z.object({
  title: z.string().min(1).max(MAX_FORK_TITLE_LEN).optional(),
  includeNodeScope: z.boolean().default(true),
});

export type ForkTurnSnapshotInput = z.input<typeof ForkTurnSnapshotSchema>;

export const TurnForkResultSchema = z.object({
  forkedSessionId: z.string(),
  forkedFromSessionId: z.string(),
  forkedFromMessageId: z.string(),
  turnSnapshotId: z.string(),
  copiedMessagesCount: z.number().int().nonnegative(),
  detachedNodeIds: z.array(z.string()).default([]),
  createdAt: z.string(),
});

export type TurnForkResultResponse = z.infer<typeof TurnForkResultSchema>;

export const ExportZipSchema = z.object({
  includeFixtures: z.boolean().default(true),
  includeTokens: z.boolean().default(true),
  includeIcons: z.boolean().default(true),
  format: z.literal("zip").default("zip"),
});

export type ExportZipInput = z.input<typeof ExportZipSchema>;

export const ExportZipResultSchema = z.object({
  fileName: z.string(),
  byteLength: z.number().nonnegative(),
  sha256: z.string(),
  filesIncluded: z.array(z.string()),
  readyToRun: z.boolean(),
  handoffManifestIncluded: z.boolean().default(true),
  archiveBase64: z.string().optional(),
  downloadUrl: z.string().optional(),
  artifactPath: z.string().optional(),
});

export type ExportZipResponse = z.infer<typeof ExportZipResultSchema>;

export const GitHubPRSchema = z.object({
  branchName: GitRefStringSchema,
  prTitle: z.string().min(1, "Pull Request title is required").max(MAX_PR_TITLE_LEN),
  prBody: z.string().default(""),
  targetBranch: GitRefStringSchema.default("main"),
  snapshotId: z.string().uuid().optional(),
  includeVisualDiffTable: z.boolean().default(true),
});

export type GitHubPRInput = z.input<typeof GitHubPRSchema>;

export const GitHubPRResultSchema = z.object({
  prUrl: z.string().url(),
  prNumber: z.number().int().positive(),
  branchName: z.string(),
  status: z.enum(["created", "updated"]),
  handoffManifestSha256: z.string().optional(),
});

export type GitHubPRResponse = z.infer<typeof GitHubPRResultSchema>;

export const IdeTargetSchema = z.enum([
  "antigravity",
  "cursor",
  "vscode",
  "claude-code",
]);

export type IdeTarget = z.infer<typeof IdeTargetSchema>;

export const OpenIdeHandoffSchema = z.object({
  ide: IdeTargetSchema,
  relativeEntryPath: z
    .string()
    .max(MAX_IDE_ENTRY_PATH_LEN)
    .refine(
      (val) =>
        !val.startsWith("/") &&
        !val.includes("\\") &&
        !val.includes("..") &&
        !/^[a-zA-Z]:/.test(val),
      {
        message:
          "relativeEntryPath must be a jailed relative path without '..' or drive prefixes",
      }
    )
    .optional(),
});

export type OpenIdeHandoffInput = z.input<typeof OpenIdeHandoffSchema>;

export const OpenIdeHandoffResultSchema = z.object({
  ide: IdeTargetSchema,
  launchUri: z.string().min(1),
  cliFallbackCommand: z.string().min(1),
  manifestPath: z.string().min(1),
  manifestSha256: z.string().min(1),
});

export type OpenIdeHandoffResponse = z.infer<typeof OpenIdeHandoffResultSchema>;
