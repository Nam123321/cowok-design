// @story 05.3
import { z } from "zod";
import { IdeTargetSchema } from "./version-snapshot";

const MIN_GITHUB_TOKEN_LEN = 16;
const MIN_GIT_REMOTE_LEN = 8;

export const GitHubAccountEntrySchema = z.object({
  login: z.string().min(1),
  displayName: z.string(),
  avatarUrl: z.string().nullable().optional(),
  source: z.string(),
  maskedHint: z.string(),
  active: z.boolean(),
});

export const IdeReadinessItemSchema = z.object({
  id: IdeTargetSchema,
  label: z.string(),
  command: z.string(),
  launchScheme: z.string(),
  installed: z.boolean(),
});

export const ProjectIntegrationsStatusSchema = z.object({
  github: z.object({
    connected: z.boolean(),
    activeLogin: z.string().nullable().optional(),
    activeSource: z.string().nullable().optional(),
    accounts: z.array(GitHubAccountEntrySchema),
    ghCliInstalled: z.boolean(),
    ghCliAuthenticated: z.boolean(),
    remoteUrl: z.string().nullable().optional(),
    readyForPr: z.boolean(),
  }),
  ides: z.array(IdeReadinessItemSchema),
  preferredIde: IdeTargetSchema,
});

export type ProjectIntegrationsStatus = z.infer<typeof ProjectIntegrationsStatusSchema>;

export const ConnectGitHubSchema = z.object({
  token: z.string().trim().min(MIN_GITHUB_TOKEN_LEN).optional(),
  useGhCli: z.boolean().optional().default(false),
});

export type ConnectGitHubInput = z.input<typeof ConnectGitHubSchema>;

export const SelectGitHubSchema = z.object({
  login: z.string().trim().min(1),
});

export const BindGitRemoteSchema = z.object({
  remoteUrl: z
    .string()
    .trim()
    .min(MIN_GIT_REMOTE_LEN)
    .refine((u) => u.startsWith("https://") || u.startsWith("git@"), {
      message: "Remote URL must start with https:// or git@",
    }),
});

export const SetDefaultIdeSchema = z.object({
  ide: IdeTargetSchema,
});
