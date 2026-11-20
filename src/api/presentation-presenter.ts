// @story 06.4
// @implements AC-01, AC-02, AC-04
import { z } from "zod";
import { HarnessDaemonTransport } from "./harness-daemon-transport";
import {
  PresenterHandshakeRequestSchema,
  PresenterHandshakeResponseSchema,
  type PresenterHandshakeRequest,
  type PresenterHandshakeResponse,
} from "../lib/validations/presentation-presenter";

export interface PresenterApiClientLike {
  request<T>(
    path: string,
    schema: z.ZodType<T, z.ZodTypeDef, unknown>,
    options?: RequestInit & { timeoutMs?: number }
  ): Promise<T>;
}

export interface PresenterRequestOptions {
  timeoutMs?: number;
}

export class PresentationPresenterClient {
  private readonly client: PresenterApiClientLike;

  constructor(client: PresenterApiClientLike = new HarnessDaemonTransport()) {
    this.client = client;
  }

  async createPresenterSession(
    target: { projectId?: string; deckId: string },
    input: Partial<PresenterHandshakeRequest> = {},
    opts: PresenterRequestOptions = {}
  ): Promise<PresenterHandshakeResponse> {
    const validatedBody = PresenterHandshakeRequestSchema.parse({
      projectId: target.projectId,
      ...input,
    });
    const path = target.projectId
      ? `/api/projects/${encodeURIComponent(target.projectId)}/decks/${encodeURIComponent(target.deckId)}/presenter/handshake`
      : `/api/decks/${encodeURIComponent(target.deckId)}/presenter/handshake`;

    const res = await this.client.request(path, PresenterHandshakeResponseSchema, {
      method: "POST",
      body: JSON.stringify(validatedBody),
      timeoutMs: opts.timeoutMs,
    });
    return PresenterHandshakeResponseSchema.parse(res);
  }
}

export const apiPresentationPresenter = new PresentationPresenterClient();
export default apiPresentationPresenter;
