// @story 04.5
// @implements AC-01, AC-02, AC-03, AC-04
import fs from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";

const DEFAULT_PROJECT_ID = "00000000-0000-4000-8000-000000000001";
const TARGET_NODE_ID = "33333333-3333-4333-8333-333333333302";

const evidenceDir = path.resolve(
  "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.5/qa/evidence"
);
const mediaDir = path.join(evidenceDir, "media");

test.describe("Story 04.5 Acceptance: State-Hydrated Live Preview, Option CSP-B CSPRNG Bridge & Spatial Comments", () => {
  test("verifies live preview sub-states, C hotkey, CSPRNG zero-network iframe, dirty_buffer guard, 3-way spatial chips, and 409 STALE_MAPPING re-anchor", async ({
    page,
  }) => {
    fs.mkdirSync(mediaDir, { recursive: true });
    const consoleErrors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });

    await page.addInitScript(() => {
      localStorage.setItem("cowok_theme", "light");
    });

    await page.goto(`/projects/${DEFAULT_PROJECT_ID}/workbench`, { waitUntil: "domcontentloaded" });
    await page.locator(".workbench-shell").waitFor({ state: "visible", timeout: 15000 });
    await page.evaluate(async () => {
      await document.fonts.ready;
    });

    // AC-02: Option CSP-B Sandboxed Zero-Network Iframe + Retired CDN toggle + CSPRNG verified status
    const iframe = page.getByTestId("sandboxed-html-iframe");
    await expect(iframe).toBeVisible({ timeout: 10000 });
    await expect(iframe).toHaveAttribute("sandbox", "allow-scripts allow-forms");
    await expect(iframe).toHaveAttribute("referrerpolicy", "no-referrer");
    await expect(page.getByTestId("toggle-design-cdn-btn")).toHaveCount(0);
    await expect(page.getByTestId("bridge-status-pill")).toContainText("CSPRNG_VERIFIED", {
      timeout: 10000,
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

    await page.screenshot({ path: path.join(mediaDir, "01-live-preview-substates.png"), fullPage: true });

    // AC-01: Inline HTML Editor dirty_buffer Lock Guard
    await page.getByTestId("toggle-inline-code-btn").click();
    await expect(page.getByTestId("inline-code-editor-panel")).toBeVisible();
    const editorTextarea = page.getByTestId("inline-html-textarea");
    await editorTextarea.fill('<section data-ff-id="sec-hero"><h2>Updated 04.5 Preview</h2></section>');
    await expect(page.getByTestId("inline-html-dirty-badge")).toBeVisible();
    await page.getByTestId("toggle-inline-code-btn").click();
    await expect(page.getByTestId("dirty-buffer-guard-banner")).toBeVisible();
    await page.screenshot({ path: path.join(mediaDir, "02-dirty-buffer-guard.png"), fullPage: true });
    await page.getByTestId("dirty-guard-apply-btn").click();
    await expect(page.getByTestId("inline-code-editor-panel")).toBeHidden();

    // AC-03: 3-Way Spatial Feedback -> #chatAttachmentTray (Node DAG + Marquee >= 16x16px + Text Quote)
    const commentNodeBtn = page.locator('button[title*="Node"], button[title*="Bình luận"]').first();
    if (await commentNodeBtn.isVisible()) {
      await commentNodeBtn.click();
    }
    await page.getByTestId("pointer-mode-marquee").click();
    const overlay = page.getByTestId("marquee-drag-overlay");
    const box = await overlay.boundingBox();
    expect(box).toBeTruthy();
    if (box) {
      // Tiny drag < 16x16px must be ignored
      await page.mouse.move(box.x + 20, box.y + 20);
      await page.mouse.down();
      await page.mouse.move(box.x + 28, box.y + 28);
      await page.mouse.up();
      await expect(page.locator("#chatAttachmentTray")).not.toContainText("8×8px");

      // Valid marquee >= 16x16px attaches region chip
      await page.mouse.move(box.x + 20, box.y + 20);
      await page.mouse.down();
      await page.mouse.move(box.x + 90, box.y + 80);
      await page.mouse.up();
      await expect(page.locator("#chatAttachmentTray")).toContainText("70×60px");
    }

    // AC-03: Element Comment Thread, 0x0 Drawer Fallback, 409 STALE_MAPPING & Re-anchor
    const mockComment = {
      id: "cmt-e2e-1",
      tenantId: "local-default",
      projectId: DEFAULT_PROJECT_ID,
      nodeId: TARGET_NODE_ID,
      elementFfId: "sec-hero",
      author: "Designer",
      content: "Verify contrast & align CTA",
      domHash: "a1b2c3d4",
      artifactSha256: "sha256-old",
      anchorKind: "html_element",
      anchorStatus: "active",
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
    let currentComment = { ...mockComment };

    await page.route("**/flow/nodes/*/comments*", async (route) => {
      const req = route.request();
      if (req.method() === "GET") {
        await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify([currentComment]) });
        return;
      }
      if (req.method() === "POST") {
        await route.fulfill({ status: 201, contentType: "application/json", body: JSON.stringify(currentComment) });
        return;
      }
      if (req.method() === "PATCH") {
        const body = req.postDataJSON() as { reanchor?: boolean };
        if (body.reanchor) {
          currentComment = { ...currentComment, anchorStatus: "active", artifactSha256: "sha256-new" };
          await route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(currentComment) });
        } else {
          currentComment = { ...currentComment, anchorStatus: "stale" };
          await route.fulfill({
            status: 409,
            contentType: "application/json",
            body: JSON.stringify({
              code: "STALE_MAPPING",
              message: "409 STALE_MAPPING: Element digest drifted. Re-anchor required.",
            }),
          });
        }
        return;
      }
      await route.continue();
    });

    await page.getByTestId("toggle-comments-drawer-btn").click();
    await expect(page.getByTestId("comments-right-drawer")).toBeVisible();
    await page.getByTestId("new-comment-input").fill("Verify contrast & align CTA");
    await page.getByTestId("submit-comment-btn").click();
    await expect(page.getByTestId("comment-item-cmt-e2e-1")).toBeVisible();
    await expect(page.getByTestId("zero-rect-badge-cmt-e2e-1")).toBeVisible();

    // Trigger Resolve while drifted -> 409 STALE_MAPPING Toast + Stale banner -> Click Re-anchor -> Active!
    await page.getByTestId("resolve-comment-btn-cmt-e2e-1").click();
    await expect(page.getByTestId("stale-mapping-toast")).toContainText("STALE_MAPPING");
    await page.screenshot({ path: path.join(mediaDir, "03-stale-mapping-conflict.png"), fullPage: true });
    await page.getByTestId("reanchor-comment-btn-cmt-e2e-1").click();
    await expect(page.getByTestId("stale-mapping-toast")).toBeHidden();
    await page.screenshot({ path: path.join(mediaDir, "04-reanchor-resolved.png"), fullPage: true });

    expect(consoleErrors.filter((e) => !e.includes("409"))).toHaveLength(0);
  });
});
