// @vitest-environment jsdom
// @story 14.1
// @implements AC-01, AC-02, AC-03, AC-04 (Non-Tech Support Drawer, Privacy Switch, CLI feedback & 5-Host MCP cowok_submit_feedback)
import React from "react";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SupportRequestDrawer } from "../SupportRequestDrawer";
import { WorkbenchTopNav } from "../../../pages/workbench/components/WorkbenchTopNav";
import { SettingsGeneralModelsPane } from "../../../pages/workbench/components/SettingsGeneralModelsPane";
import { apiSupport } from "../../../api/support";
import type { LocalSupportTicket } from "../../../lib/validations/support";

function makeSampleTicket(overrides: Partial<LocalSupportTicket> = {}): LocalSupportTicket {
  return {
    id: "tkt-141-01",
    ticketCode: "CWK-A1B2C3",
    installId: "inst_0123456789abcdef",
    channel: "desktop",
    category: "feature_request",
    title: "Xuất báo cáo slide 1 chạm",
    description: "Thêm nút xuất PNG cho slide tại <HOME>/exports",
    contactEmail: "nontech@example.com",
    screenshotSha256: null,
    diagnostics: {
      appVersion: "0.1.0",
      channel: "desktop",
      os: "macos",
      locale: "vi-VN",
      recentErrorCodes: [],
      activeSkills: ["design-craft"],
      redactionCount: 1,
    },
    status: "in_progress",
    adminReplies: [
      {
        id: "rep-1",
        author: "SuperAdmin Desk",
        message: "Cảm ơn bạn! Tính năng này đã vào hàng đợi sprint tiếp theo.",
        createdAt: 1_732_350_000_000,
      },
    ],
    syncStatus: "queued",
    createdAt: 1_732_340_000_000,
    updatedAt: 1_732_350_000_000,
    ...overrides,
  };
}

describe("Story 14.1: Client Edge Support Drawer, Privacy Switch, CLI & MCP", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("AC-01: selects all 4 categories, submits real ticket with toast receipt, and renders 2-way SuperAdmin reply in My Requests", async () => {
    const persistedTicket = makeSampleTicket();
    const listSupportTickets = vi.fn().mockResolvedValue({
      ok: true,
      data: { tickets: [] },
      requestId: "req-list-1",
    });
    const createSupportTicket = vi.fn().mockResolvedValue({
      ok: true,
      data: persistedTicket,
      requestId: "req-create-1",
    });

    render(
      <SupportRequestDrawer
        isOpen
        onClose={vi.fn()}
        locale="vi-VN"
        client={{ createSupportTicket, listSupportTickets }}
      />
    );

    expect(screen.getByTestId("support-request-drawer")).toBeTruthy();

    // Verify all 4 category pills toggle aria-pressed cleanly.
    for (const cat of [
      "bug_report",
      "feature_request",
      "skill_template_request",
      "question",
    ] as const) {
      const btn = screen.getByTestId(`support-category-${cat}`);
      fireEvent.click(btn);
      expect(btn.getAttribute("aria-pressed")).toBe("true");
    }

    fireEvent.click(screen.getByTestId("support-category-feature_request"));
    fireEvent.change(screen.getByTestId("support-title-input"), {
      target: { value: "Xuất báo cáo slide 1 chạm" },
    });
    fireEvent.change(screen.getByTestId("support-description-input"), {
      target: { value: "Thêm nút xuất PNG cho slide tại /Users/alice/exports" },
    });
    fireEvent.change(screen.getByTestId("support-email-input"), {
      target: { value: "nontech@example.com" },
    });

    // Expand Zero-PII diagnostics preview.
    fireEvent.click(screen.getByTestId("support-preview-diagnostics-btn"));
    expect(screen.getByTestId("support-diagnostics-preview").textContent).toContain(
      '"sourceCodeAttached": false'
    );

    // Strictly 1 primary submit CTA.
    fireEvent.click(screen.getByTestId("support-submit-btn"));

    await waitFor(() => {
      expect(createSupportTicket).toHaveBeenCalledTimes(1);
    });
    expect(createSupportTicket.mock.calls[0][0]).toMatchObject({
      category: "feature_request",
      title: "Xuất báo cáo slide 1 chạm",
      channel: "desktop",
      includeDiagnostics: true,
    });

    // Toast displays ticket code and tab switches to My Requests showing persisted ticket + reply.
    await waitFor(() => {
      expect(screen.getByTestId("support-drawer-toast").textContent).toContain("CWK-A1B2C3");
    });
    expect(screen.getByTestId("support-ticket-card-CWK-A1B2C3")).toBeTruthy();
    expect(screen.getByTestId("support-ticket-replies-CWK-A1B2C3").textContent).toContain(
      "SuperAdmin Desk"
    );
  });

  it("AC-01: WorkbenchTopNav renders Support & Feedback button and invokes onOpenSupportDrawer", () => {
    const onOpenSupportDrawer = vi.fn();
    render(
      <WorkbenchTopNav
        projectId="proj-141"
        projectName="demo-workspace"
        onOpenSupportDrawer={onOpenSupportDrawer}
      />
    );
    const btn = screen.getByTestId("nav-support-drawer-btn");
    fireEvent.click(btn);
    expect(onOpenSupportDrawer).toHaveBeenCalledTimes(1);
  });

  it("AC-04: SettingsGeneralModelsPane toggles Telemetry opt-out and displays env-locked badge when COWOK_TELEMETRY=0", async () => {
    const getSpy = vi.spyOn(apiSupport, "getTelemetryPreferences").mockResolvedValueOnce({
      ok: true,
      data: {
        telemetryEnabled: true,
        effectiveEnabled: true,
        envLocked: false,
        lockedReason: null,
        airgapped: false,
        installId: "inst_abc123",
        pendingOutboxCount: 3,
        updatedAt: 1_732_340_000_000,
      },
      requestId: "req-pref-1",
    });
    const putSpy = vi.spyOn(apiSupport, "updateTelemetryPreferences").mockResolvedValueOnce({
      ok: true,
      data: {
        telemetryEnabled: false,
        effectiveEnabled: false,
        envLocked: false,
        lockedReason: null,
        airgapped: false,
        installId: "inst_abc123",
        pendingOutboxCount: 3,
        updatedAt: 1_732_340_010_000,
      },
      requestId: "req-pref-2",
    });

    render(
      <SettingsGeneralModelsPane
        selectedTab="general"
        currentTheme="warm"
        onThemeChange={vi.fn()}
        currentLocale="vi-VN"
        onLocaleChange={vi.fn()}
        currentFontSize="13px"
        onFontSizeChange={vi.fn()}
        defaultModel="cowok-swarm"
        onDefaultModelChange={vi.fn()}
        byosEndpoint=""
        onByosEndpointChange={vi.fn()}
        ephemeralApiKeys={{ gemini: "", claude: "", openai: "", byosToken: "" }}
        onEphemeralKeyChange={vi.fn()}
        showKeys={false}
        onToggleShowKeys={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(getSpy).toHaveBeenCalledTimes(1);
    });
    const toggle = screen.getByTestId("settings-telemetry-optin-toggle");
    await waitFor(() => {
      expect(toggle.getAttribute("aria-checked")).toBe("true");
    });
    fireEvent.click(toggle);

    await waitFor(() => {
      expect(putSpy).toHaveBeenCalledWith({ telemetryEnabled: false });
    });
    expect(toggle.getAttribute("aria-checked")).toBe("false");

    cleanup();
    vi.spyOn(apiSupport, "getTelemetryPreferences").mockResolvedValueOnce({
      ok: true,
      data: {
        telemetryEnabled: true,
        effectiveEnabled: false,
        envLocked: true,
        lockedReason: "COWOK_TELEMETRY=0",
        airgapped: false,
        installId: "inst_abc123",
        pendingOutboxCount: 0,
        updatedAt: 1_732_340_020_000,
      },
      requestId: "req-pref-3",
    });

    render(
      <SettingsGeneralModelsPane
        selectedTab="general"
        currentTheme="warm"
        onThemeChange={vi.fn()}
        currentLocale="vi-VN"
        onLocaleChange={vi.fn()}
        currentFontSize="13px"
        onFontSizeChange={vi.fn()}
        defaultModel="cowok-swarm"
        onDefaultModelChange={vi.fn()}
        byosEndpoint=""
        onByosEndpointChange={vi.fn()}
        ephemeralApiKeys={{ gemini: "", claude: "", openai: "", byosToken: "" }}
        onEphemeralKeyChange={vi.fn()}
        showKeys={false}
        onToggleShowKeys={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("settings-telemetry-env-locked-badge").textContent).toContain(
        "COWOK_TELEMETRY=0"
      );
    });
    expect(
      (screen.getByTestId("settings-telemetry-optin-toggle") as HTMLButtonElement).disabled
    ).toBe(true);
  });

  it("AC-02 & AC-03: CLI feedback and MCP cowok_submit_feedback scrub dynamic secrets and attribute all 5 plugin hosts", async () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "story-14-1-cli-mcp-"));
    try {
      const cliMod = await import(
        pathToFileURL(
          path.resolve(process.cwd(), "packages/builtin-skills/design-craft/bin/design-craft.mjs")
        ).href
      );
      const mcpMod = await import(
        pathToFileURL(
          path.resolve(process.cwd(), "packages/builtin-skills/design-craft/mcp/server.mjs")
        ).href
      );

      // Runtime-assembled synthetic secrets (Gitleaks-safe).
      const fakeSk = ["sk", "-", "test_", "abc1234567890xyz"].join("");
      const fakeJwt = [
        "eyJ",
        "hbGciOiJIUzI1NiJ9",
        ".",
        "eyJ",
        "zdWIiOiIxNDEifQ",
        ".",
        "signaturePart123456",
      ].join("");

      const cliRes = await cliMod.runFeedbackCommand({
        category: "bug_report",
        title: `Crash at /Users/alice/app with ${fakeSk}`,
        description: `Auth failed with ${fakeJwt} in /home/bob/work`,
        channel: "cli",
        dir: tmpDir,
        env: { COWOK_AIRGAPPED: "1" },
      });
      expect(cliRes.ok).toBe(true);
      expect(cliRes.mode).toBe("local_outbox");
      expect(cliRes.scrubbedRedactions).toBeGreaterThanOrEqual(4);
      expect(cliRes.ticket.title).not.toContain(fakeSk);
      expect(cliRes.ticket.title).toContain("<HOME>/app");
      expect(cliRes.ticket.description).not.toContain(fakeJwt);
      expect(cliRes.ticket.description).toContain("<HOME>/work");

      // Verify CLI hardens --daemon-url (SSRF guard), propagates 4xx, scrubs E_AUTH_ghp_ & C:/Users, validates email, and writes 0o600.
      const fakeGhp = ["ghp", "_", "1234567890abcdef1234"].join("");
      await expect(
        cliMod.runFeedbackCommand({
          category: "bug_report",
          title: "SSRF test",
          description: "Reject remote non-loopback URL",
          daemonUrl: "https://evil.example.com:443",
          dir: tmpDir,
        })
      ).rejects.toThrow(/INVALID_DAEMON_URL/);

      await expect(
        cliMod.runFeedbackCommand({
          category: "bug_report",
          title: "Propagate 4xx test",
          description: "Daemon returns 422 validation error",
          daemonUrl: "http://127.0.0.1:4171",
          dir: tmpDir,
          fetchImpl: async () =>
            new Response(
              JSON.stringify({ error: { code: "INVALID_REQUEST", message: "Bad field" } }),
              { status: 422, headers: { "Content-Type": "application/json" } }
            ),
        })
      ).rejects.toThrow(/Daemon rejected feedback \(422\)/);

      await expect(
        cliMod.runFeedbackCommand({
          category: "bug_report",
          title: "Bad email test",
          description: "Reject double-dot domain email",
          email: "user@bad..domain.com",
          dir: tmpDir,
          env: { COWOK_AIRGAPPED: "1" },
        })
      ).rejects.toThrow(/INVALID_EMAIL/);

      const edgeCliRes = await cliMod.runFeedbackCommand({
        category: "bug_report",
        title: "Windows slash path C:/Users/alice/app",
        description: `Code E_AUTH_${fakeGhp} and /users/bob/doc`,
        recentErrorCodes: [`E_AUTH_${fakeGhp}`],
        activeSkills: [`skill_C:/Users/alice/s`],
        channel: "cli",
        dir: tmpDir,
        env: { COWOK_AIRGAPPED: "1" },
      });
      expect(edgeCliRes.ticket.title).toContain("<HOME>/app");
      expect(edgeCliRes.ticket.description).not.toContain(fakeGhp);
      expect(edgeCliRes.ticket.description).toContain("<HOME>/doc");
      expect(edgeCliRes.ticket.diagnostics.recentErrorCodes[0]).not.toContain(fakeGhp);
      expect(edgeCliRes.ticket.diagnostics.activeSkills[0]).toContain("<HOME>/s");

      const outboxFile = path.join(
        tmpDir,
        ".cowok-design",
        "support-outbox",
        `${edgeCliRes.ticket.ticketCode}.json`
      );
      expect(fs.existsSync(outboxFile)).toBe(true);
      if (process.platform !== "win32") {
        expect(fs.statSync(outboxFile).mode & 0o777).toBe(0o600);
      }

      const pluginHosts = ["claude_code", "codex", "antigravity", "cursor", "cowok_ai"] as const;
      for (const host of pluginHosts) {
        const rpcResp = mcpMod.handleJsonRpcRequest(
          {
            jsonrpc: "2.0",
            id: host,
            method: "tools/call",
            params: {
              name: "cowok_submit_feedback",
              arguments: {
                category: "skill_template_request",
                title: `Need template for ${host}`,
                description: `Reported from /Users/alice/${host} token=${fakeSk}`,
                channel: host,
              },
            },
          },
          { rootDir: tmpDir, env: {} }
        );
        expect(rpcResp.result.isError).toBe(false);
        const payload = JSON.parse(rpcResp.result.content[0].text);
        expect(payload.ok).toBe(true);
        expect(payload.channel).toBe(host);
        expect(payload.description).not.toContain(fakeSk);
        expect(payload.description).toContain(`<HOME>/${host}`);
      }

      // MCP rejects invalid email cleanly with INVALID_PARAMS.
      const badEmailRpc = mcpMod.handleJsonRpcRequest(
        {
          jsonrpc: "2.0",
          id: "bad-email",
          method: "tools/call",
          params: {
            name: "cowok_submit_feedback",
            arguments: {
              category: "question",
              title: "Invalid email check",
              description: "Should fail email validation",
              email: "not..valid@bad..host",
            },
          },
        },
        { rootDir: tmpDir, env: {} }
      );
      expect(badEmailRpc.result.isError).toBe(true);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it("Remediation: SettingsGeneralModelsPane stays disabled/unchecked when daemon is offline, and SupportRequestDrawer maps ZodError & rejects non-image uploads", async () => {
    vi.spyOn(apiSupport, "getTelemetryPreferences").mockRejectedValueOnce(new Error("Daemon offline"));
    render(
      <SettingsGeneralModelsPane
        selectedTab="general"
        currentTheme="warm"
        onThemeChange={vi.fn()}
        currentLocale="vi-VN"
        onLocaleChange={vi.fn()}
        currentFontSize="13px"
        onFontSizeChange={vi.fn()}
        defaultModel="cowok-swarm"
        onDefaultModelChange={vi.fn()}
        byosEndpoint=""
        onByosEndpointChange={vi.fn()}
        ephemeralApiKeys={{ gemini: "", claude: "", openai: "", byosToken: "" }}
        onEphemeralKeyChange={vi.fn()}
        showKeys={false}
        onToggleShowKeys={vi.fn()}
      />
    );
    await waitFor(() => {
      expect(screen.getByTestId("settings-telemetry-offline")).toBeTruthy();
    });
    const offlineToggle = screen.getByTestId("settings-telemetry-optin-toggle") as HTMLButtonElement;
    expect(offlineToggle.disabled).toBe(true);
    expect(offlineToggle.getAttribute("aria-checked")).toBe("false");

    cleanup();
    const listSupportTickets = vi.fn().mockResolvedValue({
      ok: true,
      data: { tickets: [] },
      requestId: "req-list-2",
    });
    const { ZodError } = await import("zod");
    const createSupportTicket = vi.fn().mockRejectedValueOnce(
      new ZodError([
        {
          code: "invalid_string",
          validation: "email",
          path: ["contactEmail"],
          message: "Invalid email",
        },
      ])
    );

    render(
      <SupportRequestDrawer
        isOpen
        onClose={vi.fn()}
        locale="vi-VN"
        client={{ createSupportTicket, listSupportTickets }}
      />
    );

    // Reject SVG file type and show friendly inline error.
    const fileInput = screen.getByTestId("support-screenshot-input") as HTMLInputElement;
    const svgFile = new File(["<svg></svg>"], "bad.svg", { type: "image/svg+xml" });
    fireEvent.change(fileInput, { target: { files: [svgFile] } });
    expect(screen.getByTestId("support-form-error").textContent).toContain("PNG, JPEG");

    // ZodError maps to friendly localized message (never raw JSON).
    fireEvent.change(screen.getByTestId("support-title-input"), { target: { value: "Valid title" } });
    fireEvent.change(screen.getByTestId("support-description-input"), {
      target: { value: "Valid description text." },
    });
    fireEvent.click(screen.getByTestId("support-submit-btn"));
    await waitFor(() => {
      const toastText = screen.getByTestId("support-drawer-toast").textContent ?? "";
      expect(toastText).toContain("email");
      expect(toastText).not.toContain('"code":');
    });
  });

  it("UX Polish: supports PNG image & MP4 video upload up to 5 MB, shows preview chip + remove button, and rejects files > 5 MB", async () => {
    const listSupportTickets = vi.fn().mockResolvedValue({
      ok: true,
      data: { tickets: [] },
      requestId: "req-list-media",
    });
    const createSupportTicket = vi.fn().mockResolvedValue({
      ok: true,
      data: makeSampleTicket({ ticketCode: "CWK-VID01" }),
      requestId: "req-create-media",
    });

    render(
      <SupportRequestDrawer
        isOpen
        onClose={vi.fn()}
        locale="vi-VN"
        client={{ createSupportTicket, listSupportTickets }}
      />
    );

    const uploadBtn = screen.getByTestId("support-media-upload-btn");
    expect(uploadBtn).toBeTruthy();
    const fileInput = screen.getByTestId("support-screenshot-input") as HTMLInputElement;
    expect(fileInput.accept).toContain("video/mp4");
    expect(fileInput.accept).toContain("video/webm");

    // 1) Upload PNG image -> renders image thumbnail preview chip & can be removed.
    const pngFile = new File([new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10])], "capture.png", {
      type: "image/png",
    });
    fireEvent.change(fileInput, { target: { files: [pngFile] } });
    await waitFor(() => {
      expect(screen.getByTestId("support-media-preview-img")).toBeTruthy();
      expect(screen.getByTestId("support-media-preview-chip").textContent).toContain("capture.png");
    });

    fireEvent.click(screen.getByTestId("support-media-remove-btn"));
    expect(screen.queryByTestId("support-media-preview-chip")).toBeNull();

    // 2) Reject > 5 MB file cleanly.
    const oversizedVideo = new File([new Uint8Array([0, 0, 0, 24])], "huge.mp4", {
      type: "video/mp4",
    });
    Object.defineProperty(oversizedVideo, "size", { value: 5_000_001 });
    fireEvent.change(fileInput, { target: { files: [oversizedVideo] } });
    expect(screen.getByTestId("support-form-error").textContent).toContain("5 MB");
    expect(screen.queryByTestId("support-media-preview-chip")).toBeNull();

    // 3) Upload valid short MP4 video (<= 5 MB) -> renders video badge & submits data:video/mp4;base64,...
    const mp4File = new File([new Uint8Array([0, 0, 0, 24, 102, 116, 121, 112])], "repro.mp4", {
      type: "video/mp4",
    });
    fireEvent.change(fileInput, { target: { files: [mp4File] } });
    await waitFor(() => {
      expect(screen.getByTestId("support-media-preview-video")).toBeTruthy();
      expect(screen.getByTestId("support-media-preview-chip").textContent).toContain("repro.mp4");
    });

    fireEvent.change(screen.getByTestId("support-title-input"), { target: { value: "Lỗi animation video" } });
    fireEvent.change(screen.getByTestId("support-description-input"), {
      target: { value: "Đính kèm clip ngắn minh họa lỗi kéo thả canvas." },
    });
    fireEvent.click(screen.getByTestId("support-submit-btn"));

    await waitFor(() => {
      expect(createSupportTicket).toHaveBeenCalledTimes(1);
    });
    expect(createSupportTicket.mock.calls[0][0].screenshotDataUrl).toMatch(/^data:video\/mp4;base64,/);
  });

  it("UX Polish (Image 5): syncs My Requests heading and opens click-to-view Media Lightbox for image & video attachments", async () => {
    const imgTicket = makeSampleTicket({
      id: "tkt-img-01",
      ticketCode: "CWK-IMG01",
      title: "Ảnh chụp lỗi thanh công cụ",
      screenshotSha256: "a".repeat(64),
      attachmentMime: "image/png",
    });
    const vidTicket = makeSampleTicket({
      id: "tkt-vid-02",
      ticketCode: "CWK-VID02",
      title: "Video minh họa kéo thả",
      screenshotSha256: "b".repeat(64),
      attachmentMime: "video/mp4",
    });
    const plainTicket = makeSampleTicket({
      id: "tkt-plain-03",
      ticketCode: "CWK-PLN03",
      title: "Yêu cầu không có đính kèm",
      screenshotSha256: null,
      attachmentMime: null,
    });

    const fetchAttachmentSpy = vi
      .spyOn(apiSupport, "fetchSupportTicketAttachment")
      .mockImplementation(async (ticketId: string) => {
        if (ticketId === "tkt-vid-02") {
          return new Blob([new Uint8Array([0, 0, 0, 24])], { type: "video/mp4" });
        }
        return new Blob([new Uint8Array([137, 80, 78, 71])], { type: "image/png" });
      });
    const createObjectUrlSpy = vi.fn().mockImplementation((blob: Blob) =>
      blob.type.startsWith("video/") ? "blob:mock-video-url" : "blob:mock-image-url"
    );
    const revokeObjectUrlSpy = vi.fn();
    const origCreateObjectUrl = URL.createObjectURL;
    const origRevokeObjectUrl = URL.revokeObjectURL;
    URL.createObjectURL = createObjectUrlSpy;
    URL.revokeObjectURL = revokeObjectUrlSpy;

    const onCloseDrawer = vi.fn();
    try {
      render(
        <SupportRequestDrawer
          isOpen
          onClose={onCloseDrawer}
          locale="vi-VN"
          client={{
            createSupportTicket: vi.fn(),
            listSupportTickets: vi.fn().mockResolvedValue({
              ok: true,
              data: { tickets: [imgTicket, vidTicket, plainTicket] },
              requestId: "req-list-history",
            }),
          }}
        />
      );

      // Switch to My Requests tab & verify heading matches tab label ("Yêu cầu của tôi").
      fireEvent.click(screen.getByTestId("support-tab-my-requests"));
      await waitFor(() => {
        expect(screen.getByTestId("support-ticket-card-CWK-IMG01")).toBeTruthy();
      });
      expect(screen.getByTestId("support-my-requests-heading").textContent).toBe("Yêu cầu của tôi");
      expect(screen.getByTestId("support-tab-my-requests").textContent).toContain("Yêu cầu của tôi");
      expect(screen.queryByTestId("support-ticket-attachment-CWK-PLN03")).toBeNull();

      // 1) Open image attachment Lightbox & close via close button.
      const imgChip = screen.getByTestId("support-ticket-attachment-CWK-IMG01");
      expect(imgChip.textContent).toContain("Xem ảnh đính kèm");
      fireEvent.click(imgChip);

      const lightbox = await screen.findByTestId("support-media-lightbox");
      expect(lightbox.getAttribute("role")).toBe("dialog");
      await waitFor(() => {
        const imgEl = lightbox.querySelector("img");
        expect(imgEl).toBeTruthy();
        expect(imgEl?.getAttribute("src")).toBe("blob:mock-image-url");
        expect(imgEl?.getAttribute("alt")).toBe("Ảnh chụp lỗi thanh công cụ");
      });
      const downloadImg = screen.getByTestId("support-media-lightbox-download") as HTMLAnchorElement;
      expect(downloadImg.getAttribute("download")).toBe("CWK-IMG01.png");
      expect(fetchAttachmentSpy).toHaveBeenCalledWith("tkt-img-01", expect.any(AbortSignal));

      fireEvent.click(screen.getByTestId("support-media-lightbox-close"));
      expect(screen.queryByTestId("support-media-lightbox")).toBeNull();
      expect(revokeObjectUrlSpy).toHaveBeenCalledWith("blob:mock-image-url");

      // 2) Open video attachment Lightbox & close via Escape (drawer remains open!).
      const vidChip = screen.getByTestId("support-ticket-attachment-CWK-VID02");
      expect(vidChip.textContent).toContain("Xem video đính kèm");
      fireEvent.click(vidChip);

      const vidLightbox = await screen.findByTestId("support-media-lightbox");
      await waitFor(() => {
        const videoEl = vidLightbox.querySelector("video");
        expect(videoEl).toBeTruthy();
        expect(videoEl?.getAttribute("src")).toBe("blob:mock-video-url");
      });
      const downloadVid = screen.getByTestId("support-media-lightbox-download") as HTMLAnchorElement;
      expect(downloadVid.getAttribute("download")).toBe("CWK-VID02.mp4");

      fireEvent.keyDown(window, { key: "Escape" });
      expect(screen.queryByTestId("support-media-lightbox")).toBeNull();
      expect(onCloseDrawer).not.toHaveBeenCalled();
    } finally {
      URL.createObjectURL = origCreateObjectUrl;
      URL.revokeObjectURL = origRevokeObjectUrl;
    }
  });
});
