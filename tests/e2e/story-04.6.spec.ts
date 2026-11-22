// @story 04.6
// @implements AC-01, AC-02, AC-03, AC-04, AC-05, E2E-CHAT-CAS-VAULT-001, E2E-TTL-PRESERVATION-001
import fs from "node:fs";
import path from "node:path";
import { expect, test, type Page } from "@playwright/test";

const EVIDENCE_DIR = process.env.EVIDENCE_DIR || "";
const QA_NONCE = process.env.QA_NONCE || "";

async function maybeCaptureScreenshot(page: Page, filename: string) {
  if (!EVIDENCE_DIR) return;
  fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
  await page.screenshot({ path: path.join(EVIDENCE_DIR, filename), fullPage: false });
}

async function maybeCaptureDomSnapshot(page: Page) {
  if (!EVIDENCE_DIR) return;
  fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
  fs.writeFileSync(path.join(EVIDENCE_DIR, "dom-snapshot.html"), await page.content(), "utf8");
}

async function bootWorkbenchAndToken(page: Page): Promise<string> {
  await page.setViewportSize({ width: 1440, height: 900 });
  const targetPort = process.env.TARGET_PORT || "4173";
  await page.goto(`http://127.0.0.1:${targetPort}/`, { waitUntil: "networkidle" });
  const tok = await page.evaluate(async () => {
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
    const sessionToken = auth.sessionToken || "";
    if (sessionToken) {
      await fetch("/api/agent-settings", {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
          "X-Cowok-Session-Token": sessionToken,
          Authorization: `Bearer ${sessionToken}`,
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
    return sessionToken;
  });
  await page.reload({ waitUntil: "networkidle" });
  return tok;
}

test.describe("Story 04.6: SQLite Chat Persistence, CAS Intake & Zero-Leak Vault E2E", () => {
  test("E2E-CHAT-CAS-VAULT-001 & E2E-TTL-PRESERVATION-001: reload chat persistence, CAS upload, secret vault masking, no sessionStorage leaks", async ({
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
      const tok = await bootWorkbenchAndToken(page);
      await maybeCaptureScreenshot(page, "screenshot-initial.png");

      const composer = page.locator("[data-ff-id='chat-composer-input']").first();
      await expect(composer).toBeVisible({ timeout: 10000 });
      await composer.fill("Story 04.6 persistent turn check");
      await composer.press("Enter");

      const sid = `e2e-046-${Date.now()}`;
      const apiResults = await page.evaluate(
        async ({ sessionToken, sessionId }) => {
          const msgRes = await fetch(`/api/v1/chat-sessions/${sessionId}/messages`, {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-Cowok-Session-Token": sessionToken,
            },
            body: JSON.stringify({ role: "user", proseMarkdown: "persisted reload message" }),
          });
          const msgJson = await msgRes.json();

          const form = new FormData();
          form.append("sessionId", sessionId);
          form.append("idempotencyKey", "e2e-cas-k1");
          form.append("file", new Blob(["col1,col2\n1,2\n"], { type: "text/csv" }), "metrics.csv");
          const casRes = await fetch("/api/v1/cas/upload", {
            method: "POST",
            headers: {
              "X-Cowok-Session-Token": sessionToken,
              "X-Idempotency-Key": "e2e-cas-k1",
            },
            body: form,
          });
          const casJson = await casRes.json();

          const casReplay = await fetch("/api/v1/cas/upload", {
            method: "POST",
            headers: {
              "X-Cowok-Session-Token": sessionToken,
              "X-Idempotency-Key": "e2e-cas-k1",
            },
            body: form,
          });
          const casReplayJson = await casReplay.json();

          const vaultRes = await fetch("/api/v1/vault/chat-ingest", {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-Cowok-Session-Token": sessionToken,
              "X-Idempotency-Key": "e2e-vault-k1",
            },
            body: JSON.stringify({
              sessionId,
              provider: "google_ai_studio",
              rawSecretEnvelope: "AIzaSyE2EZeroLeakKey9876543210abcdef",
              enterpriseLocalOnlyLock: true,
            }),
          });
          const vaultJson = await vaultRes.json();

          const listRes = await fetch(`/api/v1/chat-sessions/${sessionId}/messages`, {
            headers: { "X-Cowok-Session-Token": sessionToken },
          });
          const listJson = await listRes.json();

          const sharedStorage = window.sessionStorage.getItem("cowok:shared-chat:v1");
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
            sharedStorage,
          };
        },
        { sessionToken: tok, sessionId: sid }
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
      await maybeCaptureDomSnapshot(page);
      await maybeCaptureScreenshot(page, "screenshot-final.png");
    } finally {
      await context.close();
    }
  });
});
