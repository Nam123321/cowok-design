// @story 04.1
// @implements AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
import {
  BatchUpdateNodePositionsInput,
  BatchUpdateNodePositionsResponseSchema,
  BatchUpdateNodePositionsSchema,
  CreateFlowNodeInput,
  CreateFlowNodePayloadSchema,
  CreateFlowTransitionInput,
  CreateFlowTransitionPayloadSchema,
  DeleteFlowNodeResponseSchema,
  DeleteTransitionResponseSchema,
  DeployDraftInput,
  DeployDraftResponseSchema,
  DeployFlowDraftPayloadSchema,
  FlowGraphSnapshot,
  FlowGraphSnapshotSchema,
  FlowNodeResponse,
  FlowNodeResponseDTOSchema,
  FlowTransitionResponse,
  FlowTransitionResponseDTOSchema,
  UpdateFlowNodeInput,
  UpdateFlowNodePayloadSchema,
  UpdateFlowTransitionInput,
  UpdateFlowTransitionPayloadSchema,
} from "../lib/validations/flow-workbench";

export interface ApiClientConfig {
  baseUrl?: string;
  getSessionToken?: () => string | null;
  role?: "viewer" | "editor" | "admin";
  tenantId?: string;
  userId?: string;
}

export class FlowApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "FlowApiError";
    this.status = status;
    this.code = code;
  }
}

function nextCorrelationId(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  const bytes = new Uint8Array(8);
  if (typeof globalThis.crypto?.getRandomValues === "function") {
    globalThis.crypto.getRandomValues(bytes);
  }
  return `corr-${Date.now()}-${Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("")}`;
}

export class FlowWorkbenchClient {
  private readonly baseUrl: string;
  private readonly getSessionToken?: () => string | null;
  private readonly role?: "viewer" | "editor" | "admin";
  private readonly tenantId?: string;
  private readonly userId?: string;
  private cachedToken: string | null = null;
  private inflightBootstrap: Promise<string> | null = null;

  constructor(config: ApiClientConfig = {}) {
    this.baseUrl = config.baseUrl || "";
    this.getSessionToken = config.getSessionToken;
    this.role = config.role;
    this.tenantId = config.tenantId;
    this.userId = config.userId;
  }

  private async ensureSessionToken(forceRefresh = false): Promise<string> {
    if (this.getSessionToken) {
      const injected = this.getSessionToken();
      if (injected) return injected;
    }
    if (!forceRefresh && this.cachedToken) return this.cachedToken;
    if (!forceRefresh && this.inflightBootstrap) return this.inflightBootstrap;

    this.inflightBootstrap = (async () => {
      try {
        const res = await fetch(`${this.baseUrl}/api/auth/local-session`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            tenantId: this.tenantId,
            userId: this.userId,
            role: this.role || "editor",
          }),
        });
        if (!res.ok) {
          throw new FlowApiError(res.status, "SESSION_BOOTSTRAP_FAILED", `Bootstrap failed: ${res.status}`);
        }
        const json = (await res.json()) as { sessionToken?: string };
        const tok = json.sessionToken || "";
        this.cachedToken = tok;
        return tok;
      } finally {
        this.inflightBootstrap = null;
      }
    })();

    return this.inflightBootstrap;
  }

  private async request<T>(path: string, options: RequestInit = {}, retried = false): Promise<T> {
    const token = await this.ensureSessionToken(false);
    const headers = new Headers(options.headers || {});
    headers.set("Content-Type", "application/json");
    headers.set("X-Cowok-Session-Token", token);
    headers.set("X-Correlation-Id", nextCorrelationId());

    const response = await fetch(`${this.baseUrl}${path}`, { ...options, headers });
    if (response.status === 401 && !retried && !this.getSessionToken) {
      await this.ensureSessionToken(true);
      return this.request<T>(path, options, true);
    }

    if (!response.ok) {
      const errBody = (await response.json().catch(() => ({}))) as { code?: string; error?: string; message?: string };
      throw new FlowApiError(
        response.status,
        errBody.code || errBody.error || "API_ERROR",
        errBody.message || errBody.error || `HTTP ${response.status}`
      );
    }
    return response.json() as Promise<T>;
  }

  async getSnapshot(projectId: string): Promise<FlowGraphSnapshot> {
    const raw = await this.request<unknown>(`/api/projects/${projectId}/flow/snapshot`);
    return FlowGraphSnapshotSchema.parse(raw);
  }

  async getNodes(projectId: string): Promise<FlowNodeResponse[]> {
    const raw = await this.request<unknown[]>(`/api/projects/${projectId}/flow/nodes`);
    return raw.map((item) => FlowNodeResponseDTOSchema.parse(item));
  }

  async createNode(projectId: string, input: CreateFlowNodeInput): Promise<FlowNodeResponse> {
    const validated = CreateFlowNodePayloadSchema.parse(input);
    const raw = await this.request<unknown>(`/api/projects/${projectId}/flow/nodes`, {
      method: "POST",
      body: JSON.stringify(validated),
    });
    return FlowNodeResponseDTOSchema.parse(raw);
  }

  async updateNode(projectId: string, nodeId: string, input: UpdateFlowNodeInput): Promise<FlowNodeResponse> {
    const validated = UpdateFlowNodePayloadSchema.parse(input);
    const raw = await this.request<unknown>(`/api/projects/${projectId}/flow/nodes/${nodeId}`, {
      method: "PATCH",
      body: JSON.stringify(validated),
    });
    return FlowNodeResponseDTOSchema.parse(raw);
  }

  async batchUpdateNodePositions(projectId: string, input: BatchUpdateNodePositionsInput) {
    const validated = BatchUpdateNodePositionsSchema.parse(input);
    const raw = await this.request<unknown>(`/api/projects/${projectId}/flow/nodes/positions`, {
      method: "PATCH",
      body: JSON.stringify(validated),
    });
    return BatchUpdateNodePositionsResponseSchema.parse(raw);
  }

  async deleteNode(projectId: string, nodeId: string, query?: { expectedVersion?: number; expectedGraphVersion?: number }) {
    const qs = new URLSearchParams();
    if (query?.expectedVersion !== undefined) qs.set("expectedVersion", String(query.expectedVersion));
    if (query?.expectedGraphVersion !== undefined) qs.set("expectedGraphVersion", String(query.expectedGraphVersion));
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    const raw = await this.request<unknown>(`/api/projects/${projectId}/flow/nodes/${nodeId}${suffix}`, { method: "DELETE" });
    return DeleteFlowNodeResponseSchema.parse(raw);
  }

  async getTransitions(projectId: string): Promise<FlowTransitionResponse[]> {
    const raw = await this.request<unknown[]>(`/api/projects/${projectId}/flow/transitions`);
    return raw.map((item) => FlowTransitionResponseDTOSchema.parse(item));
  }

  async createTransition(projectId: string, input: CreateFlowTransitionInput): Promise<FlowTransitionResponse> {
    const validated = CreateFlowTransitionPayloadSchema.parse(input);
    const raw = await this.request<unknown>(`/api/projects/${projectId}/flow/transitions`, {
      method: "POST",
      body: JSON.stringify(validated),
    });
    return FlowTransitionResponseDTOSchema.parse(raw);
  }

  async updateTransition(projectId: string, transitionId: string, input: UpdateFlowTransitionInput): Promise<FlowTransitionResponse> {
    const validated = UpdateFlowTransitionPayloadSchema.parse(input);
    const raw = await this.request<unknown>(`/api/projects/${projectId}/flow/transitions/${transitionId}`, {
      method: "PATCH",
      body: JSON.stringify(validated),
    });
    return FlowTransitionResponseDTOSchema.parse(raw);
  }

  async deleteTransition(projectId: string, transitionId: string, query?: { expectedVersion?: number }) {
    const qs = query?.expectedVersion !== undefined ? `?expectedVersion=${query.expectedVersion}` : "";
    const raw = await this.request<unknown>(`/api/projects/${projectId}/flow/transitions/${transitionId}${qs}`, { method: "DELETE" });
    return DeleteTransitionResponseSchema.parse(raw);
  }

  async deployDraft(projectId: string, input: DeployDraftInput): Promise<FlowGraphSnapshot> {
    const validated = DeployFlowDraftPayloadSchema.parse(input);
    const raw = await this.request<unknown>(`/api/projects/${projectId}/flow/deploy-draft`, {
      method: "POST",
      body: JSON.stringify(validated),
    });
    return DeployDraftResponseSchema.parse(raw);
  }
}

const defaultFlowClient = new FlowWorkbenchClient();

export const apiFlowWorkbench = {
  getSnapshot: (projectId: string) => defaultFlowClient.getSnapshot(projectId),
  getNodes: (projectId: string) => defaultFlowClient.getNodes(projectId),
  createNode: (projectId: string, input: CreateFlowNodeInput) => defaultFlowClient.createNode(projectId, input),
  updateNode: (projectId: string, nodeId: string, input: UpdateFlowNodeInput) =>
    defaultFlowClient.updateNode(projectId, nodeId, input),
  batchUpdateNodePositions: (projectId: string, input: BatchUpdateNodePositionsInput) =>
    defaultFlowClient.batchUpdateNodePositions(projectId, input),
  deleteNode: (projectId: string, nodeId: string, query?: { expectedVersion?: number; expectedGraphVersion?: number }) =>
    defaultFlowClient.deleteNode(projectId, nodeId, query),
  getTransitions: (projectId: string) => defaultFlowClient.getTransitions(projectId),
  createTransition: (projectId: string, input: CreateFlowTransitionInput) =>
    defaultFlowClient.createTransition(projectId, input),
  updateTransition: (projectId: string, transitionId: string, input: UpdateFlowTransitionInput) =>
    defaultFlowClient.updateTransition(projectId, transitionId, input),
  deleteTransition: (projectId: string, transitionId: string, query?: { expectedVersion?: number }) =>
    defaultFlowClient.deleteTransition(projectId, transitionId, query),
  deployDraft: (projectId: string, input: DeployDraftInput) => defaultFlowClient.deployDraft(projectId, input),
};
