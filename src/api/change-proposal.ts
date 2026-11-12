import fs from "node:fs";
import path from "node:path";
import {
  ApplyProposalInput,
  ApplyProposalSchema,
  ChangeProposalResponse,
  ChangeProposalResponseSchema,
  CreateProposalInput,
  CreateProposalSchema,
} from "../lib/validations/change-proposal";

export interface ApiClientConfig {
  baseUrl?: string;
  getAuthToken?: () => string | null;
  tenantId?: string;
}

export class ChangeProposalClient {
  private baseUrl: string;
  private getAuthToken?: () => string | null;
  private tenantId?: string;

  constructor(config: ApiClientConfig = {}) {
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

  async getProposals(projectId: string): Promise<ChangeProposalResponse[]> {
    const raw = await this.request<ChangeProposalResponse[]>(
      `/api/v1/projects/${projectId}/proposals`
    );
    return raw.map((item) => ChangeProposalResponseSchema.parse(item));
  }

  async getProposal(projectId: string, proposalId: string): Promise<ChangeProposalResponse> {
    const raw = await this.request<ChangeProposalResponse>(
      `/api/v1/projects/${projectId}/proposals/${proposalId}`
    );
    return ChangeProposalResponseSchema.parse(raw);
  }

  async createProposal(projectId: string, input: CreateProposalInput): Promise<ChangeProposalResponse> {
    const validated = CreateProposalSchema.parse(input);
    const raw = await this.request<ChangeProposalResponse>(
      `/api/v1/projects/${projectId}/proposals`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
    return ChangeProposalResponseSchema.parse(raw);
  }

  async validateProposal(projectId: string, proposalId: string): Promise<ChangeProposalResponse> {
    const raw = await this.request<ChangeProposalResponse>(
      `/api/v1/projects/${projectId}/proposals/${proposalId}/validate`,
      { method: "POST" }
    );
    return ChangeProposalResponseSchema.parse(raw);
  }

  async applyProposal(
    projectId: string,
    proposalId: string,
    input: ApplyProposalInput = {}
  ): Promise<ChangeProposalResponse> {
    const validated = ApplyProposalSchema.parse(input);
    const raw = await this.request<ChangeProposalResponse>(
      `/api/v1/projects/${projectId}/proposals/${proposalId}/apply`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
    return ChangeProposalResponseSchema.parse(raw);
  }

  async rejectProposal(projectId: string, proposalId: string): Promise<ChangeProposalResponse> {
    const raw = await this.request<ChangeProposalResponse>(
      `/api/v1/projects/${projectId}/proposals/${proposalId}/reject`,
      { method: "POST" }
    );
    return ChangeProposalResponseSchema.parse(raw);
  }
}

export const apiChangeProposal = new ChangeProposalClient();

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
    // Fall back to default if lockfile is unreadable
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

function createDaemonClient(tenantId?: string): ChangeProposalClient {
  const baseUrl = resolveDaemonUrl();
  return new ChangeProposalClient({
    baseUrl,
    getAuthToken: getDaemonAuthToken,
    tenantId,
  });
}

export async function syncProposalWithDaemon(
  projectId: string,
  proposal: ChangeProposalResponse,
  tenantId?: string
): Promise<ChangeProposalResponse> {
  const resolvedTenant = tenantId || proposal.tenantId;
  const client = createDaemonClient(resolvedTenant);
  return client.createProposal(projectId, {
    id: proposal.id,
    title: proposal.title,
    description: proposal.description,
    intentId: proposal.intentId,
    files: proposal.files,
  });
}

export async function syncApplyWithDaemon(
  projectId: string,
  proposalId: string,
  tenantId?: string
): Promise<ChangeProposalResponse> {
  const client = createDaemonClient(tenantId);
  return client.applyProposal(projectId, proposalId);
}

export async function syncValidateWithDaemon(
  projectId: string,
  proposalId: string,
  tenantId?: string
): Promise<ChangeProposalResponse> {
  const client = createDaemonClient(tenantId);
  return client.validateProposal(projectId, proposalId);
}

export async function syncRejectWithDaemon(
  projectId: string,
  proposalId: string,
  tenantId?: string
): Promise<ChangeProposalResponse> {
  const client = createDaemonClient(tenantId);
  return client.rejectProposal(projectId, proposalId);
}
