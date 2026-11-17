// @story 05.3
import { z } from "zod";
import {
  BindGitRemoteSchema,
  type ConnectGitHubInput,
  ConnectGitHubSchema,
  type ProjectIntegrationsStatus,
  ProjectIntegrationsStatusSchema,
  SelectGitHubSchema,
  SetDefaultIdeSchema,
} from "../lib/validations/github-ide-integration";
import {
  ChatArtifactSnapshotDtoSchema,
  type ChatArtifactSnapshotResponse,
  type CreateSnapshotInput,
  CreateSnapshotSchema,
  type ExportZipInput,
  type ExportZipResponse,
  ExportZipResultSchema,
  ExportZipSchema,
  type ForkTurnSnapshotInput,
  ForkTurnSnapshotSchema,
  type GitHubPRInput,
  type GitHubPRResponse,
  GitHubPRResultSchema,
  GitHubPRSchema,
  type IdeTarget,
  type OpenIdeHandoffInput,
  type OpenIdeHandoffResponse,
  OpenIdeHandoffResultSchema,
  OpenIdeHandoffSchema,
  type RevertResultResponse,
  RevertResultSchema,
  type RevertSnapshotInput,
  RevertSnapshotSchema,
  type TurnForkResultResponse,
  TurnForkResultSchema,
  VersionSnapshotDtoSchema,
  type VersionSnapshotResponse,
} from "../lib/validations/version-snapshot";
import {
  HarnessDaemonApiError,
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
  readBootCapHeaders,
} from "./harness-daemon-transport";

export interface VersionApiClientConfig extends HarnessDaemonClientConfig {
  getAuthToken?: () => string | null;
}

export interface BundleDownloadResult {
  blob: Blob;
  fileName: string;
  sha256: string;
}

const TurnSnapshotEnvelopeSchema = z.union([
  z.array(ChatArtifactSnapshotDtoSchema),
  z
    .object({ turnSnapshots: z.array(ChatArtifactSnapshotDtoSchema) })
    .transform((payload) => payload.turnSnapshots),
]);

const DEFAULT_EXPORT_INPUT: ExportZipInput = {
  includeFixtures: true,
  includeTokens: true,
  includeIcons: true,
  format: "zip",
};

export class VersionSnapshotClient extends HarnessDaemonTransport {
  private readonly sdkOptions: VersionApiClientConfig;

  constructor(config: VersionApiClientConfig = {}) {
    super({
      ...config,
      getSessionToken: config.getSessionToken || config.getAuthToken,
    });
    this.sdkOptions = config;
  }

  getSnapshots(projectId: string): Promise<VersionSnapshotResponse[]> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/snapshots`,
      z.array(VersionSnapshotDtoSchema),
      { method: "GET" }
    );
  }

  listTurnSnapshots(projectId: string): Promise<ChatArtifactSnapshotResponse[]> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/snapshots?kind=turns`,
      TurnSnapshotEnvelopeSchema,
      { method: "GET" }
    );
  }

  getSnapshot(projectId: string, snapshotId: string): Promise<VersionSnapshotResponse> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/snapshots/${encodeURIComponent(snapshotId)}`,
      VersionSnapshotDtoSchema,
      { method: "GET" }
    );
  }

  createSnapshot(
    projectId: string,
    input: CreateSnapshotInput
  ): Promise<VersionSnapshotResponse> {
    const validated = CreateSnapshotSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/snapshots`,
      VersionSnapshotDtoSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  revertSnapshot(
    projectId: string,
    snapshotId: string,
    input: RevertSnapshotInput = {}
  ): Promise<RevertResultResponse> {
    const validated = RevertSnapshotSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/snapshots/${encodeURIComponent(snapshotId)}/revert`,
      RevertResultSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  forkSessionFromTurn(
    projectId: string,
    turnSnapshotId: string,
    input: ForkTurnSnapshotInput = {}
  ): Promise<TurnForkResultResponse> {
    const validated = ForkTurnSnapshotSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/snapshots/${encodeURIComponent(turnSnapshotId)}/fork`,
      TurnForkResultSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  exportBundle(
    projectId: string,
    input: ExportZipInput = DEFAULT_EXPORT_INPUT
  ): Promise<ExportZipResponse> {
    const validated = ExportZipSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/snapshots/export`,
      ExportZipResultSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  getExportDownloadUrl(projectId: string): string {
    return `/api/projects/${encodeURIComponent(projectId)}/snapshots/export/download`;
  }

  private resolveBinaryFetch(): typeof fetch {
    return this.sdkOptions.fetchImpl || globalThis.fetch.bind(globalThis);
  }

  private async resolveBinarySessionToken(baseUrl: string): Promise<string> {
    const injected =
      this.sdkOptions.getSessionToken?.() || this.sdkOptions.getAuthToken?.() || "";
    if (injected) return injected;
    const doFetch = this.resolveBinaryFetch();
    const authRes = await doFetch(`${baseUrl}/api/auth/local-session`, {
      method: "POST",
      headers: { "Content-Type": "application/json", ...readBootCapHeaders() },
      body: JSON.stringify({
        tenantId: this.sdkOptions.tenantId,
        userId: this.sdkOptions.userId,
        role: this.sdkOptions.role || "editor",
      }),
    });
    if (!authRes.ok) {
      throw new HarnessDaemonApiError(
        authRes.status,
        "SESSION_BOOTSTRAP_FAILED",
        `Session bootstrap failed with HTTP ${authRes.status}`
      );
    }
    const authJson = (await authRes.json()) as { sessionToken?: string };
    return typeof authJson.sessionToken === "string" ? authJson.sessionToken : "";
  }

  private async buildDownloadError(res: Response): Promise<HarnessDaemonApiError> {
    let code = "EXPORT_DOWNLOAD_FAILED";
    let message = `Bundle download failed with HTTP ${res.status}`;
    try {
      const err = (await res.json()) as { code?: string; error?: string; message?: string };
      code = err.code || err.error || code;
      message = err.message || message;
    } catch {
      // Ignore non-JSON error body
    }
    return new HarnessDaemonApiError(res.status, code, message);
  }

  async downloadBundle(projectId: string): Promise<BundleDownloadResult> {
    const baseUrl = this.sdkOptions.baseUrl || "";
    const token = await this.resolveBinarySessionToken(baseUrl);
    const res = await this.resolveBinaryFetch()(
      `${baseUrl}${this.getExportDownloadUrl(projectId)}`,
      {
        method: "GET",
        headers: token ? { "X-Cowok-Session-Token": token } : {},
      }
    );
    if (!res.ok) {
      throw await this.buildDownloadError(res);
    }
    const disposition = res.headers.get("Content-Disposition") || "";
    const match = /filename="?([^";]+)"?/i.exec(disposition);
    const fileName = match?.[1] || `${projectId}-bundle.zip`;
    const sha256 = res.headers.get("x-cowok-bundle-sha256") || "";
    const blob = await res.blob();
    return { blob, fileName, sha256 };
  }

  createGitHubPR(
    projectId: string,
    input: GitHubPRInput
  ): Promise<GitHubPRResponse> {
    const validated = GitHubPRSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/snapshots/pr`,
      GitHubPRResultSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  openIdeHandoff(
    projectId: string,
    input: OpenIdeHandoffInput
  ): Promise<OpenIdeHandoffResponse> {
    const validated = OpenIdeHandoffSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/handoff/open-ide`,
      OpenIdeHandoffResultSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  getIntegrationsStatus(projectId: string): Promise<ProjectIntegrationsStatus> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/integrations/status`,
      ProjectIntegrationsStatusSchema,
      { method: "GET" }
    );
  }

  connectGitHubAccount(projectId: string, input: ConnectGitHubInput): Promise<ProjectIntegrationsStatus> {
    const validated = ConnectGitHubSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/integrations/github/connect`,
      ProjectIntegrationsStatusSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  selectGitHubAccount(projectId: string, login: string): Promise<ProjectIntegrationsStatus> {
    const validated = SelectGitHubSchema.parse({ login });
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/integrations/github/select`,
      ProjectIntegrationsStatusSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  bindGitRemote(projectId: string, remoteUrl: string): Promise<ProjectIntegrationsStatus> {
    const validated = BindGitRemoteSchema.parse({ remoteUrl });
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/integrations/git-remote`,
      ProjectIntegrationsStatusSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  setDefaultIde(projectId: string, ide: IdeTarget): Promise<ProjectIntegrationsStatus> {
    const validated = SetDefaultIdeSchema.parse({ ide });
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/integrations/ide/default`,
      ProjectIntegrationsStatusSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }
}

export const apiVersionSnapshot = new VersionSnapshotClient();
export default apiVersionSnapshot;
