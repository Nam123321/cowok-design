// @story 02.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-02/Story-02.4"
);
const EVIDENCE_DIR = path.join(STORY_DIR, "qa", "evidence");

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
  return sessionToken;
}

test.describe("Story 02.4 Live Browser E2E Full-Userflow (V18 SSOT, AppShell Lock, Export & Skill Router)", () => {
  test("verifies live AppShell lock toggle, SSOT export toast, Pattern Drawer (Light/Dark/Mobile), and 3-Zone Skill Router", async ({
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
      recordHar: { path: path.join(EVIDENCE_DIR, "story-02.4-network.har"), mode: "minimal" },
      recordVideo: { dir: EVIDENCE_DIR, size: { width: 1280, height: 720 } },
    });

    await context.route(`http://127.0.0.1:${targetPort}/**`, async (route) => {
      const headers = {
        ...route.request().headers(),
        "x-iwish-qa-nonce": qaNonce,
        "x-tenant-id": "local-default",
      };
      await route.continue({ headers });
    });

    const page = await context.newPage();
    const pageErrors: string[] = [];
    const consoleErrors: string[] = [];
    const server500s: string[] = [];
    let expectingCrossPortal403 = false;

    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        const text = msg.text();
        if (expectingCrossPortal403 && text.includes("status of 403 (Forbidden)")) {
          return;
        }
        consoleErrors.push(text);
      }
    });
    page.on("response", (res) => {
      if (res.status() >= 500) server500s.push(`${res.status()} ${res.url()}`);
    });

    try {
      const sessionToken = await bootEditorSession(page, baseUrl, qaNonce);

      const lockPanel = page.locator("[data-ff-id='appshell-lock-panel']").first();
      const lockBadge = page.getByTestId("appshell-lock-badge").first();
      const lockToggle = page.getByTestId("appshell-lock-toggle").first();
      const exportBtn = page.locator("[data-ff-id='ssot-export-button']").first();
      const patternsBtn = page.getByTestId("ssot-patterns-trigger").first();

      await expect(lockPanel).toBeVisible({ timeout: 10000 });
      await expect(exportBtn).toBeVisible({ timeout: 10000 });
      await expect(patternsBtn).toBeVisible({ timeout: 10000 });
      await expect(lockBadge).toHaveText(/Unlocked|Chưa khóa/);
      await expect(lockToggle).toBeEnabled({ timeout: 10000 });

      // 1. Live UI AppShell Lock Acquire
      const lockRespPromise = page.waitForResponse(
        (r) => r.url().includes("/api/v1/ssot/appshell/lock") && r.request().method() === "POST"
      );
      await lockToggle.click();
      const lockResp = await lockRespPromise;
      expect(lockResp.status()).toBe(200);
      await expect(lockBadge).toHaveText(/Shell locked|Đã khóa khung/);

      // 2. Live UI SSOT Export -> Toast verification
      const exportRespPromise = page.waitForResponse(
        (r) => r.url().includes("/api/v1/ssot/export") && r.request().method() === "POST"
      );
      await exportBtn.click();
      const exportResp = await exportRespPromise;
      expect(exportResp.status()).toBe(200);
      await expect(page.getByTestId("ssot-export-toast")).toContainText(/SSOT exported cleanly/);

      // 3. Live UI Pattern Registry Drawer + Light Screenshot
      await patternsBtn.click();
      const drawer = page.locator("[data-ff-id='pattern-registry-drawer']").first();
      await expect(drawer).toBeVisible({ timeout: 5000 });
      await page.waitForTimeout(200);
      await page.screenshot({ path: path.join(EVIDENCE_DIR, "desktop-light-drawer.png"), fullPage: false });

      // Switch root theme to dark (night) & verify Dark Mode screenshot
      await page.evaluate(() => {
        const root = document.querySelector(".workbench-shell");
        if (root) {
          root.setAttribute("data-theme", "night");
          root.className = "workbench-shell theme-night";
        }
      });
      await page.waitForTimeout(150);
      await page.screenshot({ path: path.join(EVIDENCE_DIR, "desktop-dark-drawer.png"), fullPage: false });

      // Close drawer via Escape & Unlock shell
      await page.keyboard.press("Escape");
      await expect(drawer).toBeHidden({ timeout: 5000 });

      const unlockRespPromise = page.waitForResponse(
        (r) => r.url().includes("/api/v1/ssot/appshell/lock") && r.request().method() === "POST"
      );
      await lockToggle.click();
      const unlockResp = await unlockRespPromise;
      expect(unlockResp.status()).toBe(200);
      await expect(lockBadge).toHaveText(/Unlocked|Chưa khóa/);

      // 4. Mobile viewport screenshot check
      await page.setViewportSize({ width: 390, height: 844 });
      await page.waitForTimeout(150);
      await page.screenshot({ path: path.join(EVIDENCE_DIR, "mobile-workbench.png"), fullPage: false });

      // 5. Verify 3-Zone Skill Router live in browser context
      const skillRouterOutcome = await page.evaluate(
        async ({ tok, nonce }) => {
          const headers = {
            "Content-Type": "application/json",
            "X-Cowok-Session-Token": tok,
            "x-iwish-qa-nonce": nonce,
            "x-cowok-portal": "DeveloperPortal",
          };
          const seededProjectId = "11111111-1111-4111-8111-111111111111";
          const r1 = await fetch("/api/v1/skills/route", {
            method: "POST",
            headers,
            body: JSON.stringify({
              projectId: seededProjectId,
              portalId: "DeveloperPortal",
              intentSummary: "Design billing metrics bento grid",
              requestedSlugs: ["typography", "dashboard-ux"],
              maxSpineTokens: 250,
            }),
          });
          const j1 = await r1.json();

          const r2 = await fetch("/api/v1/skills/route", {
            method: "POST",
            headers,
            body: JSON.stringify({
              projectId: seededProjectId,
              portalId: "DeveloperPortal",
              intentSummary: "Completely different prompt with same craft skills",
              requestedSlugs: ["dashboard-ux", "typography"],
              maxSpineTokens: 250,
            }),
          });
          const j2 = await r2.json();

          return { s1: r1.status, j1, s2: r2.status, j2 };
        },
        { tok: sessionToken, nonce: qaNonce }
      );

      expect(skillRouterOutcome.s1).toBe(200);
      expect(skillRouterOutcome.s2).toBe(200);
      expect(skillRouterOutcome.j1.data.spineTokens).toBeLessThanOrEqual(250);
      expect(skillRouterOutcome.j1.data.selectedSlugs).toEqual(["dashboard-ux", "typography"]);
      expect(skillRouterOutcome.j1.data.prefixSha256).toBe(skillRouterOutcome.j2.data.prefixSha256);

      // 6. Verify Cross-Portal Isolation live in browser context (expected negative 403 check)
      expectingCrossPortal403 = true;
      const crossPortalOutcome = await page.evaluate(
        async ({ tok, nonce }) => {
          const crossPortal = await fetch("/api/v1/skills/route", {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-Cowok-Session-Token": tok,
              "x-iwish-qa-nonce": nonce,
              "x-cowok-portal": "SalesApp",
            },
            body: JSON.stringify({
              projectId: "11111111-1111-4111-8111-111111111111",
              portalId: "DeveloperPortal",
              intentSummary: "Cross portal attempt",
              maxSpineTokens: 250,
            }),
          });
          return {
            crossStatus: crossPortal.status,
            crossJson: await crossPortal.json(),
          };
        },
        { tok: sessionToken, nonce: qaNonce }
      );
      expectingCrossPortal403 = false;

      expect(crossPortalOutcome.crossStatus).toBe(403);
      expect(crossPortalOutcome.crossJson.error.code).toBe("CROSS_PORTAL_LEAK_BLOCKED");

      expect(pageErrors).toEqual([]);
      expect(consoleErrors).toEqual([]);
      expect(server500s).toEqual([]);
    } finally {
      await context.close();
    }
  });
});
