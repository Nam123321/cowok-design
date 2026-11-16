// @vitest-environment jsdom
// @story 04.5
// @implements Clean-Room EV-001, EV-002 + Monochrome Comment Contrast & Draft Preservation
import React from "react";
import fs from "node:fs";
import path from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ElementCommentThread } from "../src/components/comments/ElementCommentThread";
import { SettingsGeneralModelsPane } from "../src/pages/workbench/components/SettingsGeneralModelsPane";
import { ModelPickerQuotaPopover } from "../src/components/workbench/ModelPickerQuotaPopover";

describe("UX Alignment: Dark Monochrome Tokens, Comment Draft Guard, Auto-Detected BYOS & 5h/Weekly Quota", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("defines dark-theme --state-*-soft and --state-* tokens under monochrome theme in tokens.css", () => {
    const css = fs.readFileSync(path.resolve(process.cwd(), "src/styles/tokens.css"), "utf-8");
    const monoStart = css.indexOf('html[data-theme="monochrome"]');
    expect(monoStart).toBeGreaterThan(-1);
    const monoBlock = css.slice(monoStart, monoStart + 2500);
    expect(monoBlock).toContain("--state-warning-soft: rgba(229, 160, 77, 0.18)");
    expect(monoBlock).toContain("--state-danger-soft: rgba(232, 116, 132, 0.18)");
    expect(monoBlock).toContain("--state-success-soft: rgba(69, 196, 134, 0.16)");
  });

  it("preserves comment draft on rejected submit and styles danger error distinct from 409 warning", async () => {
    const onAddComment = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true);
    const { rerender } = render(
      <ElementCommentThread
        nodeId="SCR-02"
        comments={[]}
        staleToastMessage="Failed to fetch"
        toastTone="danger"
        onAddComment={onAddComment}
      />
    );

    const toast = screen.getByTestId("stale-mapping-toast");
    expect(toast.style.backgroundColor).toContain("var(--state-danger-soft)");

    const input = screen.getByTestId("new-comment-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Keep this comment if network fails" } });
    fireEvent.submit(screen.getByTestId("new-comment-form"));

    await waitFor(() => expect(onAddComment).toHaveBeenCalledTimes(1));
    expect(input.value).toBe("Keep this comment if network fails");

    rerender(
      <ElementCommentThread
        nodeId="SCR-02"
        comments={[]}
        staleToastMessage={null}
        onAddComment={onAddComment}
      />
    );
    fireEvent.submit(screen.getByTestId("new-comment-form"));
    await waitFor(() => expect(input.value).toBe(""));
  });

  it("renders streamlined 3-Bento Settings (auto-connected BYOS, shorten API bento grid, shorten default Runtime) without redundant model dropdown", async () => {
    const originalFetch = globalThis.fetch;
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : input.toString();
      if (url.includes("/api/auth/local-session")) {
        return new Response(JSON.stringify({ sessionToken: "tok", tenantId: "t1", userId: "u1", role: "editor" }), { status: 200 });
      }
      if (url.includes("/api/doctor/cli-matrix")) {
        return new Response(
          JSON.stringify({
            scannedAt: new Date().toISOString(),
            scanMode: "passive",
            clis: [
              {
                id: "codex",
                name: "OpenAI Codex CLI",
                command: "codex",
                category: "cli",
                installed: true,
                authenticated: true,
                status: "ready",
                accountLabel: "user@example.com",
                latencyMs: 4,
                models: [{ id: "codex:gpt-5.3-codex", label: "GPT-5.3 Codex", providerId: "codex", recommended: true }],
              },
              {
                id: "claude",
                name: "Claude Code",
                command: "claude",
                category: "cli",
                installed: true,
                authenticated: false,
                status: "auth_needed",
                authReason: "Sign-in required",
                latencyMs: 5,
                models: [],
              },
              {
                id: "cursor",
                name: "Cursor Agent CLI",
                command: "cursor",
                category: "ide",
                installed: false,
                authenticated: false,
                status: "not_installed",
                latencyMs: 2,
                models: [],
              },
            ],
          }),
          { status: 200 }
        );
      }
      if (url.includes("/api/doctor/usage")) {
        return new Response(JSON.stringify({ providers: [], checkedAt: new Date().toISOString(), checkedAtEpochMs: Date.now() }), { status: 200 });
      }
      return new Response("[]", { status: 200 });
    }) as typeof fetch;

    const onDefaultModelChange = vi.fn();
    try {
      render(
        <SettingsGeneralModelsPane
          selectedTab="models"
          currentTheme="monochrome"
          onThemeChange={() => {}}
          currentLocale="vi-VN"
          onLocaleChange={() => {}}
          currentFontSize="13px"
          onFontSizeChange={() => {}}
          defaultModel="cowok-swarm"
          onDefaultModelChange={onDefaultModelChange}
          byosEndpoint=""
          onByosEndpointChange={() => {}}
          ephemeralApiKeys={{ gemini: "", claude: "", openai: "", byosToken: "" }}
          onEphemeralKeyChange={() => {}}
          showKeys={false}
          onToggleShowKeys={() => {}}
        />
      );

      // Redundant default model dropdown removed
      expect(screen.queryByTestId("settings-default-model-select")).toBeNull();

      // Bento 1: Ready BYOS auto-detected & auto-connected; expand shows setup flow
      await waitFor(() => expect(screen.getByTestId("provider-row-codex")).toBeTruthy());
      expect(screen.getByTestId("ready-byos-list").textContent).toContain("user@example.com");
      expect(screen.getByTestId("ready-byos-list").textContent).toContain("Đã nối");

      fireEvent.click(screen.getByTestId("toggle-more-providers-btn"));
      fireEvent.click(screen.getByTestId("provider-row-claude"));
      expect(screen.getByTestId("setup-flow-claude")).toBeTruthy();

      // Bento 2: API Providers starts shortened; click opens 6-provider Bento Grid (not just Gemini)
      expect(screen.queryByTestId("api-bento-tile-openrouter")).toBeNull();
      fireEvent.click(screen.getByTestId("toggle-api-bento-btn"));
      expect(screen.getByTestId("api-bento-tile-gemini")).toBeTruthy();
      expect(screen.getByTestId("api-bento-tile-openrouter")).toBeTruthy();
      expect(screen.getByTestId("api-bento-tile-deepseek")).toBeTruthy();

      // Bento 3: Runtime Settings starts shortened when at default; click Customize reveals controls
      expect(screen.getByTestId("runtime-bento-summary")).toBeTruthy();
      expect(screen.queryByTestId("runtime-bento-controls")).toBeNull();
      fireEvent.click(screen.getByTestId("runtime-customize-btn"));
      expect(screen.getByTestId("runtime-bento-controls")).toBeTruthy();
    } finally {
      globalThis.fetch = originalFetch;
    }
  });

  it("renders synced 5-hour and Weekly quota windows in ModelPickerQuotaPopover and refreshes on Check again", async () => {
    const originalFetch = globalThis.fetch;
    let usageCalls = 0;
    const futureMs = Date.now() + 3_600_000;
    globalThis.fetch = (async (input: RequestInfo | URL) => {
      const url = typeof input === "string" ? input : input.toString();
      if (url.includes("/api/auth/local-session")) {
        return new Response(JSON.stringify({ sessionToken: "tok", tenantId: "t1", userId: "u1", role: "editor" }), { status: 200 });
      }
      if (url.includes("/api/doctor/cli-matrix")) {
        return new Response(
          JSON.stringify({
            scannedAt: new Date().toISOString(),
            clis: [
              {
                id: "codex",
                name: "OpenAI Codex CLI",
                command: "codex",
                installed: true,
                authenticated: true,
                status: "ready",
                latencyMs: 3,
                models: [{ id: "codex:gpt-5.3-codex", label: "GPT-5.3 Codex", providerId: "codex", recommended: true }],
              },
            ],
          }),
          { status: 200 }
        );
      }
      if (url.includes("/api/doctor/usage")) {
        usageCalls += 1;
        return new Response(
          JSON.stringify({
            checkedAt: new Date().toISOString(),
            checkedAtEpochMs: Date.now(),
            providers: [
              {
                id: "codex",
                name: "OpenAI Codex CLI",
                providerGroup: "openai",
                status: "ready",
                accountLabel: "user@example.com",
                plan: "ChatGPT plus",
                ok: true,
                checkedAt: new Date().toISOString(),
                checkedAtEpochMs: Date.now(),
                fiveHour: { available: true, usedPercent: 22, remainingPercent: 78, resetsAtEpochMs: futureMs, windowMinutes: 300, resetPassed: false },
                weekly: { available: true, usedPercent: 87, remainingPercent: 13, resetsAtEpochMs: futureMs + 86_400_000, windowMinutes: 10080, resetPassed: false },
                models: [{ id: "codex:gpt-5.3-codex", label: "GPT-5.3 Codex", providerId: "codex", recommended: true }],
              },
            ],
          }),
          { status: 200 }
        );
      }
      return new Response("{}", { status: 200 });
    }) as typeof fetch;

    const onSelectModel = vi.fn();
    try {
      render(
        <ModelPickerQuotaPopover
          selectedModel={{ id: "codex:gpt-5.3-codex", label: "GPT-5.3 Codex" }}
          onSelectModel={onSelectModel}
        />
      );

      await waitFor(() => expect(screen.getByTestId("composer-quota-pill").textContent).toBe("13%"));
      fireEvent.click(screen.getByTestId("composer-model-picker-btn"));

      const popover = screen.getByTestId("composer-model-popover");
      expect(popover.textContent).toContain("78%");
      expect(popover.textContent).toContain("13%");

      fireEvent.click(screen.getByTestId("quota-check-again-btn"));
      await waitFor(() => expect(usageCalls).toBeGreaterThanOrEqual(2));
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});
