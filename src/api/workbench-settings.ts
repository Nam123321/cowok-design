// @story 04.2
import { z } from "zod";
import {
  AgentSettingDto,
  AgentSettingDtoSchema,
  DeleteMcpServerResponseSchema,
  DoctorScanResponse,
  DoctorScanResponseSchema,
  ExecutionEventDto,
  ExecutionEventDtoSchema,
  McpServerDto,
  McpServerDtoSchema,
  PostJobEventInput,
  PostJobEventInputSchema,
  UpsertAgentSettingInput,
  UpsertAgentSettingInputSchema,
  UpsertMcpServerInput,
  UpsertMcpServerInputSchema,
  VerifyProviderKeyInput,
  VerifyProviderKeyInputSchema,
  VerifyProviderKeyResponse,
  VerifyProviderKeyResponseSchema,
} from "../lib/validations/workbench-settings";
import { HarnessDaemonTransport, readBootCapHeaders } from "./harness-daemon-transport";

export class WorkbenchSettingsClient extends HarnessDaemonTransport {
  listAgentSettings(): Promise<AgentSettingDto[]> {
    return this.request("/api/agent-settings", z.array(AgentSettingDtoSchema), { method: "GET" });
  }

  upsertAgentSetting(input: UpsertAgentSettingInput): Promise<AgentSettingDto> {
    const validated = UpsertAgentSettingInputSchema.parse(input);
    return this.request("/api/agent-settings", AgentSettingDtoSchema, {
      method: "PUT",
      body: JSON.stringify(validated),
    });
  }

  listMcpServers(): Promise<McpServerDto[]> {
    return this.request("/api/mcp-servers", z.array(McpServerDtoSchema), { method: "GET" });
  }

  upsertMcpServer(input: UpsertMcpServerInput): Promise<McpServerDto> {
    const validated = UpsertMcpServerInputSchema.parse(input);
    return this.request("/api/mcp-servers", McpServerDtoSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  deleteMcpServer(serverId: string): Promise<{ deleted: boolean; id: string }> {
    return this.request(
      `/api/mcp-servers/${encodeURIComponent(serverId)}`,
      DeleteMcpServerResponseSchema,
      { method: "DELETE" }
    );
  }

  scanDoctor(): Promise<DoctorScanResponse> {
    return this.request("/api/doctor", DoctorScanResponseSchema, { method: "GET" });
  }

  verifyProviderKey(input: VerifyProviderKeyInput): Promise<VerifyProviderKeyResponse> {
    const validated = VerifyProviderKeyInputSchema.parse(input);
    return this.request("/api/doctor/verify-key", VerifyProviderKeyResponseSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  postJobEvent(projectId: string, input: PostJobEventInput): Promise<ExecutionEventDto> {
    const validated = PostJobEventInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/jobs/events`,
      ExecutionEventDtoSchema,
      {
        method: "POST",
        body: JSON.stringify(validated),
      }
    );
  }

  async streamJobEvents(
    projectId: string,
    opts: {
      jobId?: string;
      nodeId?: string;
      afterSeq?: number;
      signal?: AbortSignal;
      onEvents: (events: ExecutionEventDto[]) => void;
    }
  ): Promise<void> {
    const sessionToken = await bootstrapLocalEditorToken(opts.signal);
    const qs = buildJobStreamQuery(opts);
    const res = await fetch(
      `/api/projects/${encodeURIComponent(projectId)}/flow/jobs/stream?${qs.toString()}`,
      {
        method: "GET",
        headers: {
          Accept: "text/event-stream",
          "X-Cowok-Session-Token": sessionToken,
        },
        signal: opts.signal,
      }
    );
    if (!res.ok || !res.body) {
      throw new Error(`SSE stream failed (${res.status})`);
    }
    await consumeExecutionSseBody(res.body, opts.onEvents);
  }
}

async function bootstrapLocalEditorToken(signal?: AbortSignal): Promise<string> {
  const sessRes = await fetch("/api/auth/local-session", {
    method: "POST",
    headers: { "Content-Type": "application/json", ...readBootCapHeaders() },
    body: JSON.stringify({ role: "editor" }),
    signal,
  });
  if (!sessRes.ok) {
    throw new Error(`Session bootstrap failed (${sessRes.status})`);
  }
  const { sessionToken } = (await sessRes.json()) as { sessionToken: string };
  return sessionToken;
}

function buildJobStreamQuery(opts: {
  jobId?: string;
  nodeId?: string;
  afterSeq?: number;
}): URLSearchParams {
  const qs = new URLSearchParams();
  if (opts.jobId) qs.set("jobId", opts.jobId);
  if (opts.nodeId) qs.set("nodeId", opts.nodeId);
  if (opts.afterSeq !== undefined) qs.set("afterSeq", String(opts.afterSeq));
  return qs;
}

async function consumeExecutionSseBody(
  body: ReadableStream<Uint8Array>,
  onEvents: (events: ExecutionEventDto[]) => void
): Promise<void> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const frames = buffer.split("\n\n");
    buffer = frames.pop() || "";
    for (const frame of frames) {
      dispatchParsedSseFrame(frame, onEvents);
    }
  }
}

function dispatchParsedSseFrame(
  frame: string,
  onEvents: (events: ExecutionEventDto[]) => void
): void {
  const dataLine = frame
    .split("\n")
    .filter((l) => l.startsWith("data:"))
    .map((l) => l.slice(5).trim())
    .join("\n");
  if (!dataLine) return;
  try {
    const parsed = z.array(ExecutionEventDtoSchema).parse(JSON.parse(dataLine));
    if (parsed.length > 0) onEvents(parsed);
  } catch {
    // Ignore non-JSON heartbeat frame
  }
}

export const apiWorkbenchSettings = new WorkbenchSettingsClient();
export const apiAgentSettings = apiWorkbenchSettings;
