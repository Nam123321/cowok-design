// @story 05.1
import { z } from "zod";
import {
  ClarificationSelectInput,
  ClarificationSelectSchema,
  CompileIntentInput,
  CompileIntentSchema,
  DesignIntentResponse,
  DesignIntentResponseSchema,
} from "../lib/validations/sdi";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export interface ApiClientConfig extends HarnessDaemonClientConfig {
  getAuthToken?: () => string | null;
}

export class SdiEngineClient extends HarnessDaemonTransport {
  constructor(config: ApiClientConfig = {}) {
    super({
      ...config,
      getSessionToken: config.getSessionToken || config.getAuthToken,
    });
  }

  listIntents(projectId: string): Promise<DesignIntentResponse[]> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/sdi/intent`,
      z.array(DesignIntentResponseSchema),
      { method: "GET" }
    );
  }

  compileIntent(projectId: string, input: CompileIntentInput): Promise<DesignIntentResponse> {
    const validated = CompileIntentSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/sdi/intent`,
      DesignIntentResponseSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  selectClarification(
    projectId: string,
    input: ClarificationSelectInput
  ): Promise<DesignIntentResponse> {
    const validated = ClarificationSelectSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/sdi/clarification`,
      DesignIntentResponseSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  getIntent(projectId: string, intentId: string): Promise<DesignIntentResponse> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/sdi/intent/${encodeURIComponent(intentId)}`,
      DesignIntentResponseSchema,
      { method: "GET" }
    );
  }
}

export const apiSdiEngine = new SdiEngineClient();
