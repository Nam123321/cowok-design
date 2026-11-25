// @story 14.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05 (Live Browser E2E Full-Userflow + Multi-Locale vi-VN/en-US/ja-JP for 4-Channel Distribution Ops, Plugin Catalog, A/B Experiments, Quarantine, Offline Licenses & Audit Logs)
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
const VIETNAMESE_DIACRITIC_REGEX =
  /[àáảãạăằắẳẵặâầấẩẫậèéẻẽẹêềếểễệìíỉĩịòóỏõọôồốổỗộơờớởỡợùúủũụưừứửữựỳýỷỹỵđĐ]/;

async function setPortalTheme(page: Page, theme: "warm" | "night") {
  await page.evaluate((nextTheme) => {
    document.documentElement.setAttribute("data-theme", nextTheme);
  }, theme);
  await page.waitForTimeout(THEME_SETTLE_MS);
}

async function signInSuperAdmin(page: Page, mfaCode: string) {
  await expect(page.locator("[data-ff-id='superadmin-portal-root']")).toBeVisible({
    timeout: NAV_TIMEOUT_MS,
  });
  await page.getByTestId("oidc-mfa-input").fill(mfaCode);
  await page.getByTestId("oidc-signin-btn").click();
  await expect(page.getByTestId("superadmin-operator-badge")).toBeVisible({
    timeout: ACTION_TIMEOUT_MS,
  });
}

async function exercisePluginCatalogTab(
  page: Page,
  evidenceDir: string,
  suffix: string
): Promise<string> {
  const newPluginId = `e2e-figma-sync-${suffix}`;
  await page.getByTestId("superadmin-tab-plugin-catalog").click();
  await expect(page.locator("[data-ff-id='superadmin-plugin-catalog-panel']")).toBeVisible();
  await expect(page.getByTestId("catalog-filter-bar")).toBeVisible();
  await expect(page.getByTestId("catalog-plugins-table")).toBeVisible();
  await expect(page.getByTestId("catalog-open-create-drawer-btn")).toHaveText("Thêm Plugin");

  // Source & status filter chips
  for (const src of ["DEFAULT_BUILTIN", "RECOMMENDED_GITHUB", "ALL"] as const) {
    await page.getByTestId(`catalog-source-chip-${src}`).click();
    await expect(page.getByTestId(`catalog-source-chip-${src}`)).toHaveAttribute("aria-pressed", "true");
  }
  for (const st of ["ACTIVE", "PROCESSING", "ALL"] as const) {
    await page.getByTestId(`catalog-status-chip-${st}`).click();
    await expect(page.getByTestId(`catalog-status-chip-${st}`)).toHaveAttribute("aria-pressed", "true");
  }

  // Search input & sortable column headers
  await page.getByTestId("catalog-search-input").fill("html-forge");
  await page.getByTestId("catalog-search-input").fill("");
  for (const col of ["plugin", "source", "status", "updated"] as const) {
    await page.getByTestId(`catalog-sort-${col}`).click();
  }

  // Open Create RightDrawer, capture dark screenshot, then submit (PROCESSING)
  await page.getByTestId("catalog-open-create-drawer-btn").click();
  await expect(page.getByTestId("catalog-right-drawer")).toBeVisible();
  await expect(page.getByTestId("catalog-status-select")).toHaveValue("PROCESSING");
  await setPortalTheme(page, "night");
  await page.screenshot({ path: path.join(evidenceDir, "desktop-dark-plugin-catalog.png"), fullPage: true });

  await page.getByTestId("catalog-plugin-id-input").fill(newPluginId);
  await page.getByTestId("catalog-plugin-title-input").fill(`Figma Sync ${suffix}`);
  await page.getByTestId("catalog-plugin-summary-input").fill("Sync Figma design tokens cleanly");
  await page.getByTestId("catalog-repo-url-input").fill(`https://github.com/cowok-design/${newPluginId}`);
  await page.getByTestId("catalog-commit-sha-input").fill("a".repeat(40));
  await page.getByTestId("catalog-reason-input").fill("Register pinned release commit in E2E");
  await page.getByTestId("catalog-submit-btn").click();
  await expect(page.getByTestId("catalog-right-drawer")).toBeHidden({ timeout: ACTION_TIMEOUT_MS });
  await expect(page.getByTestId(`catalog-row-${newPluginId}`)).toBeVisible({ timeout: ACTION_TIMEOUT_MS });

  // Open Detail RightDrawer -> Quick Action Approve & Activate (ACTIVE)
  await page.getByTestId(`catalog-row-${newPluginId}`).click();
  await expect(page.getByTestId("catalog-right-drawer")).toBeVisible();
  await expect(page.getByTestId("catalog-async-security-pill")).toBeVisible();
  await page.getByTestId("catalog-quick-approve-btn").click();
  await expect(page.getByTestId("catalog-drawer-status-pill")).toHaveText("Đang hoạt động", {
    timeout: ACTION_TIMEOUT_MS,
  });
  await page.getByTestId("catalog-drawer-close-btn").click();
  await expect(page.getByTestId("catalog-right-drawer")).toBeHidden();

  // Select checkbox -> Bulk Floating Card (light screenshot) -> POST /bulk -> Clear selection
  const targetCheckbox = page.getByTestId(`catalog-row-select-${newPluginId}`);
  await targetCheckbox.check();
  await expect(page.getByTestId("catalog-bulk-floating-card")).toBeVisible();
  await setPortalTheme(page, "warm");
  await page.screenshot({ path: path.join(evidenceDir, "desktop-light-plugin-catalog.png"), fullPage: true });

  await page.getByTestId("catalog-bulk-reason-input").fill("Bulk pause verified plugin batch");
  await page.getByTestId("catalog-bulk-inactive-btn").click();
  await expect(page.getByTestId("catalog-bulk-floating-card")).toBeHidden({ timeout: ACTION_TIMEOUT_MS });
  await expect(page.getByTestId(`catalog-status-badge-${newPluginId}`)).toHaveText("Tạm dừng", {
    timeout: ACTION_TIMEOUT_MS,
  });

  await targetCheckbox.check();
  await expect(page.getByTestId("catalog-bulk-floating-card")).toBeVisible();
  await page.getByTestId("catalog-bulk-clear-btn").click();
  await expect(page.getByTestId("catalog-bulk-floating-card")).toBeHidden();
  return newPluginId;
}

async function exerciseAbCanaryQuarantineLicenseTabs(page: Page, suffix: string) {
  // 2. A/B Experiments: create + Kill-Switch
  const expKey = `exp_e2e_${suffix}`;
  await page.getByTestId("superadmin-tab-ab-experiments").click();
  await expect(page.locator("[data-ff-id='superadmin-ab-experiments-panel']")).toBeVisible();
  await page.getByTestId("ab-key-input").fill(expKey);
  await page.getByTestId("ab-title-input").fill(`Onboarding Variant ${suffix}`);
  await page.getByTestId("ab-surface-select").selectOption("PLUGIN_RANKING");
  await page.getByTestId("ab-split-select").selectOption("10/90");
  await page.getByTestId("ab-reason-input").fill("Launch deterministic E2E A/B experiment");
  await page.getByTestId("ab-submit-btn").click();
  await expect(page.getByTestId(`ab-exp-card-${expKey}`)).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
  await page.getByTestId(`ab-kill-btn-${expKey}`).click();
  await expect(page.getByTestId(`ab-kill-btn-${expKey}`)).toBeHidden({ timeout: ACTION_TIMEOUT_MS });

  // 3. Canary Rollout: publish signed rollout + Kill-Switch button
  const rolloutVer = `1.9.${parseInt(suffix.slice(0, 4), 16) % 900 + 100}`;
  await page.getByTestId("superadmin-tab-distribution").click();
  await expect(page.locator("[data-ff-id='superadmin-canary-rollout-panel']")).toBeVisible();
  await expect(page.getByTestId("canary-sha256-input")).toHaveValue("");
  await page.getByTestId("canary-version-input").fill(rolloutVer);
  await page.getByTestId("canary-url-input").fill(`https://releases.cowok.design/desktop/${rolloutVer}`);
  await page.getByTestId("canary-sha256-input").fill("b".repeat(64));
  await page.getByTestId("canary-ring-25").click();
  await page.getByTestId("canary-announcement-input").fill(`Canary release ${rolloutVer}`);
  await page.getByTestId("canary-submit-btn").click();
  await expect(page.getByTestId(`canary-row-desktop-${rolloutVer}`).first()).toBeVisible({
    timeout: ACTION_TIMEOUT_MS,
  });
  await page.getByTestId(`canary-kill-btn-desktop-${rolloutVer}`).click();
  await expect(page.getByTestId(`canary-kill-btn-desktop-${rolloutVer}`)).toBeHidden({
    timeout: ACTION_TIMEOUT_MS,
  });

  // 4. Global Skill Quarantine: publish signed advisory
  const badSkillId = `quarantine-skill-${suffix}`;
  await page.getByTestId("superadmin-tab-quarantine").click();
  await expect(page.locator("[data-ff-id='superadmin-quarantine-panel']")).toBeVisible();
  await page.getByTestId("quarantine-skill-id-input").fill(badSkillId);
  await page.getByTestId("quarantine-severity-select").selectOption("CRITICAL");
  await page.getByTestId("quarantine-action-select").selectOption("QUARANTINE");
  await page.getByTestId("quarantine-reason-input").fill("Supply-chain audit E2E quarantine");
  await page.getByTestId("quarantine-submit-btn").click();
  await expect(page.getByTestId(`advisory-row-${badSkillId}`).first()).toBeVisible({
    timeout: ACTION_TIMEOUT_MS,
  });

  // 5. Offline Licenses & Cowok.ai Federation: issue + WebCrypto offline verify
  const orgName = `Cowok Studio ${suffix}`;
  const cowokOrgId = `org_e2e_${suffix}`;
  await page.getByTestId("superadmin-tab-licensing").click();
  await expect(page.locator("[data-ff-id='superadmin-licensing-panel']")).toBeVisible();
  await page.getByTestId("license-tier-select").selectOption("ENTERPRISE");
  await page.getByTestId("license-org-input").fill(orgName);
  await page.getByTestId("license-cowok-org-input").fill(cowokOrgId);
  await page.getByTestId("license-seats-input").fill("25");
  await page.getByTestId("license-days-input").fill("365");
  await page.getByTestId("license-submit-btn").click();
  const createdLicenseRow = page.locator("tr", { hasText: orgName }).first();
  await expect(createdLicenseRow).toBeVisible({ timeout: ACTION_TIMEOUT_MS });
  await createdLicenseRow.locator("[data-testid^='license-verify-btn-']").click();
  await expect(page.getByTestId("license-offline-verify-preview")).toBeVisible({
    timeout: ACTION_TIMEOUT_MS,
  });
  await expect(page.getByTestId("license-offline-verify-preview")).toContainText(cowokOrgId);

  return { expKey, rolloutVer, badSkillId };
}

async function verifyLocaleZeroVietnameseLeak(
  page: Page,
  adminUrl: string,
  mfaCode: string,
  locale: "en-US" | "ja-JP",
  screenshotPath: string
) {
  await page.evaluate((nextLoc) => {
    window.localStorage.setItem("cowok_locale", nextLoc);
  }, locale);
  await page.goto(adminUrl, { waitUntil: "domcontentloaded", timeout: NAV_TIMEOUT_MS });
  await signInSuperAdmin(page, mfaCode);
  await setPortalTheme(page, "warm");

  await page.getByTestId("superadmin-tab-plugin-catalog").click();
  const catalogPanel = page.getByTestId("superadmin-panel-plugin-catalog");
  await expect(catalogPanel).toBeVisible();
  await expect(page.locator("[data-testid^='catalog-row-']").first()).toBeVisible({
    timeout: ACTION_TIMEOUT_MS,
  });

  const catalogText = await catalogPanel.innerText();
  expect(catalogText, `Vietnamese diacritic leaked in ${locale} Plugin Catalog`).not.toMatch(
    VIETNAMESE_DIACRITIC_REGEX
  );
  await page.screenshot({ path: screenshotPath, fullPage: true });

  // Open Right Drawer and check zero Vietnamese diacritics
  await page.locator("[data-testid^='catalog-row-']").first().click();
  const drawer = page.getByTestId("catalog-right-drawer");
  await expect(drawer).toBeVisible();
  const drawerText = await drawer.innerText();
  expect(drawerText, `Vietnamese diacritic leaked in ${locale} Right Drawer`).not.toMatch(
    VIETNAMESE_DIACRITIC_REGEX
  );
  await page.getByTestId("catalog-drawer-close-btn").click();
  await expect(drawer).toBeHidden();

  // Check Governance & Audit tab in the same locale
  await page.getByTestId("superadmin-tab-governance").click();
  const govPanel = page.getByTestId("superadmin-panel-governance");
  await expect(govPanel).toBeVisible();
  const govText = await govPanel.innerText();
  expect(govText, `Vietnamese diacritic leaked in ${locale} Governance tab`).not.toMatch(
    VIETNAMESE_DIACRITIC_REGEX
  );
}

test.describe("Story 14.3: 4-Channel Distribution Ops, Plugin Catalog, A/B & Licensing E2E", () => {
  test("verifies all 6 live Distribution Ops tabs, full button-to-backend flows, mobile 390px, and 3 locales (vi-VN, en-US, ja-JP)", async ({
    browser,
  }) => {
    test.setTimeout(180_000);
    const storyDir = path.resolve(
      process.cwd(),
      "_iwish-output/3. Development/1. Epic & Story/VS-07. Cloud Control Plane, Non-Tech Support Desk & SuperAdmin Portal/Epic-14/Story-14.3"
    );
    const evidenceDir = path.join(storyDir, "qa", "evidence");
    const videoDir = path.join(evidenceDir, "videos");
    fs.mkdirSync(videoDir, { recursive: true });

    const qaNonce = process.env.QA_NONCE_TOKEN || crypto.randomBytes(NONCE_BYTES).toString("hex");
    const suffix = qaNonce.slice(0, 8);
    fs.writeFileSync(
      path.join(evidenceDir, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, storyId: "14.3", generatedAt: new Date().toISOString() }, null, 2)
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
    const mfaCode = process.env.COWOK_CLOUD_ADMIN_MFA_CODE || "314159";
    await signInSuperAdmin(page, mfaCode);

    // 1..5 Full button-to-backend flows across Catalog, A/B, Canary, Quarantine & Licenses
    const createdPluginId = await exercisePluginCatalogTab(page, evidenceDir, suffix);
    const { expKey, badSkillId } = await exerciseAbCanaryQuarantineLicenseTabs(page, suffix);

    // 6. Governance & Immutable Audit Logs: verify real entries from Tabs 1..5 & test role filter dropdown
    await page.getByTestId("superadmin-tab-governance").click();
    await expect(page.locator("[data-ff-id='superadmin-governance-audit-panel']")).toBeVisible();
    await expect(page.getByTestId("superadmin-rbac-matrix")).toBeVisible();
    const auditTable = page.getByTestId("governance-audit-table");
    await expect(auditTable).toContainText(createdPluginId, { timeout: ACTION_TIMEOUT_MS });
    await expect(auditTable).toContainText(expKey);
    await expect(auditTable).toContainText(badSkillId);

    await page.getByTestId("audit-role-filter").selectOption("SUPER_ADMIN");
    await expect(page.locator("[data-testid^='audit-row-']").first()).toBeVisible();
    await page.getByTestId("audit-role-filter").selectOption("ANALYST_READONLY");
    await page.getByTestId("audit-role-filter").selectOption("ALL");
    await expect(page.locator("[data-testid^='audit-row-']").first()).toBeVisible();

    // Mobile 390px responsive check
    await page.setViewportSize({ width: MOBILE_WIDTH, height: MOBILE_HEIGHT });
    await setPortalTheme(page, "warm");
    await page.getByTestId("superadmin-tab-plugin-catalog").click();
    const hasOverflow = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth
    );
    expect(hasOverflow).toBe(false);
    await page.screenshot({ path: path.join(evidenceDir, "mobile-390-plugin-catalog.png"), fullPage: true });

    // Multi-locale checks (en-US & ja-JP) with zero Vietnamese diacritic leak
    await page.setViewportSize({ width: DESKTOP_WIDTH, height: DESKTOP_HEIGHT });
    await verifyLocaleZeroVietnameseLeak(
      page,
      adminUrl,
      mfaCode,
      "en-US",
      path.join(evidenceDir, "desktop-en-plugin-catalog.png")
    );
    await verifyLocaleZeroVietnameseLeak(
      page,
      adminUrl,
      mfaCode,
      "ja-JP",
      path.join(evidenceDir, "desktop-ja-plugin-catalog.png")
    );

    expect(pageErrors).toEqual([]);
    expect(unexpectedConsoleErrors).toEqual([]);
    expect(serverErrors).toEqual([]);

    await context.close();
  });
});
