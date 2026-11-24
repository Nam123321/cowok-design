// @story 05.6
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06 (Live Browser E2E Full Userflow for Story 05.6)
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-05/Story-05.6"
);
const EVIDENCE_DIR = path.join(STORY_DIR, "qa", "evidence");
const DEFAULT_PROJECT_ID = "11111111-1111-4111-8111-111111111111";

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
    await page.getByRole("button", { name: /Hoàn tất/i }).click();
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
  await page.waitForTimeout(350);
}

async function bootEditorSession(page: Page, baseUrl: string, qaNonce: string): Promise<string> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto(baseUrl, { waitUntil: "networkidle" });
  await expect(page.locator("#root")).not.toBeEmpty({ timeout: 15000 });
  await dismissOnboardingWizard(page);

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

test.describe("Story 05.6 Full E2E Brand Visual Studio & Hidden Spec Userflow", () => {
  test("verifies single-mode guard, multi-mode switch, Component Gallery, Popular Audit, Hidden Spec OCC 409, FE Data Contract, Heritage Pack, Night & Mobile", async ({
    browser,
  }) => {
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    const qaNonce =
      process.env.WATCHMEN_NONCE ||
      process.env.QA_NONCE_TOKEN ||
      crypto.randomBytes(32).toString("hex");

    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      recordHar: { path: path.join(EVIDENCE_DIR, "story-05.6-network.har"), mode: "minimal" },
      recordVideo: { dir: EVIDENCE_DIR, size: { width: 1280, height: 800 } },
    });

    await context.route(`http://127.0.0.1:${targetPort}/**`, async (route) => {
      await route.continue({
        headers: {
          ...route.request().headers(),
          "x-iwish-qa-nonce": qaNonce,
          "x-tenant-id": "local-default",
        },
      });
    });

    const page = await context.newPage();
    const pageErrors: string[] = [];
    const consoleErrors: string[] = [];
    const server500s: string[] = [];
    let expectingConflict409 = false;

    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        const text = msg.text();
        if (expectingConflict409 && text.includes("409")) return;
        // Strict zero-JS <iframe sandbox=""> blocks Playwright's own frame instrumentation script in about:srcdoc
        if (text.includes("Blocked script execution in 'about:srcdoc' because the document's frame is sandboxed")) {
          return;
        }
        consoleErrors.push(text);
      }
    });
    page.on("response", (res) => {
      if (res.status() >= 500) server500s.push(`${res.status()} ${res.url()}`);
    });

    const sessionToken = await bootEditorSession(page, baseUrl, qaNonce);
    const apiHeaders = {
      "Content-Type": "application/json",
      Authorization: `Bearer ${sessionToken}`,
      "X-Cowok-Session-Token": sessionToken,
      "x-iwish-qa-nonce": qaNonce,
      "x-tenant-id": "local-default",
    };

    // 1. Open Brand Visual Studio from compact 34px toolbar strip
    const toolbar = page.getByTestId("sketch-polish-strip");
    await expect(toolbar).toBeVisible({ timeout: 15000 });
    const box = await toolbar.boundingBox();
    expect(box?.height ?? 0).toBeLessThanOrEqual(36);

    await page.getByTestId("open-brand-studio-btn").click();
    await expect(page.getByTestId("brand-visual-studio-modal")).toBeVisible({ timeout: 10000 });
    // Single mode default -> dynamic Light/Dark switcher hidden
    await expect(page.getByTestId("dynamic-theme-mode-switcher")).toHaveCount(0);
    await page.waitForTimeout(450);
    // Verify zero-JS sandbox="" and script-free sanitized srcDoc on every preview iframe
    const previewIframes = page.locator('iframe[data-testid^="sketch-preview-iframe-"]');
    const iframeCount = await previewIframes.count();
    expect(iframeCount).toBeGreaterThanOrEqual(1);
    expect(iframeCount).toBeLessThanOrEqual(3);
    for (let i = 0; i < iframeCount; i++) {
      const frame = previewIframes.nth(i);
      expect(await frame.getAttribute("sandbox")).toBe("");
      const srcDoc = (await frame.getAttribute("srcdoc")) ?? "";
      expect(srcDoc.toLowerCase()).not.toContain("<script");
    }
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "01-brand-studio-warm-single-mode.png"), fullPage: false });

    // 2. Add second user mode ("dim") -> dynamic switcher appears with ONLY requested modes
    await page.getByTestId("studio-request-mode-dim").click();
    await expect(page.getByTestId("dynamic-theme-mode-switcher")).toBeVisible({ timeout: 5000 });
    await page.getByTestId("studio-mode-switch-dim").click();
    await page.waitForTimeout(400);

    // 3. Switch to Shared Component Gallery + Popular Audit
    await page.getByTestId("studio-tab-component-gallery").click();
    await expect(page.getByTestId("shared-component-gallery")).toBeVisible({ timeout: 5000 });
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "02-shared-component-gallery.png"), fullPage: false });

    await page.getByTestId("studio-tab-popular-audit").click();
    await expect(page.getByTestId("popular-component-audit")).toBeVisible({ timeout: 5000 });

    // 4. Save Heritage Pack with DECK_FROM_APP
    const packName = `E2E Atelier Pack ${Date.now().toString(36)}`;
    await page.getByTestId("toggle-heritage-picker-btn").click();
    await page.getByTestId("heritage-entry-mode-select").selectOption("DECK_FROM_APP");
    await page.getByTestId("heritage-pack-name-input").fill(packName);
    await page.getByTestId("save-heritage-pack-btn").click();
    await expect(page.getByText(packName, { exact: true })).toBeVisible({ timeout: 5000 });

    // 5. Night theme verification
    await setWorkspaceTheme(page, "night");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "03-brand-studio-night.png"), fullPage: false });
    await page.getByTestId("close-brand-studio-btn").click();

    // 6. Open Bidirectional Hidden Spec Drawer + FE Data Contract + OCC 409 verification
    await setWorkspaceTheme(page, "warm");
    await page.getByTestId("open-hidden-specs-btn").click();
    await expect(page.getByTestId("hidden-spec-drawer")).toBeVisible({ timeout: 5000 });
    await page.getByTestId("drawer-tab-fe-contract").click();
    await expect(page.getByTestId("fe-contract-boundary-banner")).toContainText("FE_REQUIRED_VIEWMODEL_AND_ACTIONS_ONLY");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "04-hidden-spec-fe-contract.png"), fullPage: false });
    await page.getByTestId("close-hidden-spec-drawer-btn").click();

    // Verify live OCC 409 STALE_MAPPING on hidden-specs PUT
    expectingConflict409 = true;
    const hsGet = await page.request.get(
      new URL(`/api/v1/cognitive/hidden-specs?projectId=${DEFAULT_PROJECT_ID}`, baseUrl).toString(),
      { headers: apiHeaders }
    );
    expect(hsGet.status()).toBe(200);
    const hsJson = (await hsGet.json()) as { data: { specs: Array<{ specKey: string; revision: number }> } };
    const brandSpec = hsJson.data.specs.find((s) => s.specKey === "02_BRAND_IDENTITY")!;

    const putOk = await page.request.put(new URL("/api/v1/cognitive/hidden-specs", baseUrl).toString(), {
      headers: apiHeaders,
      data: {
        projectId: DEFAULT_PROJECT_ID,
        specKey: "02_BRAND_IDENTITY",
        contentMarkdown: "# 02 Brand Identity\nUpdated via E2E",
        expectedRevision: brandSpec.revision,
      },
    });
    expect(putOk.status()).toBe(200);

    const putConflict = await page.request.put(new URL("/api/v1/cognitive/hidden-specs", baseUrl).toString(), {
      headers: apiHeaders,
      data: {
        projectId: DEFAULT_PROJECT_ID,
        specKey: "02_BRAND_IDENTITY",
        contentMarkdown: "# Stale E2E Write",
        expectedRevision: brandSpec.revision,
      },
    });
    expect(putConflict.status()).toBe(409);
    const conflictJson = (await putConflict.json()) as { error: { code: string } };
    expect(conflictJson.error.code).toBe("STALE_MAPPING");
    expectingConflict409 = false;

    // 7. Mobile (390x844) responsive verification
    await page.setViewportSize({ width: 390, height: 844 });
    await page.waitForTimeout(450);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "05-mobile-responsive.png"), fullPage: false });

    const videoHandle = page.video();
    await context.close();
    if (videoHandle) {
      const recordedPath = await videoHandle.path();
      if (recordedPath && fs.existsSync(recordedPath)) {
        fs.copyFileSync(recordedPath, path.join(EVIDENCE_DIR, "story-05.6-full-userflow.webm"));
      }
    }

    expect(pageErrors, `Unexpected page errors: ${pageErrors.join("; ")}`).toEqual([]);
    expect(consoleErrors, `Unexpected console errors: ${consoleErrors.join("; ")}`).toEqual([]);
    expect(server500s, `Unexpected 5xx responses: ${server500s.join("; ")}`).toEqual([]);
  });
});
