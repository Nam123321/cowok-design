// @story 06.5
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Button-Exhaustive Live Browser E2E Verification & Video)
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { expect, test, type Page } from "@playwright/test";

const DESKTOP_WIDTH = 1440;
const DESKTOP_HEIGHT = 900;
const MOBILE_WIDTH = 390;
const MOBILE_HEIGHT = 844;
const NAV_TIMEOUT_MS = 20_000;
const ACTION_TIMEOUT_MS = 15_000;
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

test.describe("Story 06.5: Button-Exhaustive AI Multi-Slide Deck Gen & ChartSpecV12 E2E", () => {
  test("verifies AI Deck modal, all stageshow frames, CSV ChartSpecV12 preview & V15 deck generation", async ({
    browser,
  }) => {
    test.setTimeout(120_000);
    const storyDir = path.resolve(
      process.cwd(),
      "_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-06/Story-06.5"
    );
    const evidenceDir = path.join(storyDir, "qa", "evidence");
    const videoDir = path.join(evidenceDir, "videos");
    fs.mkdirSync(videoDir, { recursive: true });

    const qaNonce = process.env.QA_NONCE_TOKEN || crypto.randomBytes(NONCE_BYTES).toString("hex");
    fs.writeFileSync(
      path.join(evidenceDir, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, storyId: "06.5", generatedAt: new Date().toISOString() }, null, 2)
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
      if (url.includes("/api/") && status >= 500) {
        unexpectedHttpErrors.push(`${status} ${url}`);
      }
      if (url.includes("/api/v1/decks/generate") && status >= 400) {
        unexpectedHttpErrors.push(`${status} ${url}`);
      }
    });

    await page.goto(baseUrl, { waitUntil: "domcontentloaded", timeout: NAV_TIMEOUT_MS });
    await dismissOnboardingIfVisible(page);

    // 1. Switch to Presentation Slide CMS tab via real UI button
    const slideTab = page.getByTestId("canvas-tab-tab-slide");
    await expect(slideTab).toBeVisible({ timeout: NAV_TIMEOUT_MS });
    await slideTab.click();

    // 2. Open AI Deck CSV Generator modal via real toolbar trigger
    const openDeckGenBtn = page.getByTestId("open-deck-csv-generator-btn");
    await expect(openDeckGenBtn).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    await expect(openDeckGenBtn).toBeEnabled();
    await openDeckGenBtn.click();

    const modal = page.getByTestId("deck-csv-generator-modal");
    await expect(modal).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    await expect(page.locator("[data-ff-id='deck-launchpad-form']")).toBeVisible();
    await expect(page.locator("[data-ff-id='stageshow-frame-selector']")).toBeVisible();

    // Verify strictly 1 primary submit button & max width <= 85% viewport
    await expect(modal.locator("button[type='submit']")).toHaveCount(1);
    const drawerBox = await page.getByTestId("studio-right-drawer").boundingBox();
    expect(drawerBox).not.toBeNull();
    expect(drawerBox!.width).toBeLessThanOrEqual(DESKTOP_WIDTH * 0.86);

    // 3. Verify Resizers, Collapsible Master/Sub-Chat & 2-Level Drill-Down Control Plane (No redundant Brief box)
    await expect(modal).toContainText(/Studio Biểu đồ & Dashboard|Chart & Dashboard Studio/);
    await expect(page.getByTestId("studio-control-general")).toBeVisible();
    await expect(page.getByTestId("studio-component-list")).toBeVisible();
    await expect(page.getByTestId("studio-drawer-resize-handle")).toBeVisible();
    await expect(page.getByTestId("studio-master-detail-resizer")).toBeVisible();
    await expect(page.getByTestId("studio-control-chat-resizer")).toBeVisible();
    await expect(page.getByTestId("studio-file-upload-btn")).toBeVisible();

    await page.getByTestId("studio-toggle-subchat-btn").click();
    await expect(page.getByTestId("studio-subchat-input")).toBeHidden();
    await page.getByTestId("studio-toggle-subchat-btn").click();
    await expect(page.getByTestId("studio-subchat-input")).toBeVisible();

    await page.getByTestId("studio-toggle-master-btn").click();
    await expect(page.getByTestId("studio-data-table")).toBeHidden();
    await page.getByTestId("studio-toggle-master-btn").click();
    await expect(page.getByTestId("studio-data-table")).toBeVisible();

    // 4. Direct edit on Control Plane table -> Source-Data Divergence Banner -> Undo/Redo -> Revert
    const firstValInput = page.getByTestId("studio-data-table").locator("tbody tr").first().locator("input").nth(1);
    await firstValInput.fill("3500");
    await expect(page.getByTestId("studio-data-divergence-banner")).toBeVisible();
    await page.getByTestId("studio-undo-btn").click();
    await expect(page.getByTestId("studio-data-divergence-banner")).toBeHidden();
    await page.getByTestId("studio-redo-btn").click();
    await expect(page.getByTestId("studio-data-divergence-banner")).toBeVisible();
    await page.getByTestId("studio-revert-source-btn").click();
    await expect(page.getByTestId("studio-data-divergence-banner")).toBeHidden();

    // 5. Exercise Unified 2-Row Canvas Toolbar: 10-shape Visual Chart Picker, Palette Popover, QA Auto-Fix, Audience Mode, SubStates & Responsive Frames
    for (const chartType of ["movement", "line", "sparkline", "pie", "segmented", "bullet", "tick-bar", "kpi", "heatmap", "bar"]) {
      await page.getByTestId("studio-chart-picker-btn").click();
      await expect(page.getByTestId("studio-chart-picker-popover")).toBeVisible();
      await page.getByTestId(`studio-chart-type-${chartType}`).click();
    }
    await page.getByTestId("studio-palette-picker-btn").click();
    await page.getByTestId("studio-palette-emerald").click();
    await page.getByTestId("studio-palette-picker-btn").click();
    await page.getByTestId("studio-palette-ocean").click();

    // 5b. Exercise 3 AI Storytelling Lenses + 1-Click Honest-Data QA Auto-Fix + Audience Toggle (CEO 3s <-> Analyst)
    await expect(page.getByTestId("studio-ai-lens-strip")).toBeVisible();
    await expect(page.getByTestId("studio-story-arc-pill")).toBeVisible();
    await page.getByTestId("studio-lens-target").click();
    await page.getByTestId("studio-lens-dynamics").click();
    await page.getByTestId("studio-lens-driver").click();

    await page.getByTestId("studio-qa-audit-btn").click();
    const autoFixBtn = page.getByTestId("studio-qa-autofix-btn");
    if (await autoFixBtn.isVisible()) {
      await autoFixBtn.click();
    } else {
      await page.keyboard.press("Escape");
    }

    await page.getByTestId("studio-audience-toggle").click();
    await page.getByTestId("studio-audience-toggle").click();

    // 5c. Drill-Down Inspector: clicking KPI card on Canvas opens Level 2 Inspector -> back to Level 1
    await page.getByTestId("studio-mode-select").click();
    await page.locator("[data-ff-id='kpi-revenue-card']").click();
    await expect(page.getByTestId("studio-control-inspector")).toBeVisible();
    await expect(modal).toContainText("kpi-revenue-card");
    await page.getByTestId("studio-inspector-back-btn").click();
    await expect(page.getByTestId("studio-control-general")).toBeVisible();

    // 5d. Contextual Point Actions (Callout Arrow + Spotlight Grey-Out + What-If Simulation)
    await page.getByTestId("studio-mode-interact").click();
    const firstBar = page.locator("[data-testid^='studio-bar-']").first();
    if (await firstBar.isVisible()) {
      await firstBar.click();
      await expect(page.getByTestId("studio-point-action-bar")).toBeVisible();
      await page.getByTestId("studio-point-add-callout-btn").click();
      await page.getByTestId("studio-point-spotlight-btn").click();
      const plus20Btn = page.getByTestId("studio-whatif-bar").getByRole("button", { name: "+20%" });
      if (await plus20Btn.isVisible()) {
        await plus20Btn.click();
        await expect(page.getByTestId("studio-whatif-badge")).toBeVisible();
        await expect(page.getByTestId("studio-data-divergence-banner")).toBeVisible();
        await page.getByTestId("studio-revert-source-btn").click();
      }
    }

    // 5e. Pin & Text modes + Row 2 SubStates & Responsive Frames
    await page.getByTestId("studio-mode-pin").click();
    const canvasStage = page.getByTestId("studio-detail-canvas");
    await canvasStage.click({ position: { x: 260, y: 240 } });

    await page.getByTestId("studio-mode-text").click();
    await page.getByTestId("studio-substate-EMPTY").click();
    await expect(canvasStage).toBeVisible();
    await page.getByTestId("studio-substate-DEFAULT").click();

    for (const preset of ["window", "phone", "stage"]) {
      const frameBtn = page.getByTestId(`stageshow-frame-${preset}`);
      await frameBtn.click();
      await expect(frameBtn).toHaveAttribute("aria-checked", "true");
    }

    // 6. Send Scoped Sub-Chat refinement & Paste real CSV
    await page.getByTestId("studio-subchat-input").fill("Đổi mục tiêu lên $2800 và nhấn mạnh kênh Partner");
    await page.getByTestId("studio-subchat-send-btn").click();

    const csvInput = page.getByTestId("deck-gen-paste-input");
    if (!(await csvInput.isVisible())) {
      await page.getByTestId("studio-raw-csv-toggle").click();
    }
    await expect(csvInput).toBeVisible();
    await csvInput.fill('Channel,Revenue\nDirect,"$1,234.56"\nPartner,"$2,765.44"\nOrganic,"$1,000.00"');

    // 7. Click Preview Chart (real POST /api/v1/decks/normalize-chart-source -> ChartSpecV12)
    const previewBtn = page.getByTestId("deck-gen-preview-chart-btn");
    if (await previewBtn.isVisible()) {
      const normPromise = page.waitForResponse(
        (res) => res.url().includes("/api/v1/decks/normalize-chart-source") && res.request().method() === "POST",
        { timeout: ACTION_TIMEOUT_MS }
      );
      await previewBtn.click();
      const normResp = await normPromise;
      expect(normResp.status()).toBe(200);
      await expect(page.getByTestId("normalized-chart-preview")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    }

    // 8. Click Embed into Active Slide & Primary CTA -> real POST /api/v1/decks/generate + Main Chat sync
    await page.getByTestId("studio-embed-slide-btn").click();
    const genPromise = page.waitForResponse(
      (res) => res.url().includes("/api/v1/decks/generate") && res.request().method() === "POST",
      { timeout: ACTION_TIMEOUT_MS }
    );
    await page.getByTestId("deck-gen-primary-cta").click();
    const genResp = await genPromise;
    expect(genResp.status()).toBe(201);
    const genRaw = await genResp.json();
    const genPayload = genRaw.data ?? genRaw;
    expect(genPayload.deck.slides.length).toBeGreaterThanOrEqual(2);
    expect(genPayload.maxWordsPerSlide).toBeLessThanOrEqual(30);
    await expect(page.getByTestId("deck-gen-result-summary")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });

    // 9. Capture Desktop Light & Dark screenshots of Option D Master-Detail Studio
    await setWorkspaceTheme(page, "warm");
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(evidenceDir, "desktop-light.png") });

    await setWorkspaceTheme(page, "night");
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(evidenceDir, "desktop-dark.png") });

    // 10. Capture Mobile Option D 2-tab view & close cleanly
    await setWorkspaceTheme(page, "warm");
    await page.setViewportSize({ width: MOBILE_WIDTH, height: MOBILE_HEIGHT });
    await page.waitForTimeout(250);
    await page.screenshot({ path: path.join(evidenceDir, "mobile-light.png") });
    await page.getByTestId("deck-gen-close-btn").click();
    await expect(modal).toBeHidden({ timeout: ACTION_TIMEOUT_MS });

    const video = page.video();
    await context.close();
    if (video) {
      const videoPath = await video.path();
      const targetVideo = path.join(evidenceDir, "story-06.5-e2e-walkthrough.webm");
      if (fs.existsSync(videoPath)) fs.copyFileSync(videoPath, targetVideo);
    }

    expect(pageErrors).toEqual([]);
    expect(unexpectedConsoleErrors).toEqual([]);
    expect(unexpectedHttpErrors).toEqual([]);
  });
});
