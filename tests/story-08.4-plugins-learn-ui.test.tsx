/** @vitest-environment jsdom */
// @story 08.4
// @cover AC-01, AC-02, AC-03, AC-04, AC-05
import fs from "node:fs";
import path from "node:path";
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { PluginEcosystemClient } from "../src/api/plugin-center";
import { CronSyncStatus } from "../src/features/plugins/CronSyncStatus";
import { LearnCardReviewQueue } from "../src/features/plugins/LearnCardReviewQueue";
import { PluginRegistryPanel } from "../src/features/plugins/PluginRegistryPanel";
import { sanitizeLearnMarkdownPreview } from "../src/features/plugins/sanitizeMarkdownPreview";
import { issueBffLoopbackSession } from "../src/lib/api-guard";
import { matchBffRoute } from "../src/lib/vite-bff-routes";
import type {
  CronScheduleStatusDto,
  KnowledgeCardDto,
  PluginSummaryDto,
} from "../src/lib/validations/plugins";
import * as scheduleRoute from "../src/app/api/plugins/[id]/schedule/route";
import * as learnApprovalsRoute from "../src/app/api/projects/[id]/learn/cards/[cardId]/approvals/route";
import * as discoveryRoute from "../src/app/api/plugins/discovery/route";

afterEach(() => {
  cleanup();
});

const SAMPLE_PLUGIN: PluginSummaryDto = {
  id: "plug-ui-1",
  tenantId: "local-default",
  projectId: "11111111-1111-4111-8111-111111111111",
  pluginSlug: "design-tokens-skill",
  version: "1.2.0",
  lifecycleState: "ACTIVE",
  enabled: true,
  sourceType: "github",
  sourceRepoUrl: "https://github.com/cowok-ai/skills",
  upstreamCommitSha: "abc1234",
  cronSchedule: "0 */5 * * * *",
  revision: 3,
  breakingChangeIsolated: true,
  stagingVersion: "2.0.0",
  stagingDiffReasons: ["Major version bump 1.2.0 -> 2.0.0", "New capability network.fetch"],
  quarantined: false,
  quarantineReason: null,
  manifestJson: { name: "design-tokens-skill", version: "1.2.0" },
  updatedAt: "2026-11-17T12:00:00Z",
};

const SAMPLE_LEARN_CARD: KnowledgeCardDto = {
  id: "card-ui-1",
  tenantId: "local-default",
  projectId: "11111111-1111-4111-8111-111111111111",
  // iwish-pattern-verified: IDEMPOTENCY_EXPIRY Unit test fixture for daemon-persisted Learn card DTO
  idempotencyKey: "idem-1",
  category: "RULE",
  title: "Zero Tailwind Policy",
  content:
    'Use CSS vars.\n<script>alert("xss")</script>\n<img src="https://evil.test/px.png" onerror="steal()" />\n![beacon](https://evil.test/tracker.gif)\n[click](javascript:alert(1))',
  contentSha256: "a".repeat(64),
  sourceEvidence: "src/styles/tokens.css:L1-L40",
  status: "PROPOSED",
  revision: 1,
  reviewedBy: null,
  decisionReason: null,
  createdAt: 1700000000,
  acceptedAt: null,
};

const SAMPLE_SCHEDULE: CronScheduleStatusDto = {
  id: "sched-ui-1",
  tenantId: "local-default",
  projectId: "11111111-1111-4111-8111-111111111111",
  pluginId: "plug-ui-1",
  cronExpression: "0 */5 * * * *",
  upstreamRepoUrl: "https://github.com/cowok-ai/skills",
  lastCommitSha: "abc1234",
  syncStatus: "AUTH_REQUIRED",
  consecutiveFailures: 2,
  nextRetryAt: null,
  lastErrorCode: "AUTH_REQUIRED",
  lastErrorMessage: "HTTP 401 unauthorized",
  updatedAt: "2026-11-17T12:00:00Z",
};

describe("Story 08.4 BFF guardedRoute, Routing Table, SDK & XSS/UI States", () => {
  it("matches all 17 Story 08.4 BFF endpoints in vite-bff-routes", () => {
    const routes: Array<[string, string]> = [
      ["GET", "/api/plugins"],
      ["GET", "/api/plugins/registry"],
      ["POST", "/api/plugins/discovery"],
      ["POST", "/api/plugins/installs"],
      ["POST", "/api/plugins/plug-1/transitions"],
      ["POST", "/api/plugins/plug-1/staging/promotions"],
      ["POST", "/api/plugins/plug-1/staging/discards"],
      ["POST", "/api/plugins/repackager/bundles"],
      ["POST", "/api/plugins/repackager/discovery-stubs"],
      ["POST", "/api/plugins/repackager/video-intakes"],
      ["POST", "/api/plugins/linter/checks"],
      ["GET", "/api/plugins/plug-1/schedule"],
      ["PUT", "/api/plugins/plug-1/schedule"],
      ["POST", "/api/plugins/plug-1/sync-jobs"],
      ["POST", "/api/projects/proj-1/skills/prompt-bundles"],
      ["GET", "/api/projects/proj-1/learn/cards"],
      ["POST", "/api/projects/proj-1/learn/cards"],
      ["POST", "/api/projects/proj-1/learn/cards/card-1/approvals"],
      ["POST", "/api/projects/proj-1/learn/cards/card-1/rejections"],
    ];
    for (const [method, pathname] of routes) {
      expect(matchBffRoute(method, pathname)).not.toBeNull();
    }
  });

  it("enforces strict-loopback auth and Zod body validation across guardedRoute handlers", async () => {
    const { sessionToken } = issueBffLoopbackSession({
      tenantId: "local-default",
      userId: "alice",
      role: "editor",
      isHumanWorkbench: true,
    });

    // Unauthenticated rejected 401
    const unauthRes = await learnApprovalsRoute.POST(
      new Request("http://127.0.0.1/api/projects/p1/learn/cards/c1/approvals", {
        method: "POST",
        headers: { host: "127.0.0.1", "content-type": "application/json" },
        body: JSON.stringify({ reason: "ok", expectedRevision: 1 }),
      }),
      { params: { id: "p1", cardId: "c1" } }
    );
    expect(unauthRes.status).toBe(401);

    // Invalid Zod payload rejected 400 VALIDATION_ERROR before hitting daemon
    const invalidCronRes = await scheduleRoute.PUT(
      new Request("http://127.0.0.1/api/plugins/plug-1/schedule", {
        method: "PUT",
        headers: {
          host: "127.0.0.1",
          "x-cowok-session-token": sessionToken,
          "content-type": "application/json",
        },
        body: JSON.stringify({ projectId: "p1", cronExpression: "*", upstreamRepoUrl: "not-a-url" }),
      }),
      { params: { id: "plug-1" } }
    );
    expect(invalidCronRes.status).toBe(400);

    const badDiscoverRes = await discoveryRoute.POST(
      new Request("http://127.0.0.1/api/plugins/discovery", {
        method: "POST",
        headers: {
          host: "127.0.0.1",
          "x-cowok-session-token": sessionToken,
          "content-type": "application/json",
        },
        body: JSON.stringify({ projectId: "" }),
      }),
      {}
    );
    expect(badDiscoverRes.status).toBe(400);
  });

  it("validates PluginEcosystemClient requests and responses via Zod", async () => {
    const mockFetch = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/api/plugins/registry")) {
        return new Response(JSON.stringify([SAMPLE_PLUGIN]), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      }
      return new Response(JSON.stringify(SAMPLE_LEARN_CARD), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    });
    const client = new PluginEcosystemClient({
      baseUrl: "http://127.0.0.1:4173",
      getSessionToken: () => "cwk_bff_test",
      fetchImpl: mockFetch as unknown as typeof fetch,
    });

    const registry = await client.getPluginRegistry();
    expect(registry).toHaveLength(1);
    expect(registry[0].stagingVersion).toBe("2.0.0");
  });

  it("strips script tags, HTML tags, event handlers, and Markdown image beacons in LEARN previews", () => {
    const clean = sanitizeLearnMarkdownPreview(SAMPLE_LEARN_CARD.content);
    expect(clean).not.toContain("<script");
    expect(clean).not.toContain("<img");
    expect(clean).not.toContain("onerror=");
    expect(clean).not.toContain("javascript:");
    expect(clean).toContain("[blocked-image-beacon: beacon]");
    expect(clean).toContain("Use CSS vars.");
  });

  it("renders all 4 UI sub-states (Loading, Empty, Error, Default) and dispatches HITL actions without raw Tailwind classes", () => {
    // 1. Loading
    const onRetry = vi.fn();
    const { rerender } = render(
      <PluginRegistryPanel
        plugins={[]}
        loading={true}
        error={null}
        pendingActionId={null}
        actionFeedback={null}
        onRefresh={onRetry}
        onTransition={vi.fn()}
        onPromoteStaging={vi.fn()}
        onDiscardStaging={vi.fn()}
      />
    );
    expect(screen.getByTestId("plugin-registry-loading")).toBeTruthy();

    // 2. Empty
    rerender(
      <PluginRegistryPanel
        plugins={[]}
        loading={false}
        error={null}
        pendingActionId={null}
        actionFeedback={null}
        onRefresh={onRetry}
        onTransition={vi.fn()}
        onPromoteStaging={vi.fn()}
        onDiscardStaging={vi.fn()}
      />
    );
    expect(screen.getByTestId("plugin-registry-empty")).toBeTruthy();

    // 3. Error + Retry
    rerender(
      <PluginRegistryPanel
        plugins={[]}
        loading={false}
        error="CAS_REVISION_MISMATCH"
        pendingActionId={null}
        actionFeedback={null}
        onRefresh={onRetry}
        onTransition={vi.fn()}
        onPromoteStaging={vi.fn()}
        onDiscardStaging={vi.fn()}
      />
    );
    expect(screen.getByTestId("plugin-registry-error")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Retry|Thử lại/ }));
    expect(onRetry).toHaveBeenCalledTimes(1);

    // 4. Default + StagingDiffCard Promote/Discard
    const onPromote = vi.fn();
    const onDiscard = vi.fn();
    rerender(
      <PluginRegistryPanel
        plugins={[SAMPLE_PLUGIN]}
        loading={false}
        error={null}
        pendingActionId={null}
        actionFeedback={null}
        onRefresh={onRetry}
        onTransition={vi.fn()}
        onPromoteStaging={onPromote}
        onDiscardStaging={onDiscard}
      />
    );
    expect(screen.getByTestId("staging-diff-card-design-tokens-skill")).toBeTruthy();
    const promoteBtn = screen.getByTestId("btn-promote-staging-design-tokens-skill") as HTMLButtonElement;
    const discardBtn = screen.getByTestId("btn-discard-staging-design-tokens-skill") as HTMLButtonElement;
    expect(promoteBtn.disabled).toBe(true);
    expect(discardBtn.disabled).toBe(true);
    const stagingReason = screen.getByTestId("input-staging-reason-design-tokens-skill");
    fireEvent.change(stagingReason, { target: { value: "Verified diff schema in review" } });
    fireEvent.click(promoteBtn);
    expect(onPromote).toHaveBeenCalledWith(SAMPLE_PLUGIN, "Verified diff schema in review");
    fireEvent.click(discardBtn);
    expect(onDiscard).toHaveBeenCalledWith(SAMPLE_PLUGIN, "Verified diff schema in review");

    // 5. LearnCardReviewQueue renders sanitized text (0 <img> / <script> tags), budget, filters, & >=4 char rationale
    const onApprove = vi.fn();
    const onReject = vi.fn();
    render(
      <LearnCardReviewQueue
        cards={[SAMPLE_LEARN_CARD]}
        loading={false}
        error={null}
        pendingActionId={null}
        onRefresh={vi.fn()}
        onApprove={onApprove}
        onReject={onReject}
      />
    );
    expect(screen.getByTestId("learn-token-budget").textContent).toContain("/1500");
    const preview = screen.getByTestId(`learn-card-preview-${SAMPLE_LEARN_CARD.id}`);
    expect(preview.querySelectorAll("img, script")).toHaveLength(0);
    const approveBtn = screen.getByTestId(`btn-approve-learn-${SAMPLE_LEARN_CARD.id}`) as HTMLButtonElement;
    expect(approveBtn.disabled).toBe(true);
    fireEvent.change(screen.getByTestId(`input-learn-reason-${SAMPLE_LEARN_CARD.id}`), {
      target: { value: "Verified clean token invariant" },
    });
    fireEvent.click(approveBtn);
    expect(onApprove).toHaveBeenCalledWith(SAMPLE_LEARN_CARD, "Verified clean token invariant");

    // 6. CronSyncStatus AUTH_REQUIRED recovery button
    const onSyncNow = vi.fn();
    render(
      <CronSyncStatus
        plugins={[SAMPLE_PLUGIN]}
        schedulesByPlugin={{ [SAMPLE_PLUGIN.id]: SAMPLE_SCHEDULE }}
        loading={false}
        error={null}
        pendingActionId={null}
        onRefresh={vi.fn()}
        onSyncNow={onSyncNow}
      />
    );
    expect(screen.getByTestId(`cron-auth-banner-${SAMPLE_PLUGIN.pluginSlug}`)).toBeTruthy();
    fireEvent.click(screen.getByTestId(`btn-refresh-auth-sync-${SAMPLE_PLUGIN.pluginSlug}`));
    expect(onSyncNow).toHaveBeenCalledWith(SAMPLE_PLUGIN, true);

    // 7. Zero Tailwind utility classes or raw fetch in src/features/plugins
    const featureDir = path.resolve(process.cwd(), "src/features/plugins");
    for (const file of fs.readdirSync(featureDir)) {
      if (!file.endsWith(".tsx") && !file.endsWith(".ts")) continue;
      const src = fs.readFileSync(path.join(featureDir, file), "utf8");
      expect(src).not.toMatch(/\bbg-(slate|purple|zinc|gray)-/);
      expect(src).not.toMatch(/\bfetch\s*\(/);
      expect(src).not.toMatch(/\buseEffect\s*\(/);
    }
  });

  it("ut_084_ui_sse_06: subscribePluginEvents sends X-Cowok-Session-Token and refreshes only on refresh frames", async () => {
    const encoder = new TextEncoder();
    const calls: Array<{ url: string; headers: Headers }> = [];
    const fetchMock: typeof fetch = vi.fn(async (input, init) => {
      const url = typeof input === "string" ? input : input.toString();
      const headers = new Headers(init?.headers || {});
      calls.push({ url, headers });
      const stream = new ReadableStream<Uint8Array>({
        start(controller) {
          controller.enqueue(
            encoder.encode(
              'event: plugin_ecosystem_update\ndata: {"projectId":"p-1","kind":"connected"}\n\n' +
                'event: plugin_ecosystem_update\ndata: {"projectId":"p-1","kind":"refresh"}\n\n'
            )
          );
          controller.close();
        },
      });
      return new Response(stream, {
        status: 200,
        headers: { "Content-Type": "text/event-stream" },
      });
    });

    const client = new PluginEcosystemClient({
      baseUrl: "http://127.0.0.1:4173",
      getSessionToken: () => "tok-sse-verified-084",
      fetchImpl: fetchMock,
    });
    const onRefresh = vi.fn();
    await client.subscribePluginEvents("p-1", { onRefresh });

    expect(calls).toHaveLength(1);
    expect(calls[0].url).toBe("http://127.0.0.1:4173/api/projects/p-1/plugins/stream");
    expect(calls[0].headers.get("X-Cowok-Session-Token")).toBe("tok-sse-verified-084");
    expect(calls[0].headers.get("Accept")).toBe("text/event-stream");
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });
});
