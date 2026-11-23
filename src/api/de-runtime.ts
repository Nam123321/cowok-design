// @story 08.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { z } from "zod";
import {
  type ExecuteEphemeralToolRequest,
  type ExecuteEphemeralToolResponse,
  ExecuteEphemeralToolRequestSchema,
  ExecuteEphemeralToolResponseSchema,
  type ImportUserSkillRequest,
  type ImportUserSkillResponse,
  ImportUserSkillRequestSchema,
  ImportUserSkillResponseSchema,
  type LearnLedgerListResponse,
  LearnLedgerListResponseSchema,
  type ListUserSkillsResponse,
  ListUserSkillsResponseSchema,
  type RecordLearnLedgerRequest,
  type RecordLearnLedgerResponse,
  RecordLearnLedgerRequestSchema,
  RecordLearnLedgerResponseSchema,
  type RunBuiltInDeRequest,
  type RunBuiltInDeResponse,
  RunBuiltInDeRequestSchema,
  RunBuiltInDeResponseSchema,
} from "@/lib/validations/de-runtime";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class DeRuntimeApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly field?: string;

  constructor(status: number, code: string, message: string, field?: string) {
    super(message);
    this.name = "DeRuntimeApiError";
    this.status = status;
    this.code = code;
    this.field = field;
  }
}

async function parseDeRuntimeError(res: Response): Promise<DeRuntimeApiError> {
  try {
    const body = (await res.json()) as {
      code?: string;
      message?: string;
      error?: { code?: string; message?: string; field?: string } | string;
    };
    if (body?.error && typeof body.error === "object") {
      return new DeRuntimeApiError(
        res.status,
        body.error.code ?? "REQUEST_FAILED",
        body.error.message ?? `Request failed (${res.status})`,
        body.error.field
      );
    }
    const flatCode = body?.code ?? (typeof body?.error === "string" ? body.error : "REQUEST_FAILED");
    const flatMsg = body?.message ?? `Request failed (${res.status})`;
    return new DeRuntimeApiError(res.status, flatCode, flatMsg);
  } catch {
    return new DeRuntimeApiError(res.status, "REQUEST_FAILED", `Request failed (${res.status})`);
  }
}

export class DeRuntimeApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async requestValidated<T>(
    path: string,
    init: RequestInit,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, init);
    if (!res.ok) throw await parseDeRuntimeError(res);
    return schema.parse(await res.json());
  }

  runBuiltInDe(payload: RunBuiltInDeRequest): Promise<RunBuiltInDeResponse> {
    const validated = RunBuiltInDeRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/de/run",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      RunBuiltInDeResponseSchema
    );
  }

  listImportedUserSkills(projectId: string): Promise<ListUserSkillsResponse> {
    const qs = new URLSearchParams({ projectId }).toString();
    return this.requestValidated(
      `/api/v1/plugins/import-repo?${qs}`,
      { method: "GET", headers: { Accept: "application/json" } },
      ListUserSkillsResponseSchema
    );
  }

  importUserSkillRepo(payload: ImportUserSkillRequest): Promise<ImportUserSkillResponse> {
    const validated = ImportUserSkillRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/plugins/import-repo",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      ImportUserSkillResponseSchema
    );
  }

  executeEphemeralTool(
    payload: ExecuteEphemeralToolRequest
  ): Promise<ExecuteEphemeralToolResponse> {
    const validated = ExecuteEphemeralToolRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/plugins/ephemeral-tools/execute",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      ExecuteEphemeralToolResponseSchema
    );
  }

  listLearnLedger(projectId: string, qualifyingOnly = false): Promise<LearnLedgerListResponse> {
    const qs = new URLSearchParams({
      projectId,
      ...(qualifyingOnly ? { qualifyingOnly: "true" } : {}),
    }).toString();
    return this.requestValidated(
      `/api/v1/learn/ledger?${qs}`,
      { method: "GET", headers: { Accept: "application/json" } },
      LearnLedgerListResponseSchema
    );
  }

  recordLearnLedgerFailure(
    payload: RecordLearnLedgerRequest
  ): Promise<RecordLearnLedgerResponse> {
    const validated = RecordLearnLedgerRequestSchema.parse(payload);
    return this.requestValidated(
      "/api/v1/learn/ledger",
      {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(validated),
      },
      RecordLearnLedgerResponseSchema
    );
  }
}

const defaultDeRuntimeClient = new DeRuntimeApiClient();

export function runBuiltInDe(payload: RunBuiltInDeRequest): Promise<RunBuiltInDeResponse> {
  return defaultDeRuntimeClient.runBuiltInDe(payload);
}

export function listImportedUserSkills(projectId: string): Promise<ListUserSkillsResponse> {
  return defaultDeRuntimeClient.listImportedUserSkills(projectId);
}

export function importUserSkillRepo(
  payload: ImportUserSkillRequest
): Promise<ImportUserSkillResponse> {
  return defaultDeRuntimeClient.importUserSkillRepo(payload);
}

export function executeEphemeralTool(
  payload: ExecuteEphemeralToolRequest
): Promise<ExecuteEphemeralToolResponse> {
  return defaultDeRuntimeClient.executeEphemeralTool(payload);
}

export function listLearnLedger(
  projectId: string,
  qualifyingOnly = false
): Promise<LearnLedgerListResponse> {
  return defaultDeRuntimeClient.listLearnLedger(projectId, qualifyingOnly);
}

export function recordLearnLedgerFailure(
  payload: RecordLearnLedgerRequest
): Promise<RecordLearnLedgerResponse> {
  return defaultDeRuntimeClient.recordLearnLedgerFailure(payload);
}
