// @story 05.5
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

test.describe("Story 05.5: Button-Exhaustive 3-Tier Interactive Flow DAG E2E", () => {
  test("verifies Synthesize 3-Tier Flow (x2 collision-free), Show Edge-Case Flows lens, and all 4 SubState pills", async ({
    browser,
  }) => {
    test.setTimeout(120_000);
    const storyDir = path.resolve(
      process.cwd(),
      "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-05/Story-05.5"
    );
    const evidenceDir = path.join(storyDir, "qa", "evidence");
    const videoDir = path.join(evidenceDir, "videos");
    fs.mkdirSync(videoDir, { recursive: true });

    const qaNonce = process.env.QA_NONCE_TOKEN || crypto.randomBytes(NONCE_BYTES).toString("hex");
    fs.writeFileSync(
      path.join(evidenceDir, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, storyId: "05.5", generatedAt: new Date().toISOString() }, null, 2)
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
    let synthesize200Count = 0;

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
      if (url.includes("/api/v1/flows/synthesize") && status === 200) {
        synthesize200Count += 1;
      }
      if (url.includes("/api/") && status >= 400) {
        unexpectedHttpErrors.push(`${status} ${url}`);
      }
    });

    await page.goto(baseUrl, { waitUntil: "domcontentloaded", timeout: NAV_TIMEOUT_MS });
    await dismissOnboardingIfVisible(page);

    // 1. Open Userflow DAG tab via real UI button
    const userflowTab = page.getByTestId("canvas-tab-tab-userflow");
    await expect(userflowTab).toBeVisible({ timeout: NAV_TIMEOUT_MS });
    await userflowTab.click();

    const lensBar = page.getByTestId("dag-edge-case-lens-bar");
    await expect(lensBar).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    const lensBox = await lensBar.boundingBox();
    expect(lensBox).not.toBeNull();
    expect(lensBox!.height).toBeLessThanOrEqual(48);
    const synthesizeBtn = page.getByTestId("synthesize-3tier-flow-btn");
    const edgeCaseToggle = page.getByTestId("show-edge-case-flows-toggle");
    const clusterBadge = page.getByTestId("dag-happy-path-cluster");
    await expect(synthesizeBtn).toBeVisible();
    await expect(edgeCaseToggle).toBeVisible();
    await expect(clusterBadge).toBeVisible();

    // 2. Click Synthesize 3-Tier Flow (Run #1) -> real POST /api/v1/flows/synthesize
    const synthResp1Promise = page.waitForResponse(
      (res) => res.url().includes("/api/v1/flows/synthesize") && res.request().method() === "POST",
      { timeout: ACTION_TIMEOUT_MS }
    );
    await synthesizeBtn.click();
    const synthResp1 = await synthResp1Promise;
    expect(synthResp1.status()).toBe(200);
    const synthJson1 = await synthResp1.json();
    expect(synthJson1.data.summary.primaryClusterCount).toBeLessThanOrEqual(8);
    expect(synthJson1.data.summary.substateCoveragePercent).toBe(100);
    expect(synthJson1.data.summary.floatLeakCount).toBe(0);
    await expect(page.getByTestId("dag-verdict-shield-badge")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });

    // Verify <= 8 visible nodes when Edge-Case lens is OFF
    await expect(edgeCaseToggle).toHaveAttribute("aria-checked", "false");
    const primaryVisibleNodes = await page.locator("[data-testid^='dag-node-']").filter({
      has: page.locator("[data-ff-id='dag-flow-node-substate-pills']"),
    }).count();
    expect(primaryVisibleNodes).toBeGreaterThan(0);
    expect(primaryVisibleNodes).toBeLessThanOrEqual(8);

    // 3. Click Synthesize 3-Tier Flow a 2nd time -> collision-free 200 OK (no 409!)
    const synthResp2Promise = page.waitForResponse(
      (res) => res.url().includes("/api/v1/flows/synthesize") && res.request().method() === "POST",
      { timeout: ACTION_TIMEOUT_MS }
    );
    await synthesizeBtn.click();
    const synthResp2 = await synthResp2Promise;
    expect(synthResp2.status()).toBe(200);
    expect(synthesize200Count).toBeGreaterThanOrEqual(2);

    // 4. Toggle [Show Edge-Case Flows] ON -> reveals FMEA nodes/badges
    await edgeCaseToggle.click();
    await expect(edgeCaseToggle).toHaveAttribute("aria-checked", "true");
    const expandedVisibleNodes = await page.locator("[data-ff-id='dag-flow-node-substate-pills']").count();
    expect(expandedVisibleNodes).toBeGreaterThan(primaryVisibleNodes);

    // 5. Click ALL 4 SubState pills (DEFAULT -> LOADING -> EMPTY -> ERROR) on first node
    const firstPillGroup = page.locator("[data-ff-id='dag-flow-node-substate-pills']").first();
    for (const sub of ["default", "loading", "empty", "error"]) {
      const pill = firstPillGroup.locator(`[data-testid^='dag-substate-${sub}-']`).first();
      await expect(pill).toBeVisible();
      await pill.click();
      await expect(pill).toHaveAttribute("aria-pressed", "true");
    }

    // 6. Capture Desktop Light screenshot (Edge-Case lens ON + 4-SubState pills active)
    await setWorkspaceTheme(page, "warm");
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(evidenceDir, "desktop-light.png") });

    // 7. Capture Desktop Dark screenshot
    await setWorkspaceTheme(page, "night");
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(evidenceDir, "desktop-dark.png") });

    // 7b. Verify single-row compact Preview toolbar (Stage dropdown + Tinh chinh popover) visible on Dual-Pane right stage
    const polishStrip = page.getByTestId("sketch-polish-strip");
    await expect(polishStrip).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    const stripBox = await polishStrip.boundingBox();
    expect(stripBox).not.toBeNull();
    expect(stripBox!.height).toBeLessThanOrEqual(44);
    await page.getByTestId("cognitive-phase-dropdown").click();
    await expect(page.getByTestId("phase-step-polish")).toBeVisible();
    await page.getByTestId("steering-chips-popover-btn").click();
    await page.keyboard.press("Escape");

    // 8. Toggle [Show Edge-Case Flows] back OFF & capture Mobile screenshot
    await edgeCaseToggle.click();
    await expect(edgeCaseToggle).toHaveAttribute("aria-checked", "false");
    await setWorkspaceTheme(page, "warm");
    await page.setViewportSize({ width: MOBILE_WIDTH, height: MOBILE_HEIGHT });
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(evidenceDir, "mobile-light.png") });

    const video = page.video();
    await context.close();
    if (video) {
      const videoPath = await video.path();
      const targetVideo = path.join(evidenceDir, "story-05.5-e2e-walkthrough.webm");
      if (fs.existsSync(videoPath)) fs.copyFileSync(videoPath, targetVideo);
    }

    expect(pageErrors).toEqual([]);
    expect(unexpectedConsoleErrors).toEqual([]);
    expect(unexpectedHttpErrors).toEqual([]);
  });
});
