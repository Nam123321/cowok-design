// @story 08.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { z } from "zod";
import {
  type CompilePromptBundleRequest,
  CompilePromptBundleRequestSchema,
  type CompiledPromptSnapshotDto,
  CompiledPromptSnapshotDtoSchema,
  type CronScheduleStatusDto,
  CronScheduleStatusDtoSchema,
  type HumanOnlyDecisionReceiptDto,
  HumanOnlyDecisionReceiptDtoSchema,
  type KnowledgeCardDto,
  KnowledgeCardDtoSchema,
  type LearnCardDecisionRequest,
  LearnCardDecisionRequestSchema,
  type ProposeLearnCardRequest,
  ProposeLearnCardRequestSchema,
  type TriggerSyncRequest,
  TriggerSyncRequestSchema,
  type UpsertCronScheduleRequest,
  UpsertCronScheduleRequestSchema,
} from "../lib/validations/plugins-learn-cron";
import {
  type DiscoverOrInstallPluginRequest,
  DiscoverOrInstallPluginRequestSchema,
  type PluginSummaryDto,
  PluginSummaryDtoSchema,
  type QuarantinePluginRequest,
  QuarantinePluginRequestSchema,
  type StagingDecisionRequest,
  StagingDecisionRequestSchema,
  type TransitionPluginRequest,
  TransitionPluginRequestSchema,
} from "../lib/validations/plugins";
import {
  type DiscoveryDescriptorRequest,
  DiscoveryDescriptorRequestSchema,
  type DiscoveryDescriptorResponse,
  DiscoveryDescriptorResponseSchema,
  type RepackageBundleRequest,
  RepackageBundleRequestSchema,
  type RepackageReceipt,
  RepackageReceiptSchema,
  type TriangleLintReport,
  TriangleLintReportSchema,
  type TriangleLintRequest,
  TriangleLintRequestSchema,
  type VideoIntakeRequest,
  VideoIntakeRequestSchema,
  type VideoStoryboardReceipt,
  VideoStoryboardReceiptSchema,
} from "../lib/validations/plugins-repackager";
import { HarnessDaemonTransport } from "./harness-daemon-transport";

const SSE_DATA_PREFIX_LEN = 5;

function parsePluginSseFrame(
  frame: string,
  onRefresh: () => void,
  onConnected?: () => void
): void {
  const dataLines = frame
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice(SSE_DATA_PREFIX_LEN).trim())
    .join("\n");
  if (!dataLines) return;
  try {
    const parsed = JSON.parse(dataLines) as { kind?: string };
    if (parsed.kind === "connected") onConnected?.();
    else if (parsed.kind === "refresh") onRefresh();
  } catch {
    // Ignore malformed heartbeat frames
  }
}

async function consumePluginSseStream(
  body: ReadableStream<Uint8Array>,
  onRefresh: () => void,
  onConnected?: () => void,
  signal?: AbortSignal
): Promise<void> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  const onAbort = () => {
    void reader.cancel().catch(() => undefined);
  };
  signal?.addEventListener("abort", onAbort, { once: true });
  try {
    while (!signal?.aborted) {
      const { value, done } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true }).replace(/\r\n/g, "\n");
      const frames = buffer.split("\n\n");
      buffer = frames.pop() ?? "";
      for (const frame of frames) parsePluginSseFrame(frame, onRefresh, onConnected);
    }
  } finally {
    signal?.removeEventListener("abort", onAbort);
    reader.releaseLock();
  }
}

export class PluginEcosystemClient extends HarnessDaemonTransport {
  listPlugins(opts?: { enabled?: boolean; projectId?: string }): Promise<PluginSummaryDto[]> {
    const params = new URLSearchParams();
    if (opts?.enabled !== undefined) params.set("enabled", opts.enabled ? "true" : "false");
    if (opts?.projectId) params.set("projectId", opts.projectId);
    const qs = params.toString();
    const target = qs ? "/api/plugins?" + qs : "/api/plugins";
    return this.request(target, z.array(PluginSummaryDtoSchema), {
      method: "GET",
    });
  }

  getPluginRegistry(opts?: { enabled?: boolean; projectId?: string }): Promise<PluginSummaryDto[]> {
    const params = new URLSearchParams();
    if (opts?.enabled !== undefined) params.set("enabled", opts.enabled ? "true" : "false");
    if (opts?.projectId) params.set("projectId", opts.projectId);
    const qs = params.toString();
    const target = qs ? "/api/plugins/registry?" + qs : "/api/plugins/registry";
    return this.request(target, z.array(PluginSummaryDtoSchema), {
      method: "GET",
    });
  }

  discoverPlugin(input: DiscoverOrInstallPluginRequest): Promise<PluginSummaryDto> {
    const validated = DiscoverOrInstallPluginRequestSchema.parse(input);
    return this.request("/api/plugins/discovery", PluginSummaryDtoSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  installPlugin(input: DiscoverOrInstallPluginRequest): Promise<PluginSummaryDto> {
    const validated = DiscoverOrInstallPluginRequestSchema.parse(input);
    return this.request("/api/plugins/installs", PluginSummaryDtoSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  transitionPlugin(pluginId: string, input: TransitionPluginRequest): Promise<PluginSummaryDto> {
    const validated = TransitionPluginRequestSchema.parse(input);
    return this.request(`/api/plugins/${encodeURIComponent(pluginId)}/transitions`, PluginSummaryDtoSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  quarantinePlugin(pluginId: string, input: QuarantinePluginRequest): Promise<PluginSummaryDto> {
    const validated = QuarantinePluginRequestSchema.parse(input);
    return this.request(`/api/plugins/${encodeURIComponent(pluginId)}/quarantine`, PluginSummaryDtoSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  promotePluginStaging(pluginId: string, input: StagingDecisionRequest): Promise<PluginSummaryDto> {
    const validated = StagingDecisionRequestSchema.parse(input);
    return this.request(
      `/api/plugins/${encodeURIComponent(pluginId)}/staging/promotions`,
      PluginSummaryDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  discardPluginStaging(pluginId: string, input: StagingDecisionRequest): Promise<PluginSummaryDto> {
    const validated = StagingDecisionRequestSchema.parse(input);
    return this.request(
      `/api/plugins/${encodeURIComponent(pluginId)}/staging/discards`,
      PluginSummaryDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  repackageBundle(input: RepackageBundleRequest): Promise<RepackageReceipt> {
    const validated = RepackageBundleRequestSchema.parse(input);
    return this.request("/api/plugins/repackager/bundles", RepackageReceiptSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  generateDiscoveryStub(input: DiscoveryDescriptorRequest): Promise<DiscoveryDescriptorResponse> {
    const validated = DiscoveryDescriptorRequestSchema.parse(input);
    return this.request("/api/plugins/repackager/discovery-stubs", DiscoveryDescriptorResponseSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  intakeVideoStoryboard(input: VideoIntakeRequest): Promise<VideoStoryboardReceipt> {
    const validated = VideoIntakeRequestSchema.parse(input);
    return this.request("/api/plugins/repackager/video-intakes", VideoStoryboardReceiptSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  runTriangleLint(input: TriangleLintRequest): Promise<TriangleLintReport> {
    const validated = TriangleLintRequestSchema.parse(input);
    return this.request("/api/plugins/linter/checks", TriangleLintReportSchema, {
      method: "POST",
      body: JSON.stringify(validated),
    });
  }

  compilePromptBundle(projectId: string, input: CompilePromptBundleRequest): Promise<CompiledPromptSnapshotDto> {
    const validated = CompilePromptBundleRequestSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/skills/prompt-bundles`,
      CompiledPromptSnapshotDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  listLearnCards(projectId: string): Promise<KnowledgeCardDto[]> {
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/learn/cards`,
      z.array(KnowledgeCardDtoSchema),
      { method: "GET" }
    );
  }

  proposeLearnCard(projectId: string, input: ProposeLearnCardRequest): Promise<KnowledgeCardDto> {
    const validated = ProposeLearnCardRequestSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/learn/cards`,
      KnowledgeCardDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  approveLearnCard(
    projectId: string,
    cardId: string,
    input: LearnCardDecisionRequest
  ): Promise<HumanOnlyDecisionReceiptDto> {
    const validated = LearnCardDecisionRequestSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/learn/cards/${encodeURIComponent(cardId)}/approvals`,
      HumanOnlyDecisionReceiptDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  rejectLearnCard(
    projectId: string,
    cardId: string,
    input: LearnCardDecisionRequest
  ): Promise<HumanOnlyDecisionReceiptDto> {
    const validated = LearnCardDecisionRequestSchema.parse(input);
    return this.request(
      `/api/projects/${encodeURIComponent(projectId)}/learn/cards/${encodeURIComponent(cardId)}/rejections`,
      HumanOnlyDecisionReceiptDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  getPluginSchedule(pluginId: string, projectId?: string): Promise<CronScheduleStatusDto> {
    const encodedId = encodeURIComponent(pluginId);
    const basePath = `/api/plugins/${encodedId}/schedule`;
    const target = projectId ? `${basePath}?projectId=${encodeURIComponent(projectId)}` : basePath;
    return this.request(target, CronScheduleStatusDtoSchema, { method: "GET" });
  }

  upsertPluginSchedule(pluginId: string, input: UpsertCronScheduleRequest): Promise<CronScheduleStatusDto> {
    const validated = UpsertCronScheduleRequestSchema.parse(input);
    return this.request(
      `/api/plugins/${encodeURIComponent(pluginId)}/schedule`,
      CronScheduleStatusDtoSchema,
      { method: "PUT", body: JSON.stringify(validated) }
    );
  }

  triggerPluginSyncNow(pluginId: string, input: TriggerSyncRequest = {}): Promise<CronScheduleStatusDto> {
    const validated = TriggerSyncRequestSchema.parse(input);
    return this.request(
      `/api/plugins/${encodeURIComponent(pluginId)}/sync-jobs`,
      CronScheduleStatusDtoSchema,
      { method: "POST", body: JSON.stringify(validated) }
    );
  }

  /** Subscribes to authenticated project-scoped plugin/LEARN/cron SSE refresh events. */
  async subscribePluginEvents(
    projectId: string,
    opts: { signal?: AbortSignal; onRefresh: () => void; onConnected?: () => void }
  ): Promise<void> {
    const streamPath = `/api/projects/${encodeURIComponent(projectId)}/plugins/stream`;
    const res = await this.openEventStream(streamPath, opts.signal);
    if (!res.body) return;
    await consumePluginSseStream(res.body, opts.onRefresh, opts.onConnected, opts.signal);
  }
}

export const apiPluginCenter = new PluginEcosystemClient();
