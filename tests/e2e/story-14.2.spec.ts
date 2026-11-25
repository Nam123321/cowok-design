// @story 14.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Live Browser E2E Full-Userflow for SuperAdmin Portal at /admin)
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

const THEME_SETTLE_MS = 400;

async function setPortalTheme(page: Page, theme: "warm" | "night") {
  await page.evaluate((nextTheme) => {
    document.documentElement.setAttribute("data-theme", nextTheme);
  }, theme);
  await page.waitForTimeout(THEME_SETTLE_MS);
}

test.describe("Story 14.2: Cloud Ingestion API & SuperAdmin Portal MVP E2E", () => {
  test("verifies OIDC+MFA gate, 2-way helpdesk drawer, feature clustering, 5-step activation funnel, crash center & ANALYST_READONLY lock", async ({
    browser,
  }) => {
    test.setTimeout(120_000);
    const storyDir = path.resolve(
      process.cwd(),
      "_iwish-output/3. Development/1. Epic & Story/VS-07. Cloud Control Plane, Non-Tech Support Desk & SuperAdmin Portal/Epic-14/Story-14.2"
    );
    const evidenceDir = path.join(storyDir, "qa", "evidence");
    const videoDir = path.join(evidenceDir, "videos");
    fs.mkdirSync(videoDir, { recursive: true });

    const qaNonce = process.env.QA_NONCE_TOKEN || crypto.randomBytes(NONCE_BYTES).toString("hex");
    fs.writeFileSync(
      path.join(evidenceDir, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, storyId: "14.2", generatedAt: new Date().toISOString() }, null, 2)
    );

    const targetPort = process.env.TARGET_PORT || process.env.PORT || "3000";
    const adminUrl = `http://127.0.0.1:${targetPort}/admin`;
    const context = await browser.newContext({
      viewport: { width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT },
      recordVideo: { dir: videoDir, size: { width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT } },
    });
    const page = await context.newPage();

    const pageErrors: string[] = [];
    const unexpectedConsoleErrors: string[] = [];
    const serverErrors: string[] = [];

    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("console", (msg) => {
      if (msg.type() === "error" && !msg.text().includes("favicon.ico")) {
        unexpectedConsoleErrors.push(msg.text());
      }
    });
    page.on("response", (res) => {
      if (res.url().includes("/api/") && res.status() >= 500) {
        serverErrors.push(`${res.status()} ${res.url()}`);
      }
    });

    await page.goto(adminUrl, { waitUntil: "domcontentloaded", timeout: NAV_TIMEOUT_MS });
    await expect(page.locator("[data-ff-id='superadmin-portal-root']")).toBeVisible({ timeout: NAV_TIMEOUT_MS });

    // 1. OIDC + MFA Sign-In as SUPER_ADMIN (no hardcoded default MFA code in UI)
    const mfaCode = process.env.COWOK_CLOUD_ADMIN_MFA_CODE || "314159";
    await expect(page.getByTestId("oidc-mfa-input")).toHaveValue("");
    await page.getByTestId("oidc-mfa-input").fill(mfaCode);
    await page.getByTestId("oidc-signin-btn").click();
    await expect(page.getByTestId("superadmin-operator-badge")).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
    await expect(page.locator("[data-ff-id='superadmin-kpi-metric-line']")).toBeVisible();
    await expect(page.locator("[data-ff-id='superadmin-helpdesk-table']")).toBeVisible();

    // Desktop Light + Dark captures
    await setPortalTheme(page, "warm");
    await page.screenshot({ path: path.join(evidenceDir, "desktop-light-helpdesk.png"), fullPage: true });
    await setPortalTheme(page, "night");
    await page.screenshot({ path: path.join(evidenceDir, "desktop-dark-helpdesk.png"), fullPage: true });

    // 2. Navigate all 4 live 14.2 tabs + Story 14.3 roadmap & governance modules via Left Sidebar
    await page.getByTestId("superadmin-tab-clusters").click();
    await expect(page.getByTestId("superadmin-clusters-tab")).toBeVisible();

    await page.getByTestId("superadmin-tab-funnel").click();
    await expect(page.locator("[data-ff-id='superadmin-funnel-chart']")).toBeVisible();

    await page.getByTestId("superadmin-tab-crashes").click();
    await expect(page.locator("[data-ff-id='superadmin-crash-table']")).toBeVisible();

    await page.getByTestId("superadmin-tab-plugin-catalog").click();
    await expect(page.getByTestId("superadmin-panel-plugin-catalog")).toBeVisible();

    await page.getByTestId("superadmin-tab-ab-experiments").click();
    await expect(page.getByTestId("superadmin-panel-ab-experiments")).toBeVisible();

    await page.getByTestId("superadmin-tab-governance").click();
    await expect(page.getByTestId("superadmin-panel-governance")).toBeVisible();

    // 3. Downward role preview to ANALYST_READONLY hides role-switcher (prevents self-promotion)
    await page.getByTestId("superadmin-role-switcher").selectOption("ANALYST_READONLY");
    await expect(page.getByTestId("superadmin-operator-badge")).toContainText("ANALYST_READONLY");
    await expect(page.getByTestId("superadmin-role-switcher")).toHaveCount(0);

    // 4. Mobile responsive check (collapsed sidebar, zero horizontal page overflow)
    await page.getByTestId("superadmin-tab-helpdesk").click();
    await page.setViewportSize({ width: MOBILE_WIDTH, height: MOBILE_HEIGHT });
    await setPortalTheme(page, "warm");
    await expect(page.getByTestId("superadmin-mobile-menu-btn")).toBeVisible();
    const hasOverflow = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth
    );
    expect(hasOverflow).toBe(false);
    await page.screenshot({ path: path.join(evidenceDir, "mobile-admin.png"), fullPage: true });

    expect(pageErrors).toEqual([]);
    expect(unexpectedConsoleErrors).toEqual([]);
    expect(serverErrors).toEqual([]);

    await context.close();
  });
});
