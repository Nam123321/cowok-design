// @story 10.3
// @implements AC-01, AC-02, AC-03, AC-04 (Full Button-to-Backend + Multi-Locale vi-VN/en-US/ja-JP E2E with Video & Screenshots)
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Locator, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-05. cowok & advisor OPC Execution Fleet, Universal MCP & 4-Channel Distribution/Epic-10/Story-10.3"
);
const EVIDENCE_DIR = path.join(STORY_DIR, "qa", "evidence");
const VIDEO_DIR = path.join(EVIDENCE_DIR, "videos");
const DESKTOP_WIDTH = 1440;
const DESKTOP_HEIGHT = 900;
const MOBILE_WIDTH = 390;
const MOBILE_HEIGHT = 844;
const THEME_SETTLE_MS = 300;
const BOOT_TIMEOUT_MS = 15_000;
const ACTION_TIMEOUT_MS = 25_000;
const HTTP_SERVER_ERROR = 500;
const MIN_FULL_VARIANTS = 4;
const VIETNAMESE_DIACRITIC_RE =
  /[àáạảãâầấậẩẫăằắặẳẵèéẹẻẽêềếệểễìíịỉĩòóọỏõôồốộổỗơờớợởỡùúụủũưừứựửữỳýỵỷỹđÀÁẠẢÃÂẦẤẬẨẪĂẰẮẶẲẴÈÉẸẺẼÊỀẾỆỂỄÌÍỊỈĨÒÓỌỎÕÔỒỐỘỔỖƠỜỚỢỞỠÙÚỤỦŨƯỪỨỰỬỮỲÝỴỶỸĐ]/;

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

async function dismissOnboardingIfVisible(page: Page) {
  const skipOnboardingBtn = page.getByRole("button", {
    name: /Bỏ qua \(Dùng mặc định\)|Skip|スキップ/i,
  });
  const appeared = await skipOnboardingBtn
    .waitFor({ state: "visible", timeout: 2500 })
    .then(() => true)
    .catch(() => false);
  if (appeared) {
    await skipOnboardingBtn.click();
    await expect(skipOnboardingBtn).toBeHidden({ timeout: 5000 }).catch(() => undefined);
  }
}

async function bootEditorSession(page: Page, baseUrl: string, qaNonce: string): Promise<string> {
  await page.setViewportSize({ width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT });
  await page.goto(baseUrl, { waitUntil: "domcontentloaded" });
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
  return sessionToken;
}

async function verifyRustTournamentApiContracts(
  page: Page,
  baseUrl: string,
  authHeaders: Record<string, string>
) {
  const projRes = await page.request.get(new URL("/api/v1/projects", baseUrl).toString(), {
    headers: authHeaders,
  });
  expect(projRes.ok()).toBeTruthy();
  const projJson = (await projRes.json()) as
    | Array<{ id: string }>
    | { projects?: Array<{ id: string }>; data?: Array<{ id: string }> };
  const projectList = Array.isArray(projJson) ? projJson : (projJson.projects ?? projJson.data ?? []);
  const projectId = projectList[0]?.id || "11111111-1111-4111-8111-111111111111";
  expect(projectId.length).toBeGreaterThan(0);

  const startRes = await page.request.post(new URL("/api/v1/tournament/start", baseUrl).toString(), {
    headers: authHeaders,
    data: { projectId, prompt: "Calm operations control room with fast executive KPI scan" },
  });
  expect(startRes.ok()).toBeTruthy();
  const startJson = (await startRes.json()) as {
    ok: boolean;
    data: { runId: string; variants: Array<{ id: string; displayLabel: string }> };
  };
  expect(startJson.ok).toBe(true);
  expect(startJson.data.variants.length).toBeGreaterThanOrEqual(2);
  expect(startJson.data.variants[0].displayLabel).toBe("Option A");

  const { runId } = startJson.data;
  const winnerId = startJson.data.variants[0].id;
  const donorId = startJson.data.variants[1].id;

  const selRes = await page.request.post(new URL("/api/v1/tournament/select", baseUrl).toString(), {
    headers: authHeaders,
    data: {
      projectId,
      runId,
      winningVariantId: winnerId,
      generationEpoch: 3,
      borrowFromVariantId: donorId,
      borrowSelectorOrInstruction: "ff-rail-navigation",
    },
  });
  expect(selRes.ok()).toBeTruthy();
  const selJson = (await selRes.json()) as {
    data: { duplicateFfIdsAfterMerge: number; generationEpoch: number };
  };
  expect(selJson.data.duplicateFfIdsAfterMerge).toBe(0);
  expect(selJson.data.generationEpoch).toBe(3);

  const staleRes = await page.request.post(new URL("/api/v1/tournament/select", baseUrl).toString(), {
    headers: authHeaders,
    data: { projectId, runId, winningVariantId: donorId, generationEpoch: 2 },
  });
  expect(staleRes.status()).toBe(409);

  const crystRes = await page.request.post(new URL("/api/v1/crystallize", baseUrl).toString(), {
    headers: authHeaders,
    data: { projectId, runId, variantId: winnerId, packageKind: "STARTER_TEMPLATE", name: "Ops Control Starter" },
  });
  expect(crystRes.ok()).toBeTruthy();

  const tamperRes = await page.request.post(new URL("/api/v1/advisor/evolve", baseUrl).toString(), {
    headers: authHeaders,
    data: { projectId, targetSkillPath: "../tests/eval/sealed/suite.json" },
  });
  expect(tamperRes.status()).toBe(403);

  const directPromoteRes = await page.request.post(new URL("/api/v1/advisor/evolve", baseUrl).toString(), {
    headers: authHeaders,
    data: { projectId, targetSkillPath: "skills/design-craft/SKILL.md", promoteDirectly: true },
  });
  expect(directPromoteRes.status()).toBe(403);
}

async function exerciseAllViewportAndCompareControls(drawer: Locator, page: Page) {
  const sliderSection = drawer.locator("[data-ff-id='tournament-blind-wipe-slider']");
  const viewportChecks: ReadonlyArray<[string, string]> = [
    ["tournament-viewport-desktop", "desktop-1440"],
    ["tournament-viewport-tablet", "tablet-768"],
    ["tournament-viewport-fold-inner", "fold-inner-712"],
    ["tournament-viewport-mobile", "mobile-390"],
    ["tournament-viewport-fold-outer", "fold-outer-344"],
    ["tournament-viewport-desktop", "desktop-1440"],
  ];
  for (const [testId, expectedMode] of viewportChecks) {
    const btn = drawer.getByTestId(testId);
    await expect(btn).toBeVisible();
    await btn.click();
    await expect(sliderSection).toHaveAttribute("data-viewport-mode", expectedMode);
  }

  await expect(drawer.getByTestId("tournament-compare-mode-switcher")).toBeVisible();
  await drawer.getByTestId("compare-mode-side-by-side").click();
  await expect(drawer.getByTestId("compare-stage-side-by-side")).toBeVisible();

  await drawer.getByTestId("compare-mode-grid").click();
  await expect(drawer.getByTestId("compare-stage-grid")).toBeVisible();
  const secondGridCardBtn = drawer.locator("[data-testid^='compare-grid-select-']").nth(1);
  await expect(secondGridCardBtn).toBeVisible();
  await secondGridCardBtn.click();
  await expect(secondGridCardBtn).toHaveAttribute("aria-pressed", "true");

  await drawer.getByTestId("compare-mode-onion").click();
  const onionSlider = drawer.getByTestId("onion-opacity-range-input");
  await expect(onionSlider).toBeVisible();
  await onionSlider.fill("35");
  await expect(drawer.getByTestId("onion-opacity-value")).toHaveText("35%");

  await drawer.getByTestId("compare-mode-wipe").click();
  const wipeSlider = page.getByTestId("blind-wipe-range-input");
  await expect(wipeSlider).toBeVisible();
  await wipeSlider.fill("68");
}

async function verifyLocaleDrawer(page: Page, locale: "en-US" | "ja-JP", screenshotName: string) {
  await page.setViewportSize({ width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT });
  await page.evaluate((nextLocale) => {
    window.localStorage.setItem("cowok_locale", nextLocale);
  }, locale);
  await page.reload({ waitUntil: "domcontentloaded" });
  await expect(page.locator("#root")).not.toBeEmpty({ timeout: BOOT_TIMEOUT_MS });
  await dismissOnboardingIfVisible(page);
  await setWorkspaceTheme(page, "warm");

  const stageToolbar = page.getByTestId("contextual-stage-toolbar");
  await expect(stageToolbar).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
  await stageToolbar.getByTestId("nav-tournament-btn").click();

  const drawer = page.getByTestId("design-tournament-studio-drawer");
  await expect(drawer).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
  const quickStartBtn = drawer.getByTestId("tournament-quick-start-btn");
  await expect(quickStartBtn).toBeEnabled({ timeout: BOOT_TIMEOUT_MS });
  await quickStartBtn.click();

  await expect(page.getByTestId("blind-wipe-range-input")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
  const drawerText = await drawer.innerText();
  expect(drawerText).not.toMatch(VIETNAMESE_DIACRITIC_RE);

  await page.screenshot({ path: path.join(EVIDENCE_DIR, screenshotName), fullPage: false });
}

test.describe("Story 10.3 Anti-Slop Design Tournament, Cross-Variant Borrow & Multi-Locale E2E", () => {
  test("verifies full button-to-backend tournament flows across vi-VN, en-US, and ja-JP with video & screenshots", async ({
    browser,
  }) => {
    test.setTimeout(180_000);
    fs.mkdirSync(VIDEO_DIR, { recursive: true });
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    const qaNonce = process.env.WATCHMEN_NONCE || crypto.randomBytes(16).toString("hex");

    const context = await browser.newContext({
      viewport: { width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT },
      recordVideo: { dir: VIDEO_DIR, size: { width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT } },
    });
    const page = await context.newPage();

    const consoleErrors: string[] = [];
    const pageErrors: string[] = [];
    const serverErrors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        const text = msg.text();
        if (text.includes("Blocked script execution in 'about:srcdoc' because the document's frame is sandboxed")) {
          return;
        }
        consoleErrors.push(text);
      }
    });
    page.on("pageerror", (err) => pageErrors.push(String(err)));
    page.on("response", (res) => {
      if (res.status() >= HTTP_SERVER_ERROR) {
        serverErrors.push(`${res.status()} ${res.url()}`);
      }
    });

    const sessionToken = await bootEditorSession(page, baseUrl, qaNonce);
    const authHeaders = {
      "Content-Type": "application/json",
      Authorization: `Bearer ${sessionToken}`,
      "X-Cowok-Session-Token": sessionToken,
      "x-iwish-qa-nonce": qaNonce,
      "x-tenant-id": "local-default",
    };

    await verifyRustTournamentApiContracts(page, baseUrl, authHeaders);
    await dismissOnboardingIfVisible(page);

    // Verify 2-Tier Scope Separation (Global TopNav vs Tab-Local Contextual Toolbar)
    const topNav = page.getByTestId("workbench-top-nav");
    await expect(topNav).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
    await expect(topNav.getByTestId("nav-tournament-btn")).toHaveCount(0);
    await expect(topNav.getByTestId("nav-visual-qc-btn")).toHaveCount(0);

    const stageToolbar = page.getByTestId("contextual-stage-toolbar");
    await expect(stageToolbar).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
    await expect(stageToolbar).toHaveAttribute("data-tab-kind", "html");
    await page.getByTestId("canvas-tab-tab-prd").click();
    await expect(stageToolbar).toHaveAttribute("data-tab-kind", "md");
    await expect(stageToolbar.getByTestId("nav-tournament-btn")).toBeVisible();
    await page.getByTestId("canvas-tab-tab-code").click();
    await expect(stageToolbar).toHaveAttribute("data-tab-kind", "code");
    await expect(stageToolbar.getByTestId("nav-tournament-btn")).toHaveCount(0);
    await page.getByTestId("canvas-tab-tab-userflow").click();
    await expect(stageToolbar).toHaveAttribute("data-tab-kind", "html");

    // 1. Open DesignTournamentStudioDrawer from ContextualStageToolbar
    await stageToolbar.getByTestId("nav-tournament-btn").click();
    const drawer = page.getByTestId("design-tournament-studio-drawer");
    await expect(drawer).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
    await expect(drawer.getByTestId("tournament-smart-launcher")).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
    await expect(drawer.getByTestId("tournament-prompt-input")).toHaveCount(0);
    await expect(drawer.getByTestId("tournament-launch-btn")).toHaveCount(0);

    // 2. Click Quick Start -> real Rust POST /api/v1/tournament/start -> 4 variants + AST % badge + no blindNotice
    const quickStartBtn = drawer.getByTestId("tournament-quick-start-btn");
    await expect(quickStartBtn).toBeEnabled({ timeout: BOOT_TIMEOUT_MS });
    await quickStartBtn.click();
    await expect(page.getByTestId("blind-wipe-range-input")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    await expect(drawer).not.toContainText("Ẩn tên mô hình");

    const optionTabs = drawer.locator("[data-testid^='tournament-option-tab-']");
    await expect(optionTabs).toHaveCount(MIN_FULL_VARIANTS, { timeout: ACTION_TIMEOUT_MS });
    const firstDiversityBadge = drawer.locator("[data-testid^='tournament-diversity-badge-']").first();
    await expect(firstDiversityBadge).toHaveText(/^(Khác biệt|Trùng lặp) \d+%$/);
    await expect(firstDiversityBadge).toHaveAttribute("title", /AST Jaccard/);

    // 3. Click Option A, B, C, D tabs
    for (let i = 0; i < MIN_FULL_VARIANTS; i += 1) {
      await optionTabs.nth(i).click();
      await expect(optionTabs.nth(i)).toHaveAttribute("aria-selected", "true");
    }

    // 4 & 5. Click all 5 viewports and all 4 comparison modes (side-by-side, grid + select card, onion + opacity, wipe + slider)
    await exerciseAllViewportAndCompareControls(drawer, page);

    // 6. Full Visual Zone Picker Cross-Variant Borrow + Commit + Sub-Chat Shortcut flow
    await optionTabs.nth(1).click();
    await expect(optionTabs.nth(1)).toHaveAttribute("aria-selected", "true");
    const donorSelect = drawer.getByTestId("cross-variant-donor-select");
    const donorOptionA = donorSelect.locator("option", { hasText: /Option A/ }).first();
    const donorValue = await donorOptionA.getAttribute("value");
    expect(donorValue).toBeTruthy();
    await donorSelect.selectOption(donorValue!);

    await drawer.getByTestId("cross-variant-open-zone-picker-btn").click();
    const zoneModal = page.getByTestId("cross-variant-zone-picker-modal");
    await expect(zoneModal).toBeVisible();
    const zoneCards = zoneModal.locator("[data-testid^='zone-picker-card-']");
    const cardCount = await zoneCards.count();
    expect(cardCount).toBeGreaterThanOrEqual(2);
    await zoneCards.nth(0).click();
    await zoneCards.nth(1).click();

    await setWorkspaceTheme(page, "warm");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "desktop-zone-picker.png"), fullPage: false });

    await zoneModal.getByTestId("zone-picker-apply-btn").click();
    await expect(zoneModal).toHaveCount(0);
    await expect(drawer.locator("[data-testid^='cross-variant-selected-chip-']")).toHaveCount(2);

    await drawer.getByTestId("tournament-confirm-winner-btn").click();
    await expect(drawer.getByTestId("tournament-commit-receipt")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });

    await drawer.getByTestId("cross-variant-chat-suggest-btn").click();
    await expect(drawer.getByTestId("studio-subchat-input")).toBeFocused();

    // 7. Full Crystallize Popover flow (open -> select kind -> fill name -> submit -> real Rust POST /crystallize)
    const crystallizeTrigger = drawer.getByTestId("tournament-crystallize-trigger-btn");
    await expect(crystallizeTrigger).toBeEnabled();
    await crystallizeTrigger.click();
    await expect(drawer.getByTestId("tournament-crystallize-popover")).toBeVisible();
    await drawer.getByTestId("crystallize-kind-select").selectOption("BRAND_KIT");
    await drawer.getByTestId("crystallize-name-input").fill("Checkout Brand Starter");
    await drawer.getByTestId("crystallize-action-button").click();
    await expect(drawer.getByTestId("tournament-crystallize-receipt")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    await expect(drawer.getByTestId("tournament-crystallize-receipt")).toContainText("Checkout Brand Starter");
    await expect(drawer.getByTestId("tournament-crystallize-popover")).toHaveCount(0);

    // 8. Full Sub-Chat refinement flow -> real Rust refinement round & Sub-Chat reply
    const subChatMessages = drawer.getByTestId("studio-subchat-messages");
    const subChatInput = drawer.getByTestId("studio-subchat-input");
    await subChatInput.fill("Refined minimal checkout dashboard layout");
    await drawer.getByTestId("studio-subchat-send-btn").click();
    await expect(subChatMessages).toContainText("Refined minimal checkout dashboard layout", { timeout: ACTION_TIMEOUT_MS });
    await expect(subChatMessages).toContainText("Đã tạo vòng so sánh mới", { timeout: ACTION_TIMEOUT_MS });

    // 9. Left-pane collapse & expand toggle
    const toggleMasterBtn = drawer.getByTestId("studio-toggle-master-btn");
    await toggleMasterBtn.click();
    await expect(drawer.getByTestId("studio-master-pane")).toHaveCount(0);
    await toggleMasterBtn.click();
    await expect(drawer.getByTestId("studio-master-pane")).toBeVisible();

    // Capture vi-VN Desktop Light, Desktop Dark, and Mobile 390px
    await setWorkspaceTheme(page, "warm");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "desktop-light.png"), fullPage: false });

    await setWorkspaceTheme(page, "night");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "desktop-dark.png"), fullPage: false });

    await page.setViewportSize({ width: MOBILE_WIDTH, height: MOBILE_HEIGHT });
    await page.waitForTimeout(THEME_SETTLE_MS);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "mobile-390.png"), fullPage: false });

    // 10. Multi-locale E2E verification (en-US & ja-JP) with ZERO Vietnamese diacritics check
    await verifyLocaleDrawer(page, "en-US", "desktop-en.png");
    await verifyLocaleDrawer(page, "ja-JP", "desktop-ja.png");

    expect(consoleErrors).toEqual([]);
    expect(pageErrors).toEqual([]);
    expect(serverErrors).toEqual([]);
    await context.close();
  });
});
