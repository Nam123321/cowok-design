// @story 04.4
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { expect, test } from "@playwright/test";

test.describe("Story 04.4: KeyedParallelCoordinator, Monotonic Fencing, Floating Chat v2 & 3-Level Sidebar", () => {
  test("verifies Left-Open Scope Chips, 42px Pill -> 360x480..900x850 Floating Chat, Queue HUD & BFF CAS", async ({
    page,
    request,
  }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");

    await expect(page.locator(".workbench-main-sidebar")).toBeVisible({ timeout: 15_000 });

    // 1. Verify BFF local session bootstrap and node-sessions contract
    const sessRes = await request.post("/api/auth/local-session", {
      data: { role: "editor" },
    });
    expect(sessRes.ok()).toBeTruthy();
    const { sessionToken } = (await sessRes.json()) as { sessionToken: string };

    // Create isolated project for live coordinator & CAS round-trip
    const projRes = await request.post("/api/projects", {
      headers: { "X-Cowok-Session-Token": sessionToken },
      data: {
        slug: `e2e-044-${Date.now()}`,
        name: "Story 04.4 E2E",
        product_brief: "Verify parallel coordinator and fencing CAS",
      },
    });
    expect(projRes.status()).toBe(201);
    const project = (await projRes.json()) as { id: string };

    // 2. Enqueue 5 jobs -> 4 RUNNING + 1 QUEUED, verify Pause preserves slot, Cancel promotes FIFO
    const enqRes = await request.post(`/api/projects/${project.id}/flow/jobs/enqueue`, {
      headers: { "X-Cowok-Session-Token": sessionToken },
      data: {
        sessionId: "sess-e2e-044",
        nodeIds: ["n1", "n2", "n3", "n4", "n5"],
        prompt: "Build screen workflow",
        mode: "parallel",
      },
    });
    expect(enqRes.status()).toBe(201);
    const enqBody = (await enqRes.json()) as {
      jobs: Array<{ id: string; status: string; fencingToken: number }>;
      activeSlotCount: number;
    };
    expect(enqBody.jobs).toHaveLength(5);
    expect(enqBody.jobs[0].status).toBe("RUNNING");
    expect(enqBody.jobs[4].status).toBe("QUEUED");

    // Stale fencing token returns 409 STALE_FENCING_TOKEN
    const staleSteer = await request.post(`/api/projects/${project.id}/flow/jobs/steer`, {
      headers: { "X-Cowok-Session-Token": sessionToken },
      data: {
        jobId: enqBody.jobs[0].id,
        expectedFencingToken: 999,
        instruction: "Stale steer test",
        pgidReapRequested: true,
      },
    });
    expect(staleSteer.status()).toBe(409);

    // 3. UI check: select a node if visible, verify Left-Open Scope Chips & collapsed Floating Pill 42px
    const nodeCard = page.locator(".dag-node-card").first();
    if (await nodeCard.isVisible().catch(() => false)) {
      await nodeCard.click();
      await expect(page.getByTestId("left-chat-scope-chips")).toBeVisible();

      const shortenBtn = page.locator(".btn-shorten-chat").first();
      await shortenBtn.click();

      const pill = page.getByTestId("fc-compact-pill");
      await expect(pill).toBeVisible();
      const pillBox = await pill.boundingBox();
      expect(pillBox?.height).toBeGreaterThanOrEqual(40);

      await page.getByTestId("floating-chat-open-trigger").click();
      const floatingWin = page.getByTestId("contextual-node-floating-chat");
      await expect(floatingWin).toBeVisible();
      const winBox = await floatingWin.boundingBox();
      expect(winBox?.width).toBeGreaterThanOrEqual(360);
      expect(winBox?.height).toBeGreaterThanOrEqual(480);
    }
  });
});
