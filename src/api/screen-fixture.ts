// @story 04.2
import { z } from "zod";
import {
  CreateScreenFixture,
  CreateScreenFixtureSchema,
  ScreenFixtureResponse,
  ScreenFixtureResponseSchema,
  UpdateScreenFixture,
  UpdateScreenFixtureSchema,
  ViewportPreset,
  ViewportPresetSchema,
} from "../lib/validations/screen-fixture";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export interface ApiClientConfig extends HarnessDaemonClientConfig {
  getAuthToken?: () => string | null;
}

const ViewportPresetUpdateResponseSchema = z.object({
  success: z.boolean(),
  viewportPreset: ViewportPresetSchema,
});

export class ScreenFixtureClient extends HarnessDaemonTransport {
  constructor(config: ApiClientConfig = {}) {
    super({
      ...config,
      getSessionToken: config.getSessionToken || config.getAuthToken,
    });
  }

  getFixtures(projectId: string, nodeId: string): Promise<ScreenFixtureResponse[]> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/fixtures`,
      z.array(ScreenFixtureResponseSchema),
      { method: "GET" }
    );
  }

  saveFixture(
    projectId: string,
    nodeId: string,
    input: CreateScreenFixture
  ): Promise<ScreenFixtureResponse> {
    const validated = CreateScreenFixtureSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/fixtures`,
      ScreenFixtureResponseSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  updateFixture(
    projectId: string,
    nodeId: string,
    fixtureId: string,
    input: UpdateScreenFixture
  ): Promise<ScreenFixtureResponse> {
    const validated = UpdateScreenFixtureSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/fixtures/${encodeURIComponent(fixtureId)}`,
      ScreenFixtureResponseSchema,
      {
        method: "PATCH",
        body: JSON.stringify(validated),
      }
    );
  }

  setViewportPreset(
    projectId: string,
    nodeId: string,
    viewportPreset: ViewportPreset
  ): Promise<{ success: boolean; viewportPreset: ViewportPreset }> {
    const validatedPreset = ViewportPresetSchema.parse(viewportPreset);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/viewport`,
      ViewportPresetUpdateResponseSchema,
      {
        method: "PATCH",
        body: JSON.stringify({ viewportPreset: validatedPreset }),
      }
    );
  }
}

export const apiScreenFixture = new ScreenFixtureClient();
