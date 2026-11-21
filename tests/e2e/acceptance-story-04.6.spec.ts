// @story 04.6
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, E2E-CHAT-CAS-VAULT-001, E2E-TTL-PRESERVATION-001
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.6"
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

test.describe("Story 04.6 Live Browser Acceptance (HAR + Nonce + Screenshots)", () => {
  test("captures live HAR, nonce-grounded DOM, and visual state deltas across chat, CAS & vault", async ({
    browser,
  }) => {
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    const qaNonce = process.env.QA_NONCE_TOKEN || crypto.randomBytes(32).toString("hex");

    fs.writeFileSync(
      path.join(EVIDENCE_DIR, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, storyId: "04.6", generatedAt: new Date().toISOString() }, null, 2),
      "utf8"
    );

    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      recordHar: { path: path.join(EVIDENCE_DIR, "story-04.6-network.har"), mode: "minimal" },
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

      const composer = page.locator("[data-ff-id='chat-composer-input']").first();
      await expect(composer).toBeVisible({ timeout: 10000 });
      await composer.fill("Story 04.6 live QA verification turn");
      await composer.press("Enter");

      const sid = `e2e-qa-046-${Date.now()}`;
      const apiResults = await page.evaluate(
        async ({ tok, sessionId, nonce }) => {
          const baseHeaders = {
            Authorization: `Bearer ${tok}`,
            "X-Cowok-Session-Token": tok,
            "x-iwish-qa-nonce": nonce,
            "x-tenant-id": "local-default",
          };
          const msgRes = await fetch(`/api/v1/chat-sessions/${sessionId}/messages`, {
            method: "POST",
            headers: { ...baseHeaders, "Content-Type": "application/json" },
            body: JSON.stringify({ role: "user", proseMarkdown: "persisted reload message" }),
          });
          const msgJson = await msgRes.json();

          const form = new FormData();
          form.append("sessionId", sessionId);
          form.append("idempotencyKey", "e2e-cas-k1");
          form.append("file", new Blob(["col1,col2\n1,2\n"], { type: "text/csv" }), "metrics.csv");
          const casRes = await fetch("/api/v1/cas/upload", {
            method: "POST",
            headers: { ...baseHeaders, "X-Idempotency-Key": "e2e-cas-k1" },
            body: form,
          });
          const casJson = await casRes.json();

          const casReplay = await fetch("/api/v1/cas/upload", {
            method: "POST",
            headers: { ...baseHeaders, "X-Idempotency-Key": "e2e-cas-k1" },
            body: form,
          });
          const casReplayJson = await casReplay.json();

          const vaultRes = await fetch("/api/v1/vault/chat-ingest", {
            method: "POST",
            headers: { ...baseHeaders, "Content-Type": "application/json", "X-Idempotency-Key": "e2e-vault-k1" },
            body: JSON.stringify({
              sessionId,
              provider: "google_ai_studio",
              rawSecretEnvelope: "AIzaSyE2EZeroLeakKey9876543210abcdef",
              enterpriseLocalOnlyLock: true,
            }),
          });
          const vaultJson = await vaultRes.json();

          const listRes = await fetch(`/api/v1/chat-sessions/${sessionId}/messages`, {
            headers: baseHeaders,
          });
          const listJson = await listRes.json();

          return {
            msgStatus: msgRes.status,
            msgJson,
            casStatus: casRes.status,
            casJson,
            casReplayStatus: casReplay.status,
            casReplayJson,
            vaultStatus: vaultRes.status,
            vaultJson,
            listStatus: listRes.status,
            listJson,
            sharedStorage: window.sessionStorage.getItem("cowok:shared-chat:v1"),
          };
        },
        { tok: sessionToken, sessionId: sid, nonce: qaNonce }
      );

      expect(apiResults.msgStatus).toBe(201);
      expect(apiResults.casStatus).toBe(201);
      expect(apiResults.casJson.fileExt).toBe("csv");
      expect(apiResults.casReplayStatus).toBe(200);
      expect(apiResults.casReplayJson.replayed).toBe(true);
      expect(apiResults.casReplayJson.id).toBe(apiResults.casJson.id);
      expect(apiResults.vaultStatus).toBe(201);
      expect(apiResults.vaultJson.presenterMask).toMatch(/^.{2,6}\.\.\..{2,4}$/);
      expect(JSON.stringify(apiResults.vaultJson)).not.toContain("AIzaSyE2EZeroLeakKey");
      expect(apiResults.listStatus).toBe(200);
      expect(apiResults.listJson.messages.length).toBeGreaterThanOrEqual(1);
      expect(apiResults.sharedStorage).toBeNull();

      await page.reload({ waitUntil: "networkidle" });
      await expect(page.locator("[data-ff-id='chat-composer-input']").first()).toBeVisible({ timeout: 10000 });
      await page.getByTestId("toggle-vault-onboarding-btn").click();
      await expect(page.getByTestId("decision-engine-onboarding-card")).toBeVisible({ timeout: 10000 });
      await page.getByTestId("enterprise-local-only-toggle").click();
      await expect(page.getByTestId("enterprise-local-only-toggle")).toBeChecked();
      fs.writeFileSync(path.join(EVIDENCE_DIR, "dom-snapshot.html"), await page.content(), "utf8");
      await page.screenshot({ path: path.join(EVIDENCE_DIR, "screenshot-final.png"), fullPage: false });

      expect(pageErrors).toEqual([]);
      expect(server500s).toEqual([]);
    } finally {
      await context.close();
    }
  });
});
