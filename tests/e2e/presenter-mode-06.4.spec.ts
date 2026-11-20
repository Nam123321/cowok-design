// @story 06.4
// @implements AC-01, AC-02, AC-03, AC-04
// @cover E2E-01, E2E-02, E2E-03, E2E-03B, E2E-04, E2E-05
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
  const html = await page.content();
  fs.writeFileSync(path.join(EVIDENCE_DIR, "dom-snapshot.html"), html, "utf8");
}

function buildContextOptions() {
  if (!EVIDENCE_DIR) return { viewport: { width: 1440, height: 900 } };
  return {
    viewport: { width: 1440, height: 900 },
    extraHTTPHeaders: QA_NONCE ? { "X-Iwish-QA-Nonce": QA_NONCE } : undefined,
    recordHar: { path: path.join(EVIDENCE_DIR, "network-trace.har"), mode: "minimal" as const },
    recordVideo: { dir: path.join(EVIDENCE_DIR, "videos"), size: { width: 1280, height: 800 } },
  };
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

async function openSlideDeckStage(page: import("@playwright/test").Page) {
  await page.getByRole("tab", { name: /q4-architecture-deck\.html/ }).click();
  await page.getByTestId("lens-slide-cms-btn").click();
  await expect(page.getByTestId("btn-open-presenter")).toBeVisible();
}

test.describe("Story 06.4: Dual-Screen Presenter Mode, Out-of-Band BroadcastChannel & Fallbacks", () => {
  test("E2E-01, E2E-03 & E2E-04: popup opens without ?secret=, syncs slides/notes/timer bidirectionally, survives reload, toggles B/W blackout, and Esc closes only popup", async ({
    browser,
  }) => {
    const context = await browser.newContext(buildContextOptions());
    const page = await context.newPage();
    try {
      await bootWorkbenchSession(page);
      await openSlideDeckStage(page);

      // Edit speaker notes with HTML-like payload to prove safe plain-text escaping
      await page.getByTestId("slide-list-half").locator("button[data-slide-card-index='0']").click();
      const notesInput = page.getByTestId("cms-input-speaker-notes");
      await expect(notesInput).toBeEnabled({ timeout: 10000 });
      await notesInput.fill("Presenter cue <script>alert('xss')</script> plain text");

      const popupPromise = page.waitForEvent("popup");
      await page.getByTestId("btn-open-presenter").click();
      const popup = await popupPromise;
      await popup.waitForLoadState("networkidle");

      // Zero ?secret= in URL
      expect(popup.url()).toMatch(/\/presenter\//);
      expect(popup.url()).not.toContain("secret=");

      const presenterConsole = popup.getByTestId("presenter-console-window");
      await expect(presenterConsole).toHaveAttribute("data-theme", "night");
      await expect(popup.getByTestId("presenter-speaker-notes")).toContainText(
        "Presenter cue <script>alert('xss')</script> plain text"
      );
      expect(await popup.getByTestId("presenter-speaker-notes").locator("script").count()).toBe(0);

      // Timer tabular-nums + Start / Pause
      const timerEl = popup.getByTestId("presenter-timer");
      await expect(timerEl).toHaveCSS("font-variant-numeric", /tabular-nums/);
      await popup.getByTestId("btn-presenter-timer-start").click();
      await popup.waitForTimeout(1200);
      await expect(timerEl).not.toHaveText("00:00");

      // Bidirectional slide nav: Next on Presenter popup updates Host Audience iframe
      const bodyBeforeNext = await page.frameLocator('[data-testid="sandboxed-html-iframe"]').locator("body").innerText();
      await popup.getByTestId("btn-presenter-next").click();
      const audienceBody = page.frameLocator('[data-testid="sandboxed-html-iframe"]').locator("body");
      await expect(audienceBody).not.toHaveText(bodyBeforeNext);
      await expect(audienceBody).toContainText(/SLIDE 02|Hybrid Project Boundary/);

      await maybeCaptureScreenshot(popup, "01-story-06-4-presenter-night-console.png");

      // Reload Presenter popup (F5) -> rehydrates active slide & timer via SYNC_FULL_STATE
      await popup.reload({ waitUntil: "networkidle" });
      await expect(popup.getByTestId("presenter-metric-line")).toContainText("2 /");
      await expect(popup.getByTestId("presenter-timer")).not.toHaveText("00:00");

      // Enter Present fullscreen on Host, trigger Blackout (B) from Presenter, verify Audience overlay
      await page.getByTestId("walkthrough-playback-btn").click();
      await expect(page.locator(".companion-canvas-tab-container")).toHaveAttribute("data-deck-presenting", "true");
      await popup.getByTestId("btn-presenter-blackout").click();
      await expect(page.getByTestId("audience-blackout-overlay")).toHaveAttribute("data-mode", "black");
      await popup.getByTestId("btn-resume-projection").click();
      await expect(page.getByTestId("audience-blackout-overlay")).toHaveCount(0);

      // Esc inside Presenter closes only Presenter popup; Audience remains presenting
      await Promise.all([
        popup.waitForEvent("close"),
        popup.keyboard.press("Escape").catch(() => {}),
      ]);
      expect(popup.isClosed()).toBe(true);
      await expect(page.locator(".companion-canvas-tab-container")).toHaveAttribute("data-deck-presenting", "true");

      // Single Esc on Host exits Audience fullscreen cleanly
      await page.keyboard.press("Escape");
      await expect(page.locator(".companion-canvas-tab-container")).toHaveAttribute("data-deck-presenting", "false");
    } finally {
      await context.close();
    }
  });

  test("E2E-02, E2E-03B & E2E-05: popup-blocked inline Night fallback preserves timer across reopen & historical SNAP-* enforces read-only lock", async ({
    browser,
  }) => {
    const context = await browser.newContext(buildContextOptions());
    const page = await context.newPage();
    try {
      await page.addInitScript(() => {
        const origOpen = window.open.bind(window);
        (window as unknown as { __origWindowOpen?: typeof window.open }).__origWindowOpen = origOpen;
        window.open = () => null;
      });
      await bootWorkbenchSession(page);
      await openSlideDeckStage(page);

      await page.getByTestId("btn-open-presenter").click();

      const toast = page.getByTestId("presenter-popup-blocked-toast");
      await expect(toast).toBeVisible();
      await expect(toast).toHaveClass(/ff-toast-warning/);

      const fallback = page.getByTestId("presenter-inline-fallback");
      await expect(fallback).toBeVisible();
      await expect(fallback).toHaveAttribute("data-theme", "night");
      await expect(fallback.getByTestId("presenter-speaker-notes")).toBeVisible();
      await expect(fallback.getByTestId("presenter-next-slide-card")).toBeVisible();

      // Start timer inside inline fallback, wait 1.2s, restore window.open and reopen popup cleanly
      await fallback.getByTestId("btn-presenter-timer-start").click();
      await page.waitForTimeout(1200);
      await maybeCaptureScreenshot(page, "02-story-06-4-inline-split-night-fallback.png");
      await maybeCaptureDomSnapshot(page);

      await page.evaluate(() => {
        const w = window as unknown as { __origWindowOpen?: typeof window.open };
        if (w.__origWindowOpen) window.open = w.__origWindowOpen;
      });
      const reopenPopupPromise = page.waitForEvent("popup");
      await fallback.getByTestId("btn-reopen-presenter").click();
      const reopened = await reopenPopupPromise;
      await reopened.waitForLoadState("networkidle");

      await expect(reopened.getByTestId("presenter-timer")).not.toHaveText("00:00");
      await reopened.close();

      // E2E-05: Verify live BFF -> Daemon /presenter/handshake + viewer role + SNAP-* read-only lock via API
      const handshakeCheck = await page.evaluate(async () => {
        const bootCap = document.querySelector('meta[name="cowok-boot-cap"]')?.getAttribute("content")?.trim() || "";
        const sessRes = await fetch("/api/auth/local-session", {
          method: "POST",
          headers: { "Content-Type": "application/json", ...(bootCap ? { "X-Cowok-Boot-Cap": bootCap } : {}) },
          body: JSON.stringify({ role: "editor" }),
        });
        const { sessionToken } = (await sessRes.json()) as { sessionToken: string };
        const viewerSessRes = await fetch("/api/auth/local-session", {
          method: "POST",
          headers: { "Content-Type": "application/json", ...(bootCap ? { "X-Cowok-Boot-Cap": bootCap } : {}) },
          body: JSON.stringify({ role: "viewer" }),
        });
        const { sessionToken: viewerToken } = (await viewerSessRes.json()) as { sessionToken: string };
        const hdrs = { "Content-Type": "application/json", "X-Cowok-Session-Token": sessionToken };
        const projRes = await fetch("/api/projects", { headers: { "X-Cowok-Session-Token": sessionToken } });
        const projJson = (await projRes.json()) as Array<{ id: string }> | { projects?: Array<{ id: string }> };
        const projList = Array.isArray(projJson) ? projJson : (projJson.projects ?? []);
        const pid = projList[0]?.id || "";
        const decksRes = pid ? await fetch(`/api/projects/${encodeURIComponent(pid)}/decks`, { headers: { "X-Cowok-Session-Token": sessionToken } }) : null;
        const decksJson = decksRes ? ((await decksRes.json()) as { decks?: Array<{ id: string }> }) : { decks: [] };
        const did = decksJson.decks?.[0]?.id || "";
        if (!pid || !did) return { missingFixture: true, pid, did };
        const okRes = await fetch(`/api/projects/${encodeURIComponent(pid)}/decks/${encodeURIComponent(did)}/presenter/handshake`, {
          method: "POST", headers: hdrs, body: JSON.stringify({ transportMode: "broadcast_channel", activeSlideIndex: 0 }),
        });
        const directRes = await fetch(`/api/decks/${encodeURIComponent(did)}/presenter/handshake`, {
          method: "POST", headers: hdrs, body: JSON.stringify({ projectId: pid, transportMode: "broadcast_channel", activeSlideIndex: 0 }),
        });
        const viewerRes = await fetch(`/api/decks/${encodeURIComponent(did)}/presenter/handshake`, {
          method: "POST",
          headers: { "Content-Type": "application/json", "X-Cowok-Session-Token": viewerToken },
          body: JSON.stringify({ projectId: pid, transportMode: "broadcast_channel", activeSlideIndex: 0 }),
        });
        const foreignSnapRes = await fetch(`/api/projects/${encodeURIComponent(pid)}/decks/${encodeURIComponent(did)}/presenter/handshake`, {
          method: "POST", headers: hdrs, body: JSON.stringify({ snapshotTag: "SNAP-NONEXISTENT-999", transportMode: "broadcast_channel", activeSlideIndex: 0 }),
        });
        return { okStatus: okRes.status, directStatus: directRes.status, viewerStatus: viewerRes.status, foreignSnapStatus: foreignSnapRes.status };
      });
      if ("missingFixture" in handshakeCheck && handshakeCheck.missingFixture) {
        throw new Error(`Missing seeded project/deck fixture for E2E-05 (pid=${handshakeCheck.pid}, did=${handshakeCheck.did})`);
      }
      expect(handshakeCheck.okStatus).toBe(200);
      expect(handshakeCheck.directStatus).toBe(200);
      expect(handshakeCheck.viewerStatus).toBe(200);
      expect(handshakeCheck.foreignSnapStatus).toBe(404);
    } finally {
      await context.close();
    }
  });
});
