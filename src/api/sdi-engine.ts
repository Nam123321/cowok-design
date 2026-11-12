import {
  ClarificationSelectInput,
  ClarificationSelectSchema,
  CompileIntentInput,
  CompileIntentSchema,
  DesignIntentResponse,
  DesignIntentResponseSchema,
} from "../lib/validations/sdi";

export interface ApiClientConfig {
  baseUrl?: string;
  getAuthToken?: () => string | null;
}

export class SdiEngineClient {
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
      if (token) headers.set("Authorization", `Bearer ${token}`);
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

  async compileIntent(projectId: string, input: CompileIntentInput): Promise<DesignIntentResponse> {
    const validated = CompileIntentSchema.parse(input);
    const raw = await this.request<DesignIntentResponse>(
      `/api/v1/projects/${projectId}/sdi/intent`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
    return DesignIntentResponseSchema.parse(raw);
  }

  async selectClarification(
    projectId: string,
    input: ClarificationSelectInput
  ): Promise<DesignIntentResponse> {
    const validated = ClarificationSelectSchema.parse(input);
    const raw = await this.request<DesignIntentResponse>(
      `/api/v1/projects/${projectId}/sdi/clarification`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
    return DesignIntentResponseSchema.parse(raw);
  }

  async getIntent(projectId: string, intentId: string): Promise<DesignIntentResponse> {
    const raw = await this.request<DesignIntentResponse>(
      `/api/v1/projects/${projectId}/sdi/intent/${intentId}`
    );
    return DesignIntentResponseSchema.parse(raw);
  }
}

export const apiSdiEngine = new SdiEngineClient();
