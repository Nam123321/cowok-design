// @story 04.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { execSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

const EVIDENCE_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.4/qa/evidence"
);
const DEFAULT_PROJ_ID = "11111111-1111-4111-8111-111111111111";

async function verifyUiAndResize(page: Page, saveShot: (name: string) => Promise<void>) {
  await expect(page.locator(".workbench-main-sidebar")).toBeVisible({ timeout: 15_000 });
  if ((await page.locator(".dag-node-card").count()) === 0) {
    await page.locator(".dag-right-vertical-dock button").first().click();
  }
  const nodeCard = page.locator(".dag-node-card").first();
  await expect(nodeCard).toBeVisible({ timeout: 10_000 });
  await nodeCard.click();
  await expect(page.getByTestId("left-chat-scope-chips")).toBeVisible();
  await saveShot("01-left-open-scope-chips.png");

  await page.locator(".btn-shorten-chat").first().click();
  const pill = page.getByTestId("fc-compact-pill");
  await expect(pill).toBeVisible();
  expect((await pill.boundingBox())?.height).toBeGreaterThanOrEqual(42);
  await saveShot("02-floating-pill-42px.png");

  await page.getByTestId("floating-chat-open-trigger").click();
  const win = page.getByTestId("contextual-node-floating-chat");
  await expect(win).toBeVisible();
  for (const dir of ["n", "s", "e", "w", "ne", "nw", "se", "sw"]) {
    await expect(page.getByTestId(`fc-resize-${dir}`)).toBeVisible();
  }
  const seBox = await page.getByTestId("fc-resize-se").boundingBox();
  if (seBox) {
    await page.mouse.move(seBox.x + 5, seBox.y + 5);
    await page.mouse.down();
    await page.mouse.move(seBox.x + 85, seBox.y + 40);
    await page.mouse.up();
  }
  const afterBox = await win.boundingBox();
  expect(afterBox?.width).toBeGreaterThanOrEqual(360);
  expect(afterBox?.width).toBeLessThanOrEqual(900);
  expect(afterBox?.height).toBeGreaterThanOrEqual(480);
  expect(afterBox?.height).toBeLessThanOrEqual(850);
  await saveShot("03-floating-chat-expanded-resized.png");
}

async function verifyDualSurfaceAndSidebarSync(page: Page, saveShot: (name: string) => Promise<void>) {
  const composer = page.getByTestId("floating-chat-composer-textarea");
  await composer.fill("Sync prompt from floating chat");
  await composer.press("Enter");
  const subRow = page.locator("[data-testid^='subsession-row-']").first();
  await expect(subRow).toBeVisible({ timeout: 10_000 });
  await subRow.click();
  await page.locator(".workbench-chat-panel-shortened .btn-shorten-chat").first().click();
  await expect(page.locator(".workbench-chat-panel")).toContainText("Sync prompt from floating chat");
  await saveShot("04-sidebar-3level-subsession.png");
}

async function verifyPluginsAndCreateProject(page: Page, request: APIRequestContext, baseUrl: string) {
  const origin = new URL(baseUrl).origin;
  const bootCap = (await page.locator('meta[name="cowok-boot-cap"]').first().getAttribute("content")) || "";
  const sessRes = await request.post(`${baseUrl}api/auth/local-session`, {
    headers: { Origin: origin, "X-Cowok-Boot-Cap": bootCap },
    data: { role: "editor" },
  });
  expect(sessRes.ok()).toBeTruthy();
  const { sessionToken } = (await sessRes.json()) as { sessionToken: string };
  const headers = { Origin: origin, "X-Cowok-Session-Token": sessionToken };

  const wsRoot = process.env.UXUI_OS_WORKSPACE_ROOT?.trim() || process.cwd();
  const dbPath = path.join(wsRoot, ".cowok-design", "cowok-design.sqlite3");
  execSync(`node src/plugins/cli-worker-subprocess-adapter/package-plugin.mjs --seed --db "${dbPath}" --project "${DEFAULT_PROJ_ID}"`, { stdio: "pipe" });

  const pluginsRes = await request.get(`${baseUrl}api/plugins?enabled=true`, { headers });
  expect(pluginsRes.status()).toBe(200);
  const pluginsBody = (await pluginsRes.json()) as Array<{ pluginSlug: string }> | { plugins: Array<{ pluginSlug: string }> };
  const pluginList = Array.isArray(pluginsBody) ? pluginsBody : pluginsBody.plugins;
  expect(pluginList.some((p) => p.pluginSlug === "cli-worker-subprocess-adapter")).toBe(true);

  const defSess = await request.get(`${baseUrl}api/projects/${DEFAULT_PROJ_ID}/flow/node-sessions`, { headers });
  const defBody = (await defSess.json()) as { jobs: Array<{ id: string; status: string; fencingToken: number }> };
  expect(defBody.jobs.some((j) => j.status === "RUNNING")).toBe(true);
  for (const job of defBody.jobs.filter((j) => j.status === "RUNNING" || j.status === "PAUSED")) {
    await request.post(`${baseUrl}api/projects/${DEFAULT_PROJ_ID}/flow/jobs/cancel`, {
      headers, data: { jobId: job.id, expectedFencingToken: job.fencingToken },
    });
  }

  const projRes = await request.post(`${baseUrl}api/projects`, {
    headers,
    data: { slug: `e2e-044-${Date.now()}`, name: "Story 04.4 Live", product_brief: "Verify FIFO, CAS, SSE" },
  });
  expect(projRes.status()).toBe(201);
  const { id: projectId } = (await projRes.json()) as { id: string };
  return { headers, projectId };
}

async function verifyFifoCasAndSse(request: APIRequestContext, baseUrl: string, headers: Record<string, string>, projectId: string) {
  const enqRes = await request.post(`${baseUrl}api/projects/${projectId}/flow/jobs/enqueue`, {
    headers,
    data: { sessionId: "sess-e2e-044", nodeIds: ["n1", "n2", "n3", "n4", "n5"], prompt: "Build screen", mode: "parallel" },
  });
  expect(enqRes.status()).toBe(201);
  const enqBody = (await enqRes.json()) as { jobs: Array<{ id: string; status: string; fencingToken: number }>; activeSlotCount: number };
  expect(enqBody.jobs.filter((j) => j.status === "RUNNING")).toHaveLength(4);
  expect(enqBody.jobs[4].status).toBe("QUEUED");

  const pauseRes = await request.post(`${baseUrl}api/projects/${projectId}/flow/jobs/pause`, {
    headers, data: { jobId: enqBody.jobs[0].id, expectedFencingToken: enqBody.jobs[0].fencingToken },
  });
  expect(pauseRes.status()).toBe(200);
  const pausedDto = (await pauseRes.json()) as { status: string; fencingToken: number };
  expect(pausedDto.status).toBe("PAUSED");

  const cancelRes = await request.post(`${baseUrl}api/projects/${projectId}/flow/jobs/cancel`, {
    headers, data: { jobId: enqBody.jobs[0].id, expectedFencingToken: pausedDto.fencingToken },
  });
  expect(cancelRes.status()).toBe(200);

  const listRes = await request.get(`${baseUrl}api/projects/${projectId}/flow/node-sessions`, { headers });
  const listed = (await listRes.json()) as { jobs: Array<{ id: string; status: string; fencingToken: number }> };
  const promoted = listed.jobs.find((j) => j.id === enqBody.jobs[4].id);
  expect(promoted?.status).toBe("RUNNING");
  expect(promoted?.fencingToken).toBe(2);

  const validSteer = await request.post(`${baseUrl}api/projects/${projectId}/flow/jobs/steer`, {
    headers, data: { jobId: enqBody.jobs[1].id, expectedFencingToken: enqBody.jobs[1].fencingToken, instruction: "Valid steer", pgidReapRequested: true },
  });
  expect(validSteer.status()).toBe(200);

  const staleSteer = await request.post(`${baseUrl}api/projects/${projectId}/flow/jobs/steer`, {
    headers, data: { jobId: enqBody.jobs[1].id, expectedFencingToken: enqBody.jobs[1].fencingToken, instruction: "Stale steer", pgidReapRequested: true },
  });
  expect(staleSteer.status()).toBe(409);

  const ev1Res = await request.post(`${baseUrl}api/projects/${projectId}/flow/jobs/events`, {
    headers,
    data: {
      jobId: enqBody.jobs[2].id, sessionId: "sess-e2e-044", nodeId: "n3",
      eventKind: "chunk", payloadText: "n3 first chunk", jobStatus: "RUNNING", expectedFencingToken: enqBody.jobs[2].fencingToken,
    },
  });
  expect([200, 201]).toContain(ev1Res.status());
  const ev1 = (await ev1Res.json()) as { projectSeq?: number; seqNo: number };
  const firstSeq = ev1.projectSeq ?? ev1.seqNo;

  const ev2Res = await request.post(`${baseUrl}api/projects/${projectId}/flow/jobs/events`, {
    headers,
    data: {
      jobId: enqBody.jobs[2].id, sessionId: "sess-e2e-044", nodeId: "n3",
      eventKind: "completed", payloadText: "n3 done", jobStatus: "COMPLETED", expectedFencingToken: enqBody.jobs[2].fencingToken + 1,
    },
  });
  expect([200, 201]).toContain(ev2Res.status());
  const ac = new AbortController();
  const sseRes = await fetch(`${baseUrl}api/projects/${projectId}/flow/jobs/stream?nodeId=n3`, {
    headers: { ...headers, "Last-Event-ID": String(firstSeq) },
    signal: ac.signal,
  });
  expect(sseRes.status).toBe(200);
  const reader = sseRes.body!.getReader();
  const decoder = new TextDecoder();
  let sseText = "";
  while (!sseText.includes("n3 done")) {
    const { done, value } = await reader.read();
    if (done) break;
    sseText += decoder.decode(value, { stream: true });
  }
  ac.abort();
  expect(sseText).toContain("n3 done");
  expect(sseText).not.toContain("n3 first chunk");
}

test.describe("Story 04.4 Unconditional Live E2E Acceptance", () => {
  test("verifies live UI surfaces, 8-dir resize, 3-level sidebar, FIFO promotion, CAS 409, SSE replay & plugins", async ({
    browser,
    request,
  }) => {
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    fs.writeFileSync(path.join(EVIDENCE_DIR, "qa-nonce.json"), JSON.stringify({ nonce: crypto.randomBytes(16).toString("hex"), generatedAt: new Date().toISOString() }, null, 2));

    const videoDir = path.join(EVIDENCE_DIR, "videos");
    fs.mkdirSync(videoDir, { recursive: true });
    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      recordVideo: { dir: videoDir, size: { width: 1440, height: 900 } },
    });
    const page = await context.newPage();

    const consoleErrors: string[] = [];
    const pageErrors: string[] = [];
    const http500s: string[] = [];
    page.on("console", (msg) => { if (msg.type() === "error") consoleErrors.push(msg.text()); });
    page.on("pageerror", (err) => pageErrors.push(err.message));
    page.on("response", (res) => { if (res.status() >= 500) http500s.push(`${res.status()} ${res.url()}`); });

    await page.setViewportSize({ width: 1440, height: 900 });
    await page.addInitScript(() => {
      if (window.self === window.top) window.localStorage.setItem("cowok_first_run_completed", "true");
    });
    const targetPort = process.env.TARGET_PORT || process.env.PORT || "4173";
    const baseUrl = `http://127.0.0.1:${targetPort}/`;
    try {
      await page.goto(baseUrl, { waitUntil: "networkidle" });

      const saveShot = async (name: string) => { await page.screenshot({ path: path.join(EVIDENCE_DIR, name), fullPage: true }); };
      await verifyUiAndResize(page, saveShot);
      await verifyDualSurfaceAndSidebarSync(page, saveShot);
      const { headers, projectId } = await verifyPluginsAndCreateProject(page, request, baseUrl);
      await verifyFifoCasAndSse(request, baseUrl, headers, projectId);
      fs.writeFileSync(path.join(EVIDENCE_DIR, "dom-snapshot.html"), await page.content(), "utf8");

      expect(consoleErrors).toEqual([]);
      expect(pageErrors).toEqual([]);
      expect(http500s).toEqual([]);
    } finally {
      const vid = page.video();
      await context.close();
      if (vid) {
        try {
          await vid.saveAs(path.join(EVIDENCE_DIR, "acceptance-story-04.4.webm"));
        } catch {
          // ignore video flush error
        }
      }
    }
  });
});
