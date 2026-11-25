// @story 11.2
// @implements AC-01, AC-03, AC-04 (Domain SDK for Open MCP Server, 1-Click Host Installer, 4-Tier Tool Toggles & Live External Activity Audit)
import {
  type McpExternalActivityData,
  type McpExternalActivityQuery,
  McpExternalActivityQuerySchema,
  McpExternalActivityResponseSchema,
  type McpHostInstallData,
  type McpHostInstallRequest,
  McpHostInstallRequestSchema,
  McpHostInstallResponseSchema,
  type McpJsonRpcRequest,
  McpJsonRpcRequestSchema,
  type McpJsonRpcResponse,
  McpJsonRpcResponseSchema,
  type McpToolPolicyDto,
  type McpToolToggleRequest,
  McpToolToggleRequestSchema,
  McpToolToggleResponseSchema,
} from "../lib/validations/mcp-gateway";
import { HarnessDaemonTransport, type HarnessDaemonClientConfig } from "./harness-daemon-transport";

export class McpGatewayClient extends HarnessDaemonTransport {
  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  async getExternalActivity(
    query?: Partial<McpExternalActivityQuery>
  ): Promise<McpExternalActivityData> {
    const validated = McpExternalActivityQuerySchema.parse(query ?? {});
    const search = new URLSearchParams();
    search.set("limit", String(validated.limit));
    if (validated.hostClient) search.set("hostClient", validated.hostClient);
    if (validated.status) search.set("status", validated.status);
    const raw = await this.request(
      `/api/v1/mcp/external-activity?${search.toString()}`,
      McpExternalActivityResponseSchema,
      { method: "GET" }
    );
    return McpExternalActivityResponseSchema.parse(raw).data;
  }

  async installOrRollbackHost(input: McpHostInstallRequest): Promise<McpHostInstallData> {
    const validated = McpHostInstallRequestSchema.parse(input);
    const raw = await this.request("/api/v1/mcp/install", McpHostInstallResponseSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
    return McpHostInstallResponseSchema.parse(raw).data;
  }

  async toggleToolPolicy(input: McpToolToggleRequest): Promise<McpToolPolicyDto> {
    const validated = McpToolToggleRequestSchema.parse(input);
    const raw = await this.request("/api/v1/mcp/tools/toggle", McpToolToggleResponseSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
    return McpToolToggleResponseSchema.parse(raw).data;
  }

  async invokeRpc(input: McpJsonRpcRequest): Promise<McpJsonRpcResponse> {
    const validated = McpJsonRpcRequestSchema.parse(input);
    const raw = await this.request("/api/v1/mcp/rpc", McpJsonRpcResponseSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
    return McpJsonRpcResponseSchema.parse(raw);
  }
}

export const apiMcpGateway = new McpGatewayClient();
