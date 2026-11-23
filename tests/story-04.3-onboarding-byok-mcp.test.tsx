/** @vitest-environment jsdom */
// @story 04.3
// @implements Gate H6 (Prioritize BYOS CLI Doctor BEFORE BYOK, Auto-Connect Zero-Click UX)
import React from "react";
import fs from "node:fs";
import path from "node:path";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, cleanup, act } from "@testing-library/react";
import { OnboardingWizardModal, type OnboardingResult } from "../src/pages/workbench/components/OnboardingWizardModal";
import { OnboardingStepsContent } from "../src/pages/workbench/components/OnboardingStepsContent";
import { apiWorkbenchSettings } from "../src/api/workbench-settings";
import type { CliDiagnosticItem, DoctorScanResponse } from "../src/lib/validations/workbench-settings";

const MOCK_CLI_ITEMS: CliDiagnosticItem[] = [
  {
    id: "agy",
    name: "Antigravity CLI",
    command: "agy",
    installed: true,
    authenticated: true,
    status: "ready",
    version: "2.1.0",
    latencyMs: 15,
  },
  {
    id: "codex",
    name: "OpenAI Codex CLI",
    command: "codex",
    installed: true,
    authenticated: true,
    status: "ready",
    version: "0.4.1",
    latencyMs: 25,
  },
  {
    id: "gemini",
    name: "Gemini CLI",
    command: "gemini",
    installed: true,
    authenticated: false,
    status: "auth_needed",
    version: "1.0.3",
    latencyMs: 40,
  },
  {
    id: "claude",
    name: "Claude Code CLI",
    command: "claude",
    installed: false,
    authenticated: false,
    status: "not_installed",
    latencyMs: 0,
  },
];

const createDoctorResponse = (items: CliDiagnosticItem[]): DoctorScanResponse => ({
  scannedAt: new Date().toISOString(),
  clis: items,
  tools: items,
});

describe("Story 04.3: Gate H6 - Rust doctor_vault Extraction Parity", () => {
  it("extracts doctor_vault.rs module and wires route handlers cleanly", () => {
    const doctorVaultPath = path.resolve("crates/harness-daemon/src/routes/doctor_vault.rs");
    const routesModPath = path.resolve("crates/harness-daemon/src/routes/mod.rs");
    const legacyRoutesPath = path.resolve("crates/harness-daemon/src/routes/legacy_workbench_v6.rs");
    const flowRoutesPath = path.resolve("crates/harness-daemon/src/flow_routes.rs");

    expect(fs.existsSync(doctorVaultPath), "doctor_vault.rs must exist").toBe(true);
    const doctorVaultSrc = fs.readFileSync(doctorVaultPath, "utf8");
    expect(doctorVaultSrc).toContain("pub async fn handle_verify_provider_key");
    expect(doctorVaultSrc).toContain("pub async fn handle_get_doctor_cli_matrix");
    expect(doctorVaultSrc).toContain("pub async fn handle_probe_cli_tool");
    expect(doctorVaultSrc).toContain("pub async fn handle_list_vault_keys");
    expect(doctorVaultSrc).toContain("pub async fn handle_upsert_vault_key");
    expect(doctorVaultSrc).toContain("pub async fn handle_delete_vault_key");

    const routesModSrc = fs.readFileSync(routesModPath, "utf8");
    expect(routesModSrc).toContain("pub mod doctor_vault;");
    expect(routesModSrc).toContain("pub use doctor_vault::*;");

    const flowRoutesSrc = fs.readFileSync(flowRoutesPath, "utf8");
    expect(flowRoutesSrc).toContain("/api/v1/doctor/cli-matrix");
    expect(flowRoutesSrc).toContain("/api/v1/doctor/probe-cli");
    expect(flowRoutesSrc).toContain("/api/v1/vault/keys");

    // legacy_workbench_v6 should re-export or reference without duplicate definition
    const legacySrc = fs.readFileSync(legacyRoutesPath, "utf8");
    expect(legacySrc).toContain("doctor_vault");
  });
});

describe("Story 04.3: Onboarding Wizard - Prioritize BYOS CLI BEFORE BYOK & Auto-Connect", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("prioritizes BYOS CLI Doctor in Section 1 and places BYOK API Key in Section 2 below", async () => {
    vi.spyOn(apiWorkbenchSettings, "scanDoctor").mockResolvedValue(createDoctorResponse(MOCK_CLI_ITEMS));

    render(
      <OnboardingWizardModal
        isOpen={true}
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("cli-doctor-accordion-toggle")).toBeTruthy();
    });

    // Verify Section 1 Header text exists
    expect(
      screen.getByText(/1\.\s*(Kết nối công cụ AI sẵn có trên máy|Connect Local AI Tools)/i)
    ).toBeTruthy();

    // Verify Section 2 API Key Header text exists
    expect(
      screen.getByText(/2\.\s*(Hoặc kết nối bằng Khóa API|Or connect with your own API key)/i)
    ).toBeTruthy();

    // Verify BYOK inputs are present
    expect(screen.getByTestId("byok-provider-select")).toBeTruthy();
    expect(screen.getByTestId("byok-api-key-input")).toBeTruthy();
    expect(screen.getByTestId("byok-verify-btn")).toBeTruthy();
  });

  it("expands the BYOS CLI section by default and toggles via accordion button", async () => {
    vi.spyOn(apiWorkbenchSettings, "scanDoctor").mockResolvedValue(createDoctorResponse(MOCK_CLI_ITEMS));

    render(
      <OnboardingWizardModal
        isOpen={true}
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("cli-tool-card-agy")).toBeTruthy();
    });

    // Cards are visible by default
    expect(screen.getByTestId("cli-tool-card-agy")).toBeTruthy();
    expect(screen.getByTestId("cli-tool-card-codex")).toBeTruthy();

    // Click accordion toggle to collapse
    const toggleBtn = screen.getByTestId("cli-doctor-accordion-toggle");
    fireEvent.click(toggleBtn);

    // Cards should now be collapsed (not in document)
    expect(screen.queryByTestId("cli-tool-card-agy")).toBeNull();

    // Click toggle again to re-expand
    fireEvent.click(toggleBtn);
    expect(screen.getByTestId("cli-tool-card-agy")).toBeTruthy();
  });

  it("auto-detects and auto-connects ready local BYOS subscriptions (Zero-Click UX Pattern)", async () => {
    vi.spyOn(apiWorkbenchSettings, "scanDoctor").mockResolvedValue(createDoctorResponse(MOCK_CLI_ITEMS));

    render(
      <OnboardingWizardModal
        isOpen={true}
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />
    );

    // Wait for doctor scan to complete and banner to appear
    await waitFor(() => {
      expect(screen.getByTestId("byos-auto-connect-banner")).toBeTruthy();
    });

    const banner = screen.getByTestId("byos-auto-connect-banner");
    expect(banner.textContent).toContain("2"); // 2 ready tools: agy and codex
    expect(banner.textContent).toContain("Antigravity CLI");
    expect(banner.textContent).toContain("OpenAI Codex CLI");

    // Both ready tools should have the "Đã tự động kết nối" badge
    const agyCard = screen.getByTestId("cli-tool-card-agy");
    expect(agyCard.textContent).toContain("Đã tự động kết nối");

    const codexCard = screen.getByTestId("cli-tool-card-codex");
    expect(codexCard.textContent).toContain("Đã tự động kết nối");

    // Gemini requires auth, so it should not have the auto-connected badge
    const geminiCard = screen.getByTestId("cli-tool-card-gemini");
    expect(geminiCard.textContent).not.toContain("Đã tự động kết nối");
    expect(geminiCard.textContent).toContain("Cần đăng nhập");
  });

  it("probes a CLI tool on demand and auto-connects when probe reports ready", async () => {
    vi.spyOn(apiWorkbenchSettings, "scanDoctor").mockResolvedValue(createDoctorResponse(MOCK_CLI_ITEMS));

    const probeSpy = vi.spyOn(apiWorkbenchSettings, "probeCliTool").mockResolvedValue({
      id: "gemini",
      name: "Gemini CLI",
      command: "gemini",
      installed: true,
      authenticated: true,
      status: "ready",
      version: "1.0.3",
      latencyMs: 35,
    });

    render(
      <OnboardingWizardModal
        isOpen={true}
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("cli-probe-btn-gemini")).toBeTruthy();
    });

    // Probe Gemini CLI
    const probeBtn = screen.getByTestId("cli-probe-btn-gemini");
    fireEvent.click(probeBtn);

    expect(probeSpy).toHaveBeenCalledWith("gemini");

    await waitFor(() => {
      const geminiCard = screen.getByTestId("cli-tool-card-gemini");
      expect(geminiCard.textContent).toContain("Đã tự động kết nối");
    });
  });

  it("verifies BYOK API Key directly in Section 2 and reflects validation status", async () => {
    vi.spyOn(apiWorkbenchSettings, "scanDoctor").mockResolvedValue(createDoctorResponse([]));

    const verifySpy = vi.spyOn(apiWorkbenchSettings, "verifyProviderKey").mockResolvedValue({
      valid: true,
      status: "verified",
      providerId: "anthropic",
      credentialRef: "vault_0600",
      message: "Anthropic Claude API Key hợp lệ!",
    });

    render(
      <OnboardingWizardModal
        isOpen={true}
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("byok-provider-select")).toBeTruthy();
    });

    const providerSelect = screen.getByTestId("byok-provider-select") as HTMLSelectElement;
    fireEvent.change(providerSelect, { target: { value: "anthropic" } });

    const fixtureAnthropicKey = ["sk-ant", "api03", "testkey", "12345"].join("-");
    const keyInput = screen.getByTestId("byok-api-key-input") as HTMLInputElement;
    fireEvent.change(keyInput, { target: { value: fixtureAnthropicKey } });

    const verifyBtn = screen.getByTestId("byok-verify-btn");
    expect((verifyBtn as HTMLButtonElement).disabled).toBe(false);

    fireEvent.click(verifyBtn);

    expect(verifySpy).toHaveBeenCalledWith({
      providerId: "anthropic",
      apiKey: fixtureAnthropicKey,
      endpointUrl: "",
    });

    await waitFor(() => {
      expect(screen.getByText("Anthropic Claude API Key hợp lệ!")).toBeTruthy();
    });
  });

  it("shows error feedback when BYOK API Key verification fails", async () => {
    vi.spyOn(apiWorkbenchSettings, "scanDoctor").mockResolvedValue(createDoctorResponse([]));

    vi.spyOn(apiWorkbenchSettings, "verifyProviderKey").mockRejectedValue(
      new Error("Invalid 401 Unauthorized API Key")
    );

    render(
      <OnboardingWizardModal
        isOpen={true}
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("byok-api-key-input")).toBeTruthy();
    });

    const keyInput = screen.getByTestId("byok-api-key-input");
    fireEvent.change(keyInput, { target: { value: "sk-invalid-key" } });

    const verifyBtn = screen.getByTestId("byok-verify-btn");
    fireEvent.click(verifyBtn);

    await waitFor(() => {
      expect(screen.getByText(/Invalid 401 Unauthorized API Key/i)).toBeTruthy();
    });
  });

  it("ensures Step 1 Welcome header block and step badge are center-aligned", async () => {
    vi.spyOn(apiWorkbenchSettings, "scanDoctor").mockResolvedValue(createDoctorResponse(MOCK_CLI_ITEMS));

    const { container } = render(
      <OnboardingWizardModal
        isOpen={true}
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />
    );

    // Welcome title is rendered
    expect(screen.getByText(/Chào mừng đến Cowok-Design/i)).toBeTruthy();

    // Step badge is rendered with centered alignment
    const stepBadge = screen.getByText(/Bước\s*1\s*\/\s*3/i);
    expect(stepBadge).toBeTruthy();
    expect(stepBadge.style.display).toBe("inline-flex");
    expect(stepBadge.style.alignItems).toBe("center");
    expect(stepBadge.style.justifyContent).toBe("center");

    // Header container is centered
    const headerContainer = container.querySelector('div[style*="text-align: center"]');
    expect(headerContainer).toBeTruthy();
  });

  it("navigates through 3 steps and submits agent configuration upon completion", async () => {
    vi.spyOn(apiWorkbenchSettings, "scanDoctor").mockResolvedValue(createDoctorResponse(MOCK_CLI_ITEMS));
    vi.spyOn(apiWorkbenchSettings, "updateVaultConfig").mockResolvedValue({
      updatedAt: new Date().toISOString(),
      vaultMode: "vault_0600",
      vaultWritable: true,
      encryptedVaultPresent: true,
    });

    const upsertSpy = vi.spyOn(apiWorkbenchSettings, "upsertAgentSetting").mockResolvedValue({
      id: "agent-1",
      tenantId: "local-default",
      providerId: "agy",
      modelId: "gemini-25",
      endpointUrl: "",
      credentialRef: "vault://vault_0600/agy",
      permissionMode: "auto_sandbox",
      vaultMode: "vault_0600",
      status: "auto_sandbox",
      onboardingDone: true,
      isDefault: true,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    });

    const onCompleteMock = vi.fn();
    const onCloseMock = vi.fn();

    render(
      <OnboardingWizardModal
        isOpen={true}
        onClose={onCloseMock}
        onComplete={onCompleteMock}
      />
    );

    await waitFor(() => {
      expect(screen.getByTestId("byos-auto-connect-banner")).toBeTruthy();
    });

    // Step 1: Click "Tiếp tục"
    const nextBtn1 = screen.getByText(/Tiếp tục/i);
    fireEvent.click(nextBtn1);

    // Step 2: Storage & Workspace
    await waitFor(() => {
      expect(screen.getByText(/2\.\s*Không gian làm việc & Cơ chế bảo mật lưu trữ/i)).toBeTruthy();
    });

    // Select linked workspace mode
    const linkedCard = screen.getByText("Liên Kết Thư Mục Local");
    fireEvent.click(linkedCard);

    // Click "Tiếp tục" to go to Step 3
    const nextBtn2 = screen.getByText(/Tiếp tục/i);
    fireEvent.click(nextBtn2);

    // Step 3: Permissions
    await waitFor(() => {
      expect(screen.getByText(/3\.\s*Quyền hạn thực thi & Khởi chạy Workbench/i)).toBeTruthy();
    });

    // Click "Hoàn tất & Mở Canvas"
    const finishBtn = screen.getByText(/Hoàn tất & Mở Canvas/i);
    fireEvent.click(finishBtn);

    await waitFor(() => {
      expect(upsertSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          providerId: "agy",
          modelId: "gemini-25",
          vaultMode: "vault_0600",
          permissionMode: "auto_sandbox",
          onboardingDone: true,
          isDefault: true,
        })
      );
      expect(onCompleteMock).toHaveBeenCalledWith(
        expect.objectContaining({
          preferredCli: "agy",
          storageMode: "vault_0600",
          permissionLevel: "auto_sandbox",
          workspaceMode: "linked",
        })
      );
      expect(onCloseMock).toHaveBeenCalled();
    });
  });
});
