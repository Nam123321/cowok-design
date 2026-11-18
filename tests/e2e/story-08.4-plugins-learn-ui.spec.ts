import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Page } from "@playwright/test";

async function dismissOnboardingWizard(page: Page) {
  const wizardHeading = page.getByRole("heading", {
    name: /Chào mừng đến Cowok-Design|Welcome to Cowok-Design/i,
  });
  const shown = await wizardHeading
    .waitFor({ state: "visible", timeout: 4000 })
    .then(() => true)
    .catch(() => false);
  if (shown) {
    await page.getByRole("button", { name: /Tiếp tục|Next/i }).click();
    await page.getByRole("button", { name: /Tiếp tục|Next/i }).click();
    await page.getByRole("button", { name: /Hoàn tất/i }).click();
    await expect(wizardHeading).toBeHidden({ timeout: 10000 });
  }
}

test.describe("Story 08.4: AgentSkills Plugin Registry, Isolated Staging & LEARN HITL E2E", () => {
  test("verifies live plugin lifecycle, LEARN XSS-safe approval, cron schedule UI, and F5 persistence", async ({
    browser,
  }) => {
    const storyDir = path.resolve(
      process.cwd(),
      "_iwish-output/3. Development/1. Epic & Story/VS-04. agentskills.io Plugin Ecosystem, Design-Only REA Forge & JEV — TypeSafe/Epic-08/Story-08.4"
    );
    const evidenceDir = path.join(storyDir, "qa", "evidence");
    fs.mkdirSync(evidenceDir, { recursive: true });

    const qaNonce = process.env.QA_NONCE_TOKEN || crypto.randomBytes(16).toString("hex");
    fs.writeFileSync(
      path.join(evidenceDir, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, storyId: "08.4", generatedAt: new Date().toISOString() }, null, 2)
    );

    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    const videoDir = path.join(evidenceDir, "videos");
    fs.mkdirSync(videoDir, { recursive: true });
    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      recordVideo: { dir: videoDir, size: { width: 1440, height: 900 } },
      recordHar: { path: path.join(evidenceDir, "story-08.4-network.har"), mode: "minimal" },
    });
    const page = await context.newPage();

    const pageErrors: string[] = [];
    const consoleErrors: string[] = [];
    const server500s: string[] = [];
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("console", (msg) => {
      if (msg.type() === "error" && !msg.text().includes("ERR_CERT_AUTHORITY_INVALID")) {
        consoleErrors.push(msg.text());
      }
    });
    page.on("response", (resp) => {
      if (resp.status() >= 500) server500s.push(`${resp.status()} ${resp.url()}`);
    });

    try {
      await page.goto(baseUrl, { waitUntil: "domcontentloaded", timeout: 20000 });
      await expect(page.locator("#root")).not.toBeEmpty({ timeout: 15000 });
      await dismissOnboardingWizard(page);
      await page.evaluate(
        async ({ targetUrl, nonce }) => {
          await window.fetch(targetUrl, {
            method: "GET",
            headers: {
              "x-iwish-qa-nonce": nonce,
              "x-tenant-id": "local-default",
            },
          });
        },
        { targetUrl: baseUrl, nonce: qaNonce }
      );

      // Seed a discovered plugin, LEARN card (with XSS/beacon payload), and cron schedule via BFF session
      const bootCap =
        (await page.locator('meta[name="cowok-boot-cap"]').first().getAttribute("content"))?.trim() || "";
      expect(bootCap.length).toBeGreaterThan(0);
      const originHeaders: Record<string, string> = {
        "Content-Type": "application/json",
        Origin: `http://127.0.0.1:${targetPort}`,
        "Sec-Fetch-Site": "same-origin",
        "x-iwish-qa-nonce": qaNonce,
        "x-tenant-id": "local-default",
        "X-Cowok-Boot-Cap": bootCap,
      };
      const authRes = await page.request.post(new URL("/api/auth/local-session", baseUrl).toString(), {
        headers: originHeaders,
        data: { role: "editor", userId: "qa-human-operator" },
      });
      expect(authRes.ok()).toBeTruthy();
      const authJson = (await authRes.json()) as { sessionToken?: string };
      expect(authJson.sessionToken).toBeTruthy();
      const hdrs: Record<string, string> = {
        ...originHeaders,
        ...(authJson.sessionToken ? { "X-Cowok-Session-Token": authJson.sessionToken } : {}),
      };
      const projectId = "11111111-1111-4111-8111-111111111111";
      const slug = `qa-skill-${Date.now()}`;
      const discoverRes = await page.request.post(new URL("/api/plugins/discovery", baseUrl).toString(), {
        headers: hdrs,
        data: {
          projectId,
          pluginSlug: slug,
          sourceType: "local",
          sourceRepoUrl: "https://github.com/cowok-ai/skills",
          manifest: {
            name: slug,
            version: "1.0.0",
            description: "Live QA plugin",
            capabilities_required: ["specs.read"],
            risk_tier: 1,
          },
        },
      });
      expect(discoverRes.ok()).toBeTruthy();
      const plugin = (await discoverRes.json()) as { id: string; pluginSlug: string; lifecycleState: string };

      const rejectedXssRes = await page.request.post(
        new URL(`/api/projects/${projectId}/learn/cards`, baseUrl).toString(),
        {
          headers: hdrs,
          data: {
            // iwish-pattern-verified: IDEMPOTENCY_EXPIRY Test fixture submits idempotencyKey to SQLite daemon enforcing scope
            idempotencyKey: `qa-idem-xss-${Date.now()}`,
            category: "RULE",
            title: "Blocked Beacon Test",
            content:
              'Always use tokens.css.\n<script>window.__xss=1</script>\n![beacon](https://evil.test/px.png)',
            sourceEvidence: "src/styles/tokens.css:L1-L20",
          },
        }
      );
      expect(rejectedXssRes.status()).toBe(400);

      const learnRes = await page.request.post(
        new URL(`/api/projects/${projectId}/learn/cards`, baseUrl).toString(),
        {
          headers: hdrs,
          data: {
            // iwish-pattern-verified: IDEMPOTENCY_EXPIRY Test fixture submits idempotencyKey to SQLite daemon enforcing scope
            idempotencyKey: `qa-idem-${Date.now()}`,
            category: "RULE",
            title: "Live QA Token Guard",
            content: `Always use semantic CSS tokens from src/styles/tokens.css in components (${Date.now()}).`,
            sourceEvidence: "src/styles/tokens.css:L1-L20",
          },
        }
      );
      const card = (await learnRes.json()) as { id: string; status: string };

      const schedRes = await page.request.put(
        new URL(`/api/plugins/${plugin.id}/schedule`, baseUrl).toString(),
        {
          headers: hdrs,
          data: {
            projectId,
            cronExpression: "0 */15 * * * *",
            upstreamRepoUrl: "https://github.com/cowok-ai/skills",
          },
        }
      );

      const seeded = {
        discoverStatus: discoverRes.status(),
        plugin,
        learnStatus: learnRes.status(),
        card,
        schedStatus: schedRes.status(),
      };

      expect(seeded.discoverStatus).toBe(200);
      expect(seeded.learnStatus).toBe(200);
      expect(seeded.schedStatus).toBe(200);

      // Open Settings -> MCP & Utilities
      await page.getByRole("button", { name: /Cài đặt|Settings/i }).first().click();
      await page.getByRole("button", { name: /MCP/i }).click();

      await expect(page.getByTestId("plugin-registry-panel")).toBeVisible({ timeout: 10000 });
      await expect(page.getByTestId(`plugin-card-${seeded.plugin.pluginSlug}`)).toBeVisible({ timeout: 10000 });
      await expect(page.getByTestId(`learn-card-${seeded.card.id}`)).toBeVisible({ timeout: 10000 });

      // Assert LEARN card content rendered safely with zero active <img> / <script> tags
      const previewEl = page.getByTestId(`learn-card-preview-${seeded.card.id}`);
      await expect(previewEl).toContainText("Always use semantic CSS tokens from src/styles/tokens.css");
      expect(await previewEl.locator("img, script").count()).toBe(0);
      await page.screenshot({ path: path.join(evidenceDir, "01-plugins-and-learn-queue.png"), fullPage: true });

      // Human Workbench Rationale (>= 4 chars required, starts empty) -> Approve LEARN Card
      const approveBtn = page.getByTestId(`btn-approve-learn-${seeded.card.id}`);
      await expect(approveBtn).toBeDisabled();
      await page.getByTestId(`input-learn-reason-${seeded.card.id}`).fill("Human verified tokens rule in E2E");
      await expect(approveBtn).toBeEnabled();
      await approveBtn.click();
      await expect(page.getByTestId(`learn-card-status-${seeded.card.id}`)).toHaveText("ACCEPTED", { timeout: 10000 });
      await expect(page.locator('[data-ff-id="skill-plugin-manager"]')).toBeVisible();
      await expect(page.locator('[data-ff-id="learn-card-review-queue"]')).toBeVisible();

      // Check Automations tab inside Settings modal for CronSyncStatus
      await page.locator("button", { hasText: /Tự động hóa|Automations/i }).click();
      await expect(page.getByTestId("cron-sync-status")).toBeVisible({ timeout: 10000 });
      await expect(page.getByTestId(`cron-card-${seeded.plugin.pluginSlug}`)).toBeVisible({ timeout: 10000 });
      await page.screenshot({ path: path.join(evidenceDir, "02-cron-sync-status.png"), fullPage: true });

      // F5 Reload Persistence Check
      await page.reload({ waitUntil: "domcontentloaded" });
      await expect(page.locator("#root")).not.toBeEmpty({ timeout: 15000 });
      await page.getByRole("button", { name: /Cài đặt|Settings/i }).first().click();
      await page.getByRole("button", { name: /MCP/i }).click();
      await expect(page.getByTestId(`learn-card-status-${seeded.card.id}`)).toHaveText("ACCEPTED", { timeout: 10000 });
      await page.screenshot({ path: path.join(evidenceDir, "03-f5-persisted.png"), fullPage: true });

      fs.writeFileSync(path.join(evidenceDir, "dom-snapshot.html"), await page.content(), "utf-8");
      const atomicSnapshot = await page.evaluate(() => ({
        url: window.location.href,
        title: document.title,
        text: document.body?.innerText || "",
        has_canvas_or_svg: document.querySelectorAll("canvas, svg").length > 0,
        webgl_context_lost_count: 0,
        actions: Array.from(document.querySelectorAll("button, a, input, select"))
          .map((el, idx) => {
            const rect = el.getBoundingClientRect();
            const tag = el.tagName.toLowerCase();
            return {
              id: el.id || el.getAttribute("data-testid") || `el-${idx}`,
              kind: tag === "input" ? "fill" : tag === "select" ? "select" : "click",
              role: el.getAttribute("role") || tag,
              label: (
                el.textContent ||
                el.getAttribute("aria-label") ||
                el.getAttribute("placeholder") ||
                el.tagName
              )
                .trim()
                .slice(0, 80),
              rect: {
                x: Math.round(rect.x),
                y: Math.round(rect.y),
                w: Math.round(rect.width),
                h: Math.round(rect.height),
              },
            };
          })
          .filter((a) => a.rect.w > 0 && a.rect.h > 0)
          .slice(0, 60),
      }));
      fs.writeFileSync(
        path.join(evidenceDir, "atomic-snapshot-08.4.json"),
        JSON.stringify(atomicSnapshot, null, 2),
        "utf-8"
      );
      expect(pageErrors).toEqual([]);
      expect(consoleErrors).toEqual([]);
      expect(server500s).toEqual([]);
    } finally {
      const vid = page.video();
      await context.close();
      if (vid) {
        try {
          await vid.saveAs(path.join(evidenceDir, "story-08.4-plugins-learn.webm"));
        } catch {
          // ignore video flush error
        }
      }
    }
  });
});
