// @story 06.1
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06
// @cover E2E-01..E2E-04
import fs from "node:fs";
import path from "node:path";
import { test, expect } from "@playwright/test";

const EVIDENCE_DIR = process.env.EVIDENCE_DIR || "";
const QA_NONCE = process.env.QA_NONCE || "";

test.use({
  extraHTTPHeaders: QA_NONCE ? { "X-Iwish-QA-Nonce": QA_NONCE } : undefined,
});

async function maybeCaptureScreenshot(page: import("@playwright/test").Page, filename: string) {
  if (!EVIDENCE_DIR) return;
  fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
  await page.screenshot({ path: path.join(EVIDENCE_DIR, filename), fullPage: false });
}

async function bootWorkbenchSession(page: import("@playwright/test").Page) {
  await page.setViewportSize({ width: 1440, height: 900 });
  const targetPort = process.env.TARGET_PORT || "5173";
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
          Authorization: `Bearer ${auth.sessionToken}`,
          "X-Cowok-Session-Token": auth.sessionToken,
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

test.describe("Story 06.1: HTML Slide Studio 16:9 / 16:10 / Auto Stage & HostRuntimeContract", () => {
  test("E2E-01 & E2E-02: deck tab switch auto-collapses chat rail, preserves aspect/zoom controls & slide-scale >= 0.45 at 1440px", async ({
    page,
  }) => {
    await bootWorkbenchSession(page);

    const deckTab = page.getByTestId("canvas-tab-tab-slide");
    await expect(deckTab).toBeVisible();
    await deckTab.click();

    await expect(page.getByTestId("walkthrough-playback-btn")).toBeVisible();
    await expect(page.getByTestId("btn-present-deck")).toHaveCount(0);
    await expect(page.getByTestId("aspect-ratio-pill-group")).toBeVisible();
    await expect(page.getByTestId("zoom-pill-group")).toBeVisible();

    const frame = page.locator(".live-html-frame");
    await expect(frame).toHaveAttribute("data-slide-stage", "true");
    await expect(frame).toHaveAttribute("data-aspect", "16-9");

    const slideScaleRaw = await frame.evaluate((el) => getComputedStyle(el).getPropertyValue("--slide-scale").trim());
    expect(slideScaleRaw).not.toBe("");
    const slideScale = Number(slideScaleRaw);
    expect(Number.isFinite(slideScale)).toBe(true);
    expect(slideScale).toBeGreaterThanOrEqual(0.45);
    expect(slideScale).toBeLessThan(1.0);
    await maybeCaptureScreenshot(page, "01-presentation-stage-16-9.png");

    // Switch 16:10 and Auto
    await page.getByTestId("aspect-pill-16-10").click();
    await expect(frame).toHaveAttribute("data-aspect", "16-10");
    await page.getByTestId("aspect-pill-auto").click();
    await expect(frame).toHaveAttribute("data-aspect", "auto");

    // Zoom 100% Pan & back to Fit
    await page.getByTestId("zoom-pill-pan-100").click();
    await expect(frame).toHaveAttribute("data-zoom-mode", "pan-100");
    await page.getByTestId("zoom-pill-fit").click();
    await expect(frame).toHaveAttribute("data-zoom-mode", "fit");

    // Narrow viewport collapses Row 2 multi-option pill groups into compact dropdown selectors (FE viewport hidden in Presentation)
    await page.setViewportSize({ width: 880, height: 900 });
    await expect(page.getByTestId("fixture-substate-dropdown")).toBeVisible();
    await expect(page.getByTestId("viewport-preset-dropdown")).toHaveCount(0);
    await expect(page.getByTestId("aspect-ratio-dropdown")).toBeVisible();
    await expect(page.getByTestId("zoom-mode-dropdown")).toBeVisible();
    await page.setViewportSize({ width: 1440, height: 900 });

    // Switch back to Userflow app tab keeps FE stage functional
    await page.getByTestId("canvas-tab-tab-userflow").click();
    await expect(page.getByTestId("aspect-ratio-toggle-btn")).toBeVisible();
  });

  test("E2E-03 & E2E-04: Present vs Walkthrough mutual exclusivity, floating bar parity (without Dual Screen toggle), Esc exit, and Dark-in-Warm isolation", async ({
    page,
  }) => {
    await bootWorkbenchSession(page);
    await page.getByTestId("canvas-tab-tab-slide").click();

    const iframe = page.getByTestId("sandboxed-html-iframe");
    await expect(iframe).toHaveAttribute("sandbox", "allow-scripts allow-forms");

    // Dark-in-Warm isolation: stage mat must not force stark white background-color rgb(255, 255, 255)
    const stageBg = await page.locator(".live-html-frame").evaluate((el) => getComputedStyle(el).backgroundColor);
    expect(stageBg).not.toBe("rgb(255, 255, 255)");

    // Unified Walkthrough button enters fullscreen Presentation + shows WalkthroughFloatingBar (WITHOUT Dual Screen toggle), ArrowRight/Next advances slide, Esc exits
    await page.getByTestId("walkthrough-playback-btn").click();
    await expect(page.locator(".companion-canvas-tab-container")).toHaveAttribute("data-deck-presenting", "true");
    await expect(page.getByTestId("walkthrough-floating-bar")).toBeVisible();
    await expect(page.getByTestId("walkthrough-mode-dual")).toHaveCount(0);
    await page.getByTestId("walkthrough-next-btn").click();
    await expect(page.frameLocator('[data-testid="sandboxed-html-iframe"]').locator("body")).toContainText("SLIDE 02 • ARCHITECTURE");
    await maybeCaptureScreenshot(page, "02-presentation-walkthrough.png");

    await page.keyboard.press("Escape");
    await expect(page.locator(".companion-canvas-tab-container")).toHaveAttribute("data-deck-presenting", "false");
    await expect(page.getByTestId("walkthrough-floating-bar")).toHaveCount(0);
    await expect(page.getByTestId("lens-slide-cms-btn")).toBeVisible();
  });
});
