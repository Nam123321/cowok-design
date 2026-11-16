// @story 04.4
// @implements AC-01, AC-02, AC-03, AC-04 (Flow Execution SDK + Last-Event-ID Cursor)
import { z } from "zod";
import {
  EnqueueNodeJobInputSchema,
  EnqueueNodeJobsResponseSchema,
  ExecutionEventDtoSchema,
  MarkNodeSessionReadInputSchema,
  MarkNodeSessionReadResponseSchema,
  NodeExecutionJobDtoSchema,
  NodeSessionsResponseSchema,
  PostJobEventV2InputSchema,
  SteerNodeJobInputSchema,
  TransitionNodeJobInputSchema,
  type EnqueueNodeJobInput,
  type EnqueueNodeJobsResponse,
  type MarkNodeSessionReadInput,
  type MarkNodeSessionReadResponse,
  type NodeExecutionJobDto,
  type NodeSessionsResponse,
  type PostJobEventV2Input,
  type SteerNodeJobInput,
  type TransitionNodeJobInput,
} from "@/lib/validations/flow-execution";
import { type ExecutionEventDto } from "@/lib/validations/workbench-settings";
import {
  HarnessDaemonTransport,
  readBootCapHeaders,
  type HarnessDaemonClientConfig,
} from "./harness-daemon-transport";

const STREAM_DATA_PREFIX_LEN = 5;
const STREAM_ID_PREFIX_LEN = 3;

export class FlowExecutionClient extends HarnessDaemonTransport {
  private readonly projectCursors = new Map<string, number>();

  constructor(config: HarnessDaemonClientConfig = {}) {
    super(config);
  }

  getLastProjectSeq(projectId: string): number {
    return this.projectCursors.get(projectId) ?? 0;
  }

  setLastProjectSeq(projectId: string, seq: number): void {
    const prev = this.projectCursors.get(projectId) ?? 0;
    if (seq > prev) {
      this.projectCursors.set(projectId, seq);
    }
  }

  enqueueJobs(projectId: string, input: EnqueueNodeJobInput): Promise<EnqueueNodeJobsResponse> {
    const validated = EnqueueNodeJobInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/jobs/enqueue`,
      EnqueueNodeJobsResponseSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  steerJob(projectId: string, input: SteerNodeJobInput): Promise<NodeExecutionJobDto> {
    const validated = SteerNodeJobInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/jobs/steer`,
      NodeExecutionJobDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  pauseJob(projectId: string, input: TransitionNodeJobInput): Promise<NodeExecutionJobDto> {
    const validated = TransitionNodeJobInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/jobs/pause`,
      NodeExecutionJobDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  resumeJob(projectId: string, input: TransitionNodeJobInput): Promise<NodeExecutionJobDto> {
    const validated = TransitionNodeJobInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/jobs/resume`,
      NodeExecutionJobDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  cancelJob(projectId: string, input: TransitionNodeJobInput): Promise<NodeExecutionJobDto> {
    const validated = TransitionNodeJobInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/jobs/cancel`,
      NodeExecutionJobDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  listNodeSessions(projectId: string, sessionId?: string): Promise<NodeSessionsResponse> {
    const qs = sessionId ? `?sessionId=${encodeURIComponent(sessionId)}` : "";
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/node-sessions${qs}`,
      NodeSessionsResponseSchema,
      { method: "GET" }
    );
  }

  markNodeSessionRead(
    projectId: string,
    input: MarkNodeSessionReadInput
  ): Promise<MarkNodeSessionReadResponse> {
    const validated = MarkNodeSessionReadInputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/node-sessions/read`,
      MarkNodeSessionReadResponseSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  postJobEvent(projectId: string, input: PostJobEventV2Input): Promise<ExecutionEventDto> {
    const validated = PostJobEventV2InputSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/flow/jobs/events`,
      ExecutionEventDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
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
    const token = await bootstrapFlowEditorToken(opts.signal);
    const cursor = opts.afterSeq ?? this.getLastProjectSeq(projectId);
    const qs = new URLSearchParams();
    if (opts.jobId) qs.set("jobId", opts.jobId);
    if (opts.nodeId) qs.set("nodeId", opts.nodeId);
    if (cursor > 0) qs.set("afterSeq", String(cursor));

    const headers: Record<string, string> = {
      Accept: "text/event-stream",
      "X-Cowok-Session-Token": token,
    };
    if (cursor > 0) headers["Last-Event-ID"] = String(cursor);

    const res = await fetch(
      `/api/projects/${encodeURIComponent(projectId)}/flow/jobs/stream?${qs.toString()}`,
      { method: "GET", headers, signal: opts.signal }
    );
    if (!res.ok || !res.body) {
      throw new Error(`SSE stream failed (${res.status})`);
    }
    await consumeFlowSseBody(res.body, (events, eventId) => {
      for (const ev of events) {
        this.setLastProjectSeq(projectId, Math.max(ev.projectSeq ?? 0, ev.seqNo ?? 0));
      }
      if (eventId > 0) this.setLastProjectSeq(projectId, eventId);
      opts.onEvents(events);
    });
  }
}

async function bootstrapFlowEditorToken(signal?: AbortSignal): Promise<string> {
  const sessRes = await fetch("/api/auth/local-session", {
    method: "POST",
    headers: { "Content-Type": "application/json", ...readBootCapHeaders() },
    body: JSON.stringify({ role: "editor" }),
    signal,
  });
  if (!sessRes.ok) throw new Error(`Session bootstrap failed (${sessRes.status})`);
  const { sessionToken } = (await sessRes.json()) as { sessionToken: string };
  return sessionToken;
}

function parseFlowSseFrame(
  frame: string,
  onBatch: (events: ExecutionEventDto[], eventId: number) => void
): void {
  const lines = frame.split("\n");
  const idRaw = lines.find((l) => l.startsWith("id:"))?.slice(STREAM_ID_PREFIX_LEN).trim();
  const eventId = idRaw ? Number(idRaw) : 0;
  const dataLine = lines
    .filter((l) => l.startsWith("data:"))
    .map((l) => l.slice(STREAM_DATA_PREFIX_LEN).trim())
    .join("\n");
  if (!dataLine) return;
  try {
    const parsed = z.array(ExecutionEventDtoSchema).parse(JSON.parse(dataLine));
    if (parsed.length > 0) onBatch(parsed, Number.isFinite(eventId) ? eventId : 0);
  } catch {
    // Ignore malformed keep-alive or partial diagnostic frames
  }
}

async function consumeFlowSseBody(
  body: ReadableStream<Uint8Array>,
  onBatch: (events: ExecutionEventDto[], eventId: number) => void
): Promise<void> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      const frames = buffer.split("\n\n");
      buffer = frames.pop() || "";
      for (const frame of frames) parseFlowSseFrame(frame, onBatch);
    }
  } finally {
    reader.releaseLock();
  }
}

export const apiFlowExecution = new FlowExecutionClient();
