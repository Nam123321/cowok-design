import {
  CreateScreenFixture,
  CreateScreenFixtureSchema,
  ScreenFixtureResponse,
  ScreenFixtureResponseSchema,
  UpdateScreenFixture,
  UpdateScreenFixtureSchema,
  ViewportPreset,
} from "../lib/validations/screen-fixture";

/**
 * Domain-Driven Client SDK for Screen State Fixtures & Live Preview (ADR-092 compliance)
 */

export interface ApiClientConfig {
  baseUrl?: string;
  getAuthToken?: () => string | null;
}

export class ScreenFixtureClient {
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

  async getFixtures(projectId: string, nodeId: string): Promise<ScreenFixtureResponse[]> {
    const raw = await this.request<ScreenFixtureResponse[]>(
      `/api/v1/projects/${projectId}/flow/nodes/${nodeId}/fixtures`
    );
    return raw.map((item) => ScreenFixtureResponseSchema.parse(item));
  }

  async saveFixture(
    projectId: string,
    nodeId: string,
    input: CreateScreenFixture
  ): Promise<ScreenFixtureResponse> {
    const validated = CreateScreenFixtureSchema.parse(input);
    const raw = await this.request<ScreenFixtureResponse>(
      `/api/v1/projects/${projectId}/flow/nodes/${nodeId}/fixtures`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
    return ScreenFixtureResponseSchema.parse(raw);
  }

  async updateFixture(
    projectId: string,
    nodeId: string,
    fixtureId: string,
    input: UpdateScreenFixture
  ): Promise<ScreenFixtureResponse> {
    const validated = UpdateScreenFixtureSchema.parse(input);
    const raw = await this.request<ScreenFixtureResponse>(
      `/api/v1/projects/${projectId}/flow/nodes/${nodeId}/fixtures/${fixtureId}`,
      {
        method: "PATCH",
        body: JSON.stringify(validated),
      }
    );
    return ScreenFixtureResponseSchema.parse(raw);
  }

  async setViewportPreset(
    projectId: string,
    nodeId: string,
    viewportPreset: ViewportPreset
  ): Promise<{ success: boolean; viewportPreset: ViewportPreset }> {
    return this.request<{ success: boolean; viewportPreset: ViewportPreset }>(
      `/api/v1/projects/${projectId}/flow/nodes/${nodeId}/viewport`,
      {
        method: "PATCH",
        body: JSON.stringify({ viewportPreset }),
      }
    );
  }
}

export const apiScreenFixture = new ScreenFixtureClient();
