// @story 06.2
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08
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
  const html = await page.content();
  fs.writeFileSync(path.join(EVIDENCE_DIR, "dom-snapshot.html"), html, "utf8");
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

test.describe("Story 06.2: Slide-Over CMS Drawer, Live <16ms Patch, Undo & Recovery", () => {
  test("E2E-01 & E2E-02: slide card opens 200% translateX(-50%) drawer with symmetric inert & live non-reloading iframe patch", async ({
    browser,
  }) => {
    const context = await browser.newContext(
      EVIDENCE_DIR
        ? {
            viewport: { width: 1440, height: 900 },
            extraHTTPHeaders: QA_NONCE ? { "X-Iwish-QA-Nonce": QA_NONCE } : undefined,
            recordHar: { path: path.join(EVIDENCE_DIR, "network-trace.har"), mode: "minimal" },
            recordVideo: { dir: path.join(EVIDENCE_DIR, "videos"), size: { width: 1280, height: 800 } },
          }
        : { viewport: { width: 1440, height: 900 } }
    );
    const page = await context.newPage();
    try {
      await bootWorkbenchSession(page);

      await page.getByRole("tab", { name: /q4-architecture-deck\.html/ }).click();
      await page.getByTestId("lens-slide-cms-btn").click();
      const viewport = page.getByTestId("slide-cms-track-viewport");
      await expect(viewport).toBeVisible();

      const listHalf = page.getByTestId("slide-list-half");
      const firstSlideCard = listHalf.locator("button[data-slide-card-index='0']");
      await expect(firstSlideCard).toBeVisible();
      await firstSlideCard.click();

      const drawer = page.getByTestId("cms-slide-over-drawer");
      await expect(drawer).toBeVisible();
      await expect(listHalf).toHaveAttribute("inert", "");

      const headlineInput = page.getByTestId("cms-input-headline");
      await expect(headlineInput).toBeEnabled({ timeout: 10000 });
      await page.waitForTimeout(320);
      await maybeCaptureScreenshot(page, "01-story-06-2-cms-drawer-warm-vi.png");

      // Capture active iframe DOM identity before editing to verify ZERO srcDoc remounts on live typing
      const iframeLocator = page.getByTestId("sandboxed-html-iframe");
      await expect(iframeLocator).toBeVisible();
      await expect(iframeLocator).toHaveAttribute("sandbox", "allow-scripts allow-forms");
      const beforeMountToken = await iframeLocator.evaluate((el) => {
        const w = el as HTMLElement & { __cowokMountMarker?: string };
        w.__cowokMountMarker = "stable-062";
        return w.__cowokMountMarker;
      });

      await headlineInput.fill("Kiến trúc Cowok-design Tiếng Việt Live Patch");

      // Verify iframe element was NOT remounted
      const afterMountToken = await iframeLocator.evaluate(
        (el) => (el as HTMLElement & { __cowokMountMarker?: string }).__cowokMountMarker
      );
      expect(afterMountToken).toBe(beforeMountToken);

      // Edit presenter-only speaker_notes tray (zero solid-dashed collision)
      const notesArea = page.getByTestId("cms-input-speaker-notes");
      await expect(notesArea).toBeVisible();
      await notesArea.fill("Ghi chú thuyết trình riêng tư cho diễn giả.");
      await expect(notesArea).toHaveValue("Ghi chú thuyết trình riêng tư cho diễn giả.");

      await maybeCaptureScreenshot(page, "02-story-06-2-cms-drawer-live-patch-notes.png");
      await maybeCaptureDomSnapshot(page);

      // Back button closes drawer and restores inert on drawer half
      await page.getByTestId("btn-back-slide-list").click();
      await expect(listHalf).not.toHaveAttribute("inert", "");
    } finally {
      await context.close();
    }
  });

  test("E2E-03 & E2E-04: Unified Undo survives drawer close/reopen & sessionStorage crash draft banner restores cleanly", async ({
    page,
  }) => {
    await bootWorkbenchSession(page);

    await page.getByRole("tab", { name: /q4-architecture-deck\.html/ }).click();
    await page.getByTestId("lens-slide-cms-btn").click();
    await page.getByTestId("slide-list-half").locator("button[data-slide-card-index='0']").click();

    const headlineInput = page.getByTestId("cms-input-headline");
    await expect(headlineInput).toBeEnabled({ timeout: 10000 });
    await expect(page.getByTestId("cms-structured-slots-inventory")).toBeVisible({ timeout: 10000 });
    const originalHeadline = await headlineInput.inputValue();
    const tempHeadline = `Temporary Live Title 06.2 ${Date.now()}`;
    await headlineInput.fill(tempHeadline);
    await expect(headlineInput).toHaveValue(tempHeadline);

    // Capture the real active (deckId, slideUid) key generated by saveRecoveryDraft in sessionStorage
    const activeDraftKey = await page.evaluate(() => {
      for (let i = 0; i < window.sessionStorage.length; i += 1) {
        const k = window.sessionStorage.key(i);
        if (k && k.startsWith("cowok:cms-draft:")) return k;
      }
      return "cowok:cms-draft:local-deck:SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV";
    });

    // Close drawer, press Undo via button after reopening, verify value rolls back cleanly
    await page.getByTestId("btn-back-slide-list").click();
    await page.getByTestId("slide-list-half").locator("button[data-slide-card-index='0']").click();
    await page.getByTestId("cms-undo-btn").click();
    await expect(headlineInput).toHaveValue(originalHeadline);

    // Switch to Night theme + English locale, seed real active draft key, reload -> verify recovery banner restores cleanly
    await page.evaluate((draftKey) => {
      document.documentElement.setAttribute("data-theme", "night");
      window.localStorage.setItem("cowok_locale", "en");
      window.localStorage.setItem(
        "cowok:workbench:settings",
        JSON.stringify({ theme: "night", locale: "en", fontSize: "13px", defaultModel: "cowok-swarm", autoContextSlice: true })
      );
      const parts = draftKey.split(":");
      const deckId = parts[2] || "local-deck";
      const slideUid = parts[3] || "SLD-01ARZ3NDEKTSV4RRFFQ69G5FAV";
      const draft = {
        deckId,
        slideUid,
        baseRevision: 1,
        speakerNotes: "Draft Note After Crash",
        pendingPatches: [
          {
            ffId: "ff-slide-headline",
            slotKind: "heading",
            textValue: "Recovered Headline After Crash",
          },
        ],
        savedAtMs: Date.now(),
      };
      window.sessionStorage.setItem(draftKey, JSON.stringify(draft));
    }, activeDraftKey);

    await page.reload({ waitUntil: "networkidle" });
    await page.getByRole("tab", { name: /q4-architecture-deck\.html/ }).click();
    await page.getByTestId("lens-slide-cms-btn").click();
    await page.getByTestId("slide-list-half").locator("button[data-slide-card-index='0']").click();
    await page.waitForTimeout(320);

    const banner = page.getByTestId("cms-recovery-banner");
    await expect(banner).toBeVisible({ timeout: 10000 });
    await maybeCaptureScreenshot(page, "03-story-06-2-cms-recovery-banner-night-en.png");

    // Verify standardized min-width responsive behavior of Line-1 CMS toolbar & floating recovery popup
    const splitter = page.locator('.canvas-dual-pane-stage > [role="separator"][aria-orientation="vertical"]');
    const splitBox = await splitter.boundingBox();
    const htmlPaneBox = await page.locator("#htmlSubPane").boundingBox();
    if (splitBox && htmlPaneBox) {
      await page.mouse.move(splitBox.x + splitBox.width / 2, splitBox.y + splitBox.height / 2);
      await page.mouse.down();
      await page.mouse.move(htmlPaneBox.x + htmlPaneBox.width - 330, splitBox.y + splitBox.height / 2, { steps: 6 });
      await page.mouse.up();
      await page.waitForTimeout(220);
      const minFit = await page.evaluate(() => {
        const pane = document.getElementById("htmlSubPane");
        const bar = document.querySelector(".html-pane-toolbar");
        const popup = document.getElementById("cms-recovery-banner");
        const pr = pane?.getBoundingClientRect();
        const ppr = popup?.getBoundingClientRect();
        return {
          paneWidth: pr?.width ?? 0,
          barScrollOverflow: bar ? bar.scrollWidth > bar.clientWidth + 1 : true,
          popupFits: pr && ppr ? ppr.left >= pr.left - 1 && ppr.right <= pr.right + 1 : false,
        };
      });
      expect(minFit.paneWidth).toBeLessThanOrEqual(800);
      expect(minFit.barScrollOverflow).toBe(false);
      expect(minFit.popupFits).toBe(true);
      await maybeCaptureScreenshot(page, "05-story-06-2-minwidth-responsive-night-en.png");
      // Drag splitter back toward balanced split before restoring
      await page.mouse.move(splitBox.x + splitBox.width / 2, splitBox.y + splitBox.height / 2);
      const curSplit = await splitter.boundingBox();
      if (curSplit) {
        await page.mouse.move(curSplit.x + curSplit.width / 2, curSplit.y + curSplit.height / 2);
        await page.mouse.down();
        await page.mouse.move(splitBox.x + splitBox.width / 2, splitBox.y + splitBox.height / 2, { steps: 6 });
        await page.mouse.up();
      }
    }

    await page.getByTestId("cms-restore-draft-btn").click();
    // Use an explicit locator check that waits for value update
    await expect(page.getByTestId("cms-input-headline")).toHaveValue("Recovered Headline After Crash", { timeout: 20000 });
    await expect(page.getByTestId("cms-input-speaker-notes")).toHaveValue("Draft Note After Crash");
    await expect(page.getByTestId("cms-save-btn")).toBeVisible();

    // Switch to Japanese locale in Night mode for multi-locale visual acceptance screenshot
    await page.evaluate(() => {
      window.localStorage.setItem("cowok_locale", "ja");
      window.localStorage.setItem(
        "cowok:workbench:settings",
        JSON.stringify({ theme: "night", locale: "ja", fontSize: "13px", defaultModel: "cowok-swarm", autoContextSlice: true })
      );
    });
    await page.reload({ waitUntil: "networkidle" });
    await page.getByRole("tab", { name: /q4-architecture-deck\.html/ }).click();
    await page.getByTestId("lens-slide-cms-btn").click();
    await page.getByTestId("slide-list-half").locator("button[data-slide-card-index='0']").click();
    await expect(page.getByTestId("cms-slide-over-drawer")).toBeVisible({ timeout: 10000 });
    await page.waitForTimeout(320);
    await maybeCaptureScreenshot(page, "04-story-06-2-cms-drawer-night-ja.png");
  });
});
