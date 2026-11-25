// @story 07.2
// @implements AC-01, AC-02, AC-03, AC-04 (Full Button-to-Backend + Multi-Locale Live Browser E2E)
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Browser, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-07/Story-07.2"
);
const EVIDENCE_DIR = path.join(STORY_DIR, "qa", "evidence");
const DEFAULT_PROJECT_ID = "11111111-1111-4111-8111-111111111111";
const SHA256_HEX_RE = /^[0-9a-f]{64}$/i;
const VI_DIACRITICS_RE =
  /[àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ]/i;
const JA_SCRIPT_RE = /[\u3040-\u30ff\u4e00-\u9faf]/;

async function dismissOnboardingWizard(page: Page) {
  const wizardHeading = page.getByRole("heading", {
    name: /Chào mừng đến Cowok-Design|Welcome to Cowok-Design|Cowok-Design/i,
  });
  const shown = await wizardHeading
    .waitFor({ state: "visible", timeout: 2500 })
    .then(() => true)
    .catch(() => false);
  if (shown) {
    const nextBtn = page.getByRole("button", { name: /Tiếp tục|Next|次へ/i });
    if (await nextBtn.isVisible().catch(() => false)) {
      await nextBtn.click();
    }
    if (await nextBtn.isVisible().catch(() => false)) {
      await nextBtn.click();
    }
    const finishBtn = page.getByRole("button", { name: /Hoàn tất|Finish|完了/i });
    if (await finishBtn.isVisible().catch(() => false)) {
      await finishBtn.click();
    }
    await expect(wizardHeading).toBeHidden({ timeout: 10000 });
  }
}

async function bootEditorSession(page: Page, baseUrl: string, qaNonce: string): Promise<string> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto(baseUrl, { waitUntil: "networkidle" });
  await expect(page.locator("#root")).not.toBeEmpty({ timeout: 15000 });

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

async function applyTheme(page: Page, theme: "warm" | "night") {
  await page.evaluate((nextTheme) => {
    document.documentElement.setAttribute("data-theme", nextTheme);
    const shell = document.querySelector(".workbench-shell");
    if (shell) {
      shell.setAttribute("data-theme", nextTheme);
      shell.classList.remove("theme-warm", "theme-dim", "theme-night");
      shell.classList.add(`theme-${nextTheme}`);
    }
  }, theme);
  await page.waitForTimeout(250);
}

async function openExportBundleModalViaRealTrigger(page: Page) {
  const wideExportBtn = page.getByTestId("toolbar-export-btn");
  if (await wideExportBtn.isVisible().catch(() => false)) {
    await wideExportBtn.click();
    const zipItem = page.getByTestId("toolbar-export-zip");
    await expect(zipItem).toBeVisible({ timeout: 5000 });
    await zipItem.click();
  } else {
    const moreBtn = page.getByTestId("toolbar-more-actions-btn");
    if (await moreBtn.isVisible().catch(() => false)) {
      await moreBtn.click();
      const overflowZip = page.getByTestId("overflow-export-zip");
      await expect(overflowZip).toBeVisible({ timeout: 5000 });
      await overflowZip.click();
    } else {
      const historyBtn = page.getByTestId("toggle-version-history-btn").first();
      await expect(historyBtn).toBeVisible({ timeout: 8000 });
      await historyBtn.click();
      const openModalBtn = page.getByTestId("open-export-bundle-modal-btn");
      await expect(openModalBtn).toBeVisible({ timeout: 5000 });
      await openModalBtn.click();
    }
  }

  const modal = page.getByTestId("export-bundle-modal");
  await expect(modal).toBeVisible({ timeout: 10000 });
  return modal;
}

async function ensureQcOverrideCheckedIfBlocked(page: Page) {
  const overrideCheckbox = page.getByTestId("qc-override-checkbox");
  if (await overrideCheckbox.isVisible().catch(() => false)) {
    if (!(await overrideCheckbox.isChecked())) {
      await overrideCheckbox.check();
    }
  }
}

async function verifyOfflineHtmlInAirgappedContext(
  browser: Browser,
  htmlContent: string,
  expectedSlideAdvance: boolean
) {
  const offlineContext = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  await offlineContext.setOffline(true);
  const offlinePage = await offlineContext.newPage();
  const externalNetworkRequests: string[] = [];
  offlinePage.on("request", (req) => {
    const reqUrl = req.url();
    if (reqUrl.startsWith("http://") || reqUrl.startsWith("https://")) {
      externalNetworkRequests.push(reqUrl);
    }
  });

  await offlinePage.setContent(htmlContent, { waitUntil: "domcontentloaded" });
  await expect(offlinePage.locator("html")).toHaveAttribute("data-stageshow-index", "1");
  if (expectedSlideAdvance) {
    await expect(offlinePage.locator("svg[viewBox='0 0 120 60']")).toBeVisible();
    await offlinePage.keyboard.press("ArrowRight");
    await expect(offlinePage.locator("html")).toHaveAttribute("data-stageshow-index", "2");
  }
  expect(externalNetworkRequests).toEqual([]);
  await offlineContext.close();
}

test.describe("Story 07.2 Split Export & ContextualStageToolbar (Full Button-to-Backend + Multi-Locale E2E)", () => {
  test("verifies ContextualStageToolbar across .html/.md/.tsx tabs, all ExportBundleModal buttons against Rust backend, offline HTML airgap, and vi-VN/en-US/ja-JP locales", async ({
    browser,
  }) => {
    test.setTimeout(120_000);
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "3000";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    const qaNonce =
      process.env.WATCHMEN_NONCE ||
      process.env.QA_NONCE_TOKEN ||
      crypto.randomBytes(32).toString("hex");

    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      recordVideo: { dir: EVIDENCE_DIR, size: { width: 1280, height: 800 } },
    });
    const page = await context.newPage();
    const consoleErrors: string[] = [];
    const pageErrors: string[] = [];
    const serverErrors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error" && !msg.text().includes("favicon.ico")) {
        consoleErrors.push(msg.text());
      }
    });
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("response", (res) => {
      if (res.url().includes("/api/") && res.status() >= 500) {
        serverErrors.push(`${res.status()} ${res.url()}`);
      }
    });

    const sessionToken = await bootEditorSession(page, baseUrl, qaNonce);
    const authedHeaders = {
      "Content-Type": "application/json",
      Authorization: `Bearer ${sessionToken}`,
      "X-Cowok-Session-Token": sessionToken,
      "x-iwish-qa-nonce": qaNonce,
      "x-tenant-id": "local-default",
    };

    // =========================================================================
    // 1. ContextualStageToolbar across .html, .md, and .tsx tabs + real modals
    // =========================================================================
    const topNav = page.getByTestId("workbench-top-nav");
    await expect(topNav).toBeVisible({ timeout: 10000 });
    await expect(topNav.getByTestId("nav-plugin-center")).toBeVisible();
    await expect(topNav.getByTestId("nav-support-drawer-btn")).toBeVisible();
    await expect(topNav.getByTestId("nav-visual-qc-btn")).toHaveCount(0);
    await expect(topNav.getByTestId("nav-tournament-btn")).toHaveCount(0);

    const stageToolbar = page.getByTestId("contextual-stage-toolbar");
    await expect(stageToolbar).toBeVisible({ timeout: 10000 });
    await expect(stageToolbar).toHaveAttribute("data-tab-kind", "html");

    // 1a. .html tab -> click Kiểm định QC -> opens ProjectReadinessModal -> close
    const htmlQcBtn = stageToolbar.getByTestId("nav-visual-qc-btn");
    await expect(htmlQcBtn).toBeVisible();
    await htmlQcBtn.click();
    const readinessPanel = page.getByTestId("project-readiness-panel");
    await expect(readinessPanel).toBeVisible({ timeout: 8000 });
    await page.keyboard.press("Escape");
    await expect(readinessPanel).toBeHidden({ timeout: 8000 });

    // 1b. .html tab -> click Đấu trường Thiết kế -> opens DesignTournamentStudioDrawer -> close
    const htmlTournamentBtn = stageToolbar.getByTestId("nav-tournament-btn");
    await expect(htmlTournamentBtn).toBeVisible();
    await htmlTournamentBtn.click();
    const tournamentDrawer = page.getByTestId("design-tournament-studio-drawer");
    await expect(tournamentDrawer).toBeVisible({ timeout: 8000 });
    await page.getByTestId("tournament-drawer-close-btn").click();
    await expect(tournamentDrawer).toBeHidden({ timeout: 8000 });

    // 1c. Switch to .md tab (DESIGN.md) -> verify adapted toolbar & click Tạo Đấu trường từ Spec
    await page.getByTestId("canvas-tab-tab-prd").click();
    await expect(stageToolbar).toHaveAttribute("data-tab-kind", "md");
    await expect(stageToolbar.getByTestId("contextual-stage-badge")).toBeVisible();
    await expect(stageToolbar.getByTestId("nav-visual-qc-btn")).toHaveCount(0);
    const mdTournamentBtn = stageToolbar.getByTestId("nav-tournament-btn");
    await expect(mdTournamentBtn).toBeVisible();
    await mdTournamentBtn.click();
    await expect(tournamentDrawer).toBeVisible({ timeout: 8000 });
    await page.getByTestId("tournament-drawer-close-btn").click();
    await expect(tournamentDrawer).toBeHidden({ timeout: 8000 });

    // 1d. Switch to .tsx tab (CheckoutForm.tsx) -> verify adapted toolbar & click Kiểm tra Token (QC)
    await page.getByTestId("canvas-tab-tab-code").click();
    await expect(stageToolbar).toHaveAttribute("data-tab-kind", "code");
    await expect(stageToolbar.getByTestId("contextual-stage-badge")).toBeVisible();
    await expect(stageToolbar.getByTestId("nav-tournament-btn")).toHaveCount(0);
    const codeQcBtn = stageToolbar.getByTestId("nav-visual-qc-btn");
    await expect(codeQcBtn).toBeVisible();
    await codeQcBtn.click();
    await expect(readinessPanel).toBeVisible({ timeout: 8000 });
    await page.keyboard.press("Escape");
    await expect(readinessPanel).toBeHidden({ timeout: 8000 });

    // Switch back to .html tab before opening ExportBundleModal
    await page.getByTestId("canvas-tab-tab-userflow").click();
    await expect(stageToolbar).toHaveAttribute("data-tab-kind", "html");

    // =========================================================================
    // 2. Open ExportBundleModal via real trigger -> Tab 1 (Share & Present)
    // =========================================================================
    await openExportBundleModalViaRealTrigger(page);
    const shareTab = page.getByTestId("share-present-tab");
    await shareTab.click();
    await expect(shareTab).toHaveAttribute("aria-selected", "true");
    await expect(page.getByTestId("share-present-panel")).toBeVisible();
    await expect(page.getByTestId("qc-gate-banner-card")).toBeVisible({ timeout: 8000 });
    await ensureQcOverrideCheckedIfBlocked(page);

    // 2a. Click Offline HTML (.html) -> verify real Rust response (0 CDN refs & SHA-256)
    const htmlExportBtn = page.getByTestId("export-offline-html-btn");
    await expect(htmlExportBtn).toBeEnabled();
    const [htmlApiResponse] = await Promise.all([
      page.waitForResponse(
        (res) => res.url().includes("/api/v1/export/html-bundle") && res.request().method() === "POST"
      ),
      htmlExportBtn.click(),
    ]);
    if (htmlApiResponse.status() === 409) {
      await ensureQcOverrideCheckedIfBlocked(page);
      const [retryRes] = await Promise.all([
        page.waitForResponse(
          (res) => res.url().includes("/api/v1/export/html-bundle") && res.request().method() === "POST"
        ),
        htmlExportBtn.click(),
      ]);
      expect(retryRes.status()).toBe(200);
      const retryBody = (await retryRes.json()) as {
        ok: boolean;
        data: { externalCdnRefsCount: number; sha256: string; presenterEnabled: boolean; htmlContent: string };
      };
      expect(retryBody.ok).toBe(true);
      expect(retryBody.data.externalCdnRefsCount).toBe(0);
      expect(retryBody.data.sha256).toMatch(SHA256_HEX_RE);
      expect(retryBody.data.presenterEnabled).toBe(true);
      await verifyOfflineHtmlInAirgappedContext(browser, retryBody.data.htmlContent, false);
    } else {
      expect(htmlApiResponse.status()).toBe(200);
      const htmlBody = (await htmlApiResponse.json()) as {
        ok: boolean;
        data: { externalCdnRefsCount: number; sha256: string; presenterEnabled: boolean; htmlContent: string };
      };
      expect(htmlBody.ok).toBe(true);
      expect(htmlBody.data.externalCdnRefsCount).toBe(0);
      expect(htmlBody.data.sha256).toMatch(SHA256_HEX_RE);
      expect(htmlBody.data.presenterEnabled).toBe(true);
      await verifyOfflineHtmlInAirgappedContext(browser, htmlBody.data.htmlContent, false);
    }
    await expect(page.getByTestId("offline-html-receipt")).toBeVisible({ timeout: 8000 });

    // 2b. Click Vector PDF 16:9 (.pdf) -> verify real PDF response
    const pdfExportBtn = page.getByTestId("export-vector-pdf-btn");
    await expect(pdfExportBtn).toBeEnabled();
    const [pdfApiResponse] = await Promise.all([
      page.waitForResponse(
        (res) => res.url().includes("/api/v1/export/vector-pdf") && res.request().method() === "POST"
      ),
      pdfExportBtn.click(),
    ]);
    expect(pdfApiResponse.status()).toBe(200);
    const pdfBody = (await pdfApiResponse.json()) as {
      data: { pdfBase64: string; engine: string; pageCount: number };
    };
    expect(pdfBody.data.engine).toBe("html_forge_vector");
    expect(pdfBody.data.pageCount).toBeGreaterThanOrEqual(1);
    expect(pdfBody.data.pdfBase64.startsWith("JVBERi0xLjQK")).toBe(true);
    await expect(page.getByTestId("vector-pdf-receipt")).toBeVisible({ timeout: 8000 });

    // 2c. Click PowerPoint (.pptx) -> verify plugin-isolated behavior & non-dead CTA
    const pptxExportBtn = page.getByTestId("export-pptx-btn");
    await expect(pptxExportBtn).toBeEnabled();
    const [pptxApiResponse] = await Promise.all([
      page.waitForResponse(
        (res) => res.url().includes("/api/v1/export/pptx") && res.request().method() === "POST"
      ),
      pptxExportBtn.click(),
    ]);
    expect(pptxApiResponse.status()).toBe(200);
    const pptxBody = (await pptxApiResponse.json()) as {
      pluginInstalled: boolean;
      pluginId: string;
      installAction: string;
    };
    expect(pptxBody.pluginId).toBe("pptx-deck-exporter-plugin");
    if (!pptxBody.pluginInstalled) {
      await expect(page.getByTestId("pptx-plugin-install-guide")).toBeVisible({ timeout: 8000 });
      const installCta = page.getByTestId("export-pptx-plugin-cta");
      await expect(installCta).toBeVisible();
      await expect(installCta).toHaveAttribute("href", "/plugins?install=pptx-deck-exporter-plugin");
    } else {
      await expect(page.getByTestId("pptx-dispatched-receipt")).toBeVisible({ timeout: 8000 });
    }

    // Capture Desktop Light & Dark screenshots with Share & Present receipts visible
    await applyTheme(page, "warm");
    await page.screenshot({
      path: path.join(EVIDENCE_DIR, "story-07.2-desktop-light.png"),
      fullPage: false,
    });
    await applyTheme(page, "night");
    await page.screenshot({
      path: path.join(EVIDENCE_DIR, "story-07.2-desktop-dark.png"),
      fullPage: false,
    });
    await applyTheme(page, "warm");

    // =========================================================================
    // 3. Switch to Tab 2 (Developer Handoff) -> click Download Dev Handoff
    // =========================================================================
    const devTab = page.getByTestId("developer-handoff-tab");
    await devTab.click();
    await expect(devTab).toHaveAttribute("aria-selected", "true");
    await expect(page.getByTestId("developer-handoff-panel")).toBeVisible();

    const devHandoffBtn = page.getByTestId("export-dev-handoff-btn");
    await expect(devHandoffBtn).toBeVisible();
    await expect(devHandoffBtn).toBeEnabled();
    const [devApiResponse] = await Promise.all([
      page.waitForResponse(
        (res) =>
          res.url().includes("/api/v1/export/developer-handoff") && res.request().method() === "POST"
      ),
      devHandoffBtn.click(),
    ]);
    expect(devApiResponse.status()).toBe(200);
    const devBody = (await devApiResponse.json()) as {
      ok: boolean;
      data: { sha256: string; files: Array<{ path: string; byteSize: number }> };
    };
    expect(devBody.ok).toBe(true);
    expect(devBody.data.sha256).toMatch(SHA256_HEX_RE);
    expect(devBody.data.files.length).toBeGreaterThan(0);
    const devReceipt = page.getByTestId("dev-handoff-receipt");
    await expect(devReceipt).toBeVisible({ timeout: 8000 });
    await expect(devReceipt).toContainText(devBody.data.sha256.slice(0, 12));

    // Verify airgapped 2-slide SVG + external-CDN stripping + ArrowRight keyboard presenter
    const sampleDeckHtml = `<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="https://cdn.example.invalid/style.css">
  <script src="https://cdn.example.invalid/tracker.js"></script>
</head>
<body>
  <section data-slide="1">
    <h1>Executive Pitch</h1>
    <svg viewBox="0 0 120 60" width="120" height="60"><rect width="120" height="60" fill="#0057FF"/></svg>
  </section>
  <section data-slide="2">
    <h2>Financial Forecast</h2>
    <p>Zero network dependency offline deck.</p>
  </section>
</body>
</html>`;
    const multiSlideRes = await page.request.post(
      new URL("/api/v1/export/html-bundle", baseUrl).toString(),
      {
        headers: authedHeaders,
        data: {
          projectId: DEFAULT_PROJECT_ID,
          title: "Executive Pitch",
          html: sampleDeckHtml,
          enablePresenter: true,
          overrideQcWarning: true,
        },
      }
    );
    expect(multiSlideRes.ok()).toBeTruthy();
    const multiSlideBody = (await multiSlideRes.json()) as {
      ok: boolean;
      data: {
        externalCdnRefsCount: number;
        presenterEnabled: boolean;
        svgCount: number;
        brokenSvgCount: number;
        htmlContent: string;
      };
    };
    expect(multiSlideBody.ok).toBe(true);
    expect(multiSlideBody.data.externalCdnRefsCount).toBe(0);
    expect(multiSlideBody.data.svgCount).toBe(1);
    expect(multiSlideBody.data.brokenSvgCount).toBe(0);
    await verifyOfflineHtmlInAirgappedContext(browser, multiSlideBody.data.htmlContent, true);

    // Verify V30 export history persistence
    const histRes = await page.request.get(
      new URL(`/api/v1/export/history?projectId=${DEFAULT_PROJECT_ID}`, baseUrl).toString(),
      { headers: authedHeaders }
    );
    expect(histRes.ok()).toBeTruthy();
    const histBody = (await histRes.json()) as { data: { items: Array<{ format: string }> } };
    expect(histBody.data.items.length).toBeGreaterThanOrEqual(3);

    // Click Install PPTX plugin CTA if visible to verify non-dead navigation to /plugins
    await shareTab.click();
    const installCtaAfter = page.getByTestId("export-pptx-plugin-cta");
    if (await installCtaAfter.isVisible().catch(() => false)) {
      await Promise.all([
        page.waitForURL(/\/plugins\?install=pptx-deck-exporter-plugin/, { timeout: 10000 }),
        installCtaAfter.click(),
      ]);
      await expect(page.locator("#root")).not.toBeEmpty();
      await page.goto(baseUrl, { waitUntil: "networkidle" });
      await dismissOnboardingWizard(page);
    } else {
      await page.getByTestId("export-modal-close-btn").click();
      await expect(page.getByTestId("export-bundle-modal")).toBeHidden({ timeout: 5000 });
    }

    // =========================================================================
    // 4. Multi-Locale Verification: en-US & ja-JP (ZERO Vietnamese diacritics)
    // =========================================================================
    // 4a. English (en-US)
    await page.evaluate(() => localStorage.setItem("cowok_locale", "en-US"));
    await page.reload({ waitUntil: "networkidle" });
    await dismissOnboardingWizard(page);
    await applyTheme(page, "warm");

    const enToolbarText = (await page.getByTestId("contextual-stage-toolbar").textContent()) || "";
    expect(enToolbarText).not.toMatch(VI_DIACRITICS_RE);

    await openExportBundleModalViaRealTrigger(page);
    await page.getByTestId("share-present-tab").click();
    const enShareModalText = (await page.getByTestId("export-bundle-modal").textContent()) || "";
    expect(enShareModalText).not.toMatch(VI_DIACRITICS_RE);
    expect(enShareModalText).toContain("Share & Present");
    await page.screenshot({
      path: path.join(EVIDENCE_DIR, "story-07.2-desktop-en.png"),
      fullPage: false,
    });

    await page.getByTestId("developer-handoff-tab").click();
    const enDevPanelText = (await page.getByTestId("developer-handoff-panel").textContent()) || "";
    expect(enDevPanelText).not.toMatch(VI_DIACRITICS_RE);
    await page.getByTestId("export-modal-close-btn").click();
    await expect(page.getByTestId("export-bundle-modal")).toBeHidden({ timeout: 5000 });

    // 4b. Japanese (ja-JP)
    await page.evaluate(() => localStorage.setItem("cowok_locale", "ja-JP"));
    await page.reload({ waitUntil: "networkidle" });
    await dismissOnboardingWizard(page);
    await applyTheme(page, "warm");

    const jaToolbarText = (await page.getByTestId("contextual-stage-toolbar").textContent()) || "";
    expect(jaToolbarText).not.toMatch(VI_DIACRITICS_RE);

    await openExportBundleModalViaRealTrigger(page);
    await page.getByTestId("share-present-tab").click();
    const jaShareModalText = (await page.getByTestId("export-bundle-modal").textContent()) || "";
    expect(jaShareModalText).not.toMatch(VI_DIACRITICS_RE);
    expect(jaShareModalText).toMatch(JA_SCRIPT_RE);
    await page.screenshot({
      path: path.join(EVIDENCE_DIR, "story-07.2-desktop-ja.png"),
      fullPage: false,
    });

    await page.getByTestId("developer-handoff-tab").click();
    const jaDevPanelText = (await page.getByTestId("developer-handoff-panel").textContent()) || "";
    expect(jaDevPanelText).not.toMatch(VI_DIACRITICS_RE);
    await page.getByTestId("export-modal-close-btn").click();
    await expect(page.getByTestId("export-bundle-modal")).toBeHidden({ timeout: 5000 });

    // =========================================================================
    // 5. Mobile 390px evidence screenshot (vi-VN)
    // =========================================================================
    await page.evaluate(() => localStorage.setItem("cowok_locale", "vi-VN"));
    await page.setViewportSize({ width: 390, height: 844 });
    await page.reload({ waitUntil: "networkidle" });
    await dismissOnboardingWizard(page);
    await applyTheme(page, "warm");
    await page.screenshot({
      path: path.join(EVIDENCE_DIR, "story-07.2-mobile-390.png"),
      fullPage: false,
    });

    expect(consoleErrors).toEqual([]);
    expect(pageErrors).toEqual([]);
    expect(serverErrors).toEqual([]);
    await context.close();
  });
});
