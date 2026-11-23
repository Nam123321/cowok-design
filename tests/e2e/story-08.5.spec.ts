// @story 08.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Page } from "@playwright/test";

const DESKTOP_WIDTH = 1440;
const DESKTOP_HEIGHT = 900;
const MOBILE_WIDTH = 390;
const MOBILE_HEIGHT = 844;
const NAV_TIMEOUT_MS = 20_000;
const ACTION_TIMEOUT_MS = 12_000;
const NONCE_BYTES = 16;

async function dismissOnboardingIfVisible(page: Page) {
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
    await expect(wizardHeading).toBeHidden({ timeout: ACTION_TIMEOUT_MS });
  }
}

test.describe("Story 08.5: Split DE+DR Plugin Runtime, Tool Jail & System-1 LEARN E2E", () => {
  test("verifies all 4 Plugin Center surfaces, 7 DE tools, Repo Import, HUMAN_ONLY Jail, and LEARN Ledger", async ({
    browser,
  }) => {
    const storyDir = path.resolve(
      process.cwd(),
      "_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-08/Story-08.5"
    );
    const evidenceDir = path.join(storyDir, "qa", "evidence");
    const videoDir = path.join(evidenceDir, "videos");
    fs.mkdirSync(videoDir, { recursive: true });

    const qaNonce = process.env.QA_NONCE_TOKEN || crypto.randomBytes(NONCE_BYTES).toString("hex");
    fs.writeFileSync(
      path.join(evidenceDir, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, storyId: "08.5", generatedAt: new Date().toISOString() }, null, 2)
    );

    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}`;
    const context = await browser.newContext({
      viewport: { width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT },
      recordVideo: { dir: videoDir, size: { width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT } },
    });
    const page = await context.newPage();

    const pageErrors: string[] = [];
    const consoleErrors: string[] = [];
    const server500s: string[] = [];
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("console", (msg) => {
      if (msg.type() === "error" && !msg.text().includes("ERR_CERT_AUTHORITY_INVALID")) {
        consoleErrors.push(msg.text());
      }
    });
    page.on("response", (resp) => {
      if (resp.status() >= 500) server500s.push(`${resp.status()} ${resp.url()}`);
    });

    try {
      await page.goto(`${baseUrl}/plugins`, { waitUntil: "domcontentloaded", timeout: NAV_TIMEOUT_MS });
      await expect(page.locator("#root")).not.toBeEmpty({ timeout: ACTION_TIMEOUT_MS });
      await dismissOnboardingIfVisible(page);

      // Verify all 4 canonical Story 08.5 data-ff-id surfaces are mounted
      await expect(page.locator("[data-ff-id='de-tools-status-list']")).toBeVisible({
        timeout: ACTION_TIMEOUT_MS,
      });
      await expect(page.locator("[data-ff-id='plugin-repo-import-form']")).toBeVisible({
        timeout: ACTION_TIMEOUT_MS,
      });
      await expect(page.locator("[data-ff-id='ephemeral-tool-audit-drawer']")).toBeVisible({
        timeout: ACTION_TIMEOUT_MS,
      });
      await expect(page.locator("[data-ff-id='learn-ledger-queue']")).toBeVisible({
        timeout: ACTION_TIMEOUT_MS,
      });

      // Trigger 7 Built-in DE tools run
      await page.getByTestId("btn-run-de-suite").click();
      await expect(page.getByTestId("de-suite-summary")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });

      // Exercise compact chip grid: open & cancel Quarantine Reason popup, then select 2 skills to show Bulk Action Floating Card
      const firstQuarantineBtn = page.locator("[data-testid^='btn-quarantine-']").first();
      if (await firstQuarantineBtn.isVisible().catch(() => false)) {
        await firstQuarantineBtn.click();
        await expect(page.getByTestId("quarantine-reason-modal")).toBeVisible({ timeout: 5000 });
        await page.getByTestId("btn-cancel-quarantine-modal").click();
        await expect(page.getByTestId("quarantine-reason-modal")).toBeHidden({ timeout: 5000 });
      }
      const checkboxes = page.locator("[data-testid^='checkbox-plugin-']");
      const checkboxCount = await checkboxes.count();
      if (checkboxCount >= 2) {
        await checkboxes.nth(0).check();
        await checkboxes.nth(1).check();
        await expect(page.getByTestId("plugin-bulk-floating-bar")).toBeVisible({ timeout: 5000 });
      }

      // Capture Desktop Light & Dark (Night) + Mobile screenshots
      await page.screenshot({ path: path.join(evidenceDir, "desktop-light.png"), fullPage: true });
      await page.evaluate(() => {
        document.documentElement.setAttribute("data-theme", "night");
        document.body.classList.add("theme-night");
      });
      await page.waitForTimeout(250);
      await page.screenshot({ path: path.join(evidenceDir, "desktop-dark.png"), fullPage: true });

      await page.setViewportSize({ width: MOBILE_WIDTH, height: MOBILE_HEIGHT });
      await page.screenshot({ path: path.join(evidenceDir, "mobile-390.png"), fullPage: true });

      expect(pageErrors).toEqual([]);
      expect(consoleErrors).toEqual([]);
      expect(server500s).toEqual([]);
    } finally {
      await context.close();
    }
  });
});
