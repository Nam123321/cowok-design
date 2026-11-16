// @story 04.5
// @implements AC-01, AC-02, AC-03, AC-04
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

const EVIDENCE_DIR = path.resolve(
  process.cwd(),
  "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.5/qa/evidence"
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

test.describe("Story 04.5 Acceptance: State-Hydrated Live Preview, Option CSP-B CSPRNG Bridge & Spatial Comments", () => {
  test("verifies live preview sub-states, C hotkey, CSPRNG zero-network iframe, dirty_buffer guard, 3-way spatial chips, and live 409 STALE_MAPPING re-anchor", async ({
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
      const headers = {
        ...route.request().headers(),
        "x-iwish-qa-nonce": qaNonce,
        "x-tenant-id": "local-default",
      };
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
      if (window.self === window.top) {
        window.localStorage.setItem("cowok_theme", "light");
        window.localStorage.setItem("cowok_first_run_completed", "true");
      }
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

      await page.locator(".workbench-shell").waitFor({ state: "visible", timeout: 15_000 });
      await expect(page.locator('.workbench-shell[data-flow-hydrated="true"]')).toBeVisible({ timeout: 15_000 });
      await page.evaluate(async () => {
        await document.fonts.ready;
      });

      // Ensure active DAG node is selected so Live Preview + Comments Drawer bind to real project/node IDs
      if ((await page.locator(".dag-node-card").count()) === 0) {
        await page.locator(".dag-right-vertical-dock button").first().click();
      }
      const firstNode = page.locator(".dag-node-card").first();
      if (await firstNode.isVisible()) {
        await firstNode.click();
      }

      // AC-02: Option CSP-B Sandboxed Zero-Network Iframe + Retired CDN toggle + CSPRNG verified status
      const iframe = page.getByTestId("sandboxed-html-iframe");
      await expect(iframe).toBeVisible({ timeout: 10_000 });
      await expect(iframe).toHaveAttribute("sandbox", "allow-scripts allow-forms");
      await expect(iframe).toHaveAttribute("referrerpolicy", "no-referrer");
      await expect(page.getByTestId("toggle-design-cdn-btn")).toHaveCount(0);
      await expect(page.getByTestId("bridge-status-pill")).toContainText("CSPRNG_VERIFIED", {
        timeout: 10_000,
      });

      // AC-01: Designer-friendly Interact vs Inspect / Comment (C) toggle + Hotkey C
      const interactBtn = page.getByTestId("pointer-mode-interact");
      const inspectBtn = page.getByTestId("pointer-mode-inspect");
      await expect(interactBtn).toHaveAttribute("aria-pressed", "true");
      await page.keyboard.press("c");
      await expect(inspectBtn).toHaveAttribute("aria-pressed", "true");
      await page.keyboard.press("c");
      await expect(interactBtn).toHaveAttribute("aria-pressed", "true");

      // AC-01: Uppercase 4-SubState Fixture Switcher (DEFAULT | LOADING | EMPTY | ERROR) + Viewport Switcher
      for (const variant of ["DEFAULT", "LOADING", "EMPTY", "ERROR"]) {
        const btn = page.getByTestId(`fixture-variant-${variant}`);
        await expect(btn).toBeVisible();
        await btn.click();
        await expect(btn).toHaveAttribute("aria-pressed", "true");
      }
      await page.getByTestId("viewport-btn-mobile-390").click();
      await expect(page.getByTestId("bridge-status-pill")).toContainText("MOBILE-390");
      await page.getByTestId("viewport-btn-desktop-1440").click();
      await page.screenshot({ path: path.join(EVIDENCE_DIR, "01-live-preview-substates.png"), fullPage: true });

      // AC-01: Inline HTML Editor dirty_buffer Lock Guard + Apply closes panel cleanly
      await page.getByTestId("toggle-inline-code-btn").click();
      await expect(page.getByTestId("inline-code-editor-panel")).toBeVisible();
      const editorTextarea = page.getByTestId("inline-html-textarea");
      await editorTextarea.fill('<section data-ff-id="sec-hero"><h2>Updated 04.5 Preview</h2></section>');
      await expect(page.getByTestId("inline-html-dirty-badge")).toBeVisible();
      await page.getByTestId("toggle-inline-code-btn").click();
      await expect(page.getByTestId("dirty-buffer-guard-banner")).toBeVisible();
      await page.screenshot({ path: path.join(EVIDENCE_DIR, "02-dirty-buffer-guard.png"), fullPage: true });
      await page.getByTestId("dirty-guard-apply-btn").click();
      await expect(page.getByTestId("inline-code-editor-panel")).toBeHidden();

      // AC-03: 3-Way Spatial Feedback -> #chatAttachmentTray (Node DAG + Marquee >= 16x16px)
      await page.getByTestId("pointer-mode-marquee").click();
      const overlay = page.getByTestId("marquee-drag-overlay");
      const box = await overlay.boundingBox();
      expect(box).toBeTruthy();
      if (box) {
        // Tiny drag < 16x16px must be ignored (tray stays empty / unmounted)
        await page.mouse.move(box.x + 20, box.y + 20);
        await page.mouse.down();
        await page.mouse.move(box.x + 28, box.y + 28);
        await page.mouse.up();
        await expect(page.locator("#chatAttachmentTray")).toHaveCount(0);

        // Valid marquee >= 16x16px attaches region chip
        await page.mouse.move(box.x + 20, box.y + 20);
        await page.mouse.down();
        await page.mouse.move(box.x + 90, box.y + 80);
        await page.mouse.up();
        await expect(page.locator("#chatAttachmentTray")).toContainText("70×60px");
      }

      // AC-03: Persistent SQLite V7 Element Comment, 0x0 Drawer Fallback, Real 409 STALE_MAPPING & Re-anchor
      await page.getByTestId("toggle-comments-drawer-btn").click();
      await expect(page.getByTestId("comments-right-drawer")).toBeVisible();
      const commentText = `Verify contrast 04.5 ${Date.now()}`;
      await page.getByTestId("new-comment-input").fill(commentText);
      const createRespPromise = page.waitForResponse(
        (res) => res.url().includes("/comments") && res.request().method() === "POST"
      );
      await page.getByTestId("submit-comment-btn").click();
      const createResp = await createRespPromise;
      expect(createResp.status()).toBe(201);
      const created = (await createResp.json()) as {
        id: string;
        projectId: string;
        nodeId: string;
        domHash: string;
        artifactSha256: string;
      };
      await expect(page.getByTestId(`comment-item-${created.id}`)).toBeVisible();
      await expect(page.getByTestId(`zero-rect-badge-${created.id}`)).toBeVisible();

      // Trigger real backend CommitAllowed CAS 409 STALE_MAPPING against live SQLite V7
      const stalePatchRes = await request.patch(
        `${baseUrl}api/projects/${created.projectId}/flow/nodes/${created.nodeId}/comments/${created.id}`,
        {
          headers,
          data: {
            content: "Attempt stale mutation",
            expectedDomHash: "drifted-0000",
          },
        }
      );
      expect(stalePatchRes.status()).toBe(409);
      const staleBody = (await stalePatchRes.json()) as { code?: string };
      expect(staleBody.code).toBe("STALE_MAPPING");

      // Reload & inspect stale banner rendered straight from live SQLite V7
      await page.reload({ waitUntil: "networkidle" });
      await expect(page.locator('.workbench-shell[data-flow-hydrated="true"]')).toBeVisible({ timeout: 15_000 });
      const reloadedNode = page.locator(".dag-node-card").first();
      if (await reloadedNode.isVisible()) {
        await reloadedNode.click();
      }
      await page.getByTestId("toggle-comments-drawer-btn").click();
      await expect(page.getByTestId(`stale-banner-${created.id}`)).toContainText("STALE_MAPPING", { timeout: 10_000 });
      await page.screenshot({ path: path.join(EVIDENCE_DIR, "03-stale-mapping-conflict.png"), fullPage: true });

      // Click Re-anchor -> clears stale status in live SQLite V7!
      const reanchorPromise = page.waitForResponse(
        (res) => res.url().includes(`/comments/${created.id}`) && res.request().method() === "PATCH"
      );
      await page.getByTestId(`reanchor-comment-btn-${created.id}`).click();
      const reanchorResp = await reanchorPromise;
      expect(reanchorResp.status()).toBe(200);
      await expect(page.getByTestId(`stale-banner-${created.id}`)).not.toContainText("STALE_MAPPING", {
        timeout: 10_000,
      });
      await page.screenshot({ path: path.join(EVIDENCE_DIR, "04-reanchor-resolved.png"), fullPage: true });

      fs.writeFileSync(path.join(EVIDENCE_DIR, "dom-snapshot.html"), await page.content(), "utf8");
      expect(consoleErrors).toEqual([]);
      expect(pageErrors).toEqual([]);
      expect(http500s).toEqual([]);
    } finally {
      const vid = page.video();
      await context.close();
      if (vid) {
        try {
          await vid.saveAs(path.join(EVIDENCE_DIR, "acceptance-story-04.5.webm"));
        } catch {
          // ignore video flush error
        }
      }
    }
  });
});
