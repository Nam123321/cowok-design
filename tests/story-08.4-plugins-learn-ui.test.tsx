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
import {
  buildWorkbenchProofCookieHeader,
  handleBootstrapSession,
  issueBootCapToken,
  revokeBootCapToken,
} from "../src/lib/vite-bff-bootstrap";
import { setBoundBffPort } from "../src/lib/api-guard";
import {
  DiscoverOrInstallPluginRequestSchema,
  PluginSummaryDtoSchema,
  QuarantinePluginRequestSchema,
  SkillManifestInputSchema,
  type PluginSummaryDto,
} from "../src/lib/validations/plugins";
import { isPublicHttpsUrl, safeNonGithubSourceUrl } from "../src/lib/validations/plugins-byte-limit";
import {
  LearnCardCategoryEnum,
  ProposeLearnCardRequestSchema,
  UpsertCronScheduleRequestSchema,
  type CronScheduleStatusDto,
  type KnowledgeCardDto,
} from "../src/lib/validations/plugins-learn-cron";
import {
  DiscoveryDescriptorRequestSchema,
  RepackageBundleRequestSchema,
  TriangleLintReportSchema,
  TriangleLintRequestSchema,
  VideoIntakeRequestSchema,
} from "../src/lib/validations/plugins-repackager";
import { validateCronSchedules, validateLearnCards, validatePluginCards } from "../src/features/plugin-center/cards";
import { PluginCenterPage } from "../src/pages/plugins/PluginCenterPage";
import * as scheduleRoute from "../src/app/api/plugins/[id]/schedule/route";
import * as quarantineRoute from "../src/app/api/plugins/[id]/quarantine/route";
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
  upstreamRepoUrl: "https://github.com/cowok-ai/skills",
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
      ["POST", "/api/plugins/plug-1/quarantine"],
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

    const stubFetch = vi.fn(async () =>
      new Response(
        JSON.stringify({
          discoveryStub: {
            name: "sdi-skill",
            version: "1.0.0",
            description: "Compact discovery stub",
            capabilities: ["specs.read"],
          },
          skillPlugin: {
            id: "sdi-skill",
            name: "sdi-skill",
            version: "1.0.0",
            entry_point: "SKILL.md",
            permissions: ["specs.read"],
          },
        }),
        { status: 200, headers: { "content-type": "application/json" } }
      )
    );
    const stubClient = new PluginEcosystemClient({
      baseUrl: "http://127.0.0.1:4173",
      getSessionToken: () => "cwk_bff_test",
      fetchImpl: stubFetch as unknown as typeof fetch,
    });
    const desc = await stubClient.generateDiscoveryStub({ bundleDir: "skills/sdi-skill" });
    expect(desc.discoveryStub.name).toBe("sdi-skill");
    expect(desc.skillPlugin.entry_point).toBe("SKILL.md");
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
    expect(screen.getByTestId("learn-token-budget").textContent).toMatch(/\/1[.,]?500/);
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

  it("ut_084_audit_remediation: unscheduled CronScheduleEditor reachable, hardened Markdown XSS/ref-beacon strip, Zod byte parity & cookie-gated HUMAN bootstrap", async () => {
    const onSaveSchedule = vi.fn();
    render(
      <CronSyncStatus
        plugins={[SAMPLE_PLUGIN]}
        schedulesByPlugin={{ [SAMPLE_PLUGIN.id]: null }}
        loading={false}
        error={null}
        pendingActionId={null}
        onRefresh={vi.fn()}
        onSyncNow={vi.fn()}
        onSaveSchedule={onSaveSchedule}
      />
    );
    expect(screen.getByTestId(`cron-status-${SAMPLE_PLUGIN.pluginSlug}`).textContent).toBe("UNSCHEDULED");
    const exprInput = screen.getByTestId(`input-cron-expr-${SAMPLE_PLUGIN.pluginSlug}`) as HTMLInputElement;
    fireEvent.change(exprInput, { target: { value: "0 * * * *" } });
    fireEvent.submit(screen.getByTestId(`cron-schedule-form-${SAMPLE_PLUGIN.pluginSlug}`));
    expect(onSaveSchedule).toHaveBeenCalledWith(SAMPLE_PLUGIN, "0 * * * *", SAMPLE_PLUGIN.sourceRepoUrl);

    const dirtyMd = `<scr<script>ipt>alert(1)</script><img\n![px][ref]\n[ref]: https://evil.test/p.png\nSafe rule`;
    const cleanMd = sanitizeLearnMarkdownPreview(dirtyMd);
    expect(cleanMd).not.toContain("<script");
    expect(cleanMd).not.toContain("<img");
    expect(cleanMd).not.toContain("https://evil.test");
    expect(cleanMd).toContain("Safe rule");

    expect(LearnCardCategoryEnum.safeParse("FLOW").success).toBe(true);
    expect(SkillManifestInputSchema.safeParse({ name: "s", version: "1.0.0", description: "d", risk_tier: 4 }).success).toBe(false);
    expect(
      SkillManifestInputSchema.parse({
        name: "s",
        version: "1.0.0",
        description: "d",
        parameters: { a: 1 },
        tags: ["t1"],
      }).tags
    ).toEqual(["t1"]);
    expect(
      ProposeLearnCardRequestSchema.safeParse({
        category: "FLOW",
        title: "á".repeat(90),
        content: "ok",
        sourceEvidence: "src",
      }).success
    ).toBe(false);

    const cap = issueBootCapToken();
    setBoundBffPort(5173);
    try {
      const { EventEmitter } = await import("node:events");
      const runReq = async (headers: Record<string, string>) => {
        const req = new EventEmitter() as unknown as import("node:http").IncomingMessage & { headers: Record<string, string> };
        req.headers = { host: "127.0.0.1:5173", ...headers };
        queueMicrotask(() => {
          req.emit("data", Buffer.from(JSON.stringify({ role: "editor" })));
          req.emit("end");
        });
        let body = "";
        const res = {
          statusCode: 200,
          setHeader: vi.fn(),
          end: (chunk: string) => { body = chunk; },
        } as unknown as import("node:http").ServerResponse;
        await handleBootstrapSession(req, res);
        return JSON.parse(body) as { isHumanWorkbench?: boolean; role: string };
      };

      const noCookie = await runReq({ "x-cowok-boot-cap": cap, "sec-fetch-site": "same-origin" });
      expect(noCookie.role).toBe("editor");
      expect(noCookie.isHumanWorkbench).toBeUndefined();

      const cookiePair = buildWorkbenchProofCookieHeader(cap).split(";")[0];
      const noSfs = await runReq({ "x-cowok-boot-cap": cap, cookie: cookiePair });
      expect(noSfs.isHumanWorkbench).toBeUndefined();

      const validHuman = await runReq({ "x-cowok-boot-cap": cap, "sec-fetch-site": "same-origin", cookie: cookiePair });
      expect(validHuman.isHumanWorkbench).toBe(true);

      // Wave 3 regression checks: keep literal math '<' without double-escaping, ARCHIVED control, decoded route params
      expect(sanitizeLearnMarkdownPreview("Ensure a < b holds cleanly")).toBe("Ensure a < b holds cleanly");
      expect(matchBffRoute("POST", "/api/plugins/my%20plugin/transitions")?.params.id).toBe("my plugin");
      render(
        <PluginRegistryPanel
          plugins={[{ ...SAMPLE_PLUGIN, lifecycleState: "DEPRECATED" }]}
          loading={false}
          error={null}
          pendingActionId={null}
          actionFeedback={null}
          onRefresh={vi.fn()}
          onTransition={vi.fn()}
          onPromoteStaging={vi.fn()}
          onDiscardStaging={vi.fn()}
        />
      );
      expect(screen.getByTestId(`btn-archive-${SAMPLE_PLUGIN.pluginSlug}`)).toBeTruthy();
    } finally {
      revokeBootCapToken(cap);
      setBoundBffPort(null);
    }
  });

  it("ut_084_round2_wire_parity: validates Rust-serialized null Option<T> fields, SSRF/quota bounds, and sanitizer edges", async () => {
    const rustLintJson = JSON.stringify({
      total_issues: 1,
      new_in_this_call: 1,
      fixed_issues: 0,
      passed: false,
      duration_ms: 2,
      issues: [
        {
          rule_id: "RULE_GEO_07_HEADING_HIERARCHY",
          category: "WcagContrast",
          severity: "Warning",
          message: "Heading hierarchy skips <h2> between <h1> and <h3>.",
          line: 1,
          column: null,
          snippet: null,
          auto_fixable: false,
        },
      ],
    });
    const lintFetch: typeof fetch = vi.fn(async () => new Response(rustLintJson, { status: 200 }));
    const client = new PluginEcosystemClient({
      baseUrl: "http://127.0.0.1:4173",
      getSessionToken: () => "tok-lint",
      fetchImpl: lintFetch,
    });
    const lintRes = await client.runTriangleLint({ content: "<h1>A</h1><h3>B</h3>" });
    expect(lintRes.issues[0].column).toBeNull();
    expect(TriangleLintReportSchema.parse(JSON.parse(rustLintJson)).issues[0].snippet).toBeUndefined();

    // SSRF & quota parity
    expect(
      UpsertCronScheduleRequestSchema.safeParse({
        projectId: "p1",
        cronExpression: "     ",
        upstreamRepoUrl: "https://github.com/cowok-ai/skills",
      }).success
    ).toBe(false);
    for (const badUrl of [
      "https://10.0.0.1/repo",
      "https://192.168.1.5/repo",
      "https://127.0.0.2/repo",
      "https://[::ffff:127.0.0.1]/repo",
    ]) {
      expect(
        UpsertCronScheduleRequestSchema.safeParse({
          projectId: "p1",
          cronExpression: "* * * * *",
          upstreamRepoUrl: badUrl,
        }).success
      ).toBe(false);
    }
    expect(
      DiscoverOrInstallPluginRequestSchema.safeParse({
        projectId: "p1",
        pluginSlug: "local-x",
        sourceType: "local",
        sourceRepoUrl: "javascript:alert(1)",
        manifest: { name: "local-x", version: "1.0.0", description: "desc" },
      }).success
    ).toBe(false);
    expect(
      RepackageBundleRequestSchema.safeParse({
        projectId: "p1",
        sourceDir: "skills/s1",
        skillName: "valid-name",
        description: "x".repeat(321),
      }).success
    ).toBe(false);

    // Sanitizer preserves `<b` math while blocking multiline & nested-paren beacons
    expect(sanitizeLearnMarkdownPreview("Ensure a <b holds")).toBe("Ensure a <b holds");
    const nestedClean = sanitizeLearnMarkdownPreview("![px]\n(https://evil.test/a(1))\n<script\nOk");
    expect(nestedClean).not.toContain("https://evil.test");
    expect(nestedClean).not.toContain("<script");
  });

  it("ut_084_round2_ui_lifecycle: preserves failed cron draft, shows actionFeedback, impact summary & ?tab=cron routing", async () => {
    // 1. Failed CronScheduleEditor preserves draft, success clears it, and actionFeedback renders
    const onSaveFail = vi.fn(async () => false);
    const { rerender } = render(
      <CronSyncStatus
        plugins={[SAMPLE_PLUGIN]}
        schedulesByPlugin={{ [SAMPLE_PLUGIN.id]: null }}
        loading={false}
        error={null}
        pendingActionId={null}
        actionFeedback="Saved cron schedule for design-tokens-skill"
        onRefresh={vi.fn()}
        onSyncNow={vi.fn()}
        onSaveSchedule={onSaveFail}
      />
    );
    expect(screen.getByTestId("cron-action-feedback").textContent).toContain("Saved cron schedule");
    const exprInput = screen.getByTestId(`input-cron-expr-${SAMPLE_PLUGIN.pluginSlug}`) as HTMLInputElement;
    fireEvent.change(exprInput, { target: { value: "0 */10 * * * *" } });
    fireEvent.submit(screen.getByTestId(`cron-schedule-form-${SAMPLE_PLUGIN.pluginSlug}`));
    await vi.waitFor(() => expect(onSaveFail).toHaveBeenCalledTimes(1));
    expect(exprInput.value).toBe("0 */10 * * * *");

    // 2. StagingDiffCard impact summary + LearnCardReviewQueue actionFeedback
    rerender(
      <>
        <PluginRegistryPanel
          plugins={[SAMPLE_PLUGIN]}
          loading={false}
          error={null}
          pendingActionId={null}
          actionFeedback={null}
          onRefresh={vi.fn()}
          onTransition={vi.fn()}
          onPromoteStaging={vi.fn()}
          onDiscardStaging={vi.fn()}
        />
        <LearnCardReviewQueue
          cards={[SAMPLE_LEARN_CARD]}
          loading={false}
          error={null}
          pendingActionId={null}
          actionFeedback="Accepted LEARN card: Zero Tailwind Policy"
          onRefresh={vi.fn()}
          onApprove={vi.fn()}
          onReject={vi.fn()}
        />
      </>
    );
    expect(screen.getByTestId(`staging-impact-summary-${SAMPLE_PLUGIN.pluginSlug}`).textContent).toContain("v1.2.0 -> v2.0.0");
    expect(screen.getByTestId("learn-action-feedback").textContent).toContain("Accepted LEARN card");

    // 3. /plugins?projectId=...&tab=cron honors URL query context without wiping on mount
    const prevUrl = window.location.href;
    window.history.replaceState({}, "", "/plugins?projectId=proj-custom-084&tab=cron");
    try {
      render(<PluginCenterPage />);
      const mainEl = document.querySelector("main[data-project-id]") as HTMLElement | null;
      expect(mainEl?.getAttribute("data-project-id")).toBe("proj-custom-084");
      expect(screen.getByTestId("cron-sync-status")).toBeTruthy();
    } finally {
      window.history.replaceState({}, "", prevUrl);
    }
  });

  it("ut_084_round4_cross_stack_parity: enforces quarantine E2E, DTO schema parity, IPv6/slug SSRF guards, and builtin Cron filtering", async () => {
    // 1. Quarantine BFF route requires human workbench session + valid >=4 char reason
    const agentSession = issueBffLoopbackSession({
      tenantId: "local-default",
      userId: "agent-1",
      role: "editor",
      isHumanWorkbench: false,
    });
    const nonHumanRes = await quarantineRoute.POST(
      new Request("http://127.0.0.1/api/plugins/plug-1/quarantine", {
        method: "POST",
        headers: {
          host: "127.0.0.1",
          "x-cowok-session-token": agentSession.sessionToken,
          "content-type": "application/json",
        },
        body: JSON.stringify({ projectId: "p1", expectedRevision: 1, reason: "Malicious script detected" }),
      }),
      { params: { id: "plug-1" } }
    );
    expect(nonHumanRes.status).toBe(403);

    const humanSession = issueBffLoopbackSession({
      tenantId: "local-default",
      userId: "alice",
      role: "editor",
      isHumanWorkbench: true,
    });
    const badReasonRes = await quarantineRoute.POST(
      new Request("http://127.0.0.1/api/plugins/plug-1/quarantine", {
        method: "POST",
        headers: {
          host: "127.0.0.1",
          "x-cowok-session-token": humanSession.sessionToken,
          "content-type": "application/json",
        },
        body: JSON.stringify({ projectId: "p1", expectedRevision: 1, reason: "   x  " }),
      }),
      { params: { id: "plug-1" } }
    );
    expect(badReasonRes.status).toBe(400);
    expect(QuarantinePluginRequestSchema.safeParse({ projectId: "p1", expectedRevision: 1, reason: "Verified compromise" }).success).toBe(true);

    // 2. PluginSummaryDtoSchema preserves upstreamRepoUrl & accepts unscoped projectId ""
    const unscopedBuiltin = PluginSummaryDtoSchema.parse({
      id: "builtin-aabbccddeeff-ui-review",
      tenantId: "local-default",
      projectId: "",
      pluginSlug: "ui-review",
      version: "1.0.0",
      lifecycleState: "ACTIVE",
      enabled: true,
      sourceType: "builtin",
      sourceRepoUrl: "builtin://packages/builtin-skills/skills/ui-review",
      upstreamRepoUrl: null,
    });
    expect(unscopedBuiltin.upstreamRepoUrl).toBeNull();
    expect(unscopedBuiltin.projectId).toBe("");

    // 3. Schema parity: autoFix, fixed_content, projectId on discovery-stubs, finite expectedDurationSecs, pluginSlug 64B safe segment
    expect(TriangleLintRequestSchema.parse({ content: "<div/>", autoFix: true }).autoFix).toBe(true);
    expect(
      TriangleLintReportSchema.parse({
        total_issues: 0,
        new_in_this_call: 0,
        fixed_issues: 1,
        passed: true,
        duration_ms: 1,
        issues: [],
        fixed_content: "<div />",
      }).fixed_content
    ).toBe("<div />");
    expect(DiscoveryDescriptorRequestSchema.parse({ projectId: "p1", bundleDir: "skills/a" }).projectId).toBe("p1");
    expect(VideoIntakeRequestSchema.safeParse({ skillName: "s1", storyboardMarkdown: "# sb", expectedDurationSecs: Infinity }).success).toBe(false);
    expect(
      DiscoverOrInstallPluginRequestSchema.safeParse({
        projectId: "p1",
        pluginSlug: "bad:slug",
        sourceType: "local",
        sourceRepoUrl: "local://skills/a",
        manifest: { name: "bad-slug", version: "1.0.0", description: "desc" },
      }).success
    ).toBe(false);

    // 4. Expanded IPv6 SSRF + builtin:// / local:// sourceRepoUrl
    for (const blocked of [
      "https://[2001:0:4136:e378::1]/a",
      "https://[2002:c000:0204::1]/a",
      "https://[64:ff9b::192.0.2.1]/a",
      "https://[100::1]/a",
      "https://[ff02::1]/a",
    ]) {
      expect(isPublicHttpsUrl(blocked)).toBe(false);
    }
    expect(safeNonGithubSourceUrl("builtin://packages/builtin-skills/skills/ui-review")).toBe(true);
    expect(safeNonGithubSourceUrl("local://skills/custom-one")).toBe(true);
    expect(safeNonGithubSourceUrl("builtin://../escape")).toBe(false);

    // 5. Quarantined DEPRECATED plugin hides Activate & displays quarantineReason inline; builtin filtered from CronSyncStatus
    const quarantinedPlugin: PluginSummaryDto = {
      ...SAMPLE_PLUGIN,
      id: "plug-quar-1",
      pluginSlug: "compromised-skill",
      lifecycleState: "DEPRECATED",
      enabled: false,
      quarantined: true,
      quarantineReason: "Blocked script exfiltration attempt",
    };
    const onQuarantine = vi.fn();
    const { rerender } = render(
      <PluginRegistryPanel
        plugins={[SAMPLE_PLUGIN, quarantinedPlugin]}
        loading={false}
        error={null}
        pendingActionId={null}
        actionFeedback={null}
        onRefresh={vi.fn()}
        onTransition={vi.fn()}
        onQuarantine={onQuarantine}
        onPromoteStaging={vi.fn()}
        onDiscardStaging={vi.fn()}
      />
    );
    expect(screen.queryByTestId("btn-activate-compromised-skill")).toBeNull();
    expect(screen.getByTestId("plugin-quarantine-reason-compromised-skill").textContent).toContain("Blocked script exfiltration attempt");

    // Compact chip grid: clicking Cách ly opens Quarantine Reason popup modal
    const qTrigger = screen.getByTestId(`btn-quarantine-${SAMPLE_PLUGIN.pluginSlug}`) as HTMLButtonElement;
    expect(qTrigger.disabled).toBe(false);
    fireEvent.click(qTrigger);
    expect(screen.getByTestId("quarantine-reason-modal")).toBeTruthy();
    const qInput = screen.getByTestId("input-quarantine-reason-modal");
    const qConfirm = screen.getByTestId("btn-confirm-quarantine-modal") as HTMLButtonElement;
    expect(qConfirm.disabled).toBe(true);
    fireEvent.change(qInput, { target: { value: "Suspicious telemetry call" } });
    expect(qConfirm.disabled).toBe(false);
    fireEvent.click(qConfirm);
    expect(onQuarantine).toHaveBeenCalledWith(SAMPLE_PLUGIN, "Suspicious telemetry call");

    // Multi-select checkbox reveals Bulk Action Floating Card
    fireEvent.click(screen.getByTestId(`checkbox-plugin-${SAMPLE_PLUGIN.pluginSlug}`));
    expect(screen.getByTestId("plugin-bulk-floating-bar")).toBeTruthy();
    fireEvent.click(screen.getByTestId("btn-bulk-clear"));
    expect(screen.queryByTestId("plugin-bulk-floating-bar")).toBeNull();

    rerender(
      <CronSyncStatus
        plugins={[unscopedBuiltin]}
        schedulesByPlugin={{}}
        loading={false}
        error={null}
        pendingActionId={null}
        onRefresh={vi.fn()}
        onSyncNow={vi.fn()}
        onSaveSchedule={vi.fn()}
      />
    );
    expect(screen.getByTestId("cron-sync-empty")).toBeTruthy();

    // 6. ?tab=learn preserved when switching to automations and back to mcp
    const prevUrl = window.location.href;
    window.history.replaceState({}, "", "/plugins?projectId=proj-custom-084&tab=learn");
    try {
      render(<PluginCenterPage />);
      fireEvent.click(screen.getByRole("button", { name: /Automations|Tự động hóa/i }));
      expect(window.location.search).toContain("tab=cron");
      fireEvent.click(screen.getByRole("button", { name: /MCP/i }));
      expect(window.location.search).toContain("tab=learn");
    } finally {
      window.history.replaceState({}, "", prevUrl);
    }

    // 7. Shared PLUGIN_CENTER_CARDS validators surface contract errors instead of silent drops
    const invalidPlugin = { ...SAMPLE_PLUGIN, id: "" };
    const pluginCheck = validatePluginCards([SAMPLE_PLUGIN, invalidPlugin]);
    expect(pluginCheck.valid).toHaveLength(1);
    expect(pluginCheck.error).toContain("1 plugin payload(s) failed contract validation");
    const invalidCard = { ...SAMPLE_LEARN_CARD, id: "" };
    const learnCheck = validateLearnCards([SAMPLE_LEARN_CARD, invalidCard]);
    expect(learnCheck.valid).toHaveLength(1);
    expect(learnCheck.error).toContain("1 LEARN card payload(s) failed contract validation");
    const schedCheck = validateCronSchedules({
      [SAMPLE_PLUGIN.id]: SAMPLE_SCHEDULE,
      bad: { ...SAMPLE_SCHEDULE, consecutiveFailures: -1 },
    });
    expect(schedCheck.valid[SAMPLE_PLUGIN.id]).toBeTruthy();
    expect(schedCheck.error).toContain("1 cron schedule payload(s) failed contract validation");
  });
});
