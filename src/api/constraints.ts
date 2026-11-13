// @story 03.2
import {
  type ComponentValidationReport,
  ComponentValidationReportSchema,
  type IconManifest,
  IconManifestSchema,
  type IconValidationReport,
  IconValidationReportSchema,
  type LintSourceInput,
  LintSourceInputSchema,
  type LintSourceResponse,
  LintSourceResponseSchema,
  type ProjectDesignContracts,
  ProjectDesignContractsSchema,
  type TypographyValidationReport,
  TypographyValidationReportSchema,
  type ValidateComponentInput,
  ValidateComponentInputSchema,
  type ValidateIconInput,
  ValidateIconInputSchema,
  type ValidateTypographyInput,
  ValidateTypographyInputSchema,
} from "../lib/validations/constraints";

export interface ConstraintsApiClientConfig {
  baseUrl?: string;
  getAuthToken?: () => string | null;
}

export class ConstraintsClient {
  private baseUrl: string;
  private getAuthToken?: () => string | null;

  constructor(config: ConstraintsApiClientConfig = {}) {
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

  async getIconManifest(projectId: string): Promise<IconManifest> {
    const raw = await this.request<IconManifest>(`/api/v1/projects/${projectId}/icon-manifest`);
    return IconManifestSchema.parse(raw);
  }

  async validateIcon(projectId: string, input: ValidateIconInput): Promise<IconValidationReport> {
    const validated = ValidateIconInputSchema.parse(input);
    const raw = await this.request<IconValidationReport>(
      `/api/v1/projects/${projectId}/validate-icon`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      },
    );
    return IconValidationReportSchema.parse(raw);
  }

  async validateTypography(
    projectId: string,
    input: ValidateTypographyInput,
  ): Promise<TypographyValidationReport> {
    const validated = ValidateTypographyInputSchema.parse(input);
    const raw = await this.request<TypographyValidationReport>(
      `/api/v1/projects/${projectId}/validate-typography`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      },
    );
    return TypographyValidationReportSchema.parse(raw);
  }

  async validateComponent(
    projectId: string,
    input: ValidateComponentInput,
  ): Promise<ComponentValidationReport> {
    const validated = ValidateComponentInputSchema.parse(input);
    const raw = await this.request<ComponentValidationReport>(
      `/api/v1/projects/${projectId}/validate-component`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      },
    );
    return ComponentValidationReportSchema.parse(raw);
  }

  async getDesignContracts(projectId: string): Promise<ProjectDesignContracts> {
    const raw = await this.request<ProjectDesignContracts>(
      `/api/v1/projects/${projectId}/design-contracts`,
    );
    return ProjectDesignContractsSchema.parse(raw);
  }

  async updateDesignContracts(
    projectId: string,
    input: ProjectDesignContracts,
  ): Promise<ProjectDesignContracts> {
    const validated = ProjectDesignContractsSchema.parse(input);
    const raw = await this.request<ProjectDesignContracts>(
      `/api/v1/projects/${projectId}/design-contracts`,
      {
        method: "PUT",
        body: JSON.stringify(validated),
      },
    );
    return ProjectDesignContractsSchema.parse(raw);
  }

  async lintSource(projectId: string, input: LintSourceInput): Promise<LintSourceResponse> {
    const validated = LintSourceInputSchema.parse(input);
    const raw = await this.request<LintSourceResponse>(
      `/api/v1/projects/${projectId}/lint`,
      {
        method: "POST",
        body: JSON.stringify(validated),
      },
    );
    return LintSourceResponseSchema.parse(raw);
  }
}

export const apiConstraints = new ConstraintsClient();
