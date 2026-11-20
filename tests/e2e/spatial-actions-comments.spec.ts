// @story 04.5, 05.1, 05.2, 06.2
// @implements AC-05, AC-10, AC-08, AC-07 (Multi-Slot CMS 200 OK, Right-Click in Interact, Context-Aware Pill, Dual-Mode Edit & CommentPod)
import { test, expect } from "@playwright/test";

async function bootWorkbenchSession(page: import("@playwright/test").Page) {
  await page.setViewportSize({ width: 1440, height: 900 });
  const targetPort = process.env.TARGET_PORT || "5173";
  await page.goto(`http://127.0.0.1:${targetPort}/`, { waitUntil: "networkidle" });
  await page.evaluate(async () => {
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
    if (auth.sessionToken) {
      await fetch("/api/agent-settings", {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
          "X-Cowok-Session-Token": auth.sessionToken,
          Authorization: `Bearer ${auth.sessionToken}`,
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
  });
  await page.reload({ waitUntil: "networkidle" });
}

test.describe("Spatial 4-Action Pill, Right-Click Interact, Multi-Slot CMS & Comment-First Flow", () => {
  test("E2E-SPATIAL-01: /slots returns 200 OK (0 404s), CMS exposes multi-slot blocks, Right-Click in Interact opens context-aware Pill (no Link to DAG on Presentation), Direct Inline Edit & CommentPod work", async ({ page }) => {
    const slotsResponses: Array<{ url: string; status: number }> = [];
    page.on("response", (res) => {
      if (res.url().includes("/slots")) {
        slotsResponses.push({ url: res.url(), status: res.status() });
      }
    });

    await bootWorkbenchSession(page);

    // 1. Open Presentation Deck tab (q4-architecture-deck.html) & Slide 1 CMS drawer
    await page.getByRole("tab", { name: /q4-architecture-deck\.html/ }).click();
    await page.getByTestId("lens-slide-cms-btn").click();
    await expect(page.getByTestId("slide-list-half")).toBeVisible();
    await page.locator('[data-slide-card-index="0"]').first().click();
    await expect(page.getByTestId("cms-slide-over-drawer")).toBeVisible();

    // Verify zero 404 on /slots and structured slots inventory hydrated (more than 2 hardcoded fields)
    await expect(page.getByTestId("cms-structured-slots-inventory")).toBeVisible({ timeout: 10_000 });
    const slotCardsCount = await page.locator('[data-testid="cms-structured-slots-inventory"] .ff-cms-field-card').count();
    expect(slotCardsCount).toBeGreaterThanOrEqual(2);
    const failed404 = slotsResponses.filter((r) => r.status === 404);
    expect(failed404).toEqual([]);

    // Close CMS drawer to return to Slide list while staying in Interact mode
    await page.getByTestId("btn-back-slide-list").click();

    // 2. Right-Click directly inside the live sandboxed iframe while in Interact mode!
    const iframe = page.locator('iframe[data-testid="sandboxed-html-iframe"]').first();
    await expect(iframe).toBeVisible();
    await expect(page.getByTestId("bridge-status-pill")).toContainText("CSPRNG_VERIFIED", { timeout: 10_000 });
    await expect(page.getByTestId("toolbar-cms-actions")).toBeVisible();
    await expect(page.getByTestId("cms-undo-btn")).toBeVisible();
    await expect(page.getByTestId("cms-redo-btn")).toBeVisible();
    await expect(page.getByTestId("cms-save-btn")).toBeVisible();

    const iframeFrame = page.frameLocator('iframe[data-testid="sandboxed-html-iframe"]').first();
    const firstFfEl = iframeFrame.locator("[data-ff-id]").first();
    await expect(firstFfEl).toBeVisible();
    await firstFfEl.click({ button: "right" });

    // Pill must open with Comment, Edit Draft, Inspect — and HIDE Link to DAG on Presentation!
    const pill = page.getByTestId("stage-spatial-action-pill");
    await expect(pill).toBeVisible();
    await expect(pill).toHaveAttribute("data-trigger-source", "interact-contextmenu");
    await expect(pill).toHaveAttribute("data-tooltip-placement", /^(bottom|top)$/);
    await expect(page.getByTestId("spatial-action-comment")).toBeVisible();
    await expect(page.getByTestId("spatial-action-edit-draft")).toBeVisible();
    await expect(page.getByTestId("spatial-action-inspect")).toBeEnabled();
    await expect(page.getByTestId("spatial-action-link-dag")).toHaveCount(0);

    // Ensure zero duplicate native title attributes on Pill icon buttons
    for (const id of ["spatial-action-comment", "spatial-action-edit-draft", "spatial-action-inspect", "spatial-action-close"]) {
      await expect(page.getByTestId(id)).not.toHaveAttribute("title", /.+/);
    }

    // 3. Single-Component Edit Draft -> Direct Inline Edit Popover synced <16ms!
    await page.getByTestId("spatial-action-edit-draft").click();
    await expect(page.getByTestId("inline-direct-edit-popover")).toBeVisible();
    const inlineInput = page.getByTestId("inline-direct-edit-input");
    await inlineInput.fill("Inline Edited Title 2026");
    await page.getByTestId("inline-edit-done").click();
    await expect(page.getByTestId("inline-direct-edit-popover")).toHaveCount(0);

    // 4. Right-click -> Comment -> CommentPodPopover -> Attach to Chat tray (batching pattern)
    await firstFfEl.click({ button: "right" });
    await expect(pill).toBeVisible();
    await page.getByTestId("spatial-action-comment").click();
    await expect(page.getByTestId("comment-pod-popover")).toBeVisible();
    await page.getByTestId("comment-pod-input").fill("Vui lòng làm rõ số liệu chính trên slide này");
    await page.getByTestId("comment-pod-send-agent").click();
    await expect(page.getByTestId("comment-pod-popover")).toHaveCount(0);

    // Comment chip must land on the Floating Chat Card while Left Chat is shortened!
    await expect(page.getByTestId("contextual-node-floating-chat")).toBeVisible();
    await expect(page.getByTestId("floating-chat-composer-wrapper")).toContainText("@[Comment");

    // Pin a 2nd comment -> must queue alongside 1st comment on the Floating Chat Card
    await firstFfEl.click({ button: "right" });
    await page.getByTestId("spatial-action-comment").click();
    await page.getByTestId("comment-pod-input").fill("Thêm icon bảo mật cho CTA");
    await page.getByTestId("comment-pod-send-agent").click();
    await expect(page.getByTestId("comment-pod-popover")).toHaveCount(0);
    await expect(
      page.getByTestId("floating-chat-composer-wrapper").locator('[data-testid^="attachment-chip-"]').filter({ hasText: /@\[Comment/ })
    ).toHaveCount(2);

    // Expanding Left Chat must seamlessly show the exact same 2 queued comments in Left Chat tray!
    await page.getByTestId("chat-collapsed-rail").locator("button.btn-shorten-chat").click();
    await expect(
      page.getByTestId("chat-composer-wrapper").locator('[data-testid^="attachment-chip-"]').filter({ hasText: /@\[Comment/ })
    ).toHaveCount(2);

    // 5. Right-click again -> Inspect Property (read-only metadata + CSS vars)
    await firstFfEl.click({ button: "right" });
    await page.getByTestId("spatial-action-inspect").click();
    await expect(page.getByTestId("element-property-inspector")).toBeVisible();
    await page.getByTestId("inspector-close").click();
  });

  test("E2E-SPATIAL-02: Undo/Redo/Save stay visible in Userflow; Marquee Edit Draft enters Left CMS Drawer; FE mode hides Link to DAG when Left DAG is open & shows it when Left Pane is hidden", async ({ page }) => {
    await bootWorkbenchSession(page);

    // Switch to Userflow (FE) lens first: Undo/Redo/Save must STILL be visible on Row 1!
    await page.getByTestId("lens-userflow-btn").click();
    await expect(page.getByTestId("bridge-status-pill")).toContainText("CSPRNG_VERIFIED", { timeout: 10_000 });
    await expect(page.getByTestId("toolbar-cms-actions")).toBeVisible();
    await expect(page.getByTestId("cms-undo-btn")).toBeVisible();
    await expect(page.getByTestId("cms-redo-btn")).toBeVisible();
    await expect(page.getByTestId("cms-save-btn")).toBeVisible();

    // Switch pointer mode to Select / Marquee using pointer-mode-toggle
    const toggleModeBtn = page.getByTestId("pointer-mode-toggle");
    if ((await toggleModeBtn.getAttribute("aria-pressed")) !== "true") {
      await toggleModeBtn.click();
    }

    const overlay = page.getByTestId("marquee-drag-overlay");
    await expect(overlay).toBeVisible();
    const box = await overlay.boundingBox();
    expect(box).toBeTruthy();
    if (box) {
      await page.mouse.move(box.x + 24, box.y + 24);
      await page.mouse.down();
      await page.mouse.move(box.x + 260, box.y + 180, { steps: 6 });
      await page.mouse.up();

      const pill = page.getByTestId("stage-spatial-action-pill");
      await expect(pill).toBeVisible();
      await expect(pill).toHaveAttribute("data-target-type", "spatial_region");
      await expect(page.getByTestId("spatial-action-inspect")).toBeDisabled();

      // Clicking Edit Draft on a region from checkout-app.html stays on App Screen (no jump to q4-architecture-deck) & opens Left CMS Drawer!
      await page.getByTestId("spatial-action-edit-draft").click();
      await expect(page.getByTestId("cms-slide-over-drawer")).toBeVisible();
      await expect(page.getByTestId("lens-slide-cms-btn")).toHaveAttribute("aria-selected", "true");
      await expect(page.getByTestId("bridge-status-pill")).toContainText("DESKTOP-1440");

      // Edit App Screen Headline via CMS Drawer without clicking Save, Undo -> reverts, Redo -> restores (stays on App Screen)
      const appHeadlineInput = page.getByTestId("cms-input-headline");
      const beforeEdit = await appHeadlineInput.inputValue();
      await appHeadlineInput.fill("Edited Checkout Screen Title");
      await page.getByTestId("cms-undo-btn").click();
      await expect(appHeadlineInput).toHaveValue(beforeEdit);
      await page.getByTestId("cms-redo-btn").click();
      await expect(appHeadlineInput).toHaveValue("Edited Checkout Screen Title");
      await expect(page.getByTestId("bridge-status-pill")).toContainText("DESKTOP-1440");
    }

    // Switch back to FE (userflow) lens & Interact mode to verify Context-Aware Link to DAG
    await page.getByTestId("lens-userflow-btn").click();
    await expect(page.getByTestId("bridge-status-pill")).toContainText("CSPRNG_VERIFIED", { timeout: 10_000 });

    if ((await toggleModeBtn.getAttribute("aria-pressed")) === "true") {
      await toggleModeBtn.click();
    }

    const iframeFrame = page.frameLocator('iframe[data-testid="sandboxed-html-iframe"]').first();
    const firstFfEl = iframeFrame.locator("[data-ff-id]").first();
    await expect(firstFfEl).toBeVisible();

    // Left Pane open on userflow & focused on active node -> Link to DAG HIDDEN
    await firstFfEl.click({ button: "right" });
    await expect(page.getByTestId("stage-spatial-action-pill")).toBeVisible();
    await expect(page.getByTestId("spatial-action-link-dag")).toHaveCount(0);
    await page.getByTestId("spatial-action-close").click();

    // Hide Left Pane (expand right HTML stage) -> Right-Click -> Link to DAG VISIBLE & restores Left DAG!
    const toggleLeftBtn = page.getByTestId("toggle-expand-html-btn");
    await toggleLeftBtn.click();
    await firstFfEl.click({ button: "right" });
    await expect(page.getByTestId("spatial-action-link-dag")).toBeVisible();
    await page.getByTestId("spatial-action-link-dag").click();
    await expect(page.locator("#structureSubPane")).toBeVisible();
  });

  test("E2E-SPATIAL-03: Night theme isolates host semantic tokens inside .live-html-frame and Korean locale translates spatial tooltips cleanly", async ({ page }) => {
    await bootWorkbenchSession(page);
    await page.evaluate(() => {
      window.localStorage.setItem("cowok_locale", "ko");
      window.localStorage.setItem(
        "cowok:workbench:settings",
        JSON.stringify({
          theme: "night",
          locale: "ko",
          fontSize: "13px",
          defaultModel: "cowok-swarm",
          autoContextSlice: true,
        })
      );
      document.documentElement.setAttribute("data-theme", "night");
    });
    await page.reload({ waitUntil: "networkidle" });

    await page.getByTestId("lens-userflow-btn").click();
    await expect(page.getByTestId("bridge-status-pill")).toContainText("CSPRNG_VERIFIED", { timeout: 10_000 });

    const iframeFrame = page.frameLocator('iframe[data-testid="sandboxed-html-iframe"]').first();
    const firstFfEl = iframeFrame.locator("[data-ff-id]").first();
    await expect(firstFfEl).toBeVisible();
    await firstFfEl.click({ button: "right" });

    const pill = page.getByTestId("stage-spatial-action-pill");
    await expect(pill).toBeVisible();

    const styles = await pill.evaluate((el) => {
      const pillCs = getComputedStyle(el);
      const commentBtn = el.querySelector('[data-testid="spatial-action-comment"]') as HTMLElement;
      const btnCs = getComputedStyle(commentBtn);
      return {
        pillBg: pillCs.backgroundColor,
        pillBorder: pillCs.borderColor,
        btnColor: btnCs.color,
        tooltip: commentBtn.getAttribute("data-tooltip") || "",
        title: commentBtn.getAttribute("title"),
      };
    });

    // Night --host-bg-elevated = rgb(26, 37, 60) (#1A253C), --host-text-primary = rgb(241, 245, 249) (#F1F5F9)
    expect(styles.pillBg).toBe("rgb(26, 37, 60)");
    expect(styles.btnColor).toBe("rgb(241, 245, 249)");
    expect(styles.title).toBeNull();
    expect(styles.tooltip).toContain("피드백");
    expect(styles.tooltip).not.toMatch(/Pin feedback|Alt\+Click/i);
  });
});
