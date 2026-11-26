// @story 13.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Live Browser 25-Case M1..M5 Certification + 3-Locale E2E with Video & 5 Screenshots)
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Locator, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-06. Release Certification & Anti-Goodhart Evaluation/Epic-13/Story-13.1"
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
const PR_TIER_SLA_MS = 15_000;
const HTTP_SERVER_ERROR = 500;
const MIN_SIM_CASES_PASS = 22;
const TOTAL_SIM_CASES = 25;
const MIN_ABLATION_BRIEFS = 12;
const MIN_FAULT_CATCH_PERCENT = 95;
const VIETNAMESE_DIACRITIC_RE =
  /[àáạảãâầấậẩẫăằắặẳẵèéẹẻẽêềếệểễìíịỉĩòóọỏõôồốộổỗơờớợởỡùúụủũưừứựửữỳýỵỷỹđÀÁẠẢÃÂẦẤẬẨẪĂẰẮẶẲẴÈÉẸẺẼÊỀẾỆỂỄÌÍỊỈĨÒÓỌỎÕÔỒỐỘỔỖƠỜỚỢỞỠÙÚỤỦŨƯỪỨỰỬỮỲÝỴỶỸĐ]/;
const JAPANESE_SCRIPT_RE = /[\u3040-\u30ff\u4e00-\u9faf]/;

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
  const skipBtn = page.getByRole("button", {
    name: /Bỏ qua \(Dùng mặc định\)|Skip|スキップ/i,
  });
  const appeared = await skipBtn
    .waitFor({ state: "visible", timeout: 2500 })
    .then(() => true)
    .catch(() => false);
  if (appeared) {
    await skipBtn.click();
    await expect(skipBtn).toBeHidden({ timeout: 5000 }).catch(() => undefined);
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

async function verifyLiveRustEvalContracts(
  page: Page,
  baseUrl: string,
  authHeaders: Record<string, string>
) {
  const startedAt = Date.now();
  const prRes = await page.request.post(new URL("/api/v1/eval/run", baseUrl).toString(), {
    headers: authHeaders,
    data: { tier: "pr", runSimulatedUsers: true },
  });
  const elapsedMs = Date.now() - startedAt;
  expect(prRes.ok()).toBeTruthy();
  expect(elapsedMs).toBeLessThan(PR_TIER_SLA_MS);

  const prJson = (await prRes.json()) as {
    ok: boolean;
    data: {
      tier: string;
      offlineEnforced: boolean;
      verdict: string;
      durationMs: number;
      estimatedCostUsd: number;
      simCasesPassed: number;
      simCasesBlocked: number;
      simCasesGap: number;
      simCasesTotal: number;
      securityGatesPassed: boolean;
      milestones: Array<{ milestone: string; securityGatePassed: boolean }>;
      simCases: Array<{ caseId: string; status: string; mandatorySecurityGate: boolean }>;
      ablation: {
        briefsEvaluated: number;
        dataStorytellingBriefsCount: number;
        liftPercent: number;
        regressionsCount: number;
        designCraftSemverPassed: boolean;
      };
      calibration: {
        providers: Array<{ providerId: string; floatLeaksCount: number; passed: boolean }>;
        totalFloatLeaks: number;
        passed: boolean;
      };
      faultInjection: {
        injectedCount: number;
        caughtCount: number;
        catchRatePercent: number;
        builderScorerIsolated: boolean;
        passed: boolean;
      };
    };
  };

  expect(prJson.ok).toBe(true);
  expect(prJson.data.tier).toBe("pr");
  expect(prJson.data.offlineEnforced).toBe(true);
  expect(prJson.data.durationMs).toBeLessThan(PR_TIER_SLA_MS);
  expect(prJson.data.estimatedCostUsd).toBe(0);
  expect(prJson.data.verdict).toBe("PASS");
  expect(prJson.data.simCasesTotal).toBe(TOTAL_SIM_CASES);
  expect(prJson.data.simCasesPassed).toBeGreaterThanOrEqual(MIN_SIM_CASES_PASS);
  expect(prJson.data.simCasesBlocked).toBe(0);
  expect(prJson.data.simCasesGap).toBe(0);
  expect(prJson.data.securityGatesPassed).toBe(true);
  expect(prJson.data.simCases).toHaveLength(TOTAL_SIM_CASES);
  expect(prJson.data.ablation.briefsEvaluated).toBeGreaterThanOrEqual(MIN_ABLATION_BRIEFS);
  expect(prJson.data.ablation.dataStorytellingBriefsCount).toBeGreaterThanOrEqual(3);
  expect(prJson.data.ablation.liftPercent).toBeGreaterThan(0);
  expect(prJson.data.ablation.regressionsCount).toBe(0);
  expect(prJson.data.ablation.designCraftSemverPassed).toBe(true);
  expect(prJson.data.calibration.providers).toHaveLength(3);
  expect(prJson.data.calibration.totalFloatLeaks).toBe(0);
  expect(prJson.data.calibration.passed).toBe(true);
  expect(prJson.data.faultInjection.catchRatePercent).toBeGreaterThanOrEqual(MIN_FAULT_CATCH_PERCENT);
  expect(prJson.data.faultInjection.builderScorerIsolated).toBe(true);

  const histRes = await page.request.get(new URL("/api/v1/eval/history?limit=10", baseUrl).toString(), {
    headers: authHeaders,
  });
  expect(histRes.ok()).toBeTruthy();
  const histJson = (await histRes.json()) as {
    ok: boolean;
    data: { sealedIntegrityOk: boolean; totalRuns: number };
  };
  expect(histJson.ok).toBe(true);
  expect(histJson.data.sealedIntegrityOk).toBe(true);
  expect(histJson.data.totalRuns).toBeGreaterThanOrEqual(1);
}

async function triggerEvalTierAndVerifyHistory(
  drawer: Locator,
  page: Page,
  tier: "pr" | "stage" | "release",
  expectedMinHistoryCount: number
) {
  await drawer.getByTestId(`eval-tier-${tier}`).click();
  const [runRes] = await Promise.all([
    page.waitForResponse(
      (r) => r.url().includes("/api/v1/eval/run") && r.request().method() === "POST"
    ),
    drawer.getByTestId("eval-run-trigger-btn").click(),
  ]);
  expect(runRes.ok()).toBeTruthy();
  await expect(drawer.getByTestId("eval-run-trigger-btn")).toBeEnabled({
    timeout: ACTION_TIMEOUT_MS,
  });
  await expect(drawer.getByTestId("eval-verdict-badge")).toBeVisible({
    timeout: ACTION_TIMEOUT_MS,
  });
  await drawer.getByTestId("eval-tab-history").click();
  const historyItems = drawer.locator('[data-testid^="eval-history-item-"]');
  await expect(historyItems).toHaveCount(expectedMinHistoryCount, {
    timeout: ACTION_TIMEOUT_MS,
  });
  await drawer.getByTestId("eval-tab-cases").click();
}

async function exerciseDrawerControls(drawer: Locator, page: Page) {
  // Capture initial history count before triggering pr -> stage -> release sequentially
  await drawer.getByTestId("eval-tab-history").click();
  const historyItems = drawer.locator('[data-testid^="eval-history-item-"]');
  const initialHistoryCount = await historyItems.count();
  await drawer.getByTestId("eval-tab-cases").click();

  // Trigger PR, Stage, and Release tiers sequentially with deterministic response synchronization
  await triggerEvalTierAndVerifyHistory(drawer, page, "pr", initialHistoryCount + 1);
  await triggerEvalTierAndVerifyHistory(drawer, page, "stage", initialHistoryCount + 2);
  await triggerEvalTierAndVerifyHistory(drawer, page, "release", initialHistoryCount + 3);
  await expect(drawer.getByTestId("eval-case-row-UC-25")).toBeVisible({
    timeout: ACTION_TIMEOUT_MS,
  });

  // Exercise M1..M5 milestone filter pills
  for (const ms of ["M1", "M2", "M3", "M4", "M5"] as const) {
    const pill = drawer.getByTestId(`eval-milestone-pill-${ms}`);
    await expect(pill).toBeVisible();
    await pill.click();
    await expect(pill).toHaveAttribute("aria-pressed", "true");
  }
  await drawer.getByTestId("eval-milestone-pill-ALL").click();

  // Exercise status dropdown filter (PASS -> FAIL -> ALL)
  const statusSelect = drawer.getByTestId("eval-case-status-select");
  await statusSelect.selectOption("PASS");
  await expect(drawer.getByTestId("eval-case-row-UC-01")).toBeVisible();
  await statusSelect.selectOption("FAIL");
  await expect(drawer.getByTestId("eval-case-row-UC-01")).toHaveCount(0);
  await statusSelect.selectOption("ALL");
  await expect(drawer.getByTestId("eval-case-row-UC-01")).toBeVisible();

  // Exercise search input & security-only toggle
  const searchInput = drawer.getByTestId("eval-case-search-input");
  await searchInput.fill("UC-14");
  await expect(drawer.getByTestId("eval-case-row-UC-14")).toBeVisible();
  await searchInput.fill("");

  const secToggle = drawer.getByTestId("eval-security-only-toggle");
  await secToggle.click();
  await expect(secToggle).toHaveAttribute("aria-pressed", "true");
  await expect(drawer.getByTestId("eval-case-row-UC-03")).toBeVisible();
  await secToggle.click();
  await expect(secToggle).toHaveAttribute("aria-pressed", "false");

  // Exercise Ablation & Calibration tab
  await drawer.getByTestId("eval-tab-ablation").click();
  await expect(drawer.getByTestId("eval-ablation-card")).toBeVisible();
  await expect(drawer.getByTestId("eval-calibration-card")).toBeVisible();
  await expect(drawer.getByTestId("eval-fault-injection-card")).toBeVisible();

  // Exercise Run History tab, Refresh button, and selecting an earlier historical run
  await drawer.getByTestId("eval-tab-history").click();
  await expect(drawer.getByTestId("eval-history-timeline")).toBeVisible();
  const [refreshRes] = await Promise.all([
    page.waitForResponse(
      (r) => r.url().includes("/api/v1/eval/history") && r.request().method() === "GET"
    ),
    drawer.getByTestId("eval-refresh-btn").click(),
  ]);
  expect(refreshRes.ok()).toBeTruthy();

  const olderRunItem = drawer.locator('[data-testid^="eval-history-item-"]').nth(1);
  await expect(olderRunItem).toBeVisible();
  await olderRunItem.click();
  await expect(drawer.getByTestId("eval-sim-cases-table")).toBeVisible();
}

async function verifyCertificationBtnAcrossCanvasTabs(page: Page) {
  const stageToolbar = page.getByTestId("contextual-stage-toolbar");

  await page.getByTestId("canvas-tab-tab-prd").click();
  await expect(stageToolbar).toHaveAttribute("data-tab-kind", "md");
  await expect(stageToolbar.getByTestId("nav-certification-btn")).toBeVisible();

  await page.getByTestId("canvas-tab-tab-code").click();
  await expect(stageToolbar).toHaveAttribute("data-tab-kind", "code");
  await expect(stageToolbar.getByTestId("nav-certification-btn")).toBeVisible();

  await page.getByTestId("canvas-tab-tab-userflow").click();
  await expect(stageToolbar).toHaveAttribute("data-tab-kind", "html");
  await expect(stageToolbar.getByTestId("nav-certification-btn")).toBeVisible();
}

async function verifyLocaleDrawer(
  page: Page,
  locale: "en-US" | "ja-JP",
  screenshotName: string
) {
  await page.setViewportSize({ width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT });
  await page.evaluate((nextLocale) => {
    const shortLocale = nextLocale.split("-")[0] || nextLocale;
    window.localStorage.setItem("cowok_locale", nextLocale);
    try {
      const raw = window.localStorage.getItem("cowok:workbench:settings");
      const parsed = raw ? (JSON.parse(raw) as Record<string, unknown>) : {};
      window.localStorage.setItem(
        "cowok:workbench:settings",
        JSON.stringify({ ...parsed, locale: shortLocale })
      );
    } catch {
      window.localStorage.removeItem("cowok:workbench:settings");
    }
  }, locale);
  await page.reload({ waitUntil: "domcontentloaded" });
  await expect(page.locator("#root")).not.toBeEmpty({ timeout: BOOT_TIMEOUT_MS });
  await dismissOnboardingIfVisible(page);
  await setWorkspaceTheme(page, "warm");

  const stageToolbar = page.getByTestId("contextual-stage-toolbar");
  await expect(stageToolbar).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
  await stageToolbar.getByTestId("nav-certification-btn").click();

  const drawer = page.getByTestId("certification-drawer");
  await expect(drawer).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
  await expect(drawer.getByTestId("eval-case-row-UC-01")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });

  const drawerText = await drawer.innerText();
  expect(drawerText).not.toMatch(VIETNAMESE_DIACRITIC_RE);
  if (locale === "ja-JP") {
    expect(drawerText).toMatch(JAPANESE_SCRIPT_RE);
  }
  expect(drawerText).not.toMatch(
    /SEALED_EVAL_TAMPER_ERROR|EVAL_ALREADY_RUNNING|Story 13\.1|V33|MPSC|chmod 0444|localStorage|CDP/
  );

  await page.screenshot({ path: path.join(EVIDENCE_DIR, screenshotName), fullPage: false });
  await drawer.getByTestId("eval-drawer-close-btn").click();
  await expect(drawer).toHaveCount(0);
}

test.describe("Story 13.1 Sealed Anti-Goodhart Benchmark & 25-Case M1..M5 Certification E2E", () => {
  test("verifies live CertificationDrawer, PR/Stage/Release tiers, 25 UC cases, and vi-VN/en-US/ja-JP locales with video & 5 screenshots", async ({
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

    // 1. Live Rust API Contract Verification (<15s $0 PR tier, 25 UC cases, >=12 briefs, 3-provider calibration, >=95% fault catch)
    await verifyLiveRustEvalContracts(page, baseUrl, authHeaders);
    await dismissOnboardingIfVisible(page);

    // 2. Verify nav-certification-btn across html, md, and code canvas tabs, then open CertificationDrawer
    const stageToolbar = page.getByTestId("contextual-stage-toolbar");
    await expect(stageToolbar).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
    await verifyCertificationBtnAcrossCanvasTabs(page);
    await stageToolbar.getByTestId("nav-certification-btn").click();

    const drawer = page.getByTestId("certification-drawer");
    await expect(drawer).toBeVisible({ timeout: BOOT_TIMEOUT_MS });
    await expect(drawer.getByTestId("eval-sealed-integrity-pill")).toBeVisible();
    await expect(drawer.getByTestId("eval-case-row-UC-25")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });

    // 3. Button-exhaustive interaction across all drawer controls + resize handle
    await exerciseDrawerControls(drawer, page);

    const resizeHandle = drawer.getByTestId("certification-drawer-resize-handle");
    await expect(resizeHandle).toBeVisible();
    const initialBox = await drawer.boundingBox();
    expect(initialBox).not.toBeNull();
    const handleBox = await resizeHandle.boundingBox();
    expect(handleBox).not.toBeNull();
    if (initialBox && handleBox) {
      await page.mouse.move(handleBox.x + handleBox.width / 2, handleBox.y + 180);
      await page.mouse.down();
      await page.mouse.move(handleBox.x - 140, handleBox.y + 180, { steps: 6 });
      await page.mouse.up();
      const resizedBox = await drawer.boundingBox();
      expect(resizedBox?.width ?? 0).toBeGreaterThan(initialBox.width + 80);
      await resizeHandle.dblclick();
      const resetBox = await drawer.boundingBox();
      expect(Math.abs((resetBox?.width ?? 0) - initialBox.width)).toBeLessThanOrEqual(2);
    }

    // 4. Capture vi-VN Desktop Light (compact), Desktop Dark (with M1–M5 Info Guide expanded), and Mobile 390px
    await setWorkspaceTheme(page, "warm");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-13.1-desktop-light.png"), fullPage: false });

    await setWorkspaceTheme(page, "night");
    await drawer.getByTestId("eval-milestone-info-toggle").click();
    await expect(drawer.getByTestId("eval-milestone-guide")).toBeVisible();
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-13.1-desktop-dark.png"), fullPage: false });
    await drawer.getByTestId("eval-milestone-guide-close").click();
    await expect(drawer.getByTestId("eval-milestone-guide")).toHaveCount(0);

    await page.setViewportSize({ width: MOBILE_WIDTH, height: MOBILE_HEIGHT });
    await page.waitForTimeout(THEME_SETTLE_MS);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-13.1-mobile-390.png"), fullPage: false });

    // Close drawer cleanly
    await drawer.getByTestId("eval-drawer-close-btn").click();
    await expect(drawer).toHaveCount(0);

    // 5. Multi-locale E2E verification (en-US & ja-JP)
    await verifyLocaleDrawer(page, "en-US", "story-13.1-desktop-en.png");
    await verifyLocaleDrawer(page, "ja-JP", "story-13.1-desktop-ja.png");

    expect(consoleErrors).toEqual([]);
    expect(pageErrors).toEqual([]);
    expect(serverErrors).toEqual([]);
    await context.close();
  });
});
