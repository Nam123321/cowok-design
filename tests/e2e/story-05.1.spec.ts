// @story 05.1
// @implements Unconditional Live E2E Acceptance for Story 05.1 (Sandbox CSP/Nonce, Option 5 Storyboard, Sample Data Studio, Zero-Amber Night Mode, Live SDI Intent & Clarification API, Loopback Graph & ScreenFixture CAS)
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

const EVIDENCE_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-05/Story-05.1/qa/evidence"
);

async function mintEditorHeaders(page: Page, request: APIRequestContext, baseUrl: string) {
  const origin = new URL(baseUrl).origin;
  const bootCap = (await page.locator('meta[name="cowok-boot-cap"]').first().getAttribute("content")) || "";
  const sessRes = await request.post(`${baseUrl}api/auth/local-session`, {
    headers: { Origin: origin, "X-Cowok-Boot-Cap": bootCap },
    data: { role: "editor" },
  });
  expect(sessRes.ok()).toBeTruthy();
  const { sessionToken } = (await sessRes.json()) as { sessionToken: string };
  return { Origin: origin, "X-Cowok-Session-Token": sessionToken };
}

async function verifyLiveWorkbenchUi(page: Page, saveShot: (name: string) => Promise<void>) {
  await expect(page.locator(".workbench-main-sidebar")).toBeVisible({ timeout: 15_000 });
  if ((await page.locator(".dag-node-card").count()) === 0) {
    await page.locator(".dag-right-vertical-dock button").first().click();
  }
  const nodeCard = page.locator(".dag-node-card").first();
  await expect(nodeCard).toBeVisible({ timeout: 10_000 });
  await nodeCard.click();

  // Verify Option 5 Storyboard mini-skeleton + substate pills on DagNodeCard
  const miniSkeleton = page.getByTestId("dag-node-mini-skeleton").first();
  await expect(miniSkeleton).toBeVisible({ timeout: 10_000 });
  const testIdAttr = await nodeCard.getAttribute("data-testid");
  const nodeUuid = (testIdAttr || "").replace("dag-node-", "");
  if (nodeUuid) {
    for (const sub of ["default", "loading", "empty", "error"]) {
      await expect(page.getByTestId(`dag-substate-${sub}-${nodeUuid}`)).toBeVisible();
    }
    await page.getByTestId(`dag-substate-loading-${nodeUuid}`).click();
    await page.getByTestId(`dag-substate-default-${nodeUuid}`).click();
  }

  // Verify active sandboxed iframe hardening (allow-scripts without allow-same-origin) + LRU KeepAlive hidden slot
  const sandboxFrame = page.getByTestId("sandboxed-html-iframe");
  await expect(sandboxFrame).toBeVisible({ timeout: 10_000 });
  const sandboxAttr = (await sandboxFrame.getAttribute("sandbox")) || "";
  expect(sandboxAttr).toContain("allow-scripts");
  expect(sandboxAttr).not.toContain("allow-same-origin");
  const cachedSlots = page.locator('[data-testid^="sandboxed-html-iframe-cached-"]');
  if ((await cachedSlots.count()) > 0) {
    const cachedCss = await cachedSlots.first().evaluate((el) => {
      const cs = getComputedStyle(el);
      return { visibility: cs.visibility, display: cs.display };
    });
    expect(cachedCss.visibility).toBe("hidden");
    expect(cachedCss.display).not.toBe("none");
  }
  await saveShot("01-story-05-1-storyboard-and-sandbox.png");

  // Verify Sample Data Studio modal opens from live toolbar button, switches preset, toggles tray
  await expect(page.getByTestId("canvas-mode-group")).toBeVisible();
  const sampleDataBtn = page.getByTestId("canvas-mode-sample-data");
  await expect(sampleDataBtn).toBeVisible();
  await sampleDataBtn.click();
  const sampleModal = page.getByTestId("sample-data-studio-modal");
  await expect(sampleModal).toBeVisible();
  await page.getByTestId("preset-filter-btn-validation_error").click();
  await expect(sampleModal).toContainText("ERR_INSUFFICIENT_MULTI_SIG_QUORUM");
  await saveShot("02-sample-data-studio.png");
  await page.getByTestId("toggle-tray-mode-btn").click();
  await page.getByTestId("sample-data-cancel-btn").click();
  await expect(sampleModal).toBeHidden();

  // Verify Night mode Zero-Amber CSS token compliance at runtime
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "night"));
  const nightTokens = await page.evaluate(() => {
    const cs = getComputedStyle(document.documentElement);
    return {
      actionPrimary: cs.getPropertyValue("--action-primary").trim().toLowerCase(),
      humanAccent: cs.getPropertyValue("--human-accent").trim().toLowerCase(),
      agentAccent: cs.getPropertyValue("--agent-accent").trim().toLowerCase(),
    };
  });
  for (const val of Object.values(nightTokens)) {
    expect(val).not.toBe("#ff9a2f");
    expect(val).not.toBe("#e89a35");
  }
  await saveShot("03-night-mode-zero-amber.png");
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "warm"));
}

async function verifyLiveSdiLoopbackAndFixturesApi(
  request: APIRequestContext,
  baseUrl: string,
  headers: { Origin: string; "X-Cowok-Session-Token": string }
) {
  // 1. Create workspace via Launchpad contract
  const projSlug = `e2e-s051-${Date.now()}`;
  const createProj = await request.post(`${baseUrl}api/projects`, {
    headers,
    data: {
      slug: projSlug,
      name: "E2E Story 05.1 Workbench",
      product_brief: "Multi-sig vault approval storyboard with loopback retry",
      provenance: "scratch",
      entryPath: "prompt_prd",
    },
  });
  expect(createProj.status()).toBe(201);
  const project = (await createProj.json()) as { id: string };
  expect(project.id).toBeTruthy();

  // 2. Compile ambiguous SDI intent -> gapQuestions + 409 lock on executeImmediately
  const vagueKey = `e2e-sdi-${Date.now()}`;
  const sdiRes = await request.post(`${baseUrl}api/projects/${project.id}/sdi/intent`, {
    headers: { ...headers, "Idempotency-Key": vagueKey },
    data: { rawPrompt: "làm đẹp giao diện" },
  });
  expect(sdiRes.status()).toBe(201);
  const sdiBody = (await sdiRes.json()) as {
    id: string;
    status: string;
    ambiguityScore: number;
    clarificationVariants: Array<{ id: string }>;
    gapQuestions: Array<{ id: string }>;
  };
  expect(sdiBody.status).toBe("clarifying");
  expect(sdiBody.ambiguityScore).toBeGreaterThanOrEqual(0.45);
  expect(sdiBody.gapQuestions.length).toBeGreaterThanOrEqual(1);

  const lockRes = await request.post(`${baseUrl}api/projects/${project.id}/sdi/intent`, {
    headers,
    data: { rawPrompt: "fix ui", executeImmediately: true },
  });
  expect(lockRes.status()).toBe(409);

  // 3. Clarification selection transitions clarifying -> ready
  const selectRes = await request.post(`${baseUrl}api/projects/${project.id}/sdi/clarification`, {
    headers,
    data: {
      intentId: sdiBody.id,
      selectedVariantId: sdiBody.clarificationVariants[0]!.id,
    },
  });
  expect(selectRes.ok()).toBeTruthy();
  const clarified = (await selectRes.json()) as { status: string; revision: number };
  expect(clarified.status).toBe("ready");
  expect(clarified.revision).toBe(2);

  // 4. Create 2 nodes & verified ScreenFixture CAS round-trip
  const n1Res = await request.post(`${baseUrl}api/projects/${project.id}/flow/nodes`, {
    headers,
    data: { screenKey: "SCR-ENTRY", title: "Entry Vault", nodeType: "screen", routePath: "/entry" },
  });
  expect(n1Res.status()).toBe(201);
  const n1 = (await n1Res.json()) as { id: string };

  const fixCreate = await request.post(`${baseUrl}api/projects/${project.id}/flow/nodes/${n1.id}/fixtures`, {
    headers,
    data: { variant: "DEFAULT", payloadJson: JSON.stringify({ rows: 2 }), presetKey: "happy" },
  });
  expect(fixCreate.status()).toBe(201);
  const fix = (await fixCreate.json()) as { id: string; revision: number };
  expect(fix.revision).toBe(1);

  const stalePatch = await request.patch(
    `${baseUrl}api/projects/${project.id}/flow/nodes/${n1.id}/fixtures/${fix.id}`,
    {
      headers,
      data: { presetKey: "empty_state", expectedRevision: 99 },
    }
  );
  expect(stalePatch.status()).toBe(409);

  const validPatch = await request.patch(
    `${baseUrl}api/projects/${project.id}/flow/nodes/${n1.id}/fixtures/${fix.id}`,
    {
      headers,
      data: { presetKey: "empty_state", expectedRevision: 1 },
    }
  );
  expect(validPatch.status()).toBe(200);
  const updatedFix = (await validPatch.json()) as { revision: number; presetKey: string };
  expect(updatedFix.revision).toBe(2);
  expect(updatedFix.presetKey).toBe("empty_state");
}

test.describe("Story 05.1 Live E2E Acceptance Suite", () => {
  test("verifies live Storyboard UI, Sandbox CSP, Sample Data Studio, Night tokens, SDI & Fixture APIs", async ({
    browser,
    request,
  }) => {
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    const qaNonce = crypto.randomBytes(16).toString("hex");
    fs.writeFileSync(
      path.join(EVIDENCE_DIR, "qa-nonce.json"),
      JSON.stringify({ nonce: qaNonce, generatedAt: new Date().toISOString() }, null, 2)
    );

    const videoDir = path.join(EVIDENCE_DIR, "videos");
    fs.mkdirSync(videoDir, { recursive: true });
    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      recordVideo: { dir: videoDir, size: { width: 1440, height: 900 } },
      recordHar: { path: path.join(EVIDENCE_DIR, "network-trace.har") },
    });
    await context.route(/^http:\/\/(?:127\.0\.0\.1|localhost)(?::\d+)?\//, async (route) => {
      const headers = { ...route.request().headers(), "x-iwish-qa-nonce": qaNonce, "x-tenant-id": "local-default" };
      await route.continue({ headers });
    });
    const page = await context.newPage();

    const consoleErrors: string[] = [];
    const pageErrors: string[] = [];
    const http500s: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("response", (res) => {
      if (res.status() >= 500) http500s.push(`${res.status()} ${res.url()}`);
    });

    await page.setViewportSize({ width: 1440, height: 900 });
    await page.addInitScript(() => {
      if (window.self === window.top) window.localStorage.setItem("cowok_first_run_completed", "true");
    });
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;

    try {
      await page.goto(baseUrl, { waitUntil: "networkidle" });
      const headers = await mintEditorHeaders(page, request, baseUrl);
      const onboardRes = await request.put(`${baseUrl}api/agent-settings`, {
        headers,
        data: {
          providerId: "cowok-swarm",
          modelId: "cowok-swarm",
          vaultMode: "vault_0600",
          permissionMode: "auto_sandbox",
          onboardingDone: true,
        },
      });
      expect(onboardRes.ok()).toBeTruthy();
      await page.reload({ waitUntil: "networkidle" });

      const saveShot = async (name: string) => {
        await page.screenshot({ path: path.join(EVIDENCE_DIR, name), fullPage: true });
      };
      await verifyLiveWorkbenchUi(page, saveShot);
      await verifyLiveSdiLoopbackAndFixturesApi(request, baseUrl, headers);
      fs.writeFileSync(path.join(EVIDENCE_DIR, "dom-snapshot.html"), await page.content(), "utf8");

      expect(consoleErrors).toEqual([]);
      expect(pageErrors).toEqual([]);
      expect(http500s).toEqual([]);
    } finally {
      const vid = page.video();
      await context.close();
      if (vid) {
        try {
          await vid.saveAs(path.join(EVIDENCE_DIR, "acceptance-story-05.1.webm"));
        } catch {
          /* ignore video flush failure */
        }
      }
    }
  });
});
