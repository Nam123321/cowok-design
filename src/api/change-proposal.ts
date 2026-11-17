// @story 05.2
import { z } from "zod";
import {
  ApplyProposalInput,
  ApplyProposalSchema,
  ChangeProposalResponse,
  ChangeProposalResponseSchema,
  ComputeVisualDiffInput,
  ComputeVisualDiffSchema,
  CreateProposalInput,
  CreateProposalSchema,
  DecideTradeoffInput,
  DecideTradeoffSchema,
  ProposalTradeoff,
  ProposalTradeoffSchema,
  ValidateProposalInput,
  ValidateProposalSchema,
  VisualDiffResult,
  VisualDiffResultSchema,
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

  getProposals(projectId: string, nodeId?: string): Promise<ChangeProposalResponse[]> {
    const query = nodeId && nodeId.trim() ? `?nodeId=${encodeURIComponent(nodeId.trim())}` : "";
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/proposals${query}`,
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

  validateProposal(
    projectId: string,
    proposalId: string,
    input: ValidateProposalInput = {}
  ): Promise<ChangeProposalResponse> {
    const validated = ValidateProposalSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/proposals/${encodeURIComponent(proposalId)}/validate`,
      ChangeProposalResponseSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  computeVisualDiff(
    projectId: string,
    proposalId: string,
    input: ComputeVisualDiffInput = {}
  ): Promise<VisualDiffResult> {
    const validated = ComputeVisualDiffSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/proposals/${encodeURIComponent(proposalId)}/diff`,
      VisualDiffResultSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  decideTradeoff(
    projectId: string,
    proposalId: string,
    tradeoffId: string,
    input: DecideTradeoffInput
  ): Promise<ProposalTradeoff> {
    const validated = DecideTradeoffSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/proposals/${encodeURIComponent(proposalId)}/tradeoffs/${encodeURIComponent(tradeoffId)}/decide`,
      ProposalTradeoffSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
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
