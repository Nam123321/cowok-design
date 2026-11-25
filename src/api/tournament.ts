// @story 10.3
// @implements AC-01, AC-02, AC-03, AC-04 (Typed Anti-Slop Design Tournament, /crystallize & advisor Domain SDK enforcing runtime .parse() on every boundary)
import { z } from "zod";
import {
  type AdvisorEvolveRequest,
  AdvisorEvolveRequestSchema,
  type AdvisorEvolveResponse,
  AdvisorEvolveResponseSchema,
  type CrystallizeRequest,
  CrystallizeRequestSchema,
  type CrystallizeResponse,
  CrystallizeResponseSchema,
  type ListTournamentRunsResponse,
  ListTournamentRunsResponseSchema,
  type SelectTournamentVariantRequest,
  SelectTournamentVariantRequestSchema,
  type SelectTournamentVariantResponse,
  SelectTournamentVariantResponseSchema,
  type StartTournamentRequest,
  StartTournamentRequestSchema,
  type StartTournamentResponse,
  StartTournamentResponseSchema,
  TournamentErrorResponseSchema,
} from "@/lib/validations/tournament";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export class TournamentApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly field?: string;
  readonly requestId?: string;
  readonly details?: Record<string, unknown>;

  constructor(params: {
    status: number;
    code: string;
    message: string;
    field?: string;
    requestId?: string;
    details?: Record<string, unknown>;
  }) {
    super(params.message);
    this.name = "TournamentApiError";
    this.status = params.status;
    this.code = params.code;
    this.field = params.field;
    this.requestId = params.requestId;
    this.details = params.details;
  }
}

async function parseTournamentError(res: Response): Promise<TournamentApiError> {
  try {
    const rawJson: unknown = await res.json();
    const parsed = TournamentErrorResponseSchema.safeParse(rawJson);
    if (parsed.success) {
      return new TournamentApiError({
        status: res.status,
        code: parsed.data.error.code,
        message: parsed.data.error.message,
        field: parsed.data.error.field,
        requestId: parsed.data.error.requestId,
        details: parsed.data.error.details,
      });
    }
    const fallback = rawJson as {
      error?: { code?: string; message?: string; details?: Record<string, unknown> };
    };
    return new TournamentApiError({
      status: res.status,
      code: fallback?.error?.code ?? "REQUEST_FAILED",
      message: fallback?.error?.message ?? `Request failed (${res.status})`,
      details: fallback?.error?.details,
    });
  } catch {
    return new TournamentApiError({
      status: res.status,
      code: "REQUEST_FAILED",
      message: `Request failed (${res.status})`,
    });
  }
}

export class TournamentApiClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  private async postJson<T>(
    path: string,
    body: unknown,
    responseSchema: z.ZodType<T, z.ZodTypeDef, unknown>,
    opts?: { signal?: AbortSignal }
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json" },
      body: JSON.stringify(body),
      signal: opts?.signal,
    });
    if (!res.ok) throw await parseTournamentError(res);
    return responseSchema.parse(await res.json());
  }

  private async getJson<T>(
    path: string,
    responseSchema: z.ZodType<T, z.ZodTypeDef, unknown>,
    opts?: { signal?: AbortSignal }
  ): Promise<T> {
    const res = await this.fetchAuthedResponse(path, {
      method: "GET",
      headers: { Accept: "application/json" },
      signal: opts?.signal,
    });
    if (!res.ok) throw await parseTournamentError(res);
    return responseSchema.parse(await res.json());
  }

  startTournament(
    payload: z.input<typeof StartTournamentRequestSchema>,
    opts?: { signal?: AbortSignal }
  ): Promise<StartTournamentResponse> {
    const validated: StartTournamentRequest = StartTournamentRequestSchema.parse(payload);
    return this.postJson("/api/v1/tournament/start", validated, StartTournamentResponseSchema, opts);
  }

  selectWinner(
    payload: z.input<typeof SelectTournamentVariantRequestSchema>,
    opts?: { signal?: AbortSignal }
  ): Promise<SelectTournamentVariantResponse> {
    const validated: SelectTournamentVariantRequest =
      SelectTournamentVariantRequestSchema.parse(payload);
    return this.postJson(
      "/api/v1/tournament/select",
      validated,
      SelectTournamentVariantResponseSchema,
      opts
    );
  }

  listRuns(projectId: string, limit = 20): Promise<ListTournamentRunsResponse> {
    const qs = new URLSearchParams({
      projectId: projectId.trim(),
      limit: String(Math.max(1, Math.min(50, Math.trunc(limit)))),
    });
    return this.getJson(`/api/v1/tournament/runs?${qs.toString()}`, ListTournamentRunsResponseSchema);
  }

  crystallize(
    payload: z.input<typeof CrystallizeRequestSchema>
  ): Promise<CrystallizeResponse> {
    const validated: CrystallizeRequest = CrystallizeRequestSchema.parse(payload);
    return this.postJson("/api/v1/crystallize", validated, CrystallizeResponseSchema);
  }

  evolveAdvisor(
    payload: z.input<typeof AdvisorEvolveRequestSchema>
  ): Promise<AdvisorEvolveResponse> {
    const validated: AdvisorEvolveRequest = AdvisorEvolveRequestSchema.parse(payload);
    return this.postJson("/api/v1/advisor/evolve", validated, AdvisorEvolveResponseSchema);
  }
}

export function createTournamentApiClient(
  config: HarnessDaemonClientConfig = {}
): TournamentApiClient {
  return new TournamentApiClient(config);
}
