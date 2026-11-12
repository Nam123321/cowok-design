import {
  CreateElementCommentInput,
  CreateElementCommentSchema,
  ElementCommentResponse,
  ElementCommentResponseSchema,
  UpdateElementCommentInput,
  UpdateElementCommentSchema,
} from "../lib/validations/element-comment";

export interface ApiClientConfig {
  baseUrl?: string;
  getAuthToken?: () => string | null;
}

export class ElementCommentClient {
  private baseUrl: string;
  private getAuthToken?: () => string | null;

  constructor(config: ApiClientConfig = {}) {
    this.baseUrl = config.baseUrl || "";
    this.getAuthToken = config.getAuthToken;
  }

  private async request<T>(path: string, options: RequestInit = {}): Promise<T> {
    const headers = new Headers(options.headers || {});
    headers.set("Content-Type", "application/json");

    if (this.getAuthToken) {
      const token = this.getAuthToken();
      if (token) {
        headers.set("Authorization", `Bearer ${token}`);
      }
    }

    const response = await fetch(`${this.baseUrl}${path}`, {
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

  async getComments(projectId: string, nodeId: string): Promise<ElementCommentResponse[]> {
    const raw = await this.request<ElementCommentResponse[]>(
      `/api/v1/projects/${projectId}/flow/nodes/${nodeId}/comments`
    );
    return raw.map((item) => ElementCommentResponseSchema.parse(item));
  }

  async createComment(
    projectId: string,
    nodeId: string,
    input: CreateElementCommentInput
  ): Promise<ElementCommentResponse> {
    const validated = CreateElementCommentSchema.parse(input);
    const raw = await this.request<ElementCommentResponse>(
      `/api/v1/projects/${projectId}/flow/nodes/${nodeId}/comments`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
    return ElementCommentResponseSchema.parse(raw);
  }

  async updateComment(
    projectId: string,
    nodeId: string,
    commentId: string,
    input: UpdateElementCommentInput
  ): Promise<ElementCommentResponse> {
    const validated = UpdateElementCommentSchema.parse(input);
    const raw = await this.request<ElementCommentResponse>(
      `/api/v1/projects/${projectId}/flow/nodes/${nodeId}/comments/${commentId}`,
      {
        method: "PATCH",
        body: JSON.stringify(validated),
      }
    );
    return ElementCommentResponseSchema.parse(raw);
  }

  async resolveComment(
    projectId: string,
    nodeId: string,
    commentId: string
  ): Promise<ElementCommentResponse> {
    return this.updateComment(projectId, nodeId, commentId, {
      anchorStatus: "resolved",
    });
  }
}

export const apiElementComment = new ElementCommentClient();
