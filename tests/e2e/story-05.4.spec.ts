// @story 05.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-05/Story-05.4"
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

test.describe("Story 05.4 Full E2E Cognitive Progressive Userflow", () => {
  test("walks through Phase 1 Artistic Sketch + Minimalist Chips -> Night Semantic Sketch -> Phase 2 Style/Modes/Components Modal -> Phase 3 Official Polish + Sketch Lens -> Tier-2 Plain Receipt -> Phase 4 Approved v1 -> Mobile", async ({
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
      recordHar: { path: path.join(EVIDENCE_DIR, "story-05.4-network.har"), mode: "minimal" },
      recordVideo: { dir: EVIDENCE_DIR, size: { width: 1280, height: 800 } },
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
    let expectingConflict409 = false;

    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        const text = msg.text();
        if (expectingConflict409 && text.includes("409")) return;
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
      "x-cowok-portal": "SalesApp",
      "x-iwish-qa-nonce": qaNonce,
      "x-tenant-id": "local-default",
    };

    // STEP 1 (Phase 1 — Artistic Warm Sketch + Minimalist Cognitive Stepper + Steering Chip Persistence)
    const ensureSteeringPopoverOpen = async () => {
      const bar = page.locator("[data-ff-id='steering-chips-bar']");
      if (!(await bar.isVisible().catch(() => false))) {
        await page.getByTestId("steering-chips-popover-btn").click();
      }
      await expect(bar).toBeVisible({ timeout: 10000 });
    };

    await expect(page.locator("[data-ff-id='stage-skin-toggle']")).toBeVisible({ timeout: 15000 });
    await expect(page.getByTestId("steering-chips-popover-btn")).toBeVisible({ timeout: 10000 });
    await ensureSteeringPopoverOpen();
    await expect(page.getByTestId("cognitive-phase-stepper")).toBeVisible();
    await expect(page.getByTestId("promote-sketch-to-style-btn")).toBeVisible();
    await expect(page.getByTestId("toggle-sketch-lens-btn")).toHaveCount(0);

    const iframeSandbox = await page.locator("iframe[sandbox]").first().getAttribute("sandbox");
    if (iframeSandbox) {
      expect(iframeSandbox).not.toContain("allow-same-origin");
    }

    const htmlPane = page.locator("#htmlSubPane");
    await expect(htmlPane).toHaveAttribute("data-stage-skin", "sketch-paper");

    const patchPromise = page.waitForResponse(
      (r) => r.url().includes("/api/v1/patch") && r.request().method() === "POST"
    );
    await page.getByTestId("steering-chip-more-whitespace").click();
    const patchResp = await patchPromise;
    expect(patchResp.status()).toBe(200);
    const patchJson = (await patchResp.json()) as {
      ok: boolean;
      data: { astRevision: number; applyTier: string; htmlSnapshot: string };
    };
    expect(patchJson.ok).toBe(true);
    expect(patchJson.data.applyTier).toBe("INSTANT_UNDO");
    expect(patchJson.data.htmlSnapshot).toContain('data-ff-id="sec-header"');
    expect(patchJson.data.htmlSnapshot).not.toContain("Workspace Screen");
    await page.waitForTimeout(700);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "01-phase1-sketch-warm.png"), fullPage: false });

    // Reload (F5) -> verify active minimalist steering chip hydrates from SQLite WAL
    await page.reload({ waitUntil: "networkidle" });
    await dismissOnboardingWizard(page);
    await ensureSteeringPopoverOpen();
    await expect(page.getByTestId("steering-chip-more-whitespace")).toHaveAttribute("aria-pressed", "true", {
      timeout: 10000,
    });
    await page.keyboard.press("Escape");
    await page.waitForTimeout(450);

    // STEP 2 (Phase 1 — Night Mode Semantic Blueprint Sketch)
    await setWorkspaceTheme(page, "night");
    await page.waitForTimeout(700);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "02-phase1-sketch-night.png"), fullPage: false });

    // STEP 3 (Phase 2 — Promote Sketch -> Choose Style, Reference Apps, Custom Theme Modes & Popular Components)
    await setWorkspaceTheme(page, "warm");
    await page.getByTestId("promote-sketch-to-style-btn").click();
    const clarifyModal = page.locator("[data-ff-id='visual-clarification-modal']");
    await expect(clarifyModal).toBeVisible({ timeout: 5000 });
    await page.waitForTimeout(450);

    await page.getByTestId("clarify-reference-apps-input").fill("Linear, Stripe, Notion");
    await page.getByTestId("theme-mode-pill-warm").click();
    await page.getByTestId("popular-comp-tooltip").click();

    const pillPolish = page.getByTestId("live-direction-pill-dir-polish-workbench");
    const pillSketch = page.getByTestId("live-direction-pill-dir-editorial-sketch");
    const targetPill = (await pillPolish.getAttribute("aria-pressed")) === "true" ? pillSketch : pillPolish;
    const clarifyPromise = page.waitForResponse(
      (r) => r.url().includes("/api/v1/sdi/clarify") && r.request().method() === "POST"
    );
    await targetPill.click();
    const clarifyResp = await clarifyPromise;
    expect(clarifyResp.status()).toBe(200);
    const clarifyJson = (await clarifyResp.json()) as { ok: boolean; data: { generationEpoch: number } };
    expect(clarifyJson.ok).toBe(true);
    expect(clarifyJson.data.generationEpoch).toBeGreaterThanOrEqual(1);
    await page.waitForTimeout(700);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "03-phase2-style-modal.png"), fullPage: false });

    // STEP 4 (Phase 3 — Apply Style & Polish -> Official Project Colors Independent of Cowok-Design Night Mode + <16ms Sketch Lens)
    await setWorkspaceTheme(page, "night");
    await page.getByTestId("clarify-confirm-btn").click();
    await expect(clarifyModal).toBeHidden({ timeout: 5000 });
    await expect(htmlPane).toHaveAttribute("data-stage-skin", "brand-color");
    await expect(page.getByTestId("toggle-sketch-lens-btn")).toBeVisible();
    await expect(page.getByTestId("approve-v1-btn")).toBeVisible();
    await expect(page.getByTestId("skin-swap-latency")).toBeVisible();
    await page.waitForTimeout(800);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "04-phase3-polish-brand-official.png"), fullPage: false });

    // Exercise secondary <16ms Sketch Lens toggle in Phase 3 -> back to Brand Color
    await page.getByTestId("toggle-sketch-lens-btn").click();
    await expect(htmlPane).toHaveAttribute("data-stage-skin", "sketch-paper");
    await page.waitForTimeout(350);
    await page.getByTestId("toggle-sketch-lens-btn").click();
    await expect(htmlPane).toHaveAttribute("data-stage-skin", "brand-color");
    await page.waitForTimeout(350);

    // STEP 5 (Phase 3 — Plain-Language Tier-2 AppShell Lock Receipt Card)
    await setWorkspaceTheme(page, "warm");
    const acquiredLock = await page.evaluate(
      async ({ projectId, token, nonce }) => {
        const hdrs: Record<string, string> = {
          "Content-Type": "application/json",
          Authorization: `Bearer ${token}`,
          "X-Cowok-Session-Token": token,
          "x-cowok-portal": "SalesApp",
          "x-iwish-qa-nonce": nonce,
          "x-tenant-id": "local-default",
        };
        const getRes = await fetch(`/api/v1/ssot/components?projectId=${projectId}&portalId=SalesApp`, {
          headers: hdrs,
        });
        const getBody = (await getRes.json().catch(() => ({}))) as {
          data?: { lock?: { revision?: number; fencingToken?: number; isExpired?: boolean; lockOwnerId?: string } | null };
        };
        const existing = getBody.data?.lock;
        let rev = existing?.revision ?? 0;
        if (existing && !existing.isExpired) {
          const relRes = await fetch("/api/v1/ssot/appshell/lock", {
            method: "POST",
            headers: hdrs,
            body: JSON.stringify({
              projectId,
              portalId: "SalesApp",
              action: "RELEASE",
              lockOwnerId: existing.lockOwnerId || "local-operator",
              expectedRevision: rev,
              expectedFencingToken: existing.fencingToken ?? null,
            }),
          });
          const relBody = (await relRes.json().catch(() => ({}))) as { data?: { lock?: { revision?: number } } };
          rev = relBody.data?.lock?.revision ?? rev + 1;
        }
        const acqRes = await fetch("/api/v1/ssot/appshell/lock", {
          method: "POST",
          headers: hdrs,
          body: JSON.stringify({
            projectId,
            portalId: "SalesApp",
            action: "ACQUIRE",
            lockOwnerId: "local-operator",
            expectedRevision: rev,
            lockedSlots: ["sec-header"],
            ttlMs: 30000,
          }),
        });
        if (!acqRes.ok) {
          throw new Error(`AppShell lock ACQUIRE failed: ${acqRes.status} ${await acqRes.text()}`);
        }
        const acqBody = (await acqRes.json()) as { data?: { lock?: { revision?: number; fencingToken?: number } } };
        return {
          rev: acqBody.data?.lock?.revision ?? rev + 1,
          fencingToken: acqBody.data?.lock?.fencingToken ?? null,
        };
      },
      { projectId: DEFAULT_PROJECT_ID, token: sessionToken, nonce: qaNonce }
    );

    await ensureSteeringPopoverOpen();
    await page.getByTestId("steering-chip-softer-corners").click();
    const receiptCard = page.locator("[data-ff-id='locked-rule-receipt-card']");
    await expect(receiptCard).toBeVisible({ timeout: 8000 });
    await expect(receiptCard).not.toContainText("APPSHELL_LOCK_BOUNDARY");
    await page.waitForTimeout(800);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "05-phase3-locked-shell-receipt.png"), fullPage: false });
    await page.getByTestId("receipt-dismiss-btn").click();
    await expect(receiptCard).toBeHidden({ timeout: 5000 });

    await page.evaluate(
      async ({ projectId, token, nonce, rev, fencingToken }) => {
        await fetch("/api/v1/ssot/appshell/lock", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${token}`,
            "X-Cowok-Session-Token": token,
            "x-cowok-portal": "SalesApp",
            "x-iwish-qa-nonce": nonce,
            "x-tenant-id": "local-default",
          },
          body: JSON.stringify({
            projectId,
            portalId: "SalesApp",
            action: "RELEASE",
            lockOwnerId: "local-operator",
            expectedRevision: rev,
            expectedFencingToken: fencingToken,
          }),
        });
      },
      {
        projectId: DEFAULT_PROJECT_ID,
        token: sessionToken,
        nonce: qaNonce,
        rev: acquiredLock.rev,
        fencingToken: acquiredLock.fencingToken,
      }
    );

    // STEP 6 (Phase 4 — Approve 1st Official Version v1)
    await page.getByTestId("approve-v1-btn").click();
    await expect(page.getByTestId("approved-v1-badge")).toBeVisible({ timeout: 5000 });
    await page.waitForTimeout(700);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "06-phase4-approved-v1.png"), fullPage: false });

    // Verify live OCC 409 STALE_MAPPING on stale SHA against live Rust Daemon
    expectingConflict409 = true;
    const occScreenId = `e2e-occ-${Date.now()}`;
    const staleResp = await page.request.post(new URL("/api/v1/patch", baseUrl).toString(), {
      headers: apiHeaders,
      data: {
        projectId: DEFAULT_PROJECT_ID,
        screenId: occScreenId,
        baseArtifactSha256: "a".repeat(64),
        ffId: "ff-slide-headline",
        opKind: "TEXT_EDIT",
        payloadJson: { text: "Rev 1" },
      },
    });
    expect(staleResp.status()).toBe(200);

    const conflictResp = await page.request.post(new URL("/api/v1/patch", baseUrl).toString(), {
      headers: apiHeaders,
      data: {
        projectId: DEFAULT_PROJECT_ID,
        screenId: occScreenId,
        baseArtifactSha256: "a".repeat(64),
        ffId: "ff-slide-headline",
        opKind: "TEXT_EDIT",
        payloadJson: { text: "Stale Concurrent Edit" },
        expectedAstRevision: 0,
      },
    });
    expect(conflictResp.status()).toBe(409);
    const conflictBody = (await conflictResp.json()) as { error: { code: string } };
    expect(conflictBody.error.code).toBe("STALE_MAPPING");
    expectingConflict409 = false;

    // STEP 7: Mobile (390x844) responsive verification
    await page.setViewportSize({ width: 390, height: 844 });
    await page.waitForTimeout(450);
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "07-mobile-responsive.png"), fullPage: false });

    const videoHandle = page.video();
    await context.close();
    if (videoHandle) {
      const recordedPath = await videoHandle.path();
      if (recordedPath && fs.existsSync(recordedPath)) {
        fs.copyFileSync(recordedPath, path.join(EVIDENCE_DIR, "story-05.4-full-userflow.webm"));
      }
    }

    expect(pageErrors, `Unexpected page errors: ${pageErrors.join("; ")}`).toEqual([]);
    expect(consoleErrors, `Unexpected console errors: ${consoleErrors.join("; ")}`).toEqual([]);
    expect(server500s, `Unexpected 5xx responses: ${server500s.join("; ")}`).toEqual([]);
  });
});
