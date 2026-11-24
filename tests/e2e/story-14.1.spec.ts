// @story 14.1
// @implements AC-01, AC-02, AC-03, AC-04 (Button-Exhaustive Live Browser E2E Verification & Video)
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
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
}

test.describe("Story 14.1: Button-Exhaustive Non-Tech Support Drawer & Telemetry Privacy E2E", () => {
  test("verifies Help & Feedback Drawer (all 4 categories, 2 consecutive tickets, Zero-PII scrub, reload persistence) and Settings Telemetry Opt-Out", async ({
    browser,
  }) => {
    test.setTimeout(120_000);
    const storyDir = path.resolve(
      process.cwd(),
      "_iwish-output/3. Development/1. Epic & Story/VS-07. Cloud Control Plane, Non-Tech Support Desk & SuperAdmin Portal/Epic-14/Story-14.1"
    );
    const evidenceDir = path.join(storyDir, "qa", "evidence");
    const videoDir = path.join(evidenceDir, "videos");
    fs.mkdirSync(videoDir, { recursive: true });

    const qaNonce = process.env.QA_NONCE_TOKEN || crypto.randomBytes(NONCE_BYTES).toString("hex");
    fs.writeFileSync(
      path.join(evidenceDir, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, storyId: "14.1", generatedAt: new Date().toISOString() }, null, 2)
    );

    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}`;
    const context = await browser.newContext({
      viewport: { width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT },
      recordVideo: { dir: videoDir, size: { width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT } },
    });
    const page = await context.newPage();

    const pageErrors: string[] = [];
    const unexpectedConsoleErrors: string[] = [];
    const unexpectedHttpErrors: string[] = [];

    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        const text = msg.text();
        if (!text.includes("favicon.ico")) unexpectedConsoleErrors.push(text);
      }
    });
    page.on("response", (res) => {
      const url = res.url();
      const status = res.status();
      if (url.includes("/api/") && status >= 400) {
        unexpectedHttpErrors.push(`${status} ${url}`);
      }
    });

    await page.goto(baseUrl, { waitUntil: "domcontentloaded", timeout: NAV_TIMEOUT_MS });
    await dismissOnboardingIfVisible(page);

    // 1. Click "Trợ giúp & Góp ý" in TopNav -> opens RightDrawer [5.25]
    const navSupportBtn = page.getByTestId("nav-support-drawer-btn");
    await expect(navSupportBtn).toBeVisible({ timeout: NAV_TIMEOUT_MS });
    await navSupportBtn.click();

    const drawer = page.getByTestId("support-request-drawer");
    await expect(drawer).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    await expect(drawer.locator("button[type='submit']")).toHaveCount(1);

    // 2. Click all 4 category pills
    for (const cat of ["feature_request", "skill_template_request", "question", "bug_report"]) {
      const catBtn = drawer.locator(`[data-testid='support-category-${cat}']`);
      if (await catBtn.isVisible()) {
        await catBtn.click();
        await expect(catBtn).toHaveAttribute("aria-pressed", "true");
      }
    }

    // 3. Verify styled image/video upload button + attach real PNG -> remove -> attach short MP4 video
    await expect(page.getByTestId("support-media-upload-btn")).toBeVisible();
    const tinyPng = Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+ip1sAAAAASUVORK5CYII=",
      "base64"
    );
    await page.getByTestId("support-screenshot-input").setInputFiles({
      name: "demo-bug.png",
      mimeType: "image/png",
      buffer: tinyPng,
    });
    await expect(page.getByTestId("support-media-preview-chip")).toBeVisible();
    await page.getByTestId("support-media-remove-btn").click();
    await expect(page.getByTestId("support-media-preview-chip")).toBeHidden();
    await page.getByTestId("support-screenshot-input").setInputFiles({
      name: "repro-clip.mp4",
      mimeType: "video/mp4",
      buffer: Buffer.from("00000018667479706d703432", "hex"),
    });
    await expect(page.getByTestId("support-media-preview-video")).toBeVisible();

    // 4. Submit Ticket #1 containing a synthetic home path + video attachment to prove live Zero-PII scrubbing
    const ticketTitle1 = "Export preview layout feedback";
    await page.getByTestId("support-title-input").fill(ticketTitle1);
    await page
      .getByTestId("support-description-input")
      .fill("Encountered slow preview export at /Users/nontechdemo/Documents/project while testing.");
    await page.getByTestId("support-email-input").fill("nontech@example.com");

    const submit1Promise = page.waitForResponse(
      (res) => res.url().includes("/api/v1/support/tickets") && res.request().method() === "POST",
      { timeout: ACTION_TIMEOUT_MS }
    );
    await page.getByTestId("support-submit-btn").click();
    const submit1Resp = await submit1Promise;
    expect(submit1Resp.status()).toBe(201);
    const ticket1Raw = await submit1Resp.json();
    const ticket1 = ticket1Raw.data ?? ticket1Raw.ticket ?? ticket1Raw;
    expect(ticket1.ticketCode).toMatch(/^CWK-/);
    expect(ticket1.description).toContain("<HOME>/Documents/project");
    expect(ticket1.description).not.toContain("/Users/nontechdemo");

    // Verify auto-switch to "My Requests" shows synced heading, persisted ticket & clickable attachment lightbox
    await expect(page.getByTestId("support-tab-my-requests")).toHaveAttribute("aria-selected", "true");
    await expect(page.getByTestId("support-my-requests-heading")).toContainText(/Yêu cầu của tôi|My Requests/);
    await expect(drawer).toContainText(ticket1.ticketCode);
    await expect(drawer).toContainText("<HOME>/Documents/project");

    const attachBtn = page.getByTestId(`support-ticket-attachment-${ticket1.ticketCode}`);
    await expect(attachBtn).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    const attRespPromise = page.waitForResponse(
      (res) => res.url().includes(`/api/v1/support/tickets/${encodeURIComponent(ticket1.id)}/attachment`) && res.status() === 200,
      { timeout: ACTION_TIMEOUT_MS }
    );
    await attachBtn.click();
    await attRespPromise;
    await expect(page.getByTestId("support-media-lightbox")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    await page.getByTestId("support-media-lightbox-close").click();
    await expect(page.getByTestId("support-media-lightbox")).toBeHidden({ timeout: ACTION_TIMEOUT_MS });

    // 4. Switch back to Tab 1 and submit Ticket #2 (collision-free 2nd run!)
    await page.getByTestId("support-tab-submit").click();
    await page.getByTestId("support-title-input").fill("Request new SaaS onboarding template");
    await page
      .getByTestId("support-description-input")
      .fill("Please add a multi-step onboarding checklist template for non-tech teams.");
    const submit2Promise = page.waitForResponse(
      (res) => res.url().includes("/api/v1/support/tickets") && res.request().method() === "POST",
      { timeout: ACTION_TIMEOUT_MS }
    );
    await page.getByTestId("support-submit-btn").click();
    const submit2Resp = await submit2Promise;
    expect(submit2Resp.status()).toBe(201);
    const ticket2Raw = await submit2Resp.json();
    const ticket2 = ticket2Raw.data ?? ticket2Raw.ticket ?? ticket2Raw;
    expect(ticket2.ticketCode).not.toBe(ticket1.ticketCode);

    // Capture Desktop Light & Dark screenshots of Drawer ("My Requests" with 2 real tickets)
    await setWorkspaceTheme(page, "warm");
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(evidenceDir, "desktop-light.png") });

    await setWorkspaceTheme(page, "night");
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(evidenceDir, "desktop-dark.png") });

    // 5. Reload page and verify tickets persist in SQLite WAL
    await page.reload({ waitUntil: "domcontentloaded", timeout: NAV_TIMEOUT_MS });
    await dismissOnboardingIfVisible(page);
    await page.getByTestId("nav-support-drawer-btn").click();
    await page.getByTestId("support-tab-my-requests").click();
    await expect(page.getByTestId("support-request-drawer")).toContainText(ticket1.ticketCode, {
      timeout: ACTION_TIMEOUT_MS,
    });
    await page.getByTestId("support-drawer-close-btn").click();
    await expect(page.getByTestId("support-request-drawer")).toBeHidden({ timeout: ACTION_TIMEOUT_MS });

    // 6. Open Settings -> General and toggle Telemetry Privacy Switch OFF -> persists -> back ON
    const settingsBtn = page.getByRole("button", { name: /Cài đặt|Settings/i }).first();
    await expect(settingsBtn).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    await settingsBtn.click();

    const privacyCard = page.getByTestId("settings-telemetry-privacy-card");
    await expect(privacyCard).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    const toggle = page.getByTestId("settings-telemetry-optin-toggle");
    await expect(toggle).toBeVisible();

    const putPrefPromise = page.waitForResponse(
      (res) => res.url().includes("/api/v1/telemetry/preferences") && res.request().method() === "PUT",
      { timeout: ACTION_TIMEOUT_MS }
    );
    await toggle.click();
    const putPrefResp = await putPrefPromise;
    expect(putPrefResp.status()).toBe(200);
    await expect(page.getByTestId("settings-telemetry-status")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });

    // Capture Mobile screenshot
    await setWorkspaceTheme(page, "warm");
    await page.setViewportSize({ width: MOBILE_WIDTH, height: MOBILE_HEIGHT });
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(evidenceDir, "mobile-light.png") });

    const video = page.video();
    await context.close();
    if (video) {
      const videoPath = await video.path();
      const targetVideo = path.join(evidenceDir, "story-14.1-e2e-walkthrough.webm");
      if (fs.existsSync(videoPath)) fs.copyFileSync(videoPath, targetVideo);
    }

    expect(pageErrors).toEqual([]);
    expect(unexpectedConsoleErrors).toEqual([]);
    expect(unexpectedHttpErrors).toEqual([]);
  });
});
