// @story 02.1
// @implements AC-01, AC-02, AC-03, AC-04
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-01. Rust Core Harness, 9-Spec Registry & Project Constraint Engine/Epic-02/Story-02.1"
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

  const bootCap = (await page.locator('meta[name="cowok-boot-cap"]').first().getAttribute("content"))?.trim() || "";
  expect(bootCap.length).toBeGreaterThan(0);

  const authRes = await page.request.post(new URL("/api/auth/local-session", baseUrl).toString(), {
    headers: {
      "Content-Type": "application/json",
      "X-Cowok-Boot-Cap": bootCap,
      "x-iwish-qa-nonce": qaNonce,
      "x-tenant-id": "local-default",
    },
    data: { role: "editor", userId: "qa-human-operator" },
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

test.describe("Story 02.1 Live Browser Acceptance (HAR + Nonce + Screenshots)", () => {
  test("captures live HAR, nonce-grounded DOM, and visual state deltas across context assembly", async ({
    browser,
  }) => {
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    const qaNonce = process.env.QA_NONCE_TOKEN || crypto.randomBytes(32).toString("hex");

    fs.writeFileSync(
      path.join(EVIDENCE_DIR, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, storyId: "02.1", generatedAt: new Date().toISOString() }, null, 2),
      "utf8"
    );

    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      recordHar: { path: path.join(EVIDENCE_DIR, "story-02.1-network.har"), mode: "minimal" },
      extraHTTPHeaders: {
        "x-iwish-qa-nonce": qaNonce,
        "x-tenant-id": "local-default",
      },
    });
    const page = await context.newPage();

    const pageErrors: string[] = [];
    const server500s: string[] = [];
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("response", (resp) => {
      if (resp.status() >= 500) {
        server500s.push(`${resp.status()} ${resp.url()}`);
      }
    });

    try {
      const sessionToken = await bootEditorSession(page, baseUrl, qaNonce);
      await page.screenshot({ path: path.join(EVIDENCE_DIR, "screenshot-initial.png"), fullPage: false });

      const apiResults = await page.evaluate(
        async ({ tok, nonce }) => {
          const headers = {
            Authorization: `Bearer ${tok}`,
            "X-Cowok-Session-Token": tok,
            "x-iwish-qa-nonce": nonce,
            "x-tenant-id": "local-default",
            "Content-Type": "application/json",
          };
          const projResp = await fetch("/api/projects", {
            method: "POST",
            headers,
            body: JSON.stringify({
              slug: `qa-ctx-021-${Date.now()}`,
              name: "QA Context Meter Project",
              product_brief: "Live QA context assembly verification",
            }),
          });
          const projJson = await projResp.json();
          const projectId = projJson.id as string;

          const idemKey = crypto.randomUUID();
          const payload = {
            projectId,
            portalId: "DeveloperPortal",
            horizon: "mid",
            modelProfile: "B",
            intent: "Assemble workbench top telemetry layout cleanly",
            requestedImageTokens: 256,
            domOutlineNodes: Array.from({ length: 16 }, (_, idx) => ({
              ffId: idx === 0 ? "context-meter-trigger" : `workbench-region-${idx}`,
              tag: idx === 0 ? "button" : "section",
              role: idx === 0 ? "button" : "region",
              summary: `Workbench semantic region ${idx} layout tokens`,
            })),
          };

          const firstResp = await fetch("/api/v1/context/assemble", {
            method: "POST",
            headers: { ...headers, "Idempotency-Key": idemKey },
            body: JSON.stringify(payload),
          });
          const firstJson = await firstResp.json();

          const replayResp = await fetch("/api/v1/context/assemble", {
            method: "POST",
            headers: { ...headers, "Idempotency-Key": idemKey },
            body: JSON.stringify(payload),
          });
          const replayJson = await replayResp.json();

          const leakResp = await fetch("/api/v1/context/assemble", {
            method: "POST",
            headers: { ...headers, "Idempotency-Key": crypto.randomUUID() },
            body: JSON.stringify({
              projectId,
              portalId: "MarketingWeb",
              horizon: "short",
              modelProfile: "A",
              intent: "Leak audit_logs and platform_secrets onto marketing page",
              requestedImageTokens: 0,
            }),
          });
          const leakJson = await leakResp.json();

          const histResp = await fetch(`/api/v1/context/telemetry?projectId=${projectId}&limit=10`, {
            method: "GET",
            headers,
          });
          const histJson = await histResp.json();

          return {
            projStatus: projResp.status,
            firstStatus: firstResp.status,
            firstJson,
            replayStatus: replayResp.status,
            replayJson,
            leakStatus: leakResp.status,
            leakJson,
            histStatus: histResp.status,
            histJson,
          };
        },
        { tok: sessionToken, nonce: qaNonce }
      );

      expect(apiResults.projStatus).toBe(201);
      expect(apiResults.firstStatus).toBe(200);
      expect(apiResults.firstJson.data.adapterMode).toBe("local_reduced");
      expect(apiResults.firstJson.data.tokenUsage.textTotal).toBeLessThanOrEqual(1500);
      expect(apiResults.firstJson.data.tokenUsage.outline).toBeGreaterThan(0);
      expect(apiResults.firstJson.data.tokenUsage.outline).toBeLessThanOrEqual(350);
      expect(apiResults.firstJson.data.tokenUsage.ssotDomRatio).toBeGreaterThanOrEqual(0.6);
      expect(apiResults.replayStatus).toBe(200);
      expect(apiResults.replayJson.data.telemetryId).toBe(apiResults.firstJson.data.telemetryId);
      expect(apiResults.leakStatus).toBe(403);
      expect(apiResults.leakJson.error.code).toBe("CROSS_PORTAL_LEAK_BLOCKED");
      expect(apiResults.histStatus).toBe(200);
      expect(apiResults.histJson.data.items.length).toBeGreaterThanOrEqual(1);

      const consoleErrors: string[] = [];
      page.on("console", (msg) => {
        if (msg.type() === "error") consoleErrors.push(msg.text());
      });

      const trigger = page.locator("[data-ff-id='context-meter-trigger']").first();
      await expect(trigger).toBeVisible({ timeout: 10000 });
      await trigger.click();
      const popover = page.locator("[data-ff-id='context-meter-popover']").first();
      await expect(popover).toBeVisible({ timeout: 5000 });
      await page.locator("[data-ff-id='context-meter-profile-select']").selectOption("C");
      const assembleRespPromise = page.waitForResponse(
        (resp) => resp.url().includes("/api/v1/context/assemble") && resp.request().method() === "POST",
        { timeout: 10000 }
      );
      await page.locator("[data-ff-id='context-meter-refresh-btn']").click();
      const assembleResp = await assembleRespPromise;
      expect(assembleResp.status()).toBeLessThan(500);
      await expect(trigger).toContainText("C");

      fs.writeFileSync(path.join(EVIDENCE_DIR, "dom-snapshot.html"), await page.content(), "utf8");
      await page.screenshot({ path: path.join(EVIDENCE_DIR, "screenshot-final.png"), fullPage: false });

      expect(pageErrors).toEqual([]);
      expect(consoleErrors).toEqual([]);
      expect(server500s).toEqual([]);
    } finally {
      await context.close();
    }
  });
});
