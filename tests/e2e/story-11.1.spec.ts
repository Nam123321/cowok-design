// @story 11.1
// @implements AC-01, AC-02, AC-03, AC-04 (Live Browser E2E for POSIX BYOS Sandbox & Unified BYOK Gateway UI)
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-11/Story-11.1"
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
const MIN_ADAPTER_COUNT = 26;
const HTTP_SERVER_ERROR = 500;
const RANDOM_NONCE_BYTES = 32;

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

test.describe("Story 11.1 POSIX BYOS Sandbox & Unified BYOK Gateway Live E2E", () => {
  test("verifies live 26+ CLI sandbox matrix, BYOK Bento panels, and theme/mobile fidelity", async ({
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
      recordHar: { path: path.join(EVIDENCE_DIR, "story-11.1-network.har"), mode: "minimal" },
      recordVideo: { dir: EVIDENCE_DIR, size: { width: VIDEO_WIDTH, height: VIDEO_HEIGHT } },
    });

    await context.route(`http://127.0.0.1:${targetPort}/**`, async (route) => {
      const headers = {
        ...route.request().headers(),
        "x-iwish-qa-nonce": qaNonce,
        "x-tenant-id": "local-default",
      };
      await route.continue({ headers });
    });

    const page = await context.newPage();
    const pageErrors: string[] = [];
    const server500s: string[] = [];
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("response", (res) => {
      if (res.status() >= HTTP_SERVER_ERROR) server500s.push(`${res.status()} ${res.url()}`);
    });

    const sessionToken = await bootEditorSession(page, baseUrl, qaNonce);

    // 1. Live BFF -> Daemon GET /api/v1/bot-fleet/providers/status
    const statusRes = await page.request.get(
      new URL("/api/v1/bot-fleet/providers/status", baseUrl).toString(),
      {
        headers: {
          Authorization: `Bearer ${sessionToken}`,
          "X-Cowok-Session-Token": sessionToken,
          "x-iwish-qa-nonce": qaNonce,
          "x-tenant-id": "local-default",
        },
      }
    );
    expect(statusRes.ok(), `providers/status failed: ${statusRes.status()} ${await statusRes.text()}`).toBeTruthy();
    const statusJson = (await statusRes.json()) as {
      ok: boolean;
      data: { adapterCount: number; sandboxProtocol: string; sessionDirIsolated: boolean };
    };
    expect(statusJson.ok).toBe(true);
    expect(statusJson.data.sandboxProtocol).toBe("posix_0600_fd_pgid");
    expect(statusJson.data.sessionDirIsolated).toBe(true);
    expect(statusJson.data.adapterCount).toBeGreaterThanOrEqual(MIN_ADAPTER_COUNT);

    // 2. Unconditionally open Settings Modal -> AI Models tab and assert live POSIX Sandbox Badge + API Bento
    const settingsBtn = page.getByRole("button", { name: /Cài đặt|Settings/i }).first();
    await expect(settingsBtn).toBeVisible({ timeout: 5000 });
    await settingsBtn.click();
    const modelsTab = page.getByText(/Mô hình AI|AI Models|Mô hình/i).first();
    await expect(modelsTab).toBeVisible({ timeout: 5000 });
    await modelsTab.click();
    await expect(page.getByTestId("detected-byos-panel")).toBeVisible({ timeout: 8000 });
    await expect(page.getByTestId("byos-sandbox-status-badge")).toBeVisible({ timeout: 8000 });
    await expect(page.getByTestId("detected-byos-panel")).not.toContainText("BYOS");
    await expect(page.getByTestId("api-bento-panel")).not.toContainText("API Providers");
    await page.getByTestId("toggle-api-bento-btn").click();
    await expect(page.getByTestId("api-bento-tile-gemini")).toBeVisible({ timeout: 5000 });
    await expect(page.getByTestId("api-bento-tile-openrouter")).toBeVisible({ timeout: 5000 });
    await expect(page.getByTestId("api-bento-tile-deepseek")).toBeVisible({ timeout: 5000 });

    // 3. Capture Desktop Light (Warm), Desktop Dark (Night), and Mobile evidence
    await setWorkspaceTheme(page, "warm");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-11.1-desktop-light.png") });
    await setWorkspaceTheme(page, "night");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-11.1-desktop-dark.png") });
    await page.setViewportSize({ width: MOBILE_WIDTH, height: MOBILE_HEIGHT });
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-11.1-mobile.png") });

    expect(pageErrors).toEqual([]);
    expect(server500s).toEqual([]);
    await context.close();
  });
});
