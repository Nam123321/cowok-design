// @story 05.3
import { z } from "zod";
import {
  CreateSnapshotInput,
  CreateSnapshotSchema,
  ExportZipInput,
  ExportZipResultSchema,
  ExportZipSchema,
  GitHubPRInput,
  GitHubPRResponse,
  GitHubPRResultSchema,
  GitHubPRSchema,
  RevertResultResponse,
  RevertResultSchema,
  RevertSnapshotInput,
  RevertSnapshotSchema,
  VersionSnapshotDtoSchema,
  VersionSnapshotResponse,
} from "../lib/validations/version-snapshot";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export interface VersionApiClientConfig extends HarnessDaemonClientConfig {
  getAuthToken?: () => string | null;
}

export class VersionSnapshotClient extends HarnessDaemonTransport {
  constructor(config: VersionApiClientConfig = {}) {
    super({
      ...config,
      getSessionToken: config.getSessionToken || config.getAuthToken,
    });
  }

  getSnapshots(projectId: string): Promise<VersionSnapshotResponse[]> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/snapshots`,
      z.array(VersionSnapshotDtoSchema),
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

  exportBundle(
    projectId: string,
    input: ExportZipInput = {
      includeFixtures: true,
      includeTokens: true,
      includeIcons: true,
      format: "zip",
    }
  ) {
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
}

export const apiVersionSnapshot = new VersionSnapshotClient();
