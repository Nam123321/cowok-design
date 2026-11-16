import { execSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";

test.describe("Story 04.3: Live Onboarding SQLite Hydration, BYOS/BYOK Doctor, Vault 409 & Plugins Acceptance", () => {
  test("verifies Day-0 SQLite onboarding wizard, BYOS/BYOK controls, SSRF/vault guards, F5 persistence & /api/plugins", async ({
    browser,
  }) => {
    const storyDir = path.resolve(
      process.cwd(),
      "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.3"
    );
    const evidenceDir = path.join(storyDir, "qa", "evidence");
    fs.mkdirSync(evidenceDir, { recursive: true });

    const qaNonce = process.env.QA_NONCE_TOKEN || crypto.randomBytes(16).toString("hex");
    fs.writeFileSync(
      path.join(evidenceDir, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, storyId: "04.3", generatedAt: new Date().toISOString() }, null, 2)
    );

    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    const videoDir = path.join(evidenceDir, "videos");
    fs.mkdirSync(videoDir, { recursive: true });
    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      recordVideo: { dir: videoDir, size: { width: 1440, height: 900 } },
      recordHar: { path: path.join(evidenceDir, "story-04.3-network.har"), mode: "minimal" },
    });
    const page = await context.newPage();
    await page.route(
      (url) => url.origin === `http://127.0.0.1:${targetPort}`,
      async (route) => {
        await route.continue({
          headers: {
            ...route.request().headers(),
            "x-iwish-qa-nonce": qaNonce,
            "x-tenant-id": "local-default",
          },
        });
      }
    );

    const consoleErrors: string[] = [];
    const pageErrors: string[] = [];
    const server500s: string[] = [];

    page.on("console", (msg) => {
      if (msg.type() !== "error") return;
      const text = msg.text();
      if (/Failed to load resource: the server responded with a status of (403 \(Forbidden\)|409 \(Conflict\))/.test(text)) {
        return;
      }
      consoleErrors.push(text);
    });
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("response", (res) => {
      if (res.status() >= 500) server500s.push(`${res.status()} ${res.url()}`);
    });

    try {
      const workspaceRoot = process.env.UXUI_OS_WORKSPACE_ROOT || process.cwd();
      const dbPath = path.resolve(workspaceRoot, ".cowok-design", "cowok-design.sqlite3");
      if (fs.existsSync(dbPath)) {
        try {
          execSync(`sqlite3 "${dbPath}" "UPDATE agent_settings SET onboarding_done = 0;"`);
        } catch {
          // ignore if sqlite lock active
        }
      }

      // Fresh browser (NO localStorage cowok_first_run_completed flag): must hydrate onboarding from SQLite
      await page.goto(baseUrl, { waitUntil: "domcontentloaded", timeout: 20000 });
      const wizardHeading = page.getByRole("heading", { name: /Chào mừng đến Cowok-Design|Welcome to Cowok-Design/i });
      await expect(wizardHeading).toBeVisible({ timeout: 15000 });

      // Gate H6 UX: 1. BYOS Local CLI Doctor rendered BEFORE 2. BYOK input + collapsible accordion works
      const accordionBtn = page.getByTestId("cli-doctor-accordion-toggle");
      await expect(accordionBtn).toBeVisible({ timeout: 10000 });
      await expect(page.getByTestId("cli-tool-card-claude")).toBeVisible({ timeout: 10000 });
      await accordionBtn.click();
      await expect(page.getByTestId("cli-tool-card-claude")).toBeHidden();
      await accordionBtn.click();
      await expect(page.getByTestId("cli-tool-card-claude")).toBeVisible();

      await expect(page.getByTestId("byok-provider-select")).toBeVisible();
      await expect(page.getByTestId("byok-api-key-input")).toBeVisible();
      await expect(page.getByTestId("byok-verify-btn")).toBeVisible();
      await page.screenshot({ path: path.join(evidenceDir, "01-onboarding-byos-byok.png"), fullPage: true });

      // API contract checks from live browser session (dryRunFormatOnly, SSRF block, 409 downgrade, /api/plugins)
      const dummyDryKey = `AIzaSyDryRunVerify${"9".repeat(16)}`;
      const dummySsrfKey = `sk-${"0".repeat(20)}`;
      const apiChecks = await page.evaluate(
        async ({ dryKey, ssrfKey }) => {
          const bootCap = document.querySelector('meta[name="cowok-boot-cap"]')?.getAttribute("content")?.trim() || "";
          const authRes = await fetch("/api/auth/local-session", {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              ...(bootCap ? { "X-Cowok-Boot-Cap": bootCap } : {}),
            },
            body: JSON.stringify({ role: "editor" }),
          });
          const authJson = (await authRes.json()) as { sessionToken?: string };
          const hdrs: Record<string, string> = {
            "Content-Type": "application/json",
            ...(authJson.sessionToken ? { "X-Cowok-Session-Token": authJson.sessionToken } : {}),
          };

          const dryRes = await fetch("/api/doctor/verify-key", {
            method: "POST",
            headers: hdrs,
            body: JSON.stringify({ providerId: "gemini", apiKey: dryKey, endpointUrl: "", dryRunFormatOnly: true }),
          });
          const dryJson = (await dryRes.json()) as { valid: boolean; status: string; liveProbed?: boolean };

          const ssrfRes = await fetch("/api/doctor/verify-key", {
            method: "POST",
            headers: hdrs,
            body: JSON.stringify({
              providerId: "byos",
              apiKey: ssrfKey,
              endpointUrl: "http://169.254.169.254/latest/meta-data",
            }),
          });
          const ssrfJson = (await ssrfRes.json()) as { valid: boolean; status: string };

          const downgradeRes = await fetch("/api/settings/vault", {
            method: "PUT",
            headers: hdrs,
            body: JSON.stringify({ vaultMode: "ephemeral" }),
          });
          const downgradeJson = (await downgradeRes.json()) as { code?: string };

          const pluginsRes = await fetch("/api/plugins?enabled=true", { headers: hdrs });
          const pluginsJson = await pluginsRes.json();

          const registryRes = await fetch("/api/plugins/registry", { headers: hdrs });
          const registryJson = await registryRes.json();

          return {
            authStatus: authRes.status,
            dryStatus: dryRes.status,
            dryJson,
            ssrfJson,
            downgradeStatus: downgradeRes.status,
            downgradeCode: downgradeJson.code,
            pluginsStatus: pluginsRes.status,
            pluginsIsArray: Array.isArray(pluginsJson),
            registryStatus: registryRes.status,
            registryIsArray: Array.isArray(registryJson),
          };
        },
        { dryKey: dummyDryKey, ssrfKey: dummySsrfKey }
      );

      expect(apiChecks.authStatus).toBe(200);
      expect(apiChecks.dryStatus).toBe(200);
      expect(apiChecks.dryJson.valid).toBe(true);
      expect(apiChecks.dryJson.status).toBe("format_checked");
      expect(apiChecks.ssrfJson.valid).toBe(false);
      expect(apiChecks.ssrfJson.status).toBe("invalid_endpoint");
      expect(apiChecks.downgradeStatus).toBe(409);
      expect(apiChecks.downgradeCode).toBe("VAULT_DOWNGRADE_FORBIDDEN");
      expect(apiChecks.pluginsStatus).toBe(200);
      expect(apiChecks.pluginsIsArray).toBe(true);
      expect(apiChecks.registryStatus).toBe(200);
      expect(apiChecks.registryIsArray).toBe(true);

      // Walk through Step 2 & Step 3 and complete Day-0 onboarding -> persists onboardingDone=true in SQLite V7
      await page.getByRole("button", { name: /Tiếp tục|Next/i }).click();
      await expect(page.getByText(/Daemon 0600 Vault/i)).toBeVisible();
      await page.getByRole("button", { name: /Tiếp tục|Next/i }).click();
      await page.getByRole("button", { name: /Hoàn tất/i }).click();
      await expect(wizardHeading).toBeHidden({ timeout: 10000 });

      // Reload without localStorage flag: SQLite onboardingDone=true must keep wizard closed
      const lsFlag = await page.evaluate(() => window.localStorage.getItem("cowok_first_run_completed"));
      expect(lsFlag).toBeNull();
      await page.reload({ waitUntil: "domcontentloaded" });
      await expect(page.getByRole("button", { name: /New Conversation|Cuộc trò chuyện mới/i })).toBeVisible({ timeout: 15000 });
      await expect(wizardHeading).toBeHidden();

      // Open Settings -> Models (3-Bento layout), verify retired default-model select is absent,
      // expand API/BYOS Bento -> configure BYOS endpoint -> save & verify F5 reload persistence
      await page.getByRole("button", { name: /Cài đặt|Settings/i }).first().click();
      await page.getByRole("button", { name: /Mô hình & BYOS|Models & BYOS/i }).click();
      await expect(page.getByTestId("settings-default-model-select")).toHaveCount(0);
      await page.getByTestId("toggle-api-bento-btn").click();
      await page.getByTestId("api-bento-tile-byos").click();
      const byosInput = page.locator('input[placeholder*="11434"], input[placeholder*="gateway"], input[placeholder*="http"]').first();
      await byosInput.fill("https://gateway.example.com/v1");
      await page.getByTestId("settings-save-btn").click();
      await expect(page.getByTestId("settings-sync-warning")).toHaveCount(0);
      await expect(page.getByTestId("settings-save-btn")).toBeHidden({ timeout: 5000 });

      await page.reload({ waitUntil: "domcontentloaded" });
      await expect(wizardHeading).toBeHidden();
      await page.getByRole("button", { name: /Cài đặt|Settings/i }).first().click();
      await page.getByRole("button", { name: /Mô hình & BYOS|Models & BYOS/i }).click();
      await expect(
        page.locator('input[placeholder*="11434"], input[placeholder*="gateway"], input[placeholder*="http"]').first()
      ).toHaveValue("https://gateway.example.com/v1");
      await page.screenshot({ path: path.join(evidenceDir, "02-settings-f5-persisted.png"), fullPage: true });

      fs.writeFileSync(path.join(evidenceDir, "dom-snapshot.html"), await page.content(), "utf-8");
      expect(pageErrors).toEqual([]);
      expect(consoleErrors).toEqual([]);
      expect(server500s).toEqual([]);
    } finally {
      const vid = page.video();
      await context.close();
      if (vid) {
        try {
          await vid.saveAs(path.join(evidenceDir, "acceptance-story-04.3.webm"));
        } catch {
          // ignore video flush error
        }
      }
    }
  });
});
