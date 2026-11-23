// @vitest-environment jsdom
// @story 11.1
// @implements AC-01, AC-02, AC-03, AC-04 (Unit Tests for BotFleet SDK, Dispatch Wiring & Settings Bento Live Status)
import React from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { apiBotFleet, BotFleetApiClient } from "../../src/api/bot-fleet";
import { apiChangeProposal } from "../../src/api/change-proposal";
import { apiFlowExecution } from "../../src/api/flow-execution";
import { apiSdiEngine } from "../../src/api/sdi-engine";
import { dispatchNodeExecutionToDaemon } from "../../src/features/flow/hooks/dispatchNodeExecution";
import { globalKeyedParallelCoordinator } from "../../src/features/flow/store/keyed-parallel-coordinator";
import type { WorkbenchNode } from "../../src/features/flow/store/flow-workbench-store";
import { setGlobalLocale } from "../../src/hooks/useTranslation";
import { BotFleetExecuteRequestSchema } from "../../src/lib/validations/bot-fleet";
import { SettingsApiBentoPanel } from "../../src/pages/workbench/components/SettingsApiBentoPanel";
import { SettingsDetectedProvidersPanel } from "../../src/pages/workbench/components/SettingsDetectedProvidersPanel";

const PROJECT_UUID = "11111111-1111-4111-8111-111111111111";
const HTTP_OK = 200;
const ADAPTER_COUNT_27 = 27;
const READY_ADAPTER_COUNT = 2;
const RETRY_AFTER_SECS = 18;
const EXEC_DURATION_MS = 145;
const EXEC_TIMESTAMP = 1730500123;

const MOCK_PROVIDERS_DATA = {
  sandboxProtocol: "posix_0600_fd_pgid",
  sessionDirIsolated: true,
  adapterCount: ADAPTER_COUNT_27,
  readyAdapterCount: READY_ADAPTER_COUNT,
  adapters: [
    {
      id: "claude",
      name: "Claude Code",
      command: "claude",
      category: "cli",
      promptTransport: "stdin_fd_0600",
      installed: true,
      authenticated: true,
      sandboxReady: true,
      supportsMcpConfig: true,
      defaultModel: "claude-sonnet-4-5",
      status: "ready",
    },
  ],
  byokProviders: [
    {
      providerId: "gemini",
      configured: true,
      inCooldown: true,
      retryAfterSecs: RETRY_AFTER_SECS,
      cooldownUntilMs: EXEC_TIMESTAMP + RETRY_AFTER_SECS * 1000,
      consecutive429s: 1,
      lastError: "HTTP 429 rate limited",
    },
    {
      providerId: "openai",
      configured: true,
      inCooldown: false,
      retryAfterSecs: 0,
      consecutive429s: 0,
    },
  ],
  checkedAt: "2026-11-22T18:00:00Z",
};

const makeNode = (id: string): WorkbenchNode => ({
  id,
  screenKey: "SCR-11",
  title: "Bot Fleet Screen",
  nodeType: "screen",
  status: "draft",
  posX: 100,
  posY: 120,
  activeSubState: "DEFAULT",
  fixtureRef: "DEFAULT",
  note: "",
  version: 1,
});

function makeCoordinatorMock() {
  return {
    ...globalKeyedParallelCoordinator,
    summary: globalKeyedParallelCoordinator.getSummary(),
    syncFromDaemon: vi.fn(async () => ({ sessions: [], jobs: [], activeSlotCount: 0, maxConcurrent: 4 })),
    reconcileSingleJob: vi.fn(),
    enqueueJob: globalKeyedParallelCoordinator.enqueueJob.bind(globalKeyedParallelCoordinator),
    steerJob: globalKeyedParallelCoordinator.steerJob.bind(globalKeyedParallelCoordinator),
    pauseJob: globalKeyedParallelCoordinator.pauseJob.bind(globalKeyedParallelCoordinator),
    resumeJob: globalKeyedParallelCoordinator.resumeJob.bind(globalKeyedParallelCoordinator),
    cancelJob: globalKeyedParallelCoordinator.cancelJob.bind(globalKeyedParallelCoordinator),
    failJob: globalKeyedParallelCoordinator.failJob.bind(globalKeyedParallelCoordinator),
    completeJob: globalKeyedParallelCoordinator.completeJob.bind(globalKeyedParallelCoordinator),
    getNodeStatus: globalKeyedParallelCoordinator.getNodeStatus.bind(globalKeyedParallelCoordinator),
    getActiveJobs: globalKeyedParallelCoordinator.getActiveJobs.bind(globalKeyedParallelCoordinator),
  };
}

describe("Story 11.1 Bot Fleet SDK, Real Execution Wiring & Settings Bento Status", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setGlobalLocale("en-US");
    globalKeyedParallelCoordinator.clearAll();
  });

  afterEach(() => {
    cleanup();
    setGlobalLocale("vi-VN");
  });

  it("UT-11.1-01: validates strict Zod contracts and BotFleetApiClient responses", async () => {
    const bad = BotFleetExecuteRequestSchema.safeParse({
      projectId: "not-a-uuid",
      sessionId: "sess-1",
      prompt: "hello",
      extraForbidden: true,
    });
    expect(bad.success).toBe(false);

    const badSession = BotFleetExecuteRequestSchema.safeParse({
      projectId: PROJECT_UUID,
      sessionId: "../escape",
      prompt: "hello",
    });
    expect(badSession.success).toBe(false);

    const badModel = BotFleetExecuteRequestSchema.safeParse({
      projectId: PROJECT_UUID,
      sessionId: "sess-1",
      modelId: "-malicious?flag",
      prompt: "hello",
    });
    expect(badModel.success).toBe(false);

    const fakeFetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes("/api/auth/local-session")) {
        return new Response(JSON.stringify({ sessionToken: "tok-111" }), { status: HTTP_OK });
      }
      if (url.includes("/api/v1/bot-fleet/providers/status")) {
        return new Response(
          JSON.stringify({ ok: true, requestId: "req-st", data: MOCK_PROVIDERS_DATA }),
          { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
        );
      }
      return new Response(
        JSON.stringify({
          ok: true,
          requestId: "req-ex",
          data: {
            executionId: "exec-1",
            mode: "byok",
            adapterId: null,
            providerId: "openai",
            modelId: "gpt-4.1",
            normalizedContent: "Normalized output",
            extractedJson: { status: "ok" },
            ansiSequencesStripped: 2,
            jsonBlocksExtracted: 1,
            svgViewboxesInjected: 1,
            cooldownApplied: true,
            failoverProviders: ["gemini"],
            pgidReaped: false,
            timedOut: false,
            durationMs: EXEC_DURATION_MS,
            executedAt: EXEC_TIMESTAMP,
          },
        }),
        { status: HTTP_OK, headers: { "Content-Type": "application/json" } }
      );
    });

    const client = new BotFleetApiClient({ fetchImpl: fakeFetch as unknown as typeof fetch });
    const statusRes = await client.getProvidersStatus();
    expect(statusRes.data.adapterCount).toBe(ADAPTER_COUNT_27);

    const execRes = await client.executePrompt({
      projectId: PROJECT_UUID,
      sessionId: "sess-1",
      prompt: "Build hero section",
    });
    expect(execRes.data.providerId).toBe("openai");
    expect(execRes.data.cooldownApplied).toBe(true);
  });

  it("UT-11.1-02: wires real POST /api/v1/bot-fleet/execute in dispatchNodeExecutionToDaemon with graceful fallback", async () => {
    const appendMsg = vi.fn();
    vi.spyOn(apiFlowExecution, "streamJobEvents").mockResolvedValue();
    vi.spyOn(apiFlowExecution, "enqueueJobs").mockResolvedValue({
      jobs: [
        {
          id: "job-111",
          projectId: PROJECT_UUID,
          sessionId: "sess-111",
          nodeId: "node-111",
          compositeScopeKey: `${PROJECT_UUID}::sess-111::node-111`,
          daemonNonce: "nonce-1",
          status: "RUNNING",
          fencingToken: 1,
          priority: 10,
          staleAfterTimestamp: null,
          createdAt: EXEC_TIMESTAMP,
          updatedAt: EXEC_TIMESTAMP,
        },
      ],
      activeSlotCount: 1,
    });
    vi.spyOn(apiSdiEngine, "compileIntent").mockResolvedValue({
      id: "intent-111",
      tenantId: "local-default",
      projectId: PROJECT_UUID,
      rawPrompt: "Build card",
      intentType: "create_screen",
      status: "ready",
      ambiguityScore: 0.1,
      riskScore: 0.1,
      targetScope: { nodeIds: ["node-111"], screenKeys: ["SCR-11"], components: [] },
      createdAt: "2026-11-22T00:00:00Z",
      updatedAt: "2026-11-22T00:00:00Z",
    });
    const fleetSpy = vi.spyOn(apiBotFleet, "executePrompt").mockResolvedValue({
      ok: true,
      requestId: "req-fleet-1",
      data: {
        executionId: "exec-live-1",
        mode: "byos",
        adapterId: "claude",
        providerId: null,
        modelId: "claude-sonnet-4-5",
        normalizedContent: "<svg viewBox=\"0 0 24 24\"></svg>",
        extractedJson: null,
        ansiSequencesStripped: 1,
        jsonBlocksExtracted: 0,
        svgViewboxesInjected: 1,
        cooldownApplied: false,
        failoverProviders: [],
        pgidReaped: true,
        timedOut: false,
        durationMs: EXEC_DURATION_MS,
        executedAt: EXEC_TIMESTAMP,
      },
    });
    vi.spyOn(apiChangeProposal, "createProposal").mockRejectedValue(new Error("offline"));

    await dispatchNodeExecutionToDaemon({
      projectId: PROJECT_UUID,
      sessionId: "sess-111",
      primaryNode: makeNode("node-111"),
      selectedNodeIds: ["node-111"],
      prompt: "Build card",
      coordinator: makeCoordinatorMock(),
      onAppendMsg: appendMsg,
      t: (_k, f) => f,
      formatNumber: String,
    });

    expect(fleetSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        projectId: PROJECT_UUID,
        sessionId: "sess-111",
        mode: "auto",
      })
    );
    expect(appendMsg).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "fleet-exec-live-1",
      })
    );
  });

  it("UT-11.1-03: renders live 26+ CLI POSIX sandbox badge and BYOK 429 cooldown indicators in Settings Bento panels", async () => {
    const onKeyChange = vi.fn();
    vi.spyOn(apiBotFleet, "getProvidersStatus").mockResolvedValue({
      ok: true,
      requestId: "req-ui-1",
      data: MOCK_PROVIDERS_DATA,
    });

    render(
      <div>
        <SettingsDetectedProvidersPanel
          readyTools={[
            {
              id: "claude",
              name: "Claude Code",
              command: "claude",
              category: "cli",
              scanMode: "passive",
              installed: true,
              authenticated: true,
              status: "ready",
              latencyMs: 5,
            },
          ]}
          setupTools={[]}
          moreTools={[]}
          connectedIds={["claude"]}
          loading={false}
          probingId={null}
          onToggleConnect={vi.fn()}
          onRecheckAll={vi.fn()}
          onProbeTool={vi.fn()}
        />
        <SettingsApiBentoPanel
          ephemeralApiKeys={{ gemini: "", claude: "", openai: "", openrouter: "", deepseek: "", byosToken: "" }}
          onEphemeralKeyChange={onKeyChange}
          byosEndpoint=""
          onByosEndpointChange={vi.fn()}
          showKeys={false}
          onToggleShowKeys={vi.fn()}
          verifyStates={{}}
          onVerifyProvider={vi.fn()}
        />
      </div>
    );

    const sandboxBadge = await screen.findByTestId("byos-sandbox-status-badge");
    expect(sandboxBadge.textContent).toContain("27");
    expect(await screen.findByTestId("adapter-transport-claude")).toBeDefined();

    const coolingSummary = await screen.findByTestId("byok-cooling-summary-badge");
    expect(coolingSummary.textContent).toContain("1");

    fireEvent.click(screen.getByTestId("toggle-api-bento-btn"));
    await waitFor(() => {
      expect(screen.getByTestId("byok-cooldown-dot-gemini")).toBeDefined();
      expect(screen.getByTestId("byok-cooldown-banner-gemini")).toBeDefined();
    });

    // M-06: OpenRouter key edits propagate to lifted SettingsModal state
    fireEvent.click(screen.getByTestId("api-bento-tile-openrouter"));
    const editorInput = screen.getByTestId("api-editor-openrouter").querySelector("input");
    expect(editorInput).not.toBeNull();
    fireEvent.change(editorInput!, { target: { value: "sk-or-test-key-1234567890" } });
    expect(onKeyChange).toHaveBeenCalledWith("openrouter", "sk-or-test-key-1234567890");
  });
});
