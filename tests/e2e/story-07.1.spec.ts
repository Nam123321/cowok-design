// @story 07.1
// @implements AC-01, AC-02, AC-03, AC-04 (Live Browser E2E Full Userflow for Story 07.1)
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { expect, test, type Page } from "@playwright/test";

const STORY_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-03. HTML-Native 16:9 Presentation & Dual-Pane CMS Studio/Epic-07/Story-07.1"
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

async function bootEditorSession(page: Page, baseUrl: string, qaNonce: string): Promise<string> {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto(baseUrl, { waitUntil: "networkidle" });
  await expect(page.locator("#root")).not.toBeEmpty({ timeout: 15000 });

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

test.describe("Story 07.1 Pooled 1-Chromium Visual QC, E2E Flow Completeness & 1-Click Auto-Fix", () => {
  test("verifies /api/v1/qc/run SHA-1 caching, honest fallback, <10ms deterministic auto-fix, MAX_ATTEMPTS=3 cap, and live UI selectors", async ({
    browser,
  }) => {
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "3000";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    const qaNonce =
      process.env.WATCHMEN_NONCE ||
      process.env.QA_NONCE_TOKEN ||
      crypto.randomBytes(32).toString("hex");

    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      recordVideo: { dir: EVIDENCE_DIR, size: { width: 1280, height: 800 } },
    });
    const page = await context.newPage();
    const consoleErrors: string[] = [];
    const pageErrors: string[] = [];
    const serverErrors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error" && !msg.text().includes("favicon.ico")) {
        consoleErrors.push(msg.text());
      }
    });
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("response", (res) => {
      if (res.url().includes("/api/") && res.status() >= 500) {
        serverErrors.push(`${res.status()} ${res.url()}`);
      }
    });

    const sessionToken = await bootEditorSession(page, baseUrl, qaNonce);
    const authedHeaders = {
      "Content-Type": "application/json",
      Authorization: `Bearer ${sessionToken}`,
      "X-Cowok-Session-Token": sessionToken,
      "x-iwish-qa-nonce": qaNonce,
      "x-tenant-id": "local-default",
    };

    const dirtyHtml =
      '<main><iframe src="https://thirdparty.example/widget"></iframe><p style="color: #cccccc; font-size: 10px;">Low contrast</p></main>';

    // 1. Run QC via BFF -> Daemon -> V26 qc_gate_runs
    const runRes = await page.request.post(new URL("/api/v1/qc/run", baseUrl).toString(), {
      headers: authedHeaders,
      data: {
        projectId: DEFAULT_PROJECT_ID,
        targetKind: "APP_SCREEN",
        targetId: "screen-e2e-071",
        html: dirtyHtml,
        constraintsJson: '{"radius":"balanced"}',
        viewports: [1440, 1024, 768, 390],
        fixtureRef: "e2e-071-v1",
        forceNoBrowser: true,
      },
    });
    expect(runRes.ok()).toBeTruthy();
    const runBody = (await runRes.json()) as {
      ok: boolean;
      data: {
        qcRunId: string;
        verdict: string;
        layoutSha1: string;
        cacheHit: boolean;
        issues: Array<{ id: string; deterministicFixable: boolean }>;
      };
    };
    expect(runBody.ok).toBe(true);
    expect(runBody.data.verdict).toBe("UNVERIFIED_NO_BROWSER");
    expect(runBody.data.layoutSha1).toHaveLength(40);

    // 2. Second identical run hits SHA-1 cache
    const cachedRes = await page.request.post(new URL("/api/v1/qc/run", baseUrl).toString(), {
      headers: authedHeaders,
      data: {
        projectId: DEFAULT_PROJECT_ID,
        targetKind: "APP_SCREEN",
        targetId: "screen-e2e-071",
        html: dirtyHtml,
        constraintsJson: '{"radius":"balanced"}',
        viewports: [1440, 1024, 768, 390],
        fixtureRef: "e2e-071-v1",
        forceNoBrowser: true,
      },
    });
    expect(cachedRes.ok()).toBeTruthy();
    const cachedBody = (await cachedRes.json()) as { data: { cacheHit: boolean; qcRunId: string } };
    expect(cachedBody.data.cacheHit).toBe(true);
    expect(cachedBody.data.qcRunId).toBe(runBody.data.qcRunId);

    // 3. Deterministic 1-Click Auto-Fix (<10ms, 0 LLM tokens)
    const deterministicIds = runBody.data.issues
      .filter((i) => i.deterministicFixable)
      .map((i) => i.id);
    expect(deterministicIds.length).toBeGreaterThan(0);

    const fixRes = await page.request.post(new URL("/api/v1/qc/auto-fix", baseUrl).toString(), {
      headers: authedHeaders,
      data: {
        projectId: DEFAULT_PROJECT_ID,
        qcRunId: runBody.data.qcRunId,
        issueIds: deterministicIds,
        mode: "DETERMINISTIC_LINT_FIX",
        attemptNumber: 1,
        currentHtml: dirtyHtml,
      },
    });
    expect(fixRes.ok()).toBeTruthy();
    const fixBody = (await fixRes.json()) as {
      data: { elapsedMs: number; llmTokensUsed: number; patchedHtml: string; previousHtml: string };
    };
    expect(fixBody.data.llmTokensUsed).toBe(0);
    expect(fixBody.data.elapsedMs).toBeLessThan(10);
    expect(fixBody.data.patchedHtml).toContain('sandbox="allow-scripts"');
    expect(fixBody.data.previousHtml).toBe(dirtyHtml);

    // 4. Attempt 4 rejected strictly (422)
    const capRes = await page.request.post(new URL("/api/v1/qc/auto-fix", baseUrl).toString(), {
      headers: authedHeaders,
      data: {
        projectId: DEFAULT_PROJECT_ID,
        qcRunId: runBody.data.qcRunId,
        issueIds: deterministicIds,
        mode: "AGENT_PATCH",
        attemptNumber: 4,
        currentHtml: dirtyHtml,
      },
    });
    expect(capRes.status()).toBe(422);

    // 5. Open live Project Readiness Modal via top-nav pill, trigger audit, capture Desktop Light/Dark + Mobile
    await page.getByTestId("nav-visual-qc-btn").click();
    await expect(page.getByTestId("project-readiness-panel")).toBeVisible({ timeout: 10000 });
    await page.getByTestId("readiness-rerun-button").click();
    await expect(page.getByTestId("readiness-metric-verdict")).not.toHaveAttribute("data-verdict", "PENDING", {
      timeout: 15000,
    });

    const applyTheme = async (theme: "warm" | "night") => {
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
    };

    await applyTheme("warm");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-07.1-desktop-light.png"), fullPage: false });

    await applyTheme("night");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-07.1-desktop-dark.png"), fullPage: false });

    await page.setViewportSize({ width: 390, height: 844 });
    await applyTheme("warm");
    await page.screenshot({ path: path.join(EVIDENCE_DIR, "story-07.1-mobile.png"), fullPage: false });

    expect(consoleErrors).toEqual([]);
    expect(pageErrors).toEqual([]);
    expect(serverErrors).toEqual([]);
    await context.close();
  });
});
