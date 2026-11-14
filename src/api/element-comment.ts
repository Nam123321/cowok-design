// @story 04.3
import { z } from "zod";
import {
  CreateElementCommentInput,
  CreateElementCommentSchema,
  ElementCommentResponse,
  ElementCommentResponseSchema,
  UpdateElementCommentInput,
  UpdateElementCommentSchema,
} from "../lib/validations/element-comment";
import {
  type HarnessDaemonClientConfig,
  HarnessDaemonTransport,
} from "./harness-daemon-transport";

export interface ApiClientConfig extends HarnessDaemonClientConfig {
  getAuthToken?: () => string | null;
}

export class ElementCommentClient extends HarnessDaemonTransport {
  constructor(config: ApiClientConfig = {}) {
    super({
      ...config,
      getSessionToken: config.getSessionToken || config.getAuthToken,
    });
  }

  getComments(projectId: string, nodeId: string): Promise<ElementCommentResponse[]> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/comments`,
      z.array(ElementCommentResponseSchema),
      { method: "GET" }
    );
  }

  createComment(
    projectId: string,
    nodeId: string,
    input: CreateElementCommentInput
  ): Promise<ElementCommentResponse> {
    const validated = CreateElementCommentSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/comments`,
      ElementCommentResponseSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  updateComment(
    projectId: string,
    nodeId: string,
    commentId: string,
    input: UpdateElementCommentInput
  ): Promise<ElementCommentResponse> {
    const validated = UpdateElementCommentSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/nodes/${encodeURIComponent(nodeId)}/comments/${encodeURIComponent(commentId)}`,
      ElementCommentResponseSchema,
      {
        method: "PATCH",
        body: JSON.stringify(validated),
      }
    );
  }

  resolveComment(
    projectId: string,
    nodeId: string,
    commentId: string
  ): Promise<ElementCommentResponse> {
    return this.updateComment(projectId, nodeId, commentId, {
      anchorStatus: "resolved",
    });
  }
}

export const apiElementComment = new ElementCommentClient();
