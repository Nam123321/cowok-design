import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

interface ScenarioResult {
  id: string;
  name: string;
  status: "PASSED" | "FAILED" | "PENDING";
  durationMs: number;
  screenshots: string[];
  assertions: string[];
  error?: string;
  pendingReason?: string;
}

test.use({
  video: {
    mode: "on",
    size: { width: 1440, height: 900 },
  },
  viewport: { width: 1440, height: 900 },
});

test.describe("Story 04.1: Direct Browser Acceptance Suite with Video & Screenshot Evidence", () => {
  test("executes sequential scenarios 1 to 6 with error isolation and video recording", async ({ page }, testInfo) => {
    const startTime = Date.now();
    const mediaDir = path.resolve(
      process.cwd(),
      "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.1/qa/acceptance-media"
    );
    fs.mkdirSync(mediaDir, { recursive: true });

    const results: ScenarioResult[] = [];
    let scenario1Passed = false;
    let scenario2Passed = false;
    let scenario3Passed = false;
    let scenario4Passed = false;
    let scenario5Passed = false;
    let scenario6Passed = false;

    let nodesBefore = 0;
    const targetPort = process.env.TARGET_PORT || "4174";

    await page.addInitScript(() => {
      window.localStorage.setItem("cowok_first_run_completed", "true");
    });

    console.log(`\n[ACCEPTANCE] Navigating to target http://127.0.0.1:${targetPort}/`);
    await page.goto(`http://127.0.0.1:${targetPort}/`, { waitUntil: "networkidle" });

    // =========================================================================
    // KỊCH BẢN 1: Workbench Shell Layout & Night Mode Ergonomics (AC-01, AC-07)
    // =========================================================================
    const s1Start = Date.now();
    const s1: ScenarioResult = {
      id: "SCENARIO-01",
      name: "Workbench Shell Layout & Night Mode Ergonomics (AC-01, AC-07)",
      status: "PENDING",
      durationMs: 0,
      screenshots: [],
      assertions: [],
    };

    try {
      console.log("\n▶ [SCENARIO-01] Testing Workbench Shell Layout & Night Mode...");
      // 1. Initial Light Mode capture
      const shotLight = path.join(mediaDir, "01-workbench-layout-light.png");
      await page.screenshot({ path: shotLight, fullPage: true });
      s1.screenshots.push("01-workbench-layout-light.png");
      s1.assertions.push("Workbench shell initial rendering captured in Light Mode");

      // 2. Open Settings modal from sidebar seamless footer
      const footerSettingsBtn = page.locator('[data-testid="sidebar-seamless-footer"] button');
      await expect(footerSettingsBtn).toBeVisible({ timeout: 10000 });
      await footerSettingsBtn.click();
      s1.assertions.push("Opened settings dialog via sidebar-seamless-footer button");

      const dialog = page.getByRole("dialog");
      await expect(dialog).toBeVisible({ timeout: 5000 });
      await dialog.getByRole("button", { name: /Night/i }).click();
      await page.getByTestId("settings-save-btn").click();
      await expect(dialog).toBeHidden({ timeout: 5000 });
      await expect(page.locator("html")).toHaveAttribute("data-theme", "night");
      s1.assertions.push("Night theme saved and html[data-theme='night'] active");

      // 3. Ergonomic verifications in Night Mode
      const footerBorderTop = await page
        .getByTestId("sidebar-seamless-footer")
        .evaluate((el) => getComputedStyle(el).borderTopWidth);
      expect(footerBorderTop).toBe("0px");
      s1.assertions.push("Sidebar seamless footer border-top is 0px (no harsh divider)");

      const activeTabStyles = await page
        .getByTestId("canvas-tab-tab-userflow")
        .evaluate((el) => {
          const cs = getComputedStyle(el);
          return { bg: cs.backgroundColor, borderBottomWidth: cs.borderBottomWidth, borderBottomColor: cs.borderBottomColor };
        });
      expect(activeTabStyles.borderBottomWidth).toBe("2px");
      expect(activeTabStyles.borderBottomColor).toBe("rgb(255, 154, 47)");
      expect(activeTabStyles.bg).not.toBe("rgb(0, 87, 255)");
      s1.assertions.push("Active tab uses 2px #FF9A2F bottom line and neutral surface (non-solid blue)");

      const composerBorderTop = await page
        .getByTestId("chat-composer-wrapper")
        .evaluate((el) => getComputedStyle(el).borderTopWidth);
      expect(composerBorderTop).toBe("0px");
      const textareaMaxHeight = await page
        .getByTestId("chat-composer-textarea")
        .evaluate((el) => getComputedStyle(el).maxHeight);
      expect(textareaMaxHeight).toBe("144px");
      s1.assertions.push("Chat composer container has seamless border-top (0px) and max-height 144px");

      const shotNight = path.join(mediaDir, "02-night-theme-activated.png");
      await page.screenshot({ path: shotNight, fullPage: true });
      s1.screenshots.push("02-night-theme-activated.png");

      s1.status = "PASSED";
      scenario1Passed = true;
      console.log("✅ [SCENARIO-01] PASSED");
    } catch (err: any) {
      s1.status = "FAILED";
      s1.error = String(err?.message || err);
      console.error("❌ [SCENARIO-01] FAILED:", s1.error);
      const shotErr = path.join(mediaDir, "err-scenario-01.png");
      await page.screenshot({ path: shotErr, fullPage: true }).catch(() => {});
      s1.screenshots.push("err-scenario-01.png");
    } finally {
      s1.durationMs = Date.now() - s1Start;
      results.push(s1);
    }

    // =========================================================================
    // KỊCH BẢN 2: Splitter Resizing & 40px Shorten Rail (AC-01, AC-02, AC-08)
    // =========================================================================
    const s2Start = Date.now();
    const s2: ScenarioResult = {
      id: "SCENARIO-02",
      name: "Splitter Resizing & 40px Shorten Rail (AC-01, AC-02, AC-08)",
      status: "PENDING",
      durationMs: 0,
      screenshots: [],
      assertions: [],
    };

    if (!scenario1Passed) {
      s2.pendingReason = "Blocked by Scenario 01 failure";
      console.log("⏸ [SCENARIO-02] PENDING (Prerequisite Scenario 01 failed)");
      results.push(s2);
    } else {
      try {
        console.log("\n▶ [SCENARIO-02] Testing Splitter & 40px Shorten Rail...");
        const shortenBtn = page.locator(".btn-shorten-chat");
        await expect(shortenBtn).toBeVisible({ timeout: 5000 });
        await shortenBtn.click();

        const collapsedRail = page.getByTestId("chat-collapsed-rail");
        await expect(collapsedRail).toBeVisible({ timeout: 5000 });
        const railWidth = await collapsedRail.evaluate((el) => getComputedStyle(el).width);
        expect(railWidth).toBe("40px");
        s2.assertions.push("Chat panel successfully collapsed to 40px rail (PD-02 = A)");

        const shotRail = path.join(mediaDir, "03-chat-collapsed-40px-rail.png");
        await page.screenshot({ path: shotRail, fullPage: true });
        s2.screenshots.push("03-chat-collapsed-40px-rail.png");

        // Restore chat panel
        await collapsedRail.locator(".btn-shorten-chat").click();
        await expect(page.locator(".workbench-chat-panel")).toBeVisible({ timeout: 5000 });
        s2.assertions.push("Chat panel restored to previous width upon expand button click");

        const shotRestored = path.join(mediaDir, "04-chat-re-expanded.png");
        await page.screenshot({ path: shotRestored, fullPage: true });
        s2.screenshots.push("04-chat-re-expanded.png");

        s2.status = "PASSED";
        scenario2Passed = true;
        console.log("✅ [SCENARIO-02] PASSED");
      } catch (err: any) {
        s2.status = "FAILED";
        s2.error = String(err?.message || err);
        console.error("❌ [SCENARIO-02] FAILED:", s2.error);
        const shotErr = path.join(mediaDir, "err-scenario-02.png");
        await page.screenshot({ path: shotErr, fullPage: true }).catch(() => {});
        s2.screenshots.push("err-scenario-02.png");
      } finally {
        s2.durationMs = Date.now() - s2Start;
        results.push(s2);
      }
    }

    // =========================================================================
    // KỊCH BẢN 3: Companion Multi-Format Tabs & Preview HTML CSP-B (AC-03, AC-04)
    // =========================================================================
    const s3Start = Date.now();
    const s3: ScenarioResult = {
      id: "SCENARIO-03",
      name: "Companion Multi-Format Tabs & Preview HTML CSP-B (AC-03, AC-04)",
      status: "PENDING",
      durationMs: 0,
      screenshots: [],
      assertions: [],
    };

    if (!scenario1Passed) {
      s3.pendingReason = "Blocked by Scenario 01 failure";
      console.log("⏸ [SCENARIO-03] PENDING (Prerequisite Scenario 01 failed)");
      results.push(s3);
    } else {
      try {
        console.log("\n▶ [SCENARIO-03] Testing Companion Preview & CSP-B Sandbox...");
        const iframe = page.getByTestId("sandboxed-html-iframe");
        await expect(iframe).toBeVisible({ timeout: 10000 });
        await expect(iframe).toHaveAttribute("sandbox", "allow-scripts allow-forms");
        await expect(iframe).toHaveAttribute("referrerpolicy", "no-referrer");

        const sandboxAttr = await iframe.getAttribute("sandbox");
        expect(sandboxAttr).not.toContain("allow-same-origin");
        s3.assertions.push("Iframe strictly enforces sandbox='allow-scripts allow-forms' with zero allow-same-origin");
        s3.assertions.push("Iframe enforces referrerpolicy='no-referrer'");

        const shotPreview = path.join(mediaDir, "05-sandboxed-preview-iframe.png");
        await page.screenshot({ path: shotPreview, fullPage: true });
        s3.screenshots.push("05-sandboxed-preview-iframe.png");

        const cdnToggle = page.getByTestId("toggle-design-cdn-btn");
        await expect(cdnToggle).toBeVisible({ timeout: 5000 });
        await expect(cdnToggle).toHaveAttribute("aria-pressed", "false");
        await cdnToggle.click();
        await expect(cdnToggle).toHaveAttribute("aria-pressed", "true");
        s3.assertions.push("Toggle Design CDN button toggled from aria-pressed false to true");

        const shotCdn = path.join(mediaDir, "06-design-cdn-toggled.png");
        await page.screenshot({ path: shotCdn, fullPage: true });
        s3.screenshots.push("06-design-cdn-toggled.png");

        // Verify Present button is compact 28x28 icon button with tooltip & BrandIcon SVG
        const presentBtn = page.getByTestId("toggle-present-btn");
        await expect(presentBtn).toBeVisible({ timeout: 5000 });
        const presentBox = await presentBtn.boundingBox();
        expect(presentBox).not.toBeNull();
        expect(presentBox!.width).toBeLessThanOrEqual(34);
        expect(presentBox!.height).toBeLessThanOrEqual(34);
        const presentTitle = await presentBtn.getAttribute("title");
        expect(presentTitle && presentTitle.trim().length > 0).toBe(true);
        await expect(presentBtn.locator("svg.brand-icon")).toHaveCount(1);
        s3.assertions.push(
          `Present button is a compact ${Math.round(presentBox!.width)}x${Math.round(presentBox!.height)}px icon button with tooltip='${presentTitle}' and SVG BrandIcon`
        );

        s3.status = "PASSED";
        scenario3Passed = true;
        console.log("✅ [SCENARIO-03] PASSED");
      } catch (err: any) {
        s3.status = "FAILED";
        s3.error = String(err?.message || err);
        console.error("❌ [SCENARIO-03] FAILED:", s3.error);
        const shotErr = path.join(mediaDir, "err-scenario-03.png");
        await page.screenshot({ path: shotErr, fullPage: true }).catch(() => {});
        s3.screenshots.push("err-scenario-03.png");
      } finally {
        s3.durationMs = Date.now() - s3Start;
        results.push(s3);
      }
    }

    // =========================================================================
    // KỊCH BẢN 4: True 2D SVG Bezier DAG Studio & Thêm Node Bản nháp (AC-02, AC-04, AC-06)
    // =========================================================================
    const s4Start = Date.now();
    const s4: ScenarioResult = {
      id: "SCENARIO-04",
      name: "True 2D SVG Bezier DAG Studio & Thêm Node Bản nháp (AC-02, AC-04, AC-06)",
      status: "PENDING",
      durationMs: 0,
      screenshots: [],
      assertions: [],
    };

    if (!scenario1Passed) {
      s4.pendingReason = "Blocked by Scenario 01 failure";
      console.log("⏸ [SCENARIO-04] PENDING (Prerequisite Scenario 01 failed)");
      results.push(s4);
    } else {
      try {
        console.log("\n▶ [SCENARIO-04] Testing True 2D DAG Studio & Add Draft Node...");
        await expect(page.locator('.workbench-shell[data-flow-hydrated="true"]')).toBeVisible({ timeout: 20000 });
        await expect(page.locator("#dagDraftBar")).toBeHidden({ timeout: 15000 });
        s4.assertions.push("DAG canvas hydrated with data-flow-hydrated='true' and initial draft bar hidden");

        nodesBefore = await page.locator(".dag-node-card").count();
        s4.assertions.push(`Counted ${nodesBefore} initial node(s) on 2D DAG canvas`);

        const shotDagInit = path.join(mediaDir, "07-dag-studio-initial.png");
        await page.screenshot({ path: shotDagInit, fullPage: true });
        s4.screenshots.push("07-dag-studio-initial.png");

        const addNodeBtn = page.getByRole("button", { name: "Thêm Node mới" });
        await expect(addNodeBtn).toBeVisible({ timeout: 5000 });
        await addNodeBtn.click();

        await expect(page.locator(".dag-node-card")).toHaveCount(nodesBefore + 1, { timeout: 10000 });
        s4.assertions.push(`Node count increased from ${nodesBefore} to ${nodesBefore + 1}`);

        const draftBar = page.locator("#dagDraftBar");
        await expect(draftBar).toBeVisible({ timeout: 5000 });
        const draftBorderLeft = await draftBar.evaluate((el) => getComputedStyle(el).borderLeftWidth);
        expect(draftBorderLeft).toBe("3px");
        s4.assertions.push("Draft banner appeared with 3px border-left accent");

        // Floating chat MUST remain hidden while left sidebar chat is open
        await expect(page.getByTestId("fc-compact-pill")).toBeHidden();
        await expect(page.getByTestId("contextual-node-floating-chat")).toBeHidden();
        s4.assertions.push("Contextual floating chat stays hidden while left sidebar chat is open");

        // Verify sidebar sub-session nodes render SVG BrandIcon (no 🎯 emoji)
        const sidebarNodeIcons = await page.locator(".sidebar-sub-sessions-tree .brand-icon svg").count();
        expect(sidebarNodeIcons).toBeGreaterThanOrEqual(1);
        const subTreeText = (await page.locator(".sidebar-sub-sessions-tree").textContent()) || "";
        expect(subTreeText).not.toContain("🎯");
        s4.assertions.push(`Sidebar node sub-session list renders ${sidebarNodeIcons} SVG BrandIcon(s) with zero emoji`);

        const shotNodeAdded = path.join(mediaDir, "08-node-added-draft-banner.png");
        await page.screenshot({ path: shotNodeAdded, fullPage: true });
        s4.screenshots.push("08-node-added-draft-banner.png");

        s4.status = "PASSED";
        scenario4Passed = true;
        console.log("✅ [SCENARIO-04] PASSED");
      } catch (err: any) {
        s4.status = "FAILED";
        s4.error = String(err?.message || err);
        console.error("❌ [SCENARIO-04] FAILED:", s4.error);
        const shotErr = path.join(mediaDir, "err-scenario-04.png");
        await page.screenshot({ path: shotErr, fullPage: true }).catch(() => {});
        s4.screenshots.push("err-scenario-04.png");
      } finally {
        s4.durationMs = Date.now() - s4Start;
        results.push(s4);
      }
    }

    // =========================================================================
    // KỊCH BẢN 5: Node Inspector, Sub-State & Cảnh báo Node Mồ côi (AC-03, AC-05)
    // =========================================================================
    const s5Start = Date.now();
    const s5: ScenarioResult = {
      id: "SCENARIO-05",
      name: "Node Inspector, Sub-State & Cảnh báo Node Mồ côi (AC-03, AC-05)",
      status: "PENDING",
      durationMs: 0,
      screenshots: [],
      assertions: [],
    };

    if (!scenario4Passed) {
      s5.pendingReason = "Blocked by Scenario 04 failure (no draft node created)";
      console.log("⏸ [SCENARIO-05] PENDING (Prerequisite Scenario 04 failed)");
      results.push(s5);
    } else {
      try {
        console.log("\n▶ [SCENARIO-05] Testing Node Inspector, Sub-State & Orphan Guard...");
        const orphanWarning = page.getByTestId("dag-draft-orphan-warning");
        await expect(orphanWarning).toBeVisible({ timeout: 5000 });
        s5.assertions.push("Orphan node warning badge is visible on draft banner");

        // Select the newly added node (last node in list)
        const lastNode = page.locator(".dag-node-card").last();
        await lastNode.click();
        const nodeBorder = await lastNode.evaluate((el) => getComputedStyle(el).border);
        s5.assertions.push(`Node card selected (border evaluated: ${nodeBorder})`);

        // Verify substate switcher pills
        const pills = lastNode.locator(".dag-substate-pill");
        const pillCount = await pills.count();
        expect(pillCount).toBeGreaterThanOrEqual(1);
        s5.assertions.push(`Node displays ${pillCount} sub-state pill options`);

        // Collapse left sidebar chat -> compact floating pill appears (NOT pre-expanded)
        await page.locator(".btn-shorten-chat").click();
        await expect(page.getByTestId("chat-collapsed-rail")).toBeVisible({ timeout: 5000 });
        const compactPill = page.getByTestId("fc-compact-pill");
        await expect(compactPill).toBeVisible({ timeout: 5000 });
        await expect(page.getByTestId("contextual-node-floating-chat")).toBeHidden();
        s5.assertions.push("Collapsing left sidebar chat reveals compact floating pill (chat window collapsed by default)");

        const shotInspector = path.join(mediaDir, "09-node-inspector-active.png");
        await page.screenshot({ path: shotInspector, fullPage: true });
        s5.screenshots.push("09-node-inspector-active.png");

        // Click compact pill to open floating chat with unified sidebar composer & progressive height
        await page.getByTestId("floating-chat-open-trigger").click();
        const floatingWin = page.getByTestId("contextual-node-floating-chat");
        await expect(floatingWin).toBeVisible({ timeout: 5000 });
        await expect(page.getByTestId("floating-chat-composer-wrapper")).toBeVisible({ timeout: 5000 });
        const initialBox = await floatingWin.boundingBox();
        expect(initialBox).not.toBeNull();
        expect(initialBox!.height).toBeLessThan(340);

        // Send a message so floating chat grows progressively instead of opening at max height
        const composerInput = page.getByTestId("floating-chat-composer-textarea");
        await composerInput.fill("Kiểm tra node mới và đề xuất điều kiện chuyển bước hợp lệ");
        await composerInput.press("Enter");
        await page.waitForTimeout(250);
        const grownBox = await floatingWin.boundingBox();
        expect(grownBox).not.toBeNull();
        expect(grownBox!.height).toBeGreaterThan(initialBox!.height);
        const humanBubble = floatingWin.getByTestId("chat-human-bubble").first();
        await expect(humanBubble).toBeVisible({ timeout: 3000 });
        const bubbleStyle = await humanBubble.evaluate((el) => {
          const cs = getComputedStyle(el);
          return { bg: cs.backgroundColor, border: cs.borderTopColor };
        });
        expect(bubbleStyle.bg).not.toBe("rgb(255, 154, 47)");
        expect(bubbleStyle.bg).not.toBe("rgb(245, 158, 11)");
        s5.assertions.push(
          `Floating chat opened via pill at compact ${Math.round(initialBox!.height)}px height, grew progressively to ${Math.round(grownBox!.height)}px, and user message bubble uses minimalist non-orange surface (${bubbleStyle.bg})`
        );

        const shotOrphan = path.join(mediaDir, "10-orphan-node-warning.png");
        await page.screenshot({ path: shotOrphan, fullPage: true });
        s5.screenshots.push("10-orphan-node-warning.png");

        // Re-open left sidebar chat -> floating chat hides again cleanly
        await page.getByTestId("chat-collapsed-rail").locator(".btn-shorten-chat").click();
        await expect(page.locator(".workbench-chat-panel")).toBeVisible({ timeout: 5000 });
        await expect(floatingWin).toBeHidden();
        s5.assertions.push("Re-expanding left sidebar chat automatically hides contextual floating chat");

        s5.status = "PASSED";
        scenario5Passed = true;
        console.log("✅ [SCENARIO-05] PASSED");
      } catch (err: any) {
        s5.status = "FAILED";
        s5.error = String(err?.message || err);
        console.error("❌ [SCENARIO-05] FAILED:", s5.error);
        const shotErr = path.join(mediaDir, "err-scenario-05.png");
        await page.screenshot({ path: shotErr, fullPage: true }).catch(() => {});
        s5.screenshots.push("err-scenario-05.png");
      } finally {
        s5.durationMs = Date.now() - s5Start;
        results.push(s5);
      }
    }

    // =========================================================================
    // KỊCH BẢN 6: Deploy Draft & Lưu trữ SQLite WAL Bền vững (AC-02, AC-06)
    // =========================================================================
    const s6Start = Date.now();
    const s6: ScenarioResult = {
      id: "SCENARIO-06",
      name: "Deploy Draft & Lưu trữ SQLite WAL Bền vững (AC-02, AC-06)",
      status: "PENDING",
      durationMs: 0,
      screenshots: [],
      assertions: [],
    };

    if (!scenario4Passed) {
      s6.pendingReason = "Blocked by Scenario 04 failure (no draft node to deploy)";
      console.log("⏸ [SCENARIO-06] PENDING (Prerequisite Scenario 04 failed)");
      results.push(s6);
    } else {
      try {
        console.log("\n▶ [SCENARIO-06] Testing Deploy Draft & SQLite WAL Persistence...");
        const deployBtn = page.getByTestId("deploy-draft-btn");
        await expect(deployBtn).toBeVisible({ timeout: 5000 });
        await deployBtn.click();

        await expect(page.getByTestId("orphan-deploy-confirm-bar")).toBeVisible({ timeout: 5000 });
        await page.getByTestId("confirm-orphan-deploy-btn").click();

        await expect(deployBtn).toBeHidden({ timeout: 20000 });
        await expect(page.locator("#dagDraftBar .dag-draft-badge")).toBeHidden({ timeout: 15000 });
        s6.assertions.push("Deploy Draft button and draft badge hidden after successful commit");

        // Non-blocking orphan warning remains visible
        await expect(page.getByTestId("dag-draft-orphan-warning")).toBeVisible();
        s6.assertions.push("Non-blocking orphan warning remains visible for unlinked node");

        const shotDeployed = path.join(mediaDir, "11-draft-deployed-committed.png");
        await page.screenshot({ path: shotDeployed, fullPage: true });
        s6.screenshots.push("11-draft-deployed-committed.png");

        // Reload page to verify persistence
        console.log("[SCENARIO-06] Reloading page to verify Rust SQLite WAL persistence...");
        await page.reload({ waitUntil: "networkidle" });
        await expect(page.locator('.workbench-shell[data-flow-hydrated="true"]')).toBeVisible({ timeout: 20000 });
        await expect(page.locator(".dag-node-card")).toHaveCount(nodesBefore + 1, { timeout: 10000 });
        s6.assertions.push(`Full page reload verified: ${nodesBefore + 1} node(s) persisted in SQLite WAL`);

        const shotReload = path.join(mediaDir, "12-sqlite-wal-persisted-after-reload.png");
        await page.screenshot({ path: shotReload, fullPage: true });
        s6.screenshots.push("12-sqlite-wal-persisted-after-reload.png");

        s6.status = "PASSED";
        scenario6Passed = true;
        console.log("✅ [SCENARIO-06] PASSED");
      } catch (err: any) {
        s6.status = "FAILED";
        s6.error = String(err?.message || err);
        console.error("❌ [SCENARIO-06] FAILED:", s6.error);
        const shotErr = path.join(mediaDir, "err-scenario-06.png");
        await page.screenshot({ path: shotErr, fullPage: true }).catch(() => {});
        s6.screenshots.push("err-scenario-06.png");
      } finally {
        s6.durationMs = Date.now() - s6Start;
        results.push(s6);
      }
    }

    // =========================================================================
    // FINAL REPORT & ARTIFACT PACKAGING
    // =========================================================================
    const totalDurationMs = Date.now() - startTime;
    const allPassed = results.every((r) => r.status === "PASSED");
    const summaryData = {
      story_id: "04.1",
      executed_at: new Date().toISOString(),
      total_duration_ms: totalDurationMs,
      all_passed: allPassed,
      scenarios: results,
    };

    const jsonReportPath = path.join(mediaDir, "acceptance-report.json");
    fs.writeFileSync(jsonReportPath, JSON.stringify(summaryData, null, 2), "utf-8");
    console.log(`\n📄 Saved JSON report to: ${jsonReportPath}`);

    // Wait slightly to let video finish recording before context closes
    await page.waitForTimeout(1000);
    await page.close();

    const rawVideo = page.video();
    if (rawVideo) {
      try {
        const videoSrcPath = await rawVideo.path();
        const webmDest = path.join(mediaDir, "acceptance-video-story-04.1.webm");
        fs.copyFileSync(videoSrcPath, webmDest);
        console.log(`🎥 Saved raw webm video to: ${webmDest}`);

        const mp4Dest = path.join(mediaDir, "acceptance-video-story-04.1.mp4");
        try {
          execSync(`/opt/homebrew/bin/ffmpeg -y -i "${webmDest}" -c:v libx264 -pix_fmt yuv420p "${mp4Dest}"`, {
            stdio: "ignore",
          });
          console.log(`🎬 Converted MP4 video to: ${mp4Dest}`);
        } catch (convErr: any) {
          console.warn(`[WARN] ffmpeg conversion skipped/failed: ${convErr?.message}`);
        }
      } catch (vidErr: any) {
        console.warn(`[WARN] Could not retrieve video path: ${vidErr?.message}`);
      }
    }

    // Final assertion: fail test if any scenario failed so runner registers exit code
    if (!allPassed) {
      const failed = results.filter((r) => r.status === "FAILED");
      const pending = results.filter((r) => r.status === "PENDING");
      throw new Error(
        `Acceptance suite encountered ${failed.length} failure(s) and ${pending.length} pending scenario(s).\n` +
          failed.map((f) => ` - [${f.id}] ${f.error}`).join("\n")
      );
    }
  });
});
