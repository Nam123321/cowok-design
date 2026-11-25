// @story 07.3
// @implements AC-01, AC-02, AC-03, AC-04 (Built-in Native Rust OOXML PPTX 2-Mode + 3-Locale E2E)
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import zlib from "node:zlib";
import { expect, test, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-07/Story-07.3"
);
const EVIDENCE_DIR = path.join(STORY_DIR, "qa", "evidence");
const DEFAULT_PROJECT_ID = "11111111-1111-4111-8111-111111111111";
const SHA256_HEX_RE = /^[0-9a-f]{64}$/i;
const VI_DIACRITICS_RE =
  /[àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđ]/i;
const JA_SCRIPT_RE = /[\u3040-\u30ff\u4e00-\u9faf]/;

/** Lightweight ZIP central-directory parser to inspect real .pptx OPC entries in Node E2E. */
function unpackZipTextEntries(zipBuf: Buffer): Map<string, string> {
  expect(zipBuf.subarray(0, 4).toString("binary")).toBe("PK\x03\x04");
  const entries = new Map<string, string>();
  let offset = 0;
  while (offset + 30 <= zipBuf.length) {
    const sig = zipBuf.readUInt32LE(offset);
    if (sig !== 0x04034b50) break;
    const method = zipBuf.readUInt16LE(offset + 8);
    const compSize = zipBuf.readUInt32LE(offset + 18);
    const nameLen = zipBuf.readUInt16LE(offset + 26);
    const extraLen = zipBuf.readUInt16LE(offset + 28);
    const nameStart = offset + 30;
    const name = zipBuf.subarray(nameStart, nameStart + nameLen).toString("utf8");
    const dataStart = nameStart + nameLen + extraLen;
    const dataSlice = zipBuf.subarray(dataStart, dataStart + compSize);
    if (name.endsWith(".xml") || name.endsWith(".rels")) {
      const raw = method === 8 ? zlib.inflateRawSync(dataSlice) : dataSlice;
      entries.set(name, raw.toString("utf8"));
    }
    offset = dataStart + compSize;
  }
  return entries;
}

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
    if (await nextBtn.isVisible().catch(() => false)) await nextBtn.click();
    if (await nextBtn.isVisible().catch(() => false)) await nextBtn.click();
    const finishBtn = page.getByRole("button", { name: /Hoàn tất|Finish|完了/i });
    if (await finishBtn.isVisible().catch(() => false)) await finishBtn.click();
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

test.describe("Story 07.3 Built-in Native Rust OOXML .pptx Exporter (2-Mode + 3-Locale Live Browser E2E)", () => {
  test("verifies Built-in Ready PPTX card, Editable & Visual-Exact exports returning real OOXML ZIP, Limitations Disclosure, Fidelity Summary, and vi-VN/en-US/ja-JP locales", async ({
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
    // 1. Open ExportBundleModal -> Share & Present -> Built-in PPTX Card (vi-VN)
    // =========================================================================
    await openExportBundleModalViaRealTrigger(page);
    await page.getByTestId("share-present-tab").click();
    await expect(page.getByTestId("share-present-panel")).toBeVisible();
    await ensureQcOverrideCheckedIfBlocked(page);

    const pptxCard = page.getByTestId("export-card-pptx");
    await expect(pptxCard).toBeVisible();
    const limitationsNotice = page.getByTestId("pptx-limitations-notice");
    await expect(limitationsNotice).toBeVisible();

    const editableRadio = page.getByTestId("pptx-mode-editable");
    const visualExactRadio = page.getByTestId("pptx-mode-visual-exact");
    await expect(editableRadio).toHaveAttribute("aria-checked", "true");
    await expect(visualExactRadio).toHaveAttribute("aria-checked", "false");

    // 1a. Export in Editable mode (default) -> verify real PK\x03\x04 ZIP + XML parts
    const exportPptxBtn = page.getByTestId("export-pptx-btn");
    const [editableRes] = await Promise.all([
      page.waitForResponse(
        (res) => res.url().includes("/api/v1/export/pptx") && res.request().method() === "POST"
      ),
      exportPptxBtn.click(),
    ]);
    let editableStatus = editableRes.status();
    let editableJson: Record<string, unknown>;
    if (editableStatus === 409) {
      await ensureQcOverrideCheckedIfBlocked(page);
      const [retryRes] = await Promise.all([
        page.waitForResponse(
          (res) => res.url().includes("/api/v1/export/pptx") && res.request().method() === "POST"
        ),
        exportPptxBtn.click(),
      ]);
      editableStatus = retryRes.status();
      editableJson = (await retryRes.json()) as Record<string, unknown>;
    } else {
      editableJson = (await editableRes.json()) as Record<string, unknown>;
    }
    expect(editableStatus).toBe(200);
    const editableData = editableJson.data as {
      pluginInstalled: boolean;
      pluginStatus: string;
      exportMode: string;
      pptxBase64: string;
      sha256: string;
      slideCount: number;
      fileName: string;
    };
    expect(editableData.pluginInstalled).toBe(true);
    expect(editableData.pluginStatus).toBe("BUILTIN_ACTIVE");
    expect(editableData.exportMode).toBe("editable");
    expect(editableData.sha256).toMatch(SHA256_HEX_RE);
    expect(editableData.fileName.endsWith(".pptx")).toBe(true);

    const editableBuf = Buffer.from(editableData.pptxBase64, "base64");
    const editableEntries = unpackZipTextEntries(editableBuf);
    expect(editableEntries.has("[Content_Types].xml")).toBe(true);
    expect(editableEntries.has("_rels/.rels")).toBe(true);
    expect(editableEntries.has("ppt/slides/slide1.xml")).toBe(true);
    expect(editableEntries.get("ppt/slides/slide1.xml")).toContain("<p:sld");

    const fidelitySummary = page.getByTestId("pptx-fidelity-warnings");
    await expect(fidelitySummary).toBeVisible({ timeout: 8000 });
    await expect(fidelitySummary).toContainText(editableData.fileName);

    // 1b. Switch to Visual-Exact mode -> click Export -> verify visual_exact ZIP
    await visualExactRadio.click();
    await expect(visualExactRadio).toHaveAttribute("aria-checked", "true");
    const [visualRes] = await Promise.all([
      page.waitForResponse(
        (res) => res.url().includes("/api/v1/export/pptx") && res.request().method() === "POST"
      ),
      exportPptxBtn.click(),
    ]);
    expect(visualRes.status()).toBe(200);
    const visualJson = (await visualRes.json()) as {
      data: {
        exportMode: string;
        pptxBase64: string;
        pluginStatus: string;
        usedStaticFallback?: boolean;
        fidelityWarnings?: Array<{ code: string }>;
      };
    };
    expect(visualJson.data.pluginStatus).toBe("BUILTIN_ACTIVE");
    expect(visualJson.data.exportMode).toBe("visual_exact");
    const visualBuf = Buffer.from(visualJson.data.pptxBase64, "base64");
    const visualEntries = unpackZipTextEntries(visualBuf);
    const visualSlide1 = visualEntries.get("ppt/slides/slide1.xml") || "";
    const visualWarningCodes = (visualJson.data.fidelityWarnings || []).map((w) => w.code);
    if (visualWarningCodes.includes("VISUAL_EXACT_RASTER_UNAVAILABLE")) {
      // ADV-04 non-blank fallback when headless CDP full-slide PNG snapshot is unavailable
      expect(visualSlide1).toContain("<p:sp>");
      expect(visualSlide1).toContain("<a:t>");
    } else {
      expect(visualSlide1).toContain("<p:pic>");
    }

    // 1c. Verify rich multi-effect HTML returns FidelityWarning[] cleanly from Rust backend
    const richRes = await page.request.post(new URL("/api/v1/export/pptx", baseUrl).toString(), {
      headers: authedHeaders,
      data: {
        projectId: DEFAULT_PROJECT_ID,
        title: "Kế hoạch Q4",
        exportMode: "editable",
        locale: "vi-VN",
        overrideQcWarning: true,
        html: `<style>@keyframes pop{from{opacity:0}to{opacity:1}} .b:hover{color:#2563eb}</style>
               <section data-slide="1" data-transition="morph">
                 <h1>Tiếng Việt chuẩn NFC</h1>
                 <div style="backdrop-filter: blur(12px);">Glass card</div>
                 <table><tr><th rowspan="2">A</th><th colspan="2">B</th></tr><tr><td>1</td><td>2</td></tr></table>
                 <svg width="80" height="40" viewBox="0 0 80 40"><rect width="80" height="40" fill="#2563eb"/></svg>
               </section>`,
      },
    });
    expect(richRes.ok()).toBeTruthy();
    const richBody = (await richRes.json()) as {
      data: { pptxBase64: string; fidelityWarnings: Array<{ code: string }> };
    };
    const richCodes = richBody.data.fidelityWarnings.map((w) => w.code);
    expect(richCodes).toContain("BACKDROP_FILTER_RASTERIZED");
    expect(richCodes).toContain("CSS_ANIMATION_OMITTED");
    const richEntries = unpackZipTextEntries(Buffer.from(richBody.data.pptxBase64, "base64"));
    expect(richEntries.get("ppt/slides/slide1.xml")).toContain("hMerge=\"1\"");
    expect(richEntries.get("ppt/slides/slide1.xml")).toContain("a16:svgBlip");

    // Capture Desktop Light & Dark screenshots (vi-VN)
    await applyTheme(page, "warm");
    await page.screenshot({
      path: path.join(EVIDENCE_DIR, "story-07.3-desktop-light.png"),
      fullPage: false,
    });
    await applyTheme(page, "night");
    await page.screenshot({
      path: path.join(EVIDENCE_DIR, "story-07.3-desktop-dark.png"),
      fullPage: false,
    });
    await applyTheme(page, "warm");
    await page.getByTestId("export-modal-close-btn").click();
    await expect(page.getByTestId("export-bundle-modal")).toBeHidden({ timeout: 5000 });

    // =========================================================================
    // 2. English (en-US) Locale Verification (ZERO Vietnamese diacritics)
    // =========================================================================
    await page.evaluate(() => localStorage.setItem("cowok_locale", "en-US"));
    await page.reload({ waitUntil: "networkidle" });
    await dismissOnboardingWizard(page);
    await applyTheme(page, "warm");

    await openExportBundleModalViaRealTrigger(page);
    await page.getByTestId("share-present-tab").click();
    const enCardText = (await page.getByTestId("export-card-pptx").textContent()) || "";
    expect(enCardText).not.toMatch(VI_DIACRITICS_RE);
    await page.screenshot({
      path: path.join(EVIDENCE_DIR, "story-07.3-desktop-en.png"),
      fullPage: false,
    });
    await page.getByTestId("export-modal-close-btn").click();
    await expect(page.getByTestId("export-bundle-modal")).toBeHidden({ timeout: 5000 });

    // =========================================================================
    // 3. Japanese (ja-JP) Locale Verification (ZERO Vietnamese diacritics + JA script)
    // =========================================================================
    await page.evaluate(() => localStorage.setItem("cowok_locale", "ja-JP"));
    await page.reload({ waitUntil: "networkidle" });
    await dismissOnboardingWizard(page);
    await applyTheme(page, "warm");

    await openExportBundleModalViaRealTrigger(page);
    await page.getByTestId("share-present-tab").click();
    const jaCardText = (await page.getByTestId("export-card-pptx").textContent()) || "";
    expect(jaCardText).not.toMatch(VI_DIACRITICS_RE);
    expect(jaCardText).toMatch(JA_SCRIPT_RE);
    await page.screenshot({
      path: path.join(EVIDENCE_DIR, "story-07.3-desktop-ja.png"),
      fullPage: false,
    });
    await page.getByTestId("export-modal-close-btn").click();
    await expect(page.getByTestId("export-bundle-modal")).toBeHidden({ timeout: 5000 });

    // =========================================================================
    // 4. Mobile 390px evidence screenshot (vi-VN) with ExportBundleModal open
    // =========================================================================
    await page.evaluate(() => localStorage.setItem("cowok_locale", "vi-VN"));
    await page.reload({ waitUntil: "networkidle" });
    await dismissOnboardingWizard(page);
    await applyTheme(page, "warm");
    await openExportBundleModalViaRealTrigger(page);
    await page.getByTestId("share-present-tab").click();
    await expect(page.getByTestId("export-card-pptx")).toBeVisible({ timeout: 8000 });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.waitForTimeout(250);
    await page.getByTestId("export-card-pptx").scrollIntoViewIfNeeded();
    await page.screenshot({
      path: path.join(EVIDENCE_DIR, "story-07.3-mobile-390.png"),
      fullPage: false,
    });
    await page.getByTestId("export-modal-close-btn").click();
    await expect(page.getByTestId("export-bundle-modal")).toBeHidden({ timeout: 5000 });

    expect(consoleErrors).toEqual([]);
    expect(pageErrors).toEqual([]);
    expect(serverErrors).toEqual([]);
    await context.close();
  });
});
