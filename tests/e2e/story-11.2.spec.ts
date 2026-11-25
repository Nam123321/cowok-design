// @story 11.2
// @implements AC-01, AC-02, AC-03, AC-04 (Live Browser E2E for Bidirectional Open MCP Server, Compact Settings Popup & Full /plugins Chip-Grid Matrix + Live Audit Table)
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/Story-11.2"
);
const EVIDENCE_DIR = path.join(STORY_DIR, "qa", "evidence");
const DESKTOP_WIDTH = 1440;
const DESKTOP_HEIGHT = 900;
const MOBILE_WIDTH = 390;
const MOBILE_HEIGHT = 844;
const VIDEO_WIDTH = 1280;
const VIDEO_HEIGHT = 800;
const THEME_SETTLE_MS = 350;
const BOOT_TIMEOUT_MS = 15000;
const HTTP_SERVER_ERROR = 500;
const RANDOM_NONCE_BYTES = 32;
const JSONRPC_HUMAN_APPROVAL_REQUIRED = -32003;

async function dismissOnboardingWizard(page: Page) {
  const wizardHeading = page.getByRole("heading", {
    name: /Chào mừng đến Cowok-Design|Welcome to Cowok-Design/i,
  });
  const shown = await wizardHeading
    .waitFor({ state: "visible", timeout: 3000 })
    .then(() => true)
    .catch(() => false);
  if (shown) {
    await page.getByRole("button", { name: /Tiếp tục|Next/i }).click();
    await page.getByRole("button", { name: /Tiếp tục|Next/i }).click();
    await page.getByRole("button", { name: /Hoàn tất|Finish/i }).click();
    await expect(wizardHeading).toBeHidden({ timeout: 10000 });
  }
}

async function setWorkspaceTheme(page: Page, theme: "warm" | "night") {
  await page.evaluate((nextTheme) => {
    document.documentElement.setAttribute("data-theme", nextTheme);
    const shell = document.querySelector(".workbench-shell");
    if (shell) {
      shell.setAttribute("data-theme", nextTheme);
      shell.classList.remove("theme-warm", "theme-dim", "theme-night");
      shell.classList.add(`theme-${nextTheme}`);
    }
  }, theme);
  await page.waitForTimeout(THEME_SETTLE_MS);
}

async function bootEditorSession(page: Page, baseUrl: string, qaNonce: string): Promise<string> {
  await page.setViewportSize({ width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT });
  await page.goto(baseUrl, { waitUntil: "networkidle" });
  await expect(page.locator("#root")).not.toBeEmpty({ timeout: BOOT_TIMEOUT_MS });

  const bootCap =
    (await page.locator('meta[name="cowok-boot-cap"]').first().getAttribute("content"))?.trim() || "";
  expect(bootCap.length).toBeGreaterThan(0);

  const authRes = await page.request.post(new URL("/api/auth/local-session", baseUrl).toString(), {
    headers: {
      "Content-Type": "application/json",
      "X-Cowok-Boot-Cap": bootCap,
      "x-iwish-qa-nonce": qaNonce,
      "x-tenant-id": "local-default",
    },
    data: { role: "editor", userId: "local-operator" },
  });
  expect(authRes.ok()).toBeTruthy();
  const auth = (await authRes.json()) as { sessionToken?: string };
  const sessionToken = auth.sessionToken || "";
  expect(sessionToken.length).toBeGreaterThan(0);

  await page.request.put(new URL("/api/agent-settings", baseUrl).toString(), {
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${sessionToken}`,
      "X-Cowok-Session-Token": sessionToken,
      "x-iwish-qa-nonce": qaNonce,
      "x-tenant-id": "local-default",
    },
    data: {
      providerId: "cowok-swarm",
      modelId: "cowok-swarm",
      vaultMode: "vault_0600",
      permissionMode: "auto_sandbox",
      onboardingDone: true,
    },
  });

  await page.reload({ waitUntil: "networkidle" });
  await dismissOnboardingWizard(page);
  return sessionToken;
}

async function runLiveRpcChecks(page: Page, baseUrl: string, authHeaders: Record<string, string>) {
  const stateRpc = await page.request.post(new URL("/api/v1/mcp/rpc", baseUrl).toString(), {
    headers: authHeaders,
    data: { jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "get_app_state", arguments: {} } },
  });
  expect(stateRpc.ok()).toBeTruthy();
  const stateJson = (await stateRpc.json()) as { result?: { structuredContent?: { projectId?: string } } };
  expect(stateJson.result?.structuredContent?.projectId).toBeTruthy();

  const denyRpc = await page.request.post(new URL("/api/v1/mcp/rpc", baseUrl).toString(), {
    headers: authHeaders,
    data: {
      jsonrpc: "2.0",
      id: 2,
      method: "tools/call",
      params: { name: "ephemeral_tool_forge", arguments: { approval_token: "forged-token" } },
    },
  });
  expect(denyRpc.ok()).toBeTruthy();
  const denyJson = (await denyRpc.json()) as { error?: { code?: number; message?: string } };
  expect(denyJson.error?.code).toBe(JSONRPC_HUMAN_APPROVAL_REQUIRED);
  expect(denyJson.error?.message).toBe("HUMAN_APPROVAL_REQUIRED");
}

async function verifySettingsCompactAndPluginsFullView(page: Page, targetPort: string) {
  const settingsBtn = page
    .locator("[data-ff-id='open-settings-btn'], [data-testid='open-settings-btn'], button[aria-label*='Cài đặt'], button[aria-label*='Settings']")
    .first();
  await settingsBtn.click();
  const mcpTab = page.getByRole("button", { name: /MCP|Máy chủ MCP/i }).first();
  await mcpTab.click();

  await expect(page.locator("[data-ff-id='mcp-host-installer-list']")).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
  await expect(page.getByTestId("open-full-mcp-center-btn")).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
  await expect(page.locator("[data-ff-id='mcp-tool-permission-matrix']")).toHaveCount(0);

  await page.goto(`http://127.0.0.1:${targetPort}/plugins`, { waitUntil: "networkidle" });
  await expect(page.locator("[data-ff-id='mcp-host-installer-list']")).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
  const matrix = page.locator("[data-ff-id='mcp-tool-permission-matrix']");
  await expect(matrix).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
  await expect(page.getByTestId("mcp-human-only-lock-ephemeral_tool_forge")).toBeVisible();
  await expect(matrix).not.toContainText("Story 08.");
  await expect(page.getByTestId("mcp-policy-row-ephemeral_tool_forge")).not.toContainText("HUMAN_ONLY");

  await expect(page.locator("[data-ff-id='mcp-external-activity-table']")).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
  await page.getByTestId("mcp-activity-refresh-btn").click();
  await expect(
    page.locator("[data-ff-id='mcp-external-activity-table'] [data-status='DENIED_HUMAN_ONLY']").first()
  ).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
}

test.describe("Story 11.2 Bidirectional Open MCP Server & 1-Click Host Installer Live E2E", () => {
  test("verifies JSON-RPC HUMAN_ONLY rejection, compact Settings popup, and full /plugins Chip-Grid + live activity log", async ({
    browser,
  }) => {
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    const qaNonce =
      process.env.WATCHMEN_NONCE ||
      process.env.QA_NONCE_TOKEN ||
      crypto.randomBytes(RANDOM_NONCE_BYTES).toString("hex");

    const context = await browser.newContext({
      viewport: { width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT },
      recordHar: { path: path.join(EVIDENCE_DIR, "story-11.2-network.har"), mode: "minimal" },
      recordVideo: { dir: EVIDENCE_DIR, size: { width: VIDEO_WIDTH, height: VIDEO_HEIGHT } },
    });

    await context.route(`http://127.0.0.1:${targetPort}/**`, async (route) => {
      const headers = { ...route.request().headers(), "x-iwish-qa-nonce": qaNonce };
      await route.continue({ headers });
    });

    const page = await context.newPage();
    const consoleErrors: string[] = [];
    const pageErrors: string[] = [];
    const serverErrors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("response", (resp) => {
      if (resp.status() >= HTTP_SERVER_ERROR) serverErrors.push(`${resp.status()} ${resp.url()}`);
    });

    const sessionToken = await bootEditorSession(page, baseUrl, qaNonce);
    const authHeaders = {
      "Content-Type": "application/json",
      Authorization: `Bearer ${sessionToken}`,
      "X-Cowok-Session-Token": sessionToken,
      "x-iwish-qa-nonce": qaNonce,
      "x-tenant-id": "local-default",
      "X-Mcp-Host-Client": "cursor",
      "X-Cowok-Client-Kind": "mcp",
    };

    await runLiveRpcChecks(page, baseUrl, authHeaders);
    await verifySettingsCompactAndPluginsFullView(page, targetPort);

    await setWorkspaceTheme(page, "warm");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-11.2-desktop-light.png"), fullPage: true });
    await setWorkspaceTheme(page, "night");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-11.2-desktop-dark.png"), fullPage: true });
    await page.setViewportSize({ width: MOBILE_WIDTH, height: MOBILE_HEIGHT });
    await page.waitForTimeout(THEME_SETTLE_MS);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-11.2-mobile.png"), fullPage: true });

    expect(consoleErrors).toEqual([]);
    expect(pageErrors).toEqual([]);
    expect(serverErrors).toEqual([]);
    await context.close();
  });
});
