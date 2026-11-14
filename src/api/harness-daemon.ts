// @story 01.3, 04.2
// @implements AC-01, AC-02, AC-03, AC-05, AC-06
import { z } from "zod";
import {
  CanonicalSpecKey,
  CanonicalSpecKeySchema,
  ChatSessionDto,
  ChatSessionDtoSchema,
  CreateChatSessionInput,
  CreateChatSessionInputSchema,
  CreateProjectInput,
  CreateProjectInputSchema,
  DeleteSpecResponse,
  DeleteSpecResponseSchema,
  IssuePathTokenInput,
  IssuePathTokenInputSchema,
  PathTokenResponse,
  PathTokenResponseSchema,
  ProjectDto,
  ProjectDtoSchema,
  ProjectSpecsListResponse,
  ProjectSpecsListResponseSchema,
  PutSpecInput,
  PutSpecInputSchema,
  SpecItemWithDrift,
  SpecItemWithDriftSchema,
  SynthesizeSpecsInput,
  SynthesizeSpecsInputSchema,
  SynthesizeSpecsResponse,
  SynthesizeSpecsResponseSchema,
  UpdateChatSessionInput,
  UpdateChatSessionInputSchema,
} from "../lib/validations/harness-daemon";
import {
  HarnessDaemonApiError,
  HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export { HarnessDaemonApiError, type HarnessDaemonClientConfig };

export class HarnessDaemonClient extends HarnessDaemonTransport {
  listProjects(): Promise<ProjectDto[]> {
    return this.request("/api/projects", z.array(ProjectDtoSchema), { method: "GET" });
  }

  createProject(input: CreateProjectInput): Promise<ProjectDto> {
    const validated = CreateProjectInputSchema.parse(input);
    return this.request("/api/projects", ProjectDtoSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  updateProject(
    projectId: string,
    input: { name?: string; product_brief?: string }
  ): Promise<ProjectDto> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}`,
      ProjectDtoSchema,
      {
        method: "PATCH",
        body: JSON.stringify(input),
      }
    );
  }

  deleteProject(projectId: string): Promise<{ deleted: boolean; id: string }> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}`,
      z.object({ deleted: z.boolean(), id: z.string() }),
      { method: "DELETE" }
    );
  }

  listSpecs(projectId: string): Promise<ProjectSpecsListResponse> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/specs`,
      ProjectSpecsListResponseSchema,
      { method: "GET" }
    );
  }

  putSpec(
    projectId: string,
    specKey: CanonicalSpecKey,
    input: PutSpecInput
  ): Promise<SpecItemWithDrift> {
    const safeKey = CanonicalSpecKeySchema.parse(specKey);
    const validated = PutSpecInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/specs/${encodeURIComponent(safeKey)}`,
      SpecItemWithDriftSchema,
      {
        method: "PUT",
        body: JSON.stringify(validated),
      }
    );
  }

  deleteSpec(
    projectId: string,
    specKey: CanonicalSpecKey,
    expectedSha256?: string
  ): Promise<DeleteSpecResponse> {
    const safeKey = CanonicalSpecKeySchema.parse(specKey);
    const qs = expectedSha256 ? `?expected_sha256=${encodeURIComponent(expectedSha256)}` : "";
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/specs/${encodeURIComponent(safeKey)}${qs}`,
      DeleteSpecResponseSchema,
      { method: "DELETE" }
    );
  }

  synthesizeSpecs(
    projectId: string,
    input: SynthesizeSpecsInput = {}
  ): Promise<SynthesizeSpecsResponse> {
    const validated = SynthesizeSpecsInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/specs/synthesize`,
      SynthesizeSpecsResponseSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  listSessions(projectId: string): Promise<ChatSessionDto[]> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/sessions`,
      z.array(ChatSessionDtoSchema),
      { method: "GET" }
    );
  }

  createSession(projectId: string, input: CreateChatSessionInput): Promise<ChatSessionDto> {
    const validated = CreateChatSessionInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/sessions`,
      ChatSessionDtoSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  updateSession(
    projectId: string,
    sessionId: string,
    input: UpdateChatSessionInput
  ): Promise<ChatSessionDto> {
    const validated = UpdateChatSessionInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/sessions/${encodeURIComponent(sessionId)}`,
      ChatSessionDtoSchema,
      {
        method: "PUT",
        body: JSON.stringify(validated),
      }
    );
  }

  deleteSession(projectId: string, sessionId: string): Promise<void> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/sessions/${encodeURIComponent(sessionId)}`,
      null,
      { method: "DELETE" }
    );
  }

  issuePathToken(projectId: string, input: IssuePathTokenInput): Promise<PathTokenResponse> {
    const validated = IssuePathTokenInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/path-token`,
      PathTokenResponseSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }
}

export const defaultHarnessDaemonClient = new HarnessDaemonClient();
export const apiHarnessDaemon = defaultHarnessDaemonClient;
