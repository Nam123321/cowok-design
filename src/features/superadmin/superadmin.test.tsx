// @vitest-environment jsdom
// @story 14.2
// @implements AC-02, AC-03, AC-04 (SuperAdmin Portal UI Tests — OIDC+MFA Gate, 4-Role RBAC Lock, 2-Way Helpdesk, Feature Clusters, Activation Funnel & Crash Center)
import React from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CloudAdminApiClient, CloudAdminApiError } from "../../api/cloud-admin";
import { SuperAdminPortalPage } from "./SuperAdminPortalPage";
import type { SuperAdminApiPort } from "./useSuperAdminController";
import type {
  AnalyticsFunnelData,
  CloudAdminRole,
  CloudFeatureCluster,
  CloudSupportTicket,
  CrashGroup,
} from "../../lib/validations/cloud-admin";

function makeTicket(overrides: Partial<CloudSupportTicket> = {}): CloudSupportTicket {
  return {
    id: "ctkt_101",
    installId: "inst_non_tech_01",
    clientTicketId: "local_tkt_01",
    category: "feature_request",
    priority: "P2",
    status: "open",
    title: "One-click Figma export button",
    descriptionSanitized: "Please add export from <HOME>/designs without CLI",
    diagnosticBundle: { os: "macos", appVersion: "1.3.0" },
    attachmentSha256: "abcdef1234567890",
    attachmentMime: "image/png",
    attachmentBytes: 4096,
    clusterId: null,
    channel: "desktop",
    hostClient: "standalone",
    appVersion: "1.3.0",
    idempotencyKey: "idem-101",
    createdAt: 1_732_350_000_000,
    updatedAt: 1_732_360_000_000,
    replies: [],
    ...overrides,
  };
}

function makeFunnelData(): AnalyticsFunnelData {
  return {
    dau: 124,
    wau: 540,
    mau: 1890,
    openTickets: 7,
    p0Tickets: 1,
    activeCrashGroups: 2,
    channelBreakdown: [
      { key: "cli", users: 410, events: 2200 },
      { key: "desktop", users: 680, events: 4900 },
      { key: "docker", users: 190, events: 980 },
      { key: "plugin", users: 610, events: 3500 },
    ],
    hostBreakdown: [
      { key: "claude_code", users: 150, events: 820 },
      { key: "codex", users: 110, events: 540 },
      { key: "antigravity", users: 180, events: 1120 },
      { key: "cursor", users: 120, events: 760 },
      { key: "cowok_ai", users: 50, events: 260 },
    ],
    funnelSteps: [
      { stage: "day_0", label: "Day-0", users: 1000, conversionRatePct: 100, dropOffCount: 0, dropOffRatePct: 0 },
      { stage: "ai_connected", label: "AI Connected", users: 720, conversionRatePct: 72, dropOffCount: 280, dropOffRatePct: 28 },
      { stage: "project", label: "Project", users: 540, conversionRatePct: 54, dropOffCount: 180, dropOffRatePct: 25 },
      { stage: "preview", label: "Preview", users: 410, conversionRatePct: 41, dropOffCount: 130, dropOffRatePct: 24.1 },
      { stage: "export", label: "Export", users: 290, conversionRatePct: 29, dropOffCount: 120, dropOffRatePct: 29.3 },
    ],
    featureAdoption: [
      { eventName: "preview_rendered", totalEvents: 3200, uniqueInstalls: 410 },
      { eventName: "export_completed", totalEvents: 1150, uniqueInstalls: 290 },
    ],
  };
}

function makeCrash(): CrashGroup {
  return {
    fingerprint: "fp_canvas_oom_01",
    appVersion: "1.3.0",
    errorSignature: "CanvasRenderer::OutOfMemory",
    sampleStackSanitized: "at <HOME>/canvas.rs:42 [REDACTED_SECRET]",
    occurrenceCount: 19,
    affectedInstalls: 6,
    status: "open",
    firstSeenAt: 1_732_350_000_000,
    lastSeenAt: 1_732_360_000_000,
  };
}

function makeCluster(): CloudFeatureCluster {
  return {
    id: "cclu_01",
    title: "Direct Figma Frame Export",
    summary: "Requested by non-tech design leads",
    ticketCount: 3,
    ticketIds: ["ctkt_101"],
    githubIssueUrl: "https://github.com/cowok-design/cowok-design/issues/new?title=Figma",
    status: "planned",
    createdAt: 1_732_350_000_000,
    updatedAt: 1_732_360_000_000,
  };
}

function createMockClient(): SuperAdminApiPort {
  const baseTicket = makeTicket();
  return {
    createAdminSession: vi.fn(async (payload) => ({
      ok: true as const,
      requestId: "req-auth-1",
      data: {
        sessionToken: `cwk_adm_v1.mock.${payload.role}`,
        expiresAt: Math.floor(Date.now() / 1000) + 3600,
        operator: {
          subject: payload.subject,
          email: payload.email,
          role: payload.role as CloudAdminRole,
          mfaVerified: payload.mfaVerified,
          oidcProvider: payload.oidcProvider,
        },
      },
    })),
    listTickets: vi.fn(async () => ({
      ok: true as const,
      requestId: "req-tkt-1",
      data: { tickets: [baseTicket], totalCount: 1 },
    })),
    replyToTicket: vi.fn(async (ticketId, payload) => {
      const reply = {
        id: "crep_99",
        ticketId,
        authorActor: "ops-admin@cowok.design",
        authorRole: "SUPER_ADMIN",
        bodySanitized: payload.body,
        syncedToClient: true,
        createdAt: 1_732_370_000_000,
      };
      return {
        ok: true as const,
        requestId: "req-rep-1",
        data: {
          ticket: {
            ...baseTicket,
            priority: payload.priority ?? baseTicket.priority,
            status: payload.status ?? "replied",
            replies: [reply],
          },
          reply,
          auditLogId: "aud_99",
        },
      };
    }),
    listClusters: vi.fn(async () => ({
      ok: true as const,
      requestId: "req-clu-1",
      data: { clusters: [makeCluster()] },
    })),
    createCluster: vi.fn(async (payload) => ({
      ok: true as const,
      requestId: "req-clu-2",
      data: {
        cluster: {
          id: "cclu_02",
          title: payload.title,
          summary: payload.summary ?? "",
          ticketCount: payload.ticketIds?.length ?? 0,
          ticketIds: payload.ticketIds ?? [],
          githubIssueUrl: "https://github.com/cowok-design/cowok-design/issues/new?title=Cluster",
          status: payload.status ?? "planned",
          createdAt: 1_732_370_000_000,
          updatedAt: 1_732_370_000_000,
        },
        auditLogId: "aud_100",
      },
    })),
    getAnalyticsFunnel: vi.fn(async () => ({
      ok: true as const,
      requestId: "req-fun-1",
      data: makeFunnelData(),
    })),
    listCrashes: vi.fn(async () => ({
      ok: true as const,
      requestId: "req-cr-1",
      data: { crashes: [makeCrash()] },
    })),
    fetchTicketAttachment: vi.fn(async () => new Blob(["pngdata"], { type: "image/png" })),
  };
}

function enterMfaAndSignIn(mfaCode = "314159") {
  fireEvent.change(screen.getByTestId("oidc-mfa-input"), { target: { value: mfaCode } });
  fireEvent.click(screen.getByTestId("oidc-signin-btn"));
}

describe("Story 14.2 SuperAdminPortalPage", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it("authenticates via OIDC+MFA (no hardcoded default MFA), renders attachment preview, and sends 2-way helpdesk reply", async () => {
    const createUrlSpy = vi.fn(() => "blob:mock-sanitized-attachment");
    const revokeUrlSpy = vi.fn();
    Object.defineProperty(URL, "createObjectURL", { value: createUrlSpy, configurable: true });
    Object.defineProperty(URL, "revokeObjectURL", { value: revokeUrlSpy, configurable: true });

    const client = createMockClient();
    render(<SuperAdminPortalPage client={client} />);

    expect(screen.getByTestId("superadmin-portal-root")).toBeTruthy();
    expect((screen.getByTestId("oidc-mfa-input") as HTMLInputElement).value).toBe("");

    enterMfaAndSignIn("314159");

    await waitFor(() => {
      expect(screen.getByTestId("superadmin-operator-badge")).toBeTruthy();
      expect(screen.getByTestId("superadmin-kpi-metric-line")).toBeTruthy();
      expect(screen.getByTestId("ticket-row-ctkt_101")).toBeTruthy();
    });

    expect(screen.getByTestId("kpi-card-dau").textContent).toContain("124");
    fireEvent.click(screen.getByTestId("ticket-row-ctkt_101"));

    await waitFor(() => {
      expect(screen.getByTestId("superadmin-ticket-drawer")).toBeTruthy();
      expect(screen.getByTestId("drawer-attachment-preview")).toBeTruthy();
    });
    expect(client.fetchTicketAttachment).toHaveBeenCalledWith("ctkt_101", "cwk_adm_v1.mock.SUPER_ADMIN");

    const funnelCallsBeforeReply = (client.getAnalyticsFunnel as ReturnType<typeof vi.fn>).mock.calls.length;
    fireEvent.change(screen.getByTestId("drawer-priority-select"), { target: { value: "P0" } });
    fireEvent.change(screen.getByTestId("drawer-reply-input"), {
      target: { value: "We prioritized Figma export for Sprint 15!" },
    });
    fireEvent.click(screen.getByTestId("drawer-reply-submit"));

    await waitFor(() => {
      expect(client.replyToTicket).toHaveBeenCalledWith(
        "ctkt_101",
        { body: "We prioritized Figma export for Sprint 15!", priority: "P0", status: "replied" },
        "cwk_adm_v1.mock.SUPER_ADMIN"
      );
      expect(screen.getByTestId("ticket-reply-crep_99")).toBeTruthy();
      expect((client.getAnalyticsFunnel as ReturnType<typeof vi.fn>).mock.calls.length).toBeGreaterThan(
        funnelCallsBeforeReply
      );
    });
  });

  it("locks reply composer and hides role-switcher for ANALYST_READONLY RBAC role (no self-promotion)", async () => {
    const client = createMockClient();
    render(<SuperAdminPortalPage client={client} />);

    fireEvent.change(screen.getByTestId("oidc-role-select"), {
      target: { value: "ANALYST_READONLY" },
    });
    enterMfaAndSignIn("314159");

    await waitFor(() => {
      expect(screen.getByTestId("ticket-row-ctkt_101")).toBeTruthy();
    });

    expect(screen.queryByTestId("superadmin-role-switcher")).toBeNull();
    expect(screen.getByTestId("superadmin-signout-btn")).toBeTruthy();

    fireEvent.click(screen.getByTestId("ticket-row-ctkt_101"));
    await waitFor(() => {
      expect(screen.getByTestId("drawer-rbac-readonly-lock")).toBeTruthy();
    });
    expect((screen.getByTestId("drawer-reply-input") as HTMLTextAreaElement).disabled).toBe(true);
    expect((screen.getByTestId("drawer-reply-submit") as HTMLButtonElement).disabled).toBe(true);

    // Sign out returns to OIDC+MFA gate with cleared MFA input
    fireEvent.click(screen.getByTestId("superadmin-signout-btn"));
    await waitFor(() => {
      expect(screen.getByTestId("superadmin-oidc-gate")).toBeTruthy();
      expect((screen.getByTestId("oidc-mfa-input") as HTMLInputElement).value).toBe("");
    });
  });

  it("navigates Feature Clusters, 5-Step Activation Funnel, and Crash Center tabs", async () => {
    const client = createMockClient();
    render(<SuperAdminPortalPage client={client} />);

    enterMfaAndSignIn("314159");
    await waitFor(() => expect(screen.getByTestId("superadmin-tab-clusters")).toBeTruthy());

    // Feature Clusters tab
    fireEvent.click(screen.getByTestId("superadmin-tab-clusters"));
    expect(screen.getByTestId("cluster-card-cclu_01")).toBeTruthy();
    fireEvent.change(screen.getByTestId("cluster-title-input"), {
      target: { value: "One-Click Brand Theme Pack" },
    });
    fireEvent.click(screen.getByTestId("cluster-ticket-check-ctkt_101"));
    fireEvent.click(screen.getByTestId("cluster-submit-btn"));
    await waitFor(() => expect(screen.getByTestId("cluster-card-cclu_02")).toBeTruthy());

    // Activation Funnel tab (5 steps + 4 channels + 5 hosts)
    fireEvent.click(screen.getByTestId("superadmin-tab-funnel"));
    expect(screen.getByTestId("superadmin-funnel-chart")).toBeTruthy();
    expect(screen.getByTestId("funnel-step-day_0")).toBeTruthy();
    expect(screen.getByTestId("funnel-step-ai_connected")).toBeTruthy();
    expect(screen.getByTestId("funnel-step-project")).toBeTruthy();
    expect(screen.getByTestId("funnel-step-preview")).toBeTruthy();
    expect(screen.getByTestId("funnel-step-export")).toBeTruthy();
    expect(screen.getByTestId("superadmin-channel-breakdown")).toBeTruthy();
    expect(screen.getByTestId("superadmin-host-breakdown")).toBeTruthy();

    // Crash Center tab
    fireEvent.click(screen.getByTestId("superadmin-tab-crashes"));
    expect(screen.getByTestId("superadmin-crash-table")).toBeTruthy();
    fireEvent.click(screen.getByTestId("crash-row-fp_canvas_oom_01"));
    expect(screen.getByTestId("crash-stack-fp_canvas_oom_01").textContent).toContain("canvas.rs:42");
  });

  it("debounces ticket search, decouples overview refetch, and discards out-of-order stale ticket responses", async () => {
    type ListTicketsRes = Awaited<ReturnType<SuperAdminApiPort["listTickets"]>>;
    let resolveFirst: ((v: ListTicketsRes) => void) | null = null;
    let resolveSecond: ((v: ListTicketsRes) => void) | null = null;
    let callIdx = 0;

    const client = createMockClient();
    client.listTickets = vi.fn((): Promise<ListTicketsRes> => {
      callIdx += 1;
      if (callIdx === 1) {
        return Promise.resolve({ ok: true as const, requestId: "r0", data: { tickets: [makeTicket()], totalCount: 1 } });
      }
      if (callIdx === 2) {
        return new Promise<ListTicketsRes>((resolve) => {
          resolveFirst = resolve;
        });
      }
      return new Promise<ListTicketsRes>((resolve) => {
        resolveSecond = resolve;
      });
    });

    render(<SuperAdminPortalPage client={client} />);
    enterMfaAndSignIn("314159");

    await waitFor(() => expect(screen.getByTestId("ticket-row-ctkt_101")).toBeTruthy());
    expect(client.listClusters).toHaveBeenCalledTimes(1);
    expect(client.getAnalyticsFunnel).toHaveBeenCalledTimes(1);
    expect(client.listCrashes).toHaveBeenCalledTimes(1);

    // Search input should debounce (not fire immediately on each keystroke)
    vi.useFakeTimers();
    fireEvent.change(screen.getByTestId("helpdesk-search-input"), { target: { value: "a" } });
    fireEvent.change(screen.getByTestId("helpdesk-search-input"), { target: { value: "alpha" } });
    expect(client.listTickets).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(300);
    });
    expect(client.listTickets).toHaveBeenCalledTimes(2);
    // Overview endpoints MUST NOT be re-invoked on ticket filter changes
    expect(client.listClusters).toHaveBeenCalledTimes(1);
    expect(client.getAnalyticsFunnel).toHaveBeenCalledTimes(1);
    expect(client.listCrashes).toHaveBeenCalledTimes(1);

    // Trigger newer priority filter while call #2 is still in flight
    fireEvent.click(screen.getByTestId("priority-pill-P0"));
    expect(client.listTickets).toHaveBeenCalledTimes(3);

    // Resolve newer call #3 FIRST, then stale call #2 SECOND
    await act(async () => {
      resolveSecond?.({
        ok: true,
        requestId: "r2",
        data: { tickets: [makeTicket({ id: "ctkt_fresh", title: "Fresh P0 Ticket" })], totalCount: 1 },
      });
      await Promise.resolve();
    });
    expect(screen.getByTestId("ticket-row-ctkt_fresh")).toBeTruthy();

    await act(async () => {
      resolveFirst?.({
        ok: true,
        requestId: "r1",
        data: { tickets: [makeTicket({ id: "ctkt_stale", title: "Stale Ticket" })], totalCount: 1 },
      });
      await Promise.resolve();
    });
    expect(screen.queryByTestId("ticket-row-ctkt_stale")).toBeNull();
    expect(screen.getByTestId("ticket-row-ctkt_fresh")).toBeTruthy();
  });

  it("does not retry loopback bootstrap on CloudAdmin 401 MFA_REQUIRED/ADMIN_AUTH_REQUIRED, but retries once on UNAUTHENTICATED", async () => {
    let bootstrapCount = 0;
    let domainCalls = 0;
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/api/auth/local-session")) {
        bootstrapCount += 1;
        return new Response(JSON.stringify({ sessionToken: `loopback-tok-${bootstrapCount}` }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }
      domainCalls += 1;
      if (url.includes("/auth/session")) {
        return new Response(
          JSON.stringify({ error: { code: "MFA_REQUIRED", message: "Invalid MFA code", requestId: "req-mfa" } }),
          { status: 401, headers: { "Content-Type": "application/json" } }
        );
      }
      if (url.includes("/tickets") && domainCalls === 2) {
        return new Response(
          JSON.stringify({ error: { code: "UNAUTHENTICATED", message: "Loopback expired", requestId: "req-exp" } }),
          { status: 401, headers: { "Content-Type": "application/json" } }
        );
      }
      return new Response(
        JSON.stringify({ ok: true, requestId: "req-ok", data: { tickets: [], totalCount: 0 } }),
        { status: 200, headers: { "Content-Type": "application/json" } }
      );
    });

    const sdk = new CloudAdminApiClient({ baseUrl: "http://127.0.0.1:3000", fetchImpl: fetchMock as typeof fetch });
    const err = await sdk
      .createAdminSession({
        oidcProvider: "google",
        subject: "oidc:ops-admin@cowok.design",
        email: "ops-admin@cowok.design",
        role: "SUPER_ADMIN",
        mfaVerified: true,
        mfaCode: "000000",
      })
      .catch((caught: unknown) => caught);
    expect(err).toBeInstanceOf(CloudAdminApiError);
    expect(err).toMatchObject({ status: 401, code: "MFA_REQUIRED" });
    expect(bootstrapCount).toBe(1);
    expect(domainCalls).toBe(1);

    // Loopback 401 UNAUTHENTICATED triggers exactly 1 bootstrap refresh and succeeds
    const listRes = await sdk.listTickets({}, "cwk_adm_v1.valid.sig");
    expect(listRes.data.totalCount).toBe(0);
    expect(bootstrapCount).toBe(2);
    expect(domainCalls).toBe(3);
  });

  it("renders 3-group Left Sidebar navigation and honest architecture panels for plugin-catalog, ab-experiments & governance", async () => {
    const client = createMockClient();
    render(<SuperAdminPortalPage client={client} />);

    enterMfaAndSignIn("314159");
    await waitFor(() => expect(screen.getByTestId("superadmin-desktop-sidebar")).toBeTruthy());

    expect(screen.getByTestId("superadmin-nav-group-operations")).toBeTruthy();
    expect(screen.getByTestId("superadmin-nav-group-growth-distribution")).toBeTruthy();
    expect(screen.getByTestId("superadmin-nav-group-system-governance")).toBeTruthy();

    fireEvent.click(screen.getByTestId("superadmin-tab-plugin-catalog"));
    const pluginPanel = screen.getByTestId("superadmin-panel-plugin-catalog");
    expect(pluginPanel.textContent).toContain("ACTIVE | INACTIVE | DEPRECATED | QUARANTINED");
    expect(pluginPanel.textContent).toContain("Local Override > Cloud Default");
    expect(pluginPanel.textContent).toContain("catalog.json");

    fireEvent.click(screen.getByTestId("superadmin-tab-ab-experiments"));
    const abPanel = screen.getByTestId("superadmin-panel-ab-experiments");
    expect(abPanel.textContent).toContain("hash(install_id + key) % 100");
    expect(abPanel.textContent).toContain("experiments.json");
    expect(abPanel.textContent).toContain("0% PII");

    fireEvent.click(screen.getByTestId("superadmin-tab-governance"));
    const govPanel = screen.getByTestId("superadmin-panel-governance");
    expect(screen.getByTestId("superadmin-rbac-matrix")).toBeTruthy();
    expect(govPanel.textContent).toMatch(/Nhật ký kiểm toán bất biến|immutable audit log/i);
    expect(govPanel.textContent).not.toContain("admin_audit_logs");
  });
});
