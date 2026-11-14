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
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export interface ConstraintsApiClientConfig extends HarnessDaemonClientConfig {
  getAuthToken?: () => string | null;
}

export class ConstraintsClient extends HarnessDaemonTransport {
  constructor(config: ConstraintsApiClientConfig = {}) {
    super({
      ...config,
      getSessionToken: config.getSessionToken || config.getAuthToken,
    });
  }

  getIconManifest(projectId: string): Promise<IconManifest> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/icon-manifest`,
      IconManifestSchema,
      { method: "GET" }
    );
  }

  validateIcon(projectId: string, input: ValidateIconInput): Promise<IconValidationReport> {
    const validated = ValidateIconInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/validate-icon`,
      IconValidationReportSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  validateTypography(
    projectId: string,
    input: ValidateTypographyInput
  ): Promise<TypographyValidationReport> {
    const validated = ValidateTypographyInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/validate-typography`,
      TypographyValidationReportSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  validateComponent(
    projectId: string,
    input: ValidateComponentInput
  ): Promise<ComponentValidationReport> {
    const validated = ValidateComponentInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/validate-component`,
      ComponentValidationReportSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  getDesignContracts(projectId: string): Promise<ProjectDesignContracts> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/design-contracts`,
      ProjectDesignContractsSchema,
      { method: "GET" }
    );
  }

  updateDesignContracts(
    projectId: string,
    input: ProjectDesignContracts
  ): Promise<ProjectDesignContracts> {
    const validated = ProjectDesignContractsSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/design-contracts`,
      ProjectDesignContractsSchema,
      {
        method: "PUT",
        body: JSON.stringify(validated),
      }
    );
  }

  lintSource(projectId: string, input: LintSourceInput): Promise<LintSourceResponse> {
    const validated = LintSourceInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/lint`,
      LintSourceResponseSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }
}

export const apiConstraints = new ConstraintsClient();
