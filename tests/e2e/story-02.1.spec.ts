// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import { expect, test } from "@playwright/test";

test.describe("Story 02.1 Context Meter Widget & Context Assembly E2E", () => {
  test("mounts ContextMeterWidget inside WorkbenchTopNav and opens portal/profile popover", async ({ page }) => {
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(baseUrl, { waitUntil: "networkidle" });
    await expect(page.locator("#root")).not.toBeEmpty({ timeout: 15000 });

    const wizardHeading = page.getByRole("heading", {
      name: /Chào mừng đến Cowok-Design|Welcome to Cowok-Design/i,
    });
    if (await wizardHeading.isVisible().catch(() => false)) {
      await page.getByRole("button", { name: /Tiếp tục|Next/i }).click();
      await page.getByRole("button", { name: /Tiếp tục|Next/i }).click();
      await page.getByRole("button", { name: /Hoàn tất/i }).click();
      await expect(wizardHeading).toBeHidden({ timeout: 10000 });
    }

    const pageErrors: string[] = [];
    page.on("pageerror", (err) => pageErrors.push(err.message));

    const trigger = page.locator("[data-ff-id='context-meter-trigger']").first();
    await expect(trigger).toBeVisible({ timeout: 10000 });
    await trigger.click();

    const popover = page.locator("[data-ff-id='context-meter-popover']").first();
    await expect(popover).toBeVisible({ timeout: 5000 });
    await expect(page.locator("[data-ff-id='context-meter-portal-select']")).toBeVisible();
    await expect(page.locator("[data-ff-id='context-meter-profile-select']")).toBeVisible();

    const telemetryReq = page.waitForResponse(
      (resp) => resp.url().includes("/api/v1/context/telemetry") && resp.status() < 500,
      { timeout: 10000 }
    );
    await page.locator("[data-ff-id='context-meter-profile-select']").selectOption("B");
    await expect(trigger).toContainText("B");
    const telemetryResp = await telemetryReq;
    expect(telemetryResp.status()).toBeLessThan(500);

    const assembleBtn = page.locator("[data-ff-id='context-meter-refresh-btn']");
    if (await assembleBtn.isEnabled()) {
      const assembleReq = page.waitForResponse(
        (resp) => resp.url().includes("/api/v1/context/assemble") && resp.request().method() === "POST",
        { timeout: 10000 }
      );
      await assembleBtn.click();
      const assembleResp = await assembleReq;
      expect(assembleResp.status()).toBeLessThan(500);
    }

    await page.keyboard.press("Escape");
    await expect(popover).toBeHidden({ timeout: 3000 });
    expect(pageErrors).toEqual([]);
  });
});
