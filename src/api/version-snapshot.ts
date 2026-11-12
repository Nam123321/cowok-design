import fs from "node:fs";
import path from "node:path";
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

export interface VersionApiClientConfig {
  baseUrl?: string;
  getAuthToken?: () => string | null;
  tenantId?: string;
}

export class VersionSnapshotClient {
  private baseUrl: string;
  private getAuthToken?: () => string | null;
  private tenantId?: string;

  constructor(config: VersionApiClientConfig = {}) {
    this.baseUrl = config.baseUrl || "";
    this.getAuthToken = config.getAuthToken;
    this.tenantId = config.tenantId;
  }

  private async request<T>(requestPath: string, options: RequestInit = {}): Promise<T> {
    const headers = new Headers(options.headers || {});
    headers.set("Content-Type", "application/json");

    if (this.getAuthToken) {
      const token = this.getAuthToken();
      if (token) headers.set("Authorization", `Bearer ${token}`);
    }

    if (this.tenantId && !headers.has("x-tenant-id")) {
      headers.set("x-tenant-id", this.tenantId);
    }

    const response = await fetch(`${this.baseUrl}${requestPath}`, {
      ...options,
      headers,
    });

    if (!response.ok) {
      const errorBody = await response.json().catch(() => ({}));
      const error = new Error(errorBody.message || `HTTP ${response.status}: ${response.statusText}`);
      (error as unknown as { status: number }).status = response.status;
      throw error;
    }

    return response.json();
  }

  async getSnapshots(projectId: string): Promise<VersionSnapshotResponse[]> {
    const raw = await this.request<VersionSnapshotResponse[]>(
      `/api/v1/projects/${projectId}/snapshots`
    );
    return raw.map((item) => VersionSnapshotDtoSchema.parse(item));
  }

  async getSnapshot(projectId: string, snapshotId: string): Promise<VersionSnapshotResponse> {
    const raw = await this.request<VersionSnapshotResponse>(
      `/api/v1/projects/${projectId}/snapshots/${snapshotId}`
    );
    return VersionSnapshotDtoSchema.parse(raw);
  }

  async createSnapshot(
    projectId: string,
    input: CreateSnapshotInput
  ): Promise<VersionSnapshotResponse> {
    const validated = CreateSnapshotSchema.parse(input);
    const raw = await this.request<VersionSnapshotResponse>(
      `/api/v1/projects/${projectId}/snapshots`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
    return VersionSnapshotDtoSchema.parse(raw);
  }

  async revertSnapshot(
    projectId: string,
    snapshotId: string,
    input: RevertSnapshotInput = {}
  ): Promise<RevertResultResponse> {
    const validated = RevertSnapshotSchema.parse(input);
    const raw = await this.request<RevertResultResponse>(
      `/api/v1/projects/${projectId}/snapshots/${snapshotId}/revert`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
    return RevertResultSchema.parse(raw);
  }

  async exportBundle(
    projectId: string,
    input: ExportZipInput = { includeFixtures: true, includeTokens: true, includeIcons: true, format: "zip" }
  ) {
    const validated = ExportZipSchema.parse(input);
    const raw = await this.request(
      `/api/v1/projects/${projectId}/snapshots/export`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
    return ExportZipResultSchema.parse(raw);
  }

  async createGitHubPR(
    projectId: string,
    input: GitHubPRInput
  ): Promise<GitHubPRResponse> {
    const validated = GitHubPRSchema.parse(input);
    const raw = await this.request(
      `/api/v1/projects/${projectId}/snapshots/pr`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
    return GitHubPRResultSchema.parse(raw);
  }
}

export const apiVersionSnapshot = new VersionSnapshotClient();

const DEFAULT_DAEMON_PORT = 4170;
const DEFAULT_DAEMON_URL = `http://127.0.0.1:${DEFAULT_DAEMON_PORT}`;

function readPortFromLockfile(): string | null {
  try {
    const rootDir = process.env.IWISH_PROJECT_ROOT || process.cwd();
    const lockPath = path.resolve(rootDir, ".cowok-design/daemon.lock");
    if (fs.existsSync(lockPath)) {
      const parsed = JSON.parse(fs.readFileSync(lockPath, "utf-8"));
      if (parsed?.host && parsed?.port) {
        return `http://${parsed.host}:${parsed.port}`;
      }
    }
  } catch {
    // Ignore lockfile reading error
  }
  return null;
}

export function resolveDaemonUrl(): string {
  if (process.env.HARNESS_DAEMON_URL) return process.env.HARNESS_DAEMON_URL;
  const fromLock = readPortFromLockfile();
  if (fromLock) return fromLock;
  return DEFAULT_DAEMON_URL;
}

export function getDaemonAuthToken(): string | null {
  if (process.env.HARNESS_DAEMON_TOKEN) return process.env.HARNESS_DAEMON_TOKEN;
  if (process.env.PROJECT_TOKEN) return process.env.PROJECT_TOKEN;
  return "cowok-daemon-token";
}

export function createDaemonClient(tenantId?: string): VersionSnapshotClient {
  const baseUrl = resolveDaemonUrl();
  return new VersionSnapshotClient({
    baseUrl,
    getAuthToken: getDaemonAuthToken,
    tenantId,
  });
}

export async function syncSnapshotWithDaemon(
  projectId: string,
  snapshot: VersionSnapshotResponse,
  tenantId?: string
): Promise<VersionSnapshotResponse> {
  const resolvedTenant = tenantId || snapshot.tenantId;
  const client = createDaemonClient(resolvedTenant);
  return client.createSnapshot(projectId, {
    id: snapshot.id,
    label: snapshot.label,
    description: snapshot.description,
    versionTag: snapshot.versionTag,
    trigger: snapshot.trigger,
    files: snapshot.files,
    metadata: snapshot.metadata,
  });
}

export async function syncRevertWithDaemon(
  projectId: string,
  snapshotId: string,
  tenantId?: string
): Promise<RevertResultResponse> {
  const client = createDaemonClient(tenantId);
  return client.revertSnapshot(projectId, snapshotId);
}

export async function syncExportWithDaemon(
  projectId: string,
  options: ExportZipInput,
  tenantId?: string
) {
  const client = createDaemonClient(tenantId);
  return client.exportBundle(projectId, options);
}

export async function syncGitHubPRWithDaemon(
  projectId: string,
  input: GitHubPRInput,
  tenantId?: string
): Promise<GitHubPRResponse> {
  const client = createDaemonClient(tenantId);
  return client.createGitHubPR(projectId, input);
}
