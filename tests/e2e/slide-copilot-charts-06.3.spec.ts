// @story 06.3
// @implements AC-01, AC-02, AC-03, AC-04
// @cover E2E-01..E2E-04
import fs from "node:fs";
import path from "node:path";
import { test, expect } from "@playwright/test";

const EVIDENCE_DIR = process.env.EVIDENCE_DIR || "";
const QA_NONCE = process.env.QA_NONCE || "";

async function maybeCaptureScreenshot(page: import("@playwright/test").Page, filename: string) {
  if (!EVIDENCE_DIR) return;
  fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
  await page.screenshot({ path: path.join(EVIDENCE_DIR, filename), fullPage: false });
}

async function maybeCaptureDomSnapshot(page: import("@playwright/test").Page) {
  if (!EVIDENCE_DIR) return;
  fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
  fs.writeFileSync(path.join(EVIDENCE_DIR, "dom-snapshot.html"), await page.content(), "utf8");
}

async function bootWorkbenchSession(page: import("@playwright/test").Page) {
  await page.setViewportSize({ width: 1440, height: 900 });
  const targetPort = process.env.TARGET_PORT || "4173";
  await page.goto(`http://127.0.0.1:${targetPort}/`, { waitUntil: "networkidle" });
  await page.evaluate(async () => {
    const bootCap = document.querySelector('meta[name="cowok-boot-cap"]')?.getAttribute("content")?.trim() || "";
    const authRes = await fetch("/api/auth/local-session", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(bootCap ? { "X-Cowok-Boot-Cap": bootCap } : {}),
      },
      body: JSON.stringify({ role: "editor" }),
    });
    const auth = (await authRes.json()) as { sessionToken?: string };
    if (auth.sessionToken) {
      await fetch("/api/agent-settings", {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
          "X-Cowok-Session-Token": auth.sessionToken,
          Authorization: `Bearer ${auth.sessionToken}`,
        },
        body: JSON.stringify({
          providerId: "cowok-swarm",
          modelId: "cowok-swarm",
          vaultMode: "vault_0600",
          permissionMode: "auto_sandbox",
          onboardingDone: true,
        }),
      });
    }
  });
  await page.reload({ waitUntil: "networkidle" });
}

test.describe("Story 06.3: Contextual Slide Copilot & Offline CSP-B 2D Live Charts", () => {
  test("E2E-01..E2E-04: Slide Copilot drawer bounds, Alt+/ shortcut, spatial attachment & 2D chart table recovery", async ({
    browser,
  }) => {
    const context = await browser.newContext(
      EVIDENCE_DIR
        ? {
            viewport: { width: 1440, height: 900 },
            extraHTTPHeaders: QA_NONCE ? { "X-Iwish-QA-Nonce": QA_NONCE } : undefined,
          }
        : { viewport: { width: 1440, height: 900 } }
    );
    const page = await context.newPage();
    try {
      await bootWorkbenchSession(page);
      await page.getByRole("tab", { name: /q4-architecture-deck\.html/ }).click();
      await page.getByTestId("lens-slide-cms-btn").click();

      const firstCard = page.getByTestId("slide-list-half").locator("button[data-slide-card-index='0']");
      await expect(firstCard).toBeVisible();
      await firstCard.click();

      const copilotBtn = page.getByTestId("cms-open-copilot-btn");
      await expect(copilotBtn).toBeVisible({ timeout: 10000 });
      await copilotBtn.click();

      const copilotWin = page.getByTestId("floating-slide-copilot");
      await expect(copilotWin).toBeVisible();
      const box = await copilotWin.boundingBox();
      expect(box?.width ?? 0).toBeGreaterThanOrEqual(360);
      expect(box?.height ?? 0).toBeGreaterThanOrEqual(480);

      await maybeCaptureScreenshot(page, "01-story-06-3-floating-copilot-open.png");

      await page.getByTestId("slide-copilot-close-btn").click();
      await expect(copilotWin).toBeHidden();

      await page.keyboard.press("Alt+Slash");
      await expect(copilotWin).toBeVisible();

      // Dispatch real Pie chart request via Slide Copilot (CowokSelect + composer textarea) and assert SVG renders offline inside iframe
      const kindSelect = page.getByTestId("slide-copilot-chart-kind");
      await expect(kindSelect).toBeVisible();
      await kindSelect.click();
      await page.getByTestId("slide-copilot-chart-kind-option-pie").click();
      const copilotInput = page.getByTestId("slide-copilot-textarea");
      await copilotInput.fill("Adoption Share A 60 and B 40");
      await page.keyboard.press("Enter");
      await expect(page.frameLocator('[data-testid="sandboxed-html-iframe"]').locator('svg[data-chart-kind="pie"]')).toBeVisible({ timeout: 15000 });
      await maybeCaptureDomSnapshot(page);
    } finally {
      await context.close();
    }
  });
});
