// @story 05.2
import { z } from "zod";
import {
  ApplyProposalInput,
  ApplyProposalSchema,
  ChangeProposalResponse,
  ChangeProposalResponseSchema,
  CreateProposalInput,
  CreateProposalSchema,
} from "../lib/validations/change-proposal";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export interface ApiClientConfig extends HarnessDaemonClientConfig {
  getAuthToken?: () => string | null;
}

export class ChangeProposalClient extends HarnessDaemonTransport {
  constructor(config: ApiClientConfig = {}) {
    super({
      ...config,
      getSessionToken: config.getSessionToken || config.getAuthToken,
    });
  }

  getProposals(projectId: string): Promise<ChangeProposalResponse[]> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/proposals`,
      z.array(ChangeProposalResponseSchema),
      { method: "GET" }
    );
  }

  getProposal(projectId: string, proposalId: string): Promise<ChangeProposalResponse> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/proposals/${encodeURIComponent(proposalId)}`,
      ChangeProposalResponseSchema,
      { method: "GET" }
    );
  }

  createProposal(
    projectId: string,
    input: CreateProposalInput
  ): Promise<ChangeProposalResponse> {
    const validated = CreateProposalSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/proposals`,
      ChangeProposalResponseSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  validateProposal(projectId: string, proposalId: string): Promise<ChangeProposalResponse> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/proposals/${encodeURIComponent(proposalId)}/validate`,
      ChangeProposalResponseSchema,
      { method: "POST" }
    );
  }

  applyProposal(
    projectId: string,
    proposalId: string,
    input: ApplyProposalInput = {}
  ): Promise<ChangeProposalResponse> {
    const validated = ApplyProposalSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/proposals/${encodeURIComponent(proposalId)}/apply`,
      ChangeProposalResponseSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  rejectProposal(projectId: string, proposalId: string): Promise<ChangeProposalResponse> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/proposals/${encodeURIComponent(proposalId)}/reject`,
      ChangeProposalResponseSchema,
      { method: "POST" }
    );
  }
}

export const apiChangeProposal = new ChangeProposalClient();
