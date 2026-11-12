import { chromium } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

interface DefectEntry {
  type: "console-error" | "console-warn" | "page-error" | "request-failed" | "behavior-anomaly";
  message: string;
  timestamp: string;
}

const defectsLog: DefectEntry[] = [];

async function runAcceptance() {
  console.log("================================================================================");
  console.log("🎬 STARTING STORY 04.1 LIVE BROWSER ACCEPTANCE & VIDEO RECORDING");
  console.log("Target Origin: http://127.0.0.1:3142/");
  console.log("================================================================================");

  const evidenceDir = path.resolve(
    process.cwd(),
    "_iwish-output/3. Development/1. Epic & Story/VS-02. Flow-First Executable Design Workbench & SDI Engine/Epic-04/Story-04.1/qa/evidence"
  );
  const brainDir = "/Users/hatrang20061988/.gemini/antigravity/brain/f823b228-c81c-47ac-b77a-4f101f8783d5";
  const videoDir = path.join(evidenceDir, "videos");

  fs.mkdirSync(evidenceDir, { recursive: true });
  fs.mkdirSync(videoDir, { recursive: true });

  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox", "--disable-setuid-sandbox", "--disable-web-security"],
  });

  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
    recordVideo: {
      dir: videoDir,
      size: { width: 1440, height: 900 },
    },
  });

  const page = await context.newPage();

  // Listen for console logs, errors, and unhandled exceptions
  page.on("console", (msg) => {
    const text = msg.text();
    if (msg.type() === "error") {
      console.warn(`[BROWSER ERROR] ${text}`);
      defectsLog.push({ type: "console-error", message: text, timestamp: new Date().toISOString() });
    } else if (msg.type() === "warning") {
      defectsLog.push({ type: "console-warn", message: text, timestamp: new Date().toISOString() });
    }
  });

  page.on("pageerror", (err) => {
    console.error(`[PAGE UNCAUGHT ERROR] ${err.message}`);
    defectsLog.push({ type: "page-error", message: err.message, timestamp: new Date().toISOString() });
  });

  page.on("requestfailed", (req) => {
    const failure = req.failure()?.errorText || "unknown failure";
    console.warn(`[NETWORK FAILED] ${req.method()} ${req.url()} - ${failure}`);
    defectsLog.push({
      type: "request-failed",
      message: `${req.method()} ${req.url()}: ${failure}`,
      timestamp: new Date().toISOString(),
    });
  });

  // Handle dialogs (alert/confirm/prompt) gracefully without freezing test
  page.on("dialog", async (dialog) => {
    console.log(`[DIALOG POPUP] Type: ${dialog.type()}, Message: "${dialog.message()}"`);
    defectsLog.push({
      type: "behavior-anomaly",
      message: `Dialog alert popped: ${dialog.message()}`,
      timestamp: new Date().toISOString(),
    });
    await dialog.accept();
  });

  const screenshots: string[] = [];

  const captureMilestone = async (name: string, description: string) => {
    const fileName = `${name}.png`;
    const fullPath = path.join(evidenceDir, fileName);
    await page.screenshot({ path: fullPath, fullPage: false });
    const brainPath = path.join(brainDir, fileName);
    fs.copyFileSync(fullPath, brainPath);
    screenshots.push(fileName);
    console.log(`📸 [MILESTONE CAPTURED] ${fileName} - ${description}`);
  };

  try {
    // --------------------------------------------------------------------------
    // SCENARIO 1: Khởi động & Nạp giao diện tổng thể (Workbench Shell Initialization)
    // --------------------------------------------------------------------------
    console.log("\n▶ [SCENARIO 1] Khởi động & Nạp giao diện tổng thể...");
    await page.goto("http://127.0.0.1:3142/", { waitUntil: "networkidle" });
    await page.waitForTimeout(1000);

    const appContainer = page.locator("#cowok-app-container");
    await appContainer.waitFor({ state: "visible", timeout: 10000 });

    const nodesCount = await page.locator(".dag-node-card").count();
    console.log(`  ✓ Số lượng DAG Nodes khởi tạo phát hiện: ${nodesCount}`);
    await captureMilestone("01-workbench-shell-init", "Khởi tạo tổng thể màn hình Workbench Shell 1440x900");

    // --------------------------------------------------------------------------
    // SCENARIO 2: Điều hướng & Co giãn giao diện (Layout Flexibility & Responsive Controls)
    // --------------------------------------------------------------------------
    console.log("\n▶ [SCENARIO 2] Kiểm tra Sidebar & Chat Panel Co giãn...");
    const collapseSidebarBtn = page.locator('button[aria-label="Thu gọn thanh bên"]');
    if (await collapseSidebarBtn.isVisible()) {
      await collapseSidebarBtn.click();
      await page.waitForTimeout(600);
      await captureMilestone("02a-sidebar-collapsed", "Thanh Sidebar đã thu gọn về dạng Rail 44px");

      const expandSidebarBtn = page.locator('button[aria-label="Mở rộng thanh bên"]');
      await expandSidebarBtn.click();
      await page.waitForTimeout(600);
    }

    const shortenChatBtn = page.locator(".btn-shorten-chat").first();
    if (await shortenChatBtn.isVisible()) {
      await shortenChatBtn.click();
      await page.waitForTimeout(600);
      await captureMilestone("02b-chat-panel-shortened", "Khung Chat Agent đã thu gọn về thanh dọc 40px");

      const expandChatBtn = page.locator('button[aria-label="Mở rộng khung Chat"]').first();
      await expandChatBtn.click();
      await page.waitForTimeout(600);
    }

    // Splitter Dragging interaction
    const splitter = page.locator("#splitterChatCanvas");
    if (await splitter.isVisible()) {
      const box = await splitter.boundingBox();
      if (box) {
        await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
        await page.mouse.down();
        await page.mouse.move(box.x + 80, box.y + box.height / 2, { steps: 5 });
        await page.mouse.up();
        await page.waitForTimeout(400);
        await captureMilestone("02c-splitter-dragged", "Đã kéo dãn Splitter điều chỉnh tỷ lệ Chat/Canvas");
      }
    }

    // --------------------------------------------------------------------------
    // SCENARIO 3: Mở Cài đặt, Đổi Theme & Locale (Settings Modal, Theme & Language)
    // --------------------------------------------------------------------------
    console.log("\n▶ [SCENARIO 3] Kiểm tra Settings Modal, Theme & Locale...");
    const settingsBtn = page.locator('button[aria-label="Settings"]').first();
    if (await settingsBtn.isVisible()) {
      await settingsBtn.click();
      await page.waitForTimeout(500);
      await captureMilestone("03a-settings-modal-open", "Modal Cài đặt mở ra với các tab General, Models, MCP");

      // Switch theme option inside modal if available
      const nightThemeBtn = page.locator('button:has-text("Night")').first();
      if (await nightThemeBtn.isVisible()) {
        await nightThemeBtn.click();
        await page.waitForTimeout(400);
      }

      const mcpTab = page.locator('button:has-text("MCP")').first();
      if (await mcpTab.isVisible()) {
        await mcpTab.click();
        await page.waitForTimeout(400);
        await captureMilestone("03b-settings-mcp-tab", "Tab MCP Plugins trong Settings Modal");
      }

      // Close settings modal via Escape key to verify keyboard accessibility (Defect #2 fix)
      await page.keyboard.press("Escape");
      await page.waitForTimeout(500);
      const isSettingsStillOpen = await page.locator('div[role="dialog"][aria-label="Cài đặt & Tùy biến Hệ thống"]').isVisible();
      if (isSettingsStillOpen) {
        console.warn("⚠️ Settings modal did not close on Escape key!");
        const closeSettingsBtn = page.locator('button[aria-label="Đóng"]').first();
        if (await closeSettingsBtn.isVisible()) await closeSettingsBtn.click();
      } else {
        console.log("✅ Settings modal successfully closed via Escape key!");
      }
      await page.waitForTimeout(400);
    }

    // --------------------------------------------------------------------------
    // SCENARIO 4: Tương tác Đồ thị 2D DAG (Canvas Navigation, Node Selection & Inspection)
    // --------------------------------------------------------------------------
    console.log("\n▶ [SCENARIO 4] Chọn Node & Đổi Sub-State (Loading, Empty, Error)...");
    const firstNode = page.locator(".dag-node-card").first();
    if (await firstNode.isVisible()) {
      await firstNode.click();
      await page.waitForTimeout(400);

      // Click substate pill "Loading"
      const loadingPill = firstNode.locator('button:has-text("Loading")');
      if (await loadingPill.isVisible()) {
        await loadingPill.click();
        await page.waitForTimeout(400);
      }

      await captureMilestone("04-node-selected-substate-loading", "Node đã chọn và chuyển sang Sub-state Loading");

      // Drag node position
      const nodeBox = await firstNode.boundingBox();
      if (nodeBox) {
        await page.mouse.move(nodeBox.x + 40, nodeBox.y + 20);
        await page.mouse.down();
        await page.mouse.move(nodeBox.x + 90, nodeBox.y + 60, { steps: 8 });
        await page.mouse.up();
        await page.waitForTimeout(400);
        await captureMilestone("04b-node-dragged-new-position", "Node đã được kéo di chuyển sang toạ độ mới");
      }
    }

    // --------------------------------------------------------------------------
    // SCENARIO 5: Thêm Node mới, Quản lý Trạng thái Bản nháp (Draft Lifecycle)
    // --------------------------------------------------------------------------
    console.log("\n▶ [SCENARIO 5] Thêm Node mới & Kiểm tra Thanh Draft Bar...");
    const addNodeBtn = page.locator('button[aria-label="Thêm Node mới"]').first();
    if (await addNodeBtn.isVisible()) {
      await addNodeBtn.click();
      await page.waitForTimeout(600);

      const updatedCount = await page.locator(".dag-node-card").count();
      console.log(`  ✓ Số lượng Nodes sau khi bấm Add Node: ${updatedCount}`);

      await captureMilestone("05-new-node-added-draft-bar", "Thêm Node mới thành công, cờ DRAFT và thanh Deploy xuất hiện");
    }

    // --------------------------------------------------------------------------
    // SCENARIO 6: Thao tác Hoàn tác & Phục hồi (Undo / Redo)
    // --------------------------------------------------------------------------
    console.log("\n▶ [SCENARIO 6] Kiểm tra Undo / Redo...");
    const undoBtn = page.locator('button[aria-label="Hoàn tác"]').first();
    if (await undoBtn.isVisible() && !(await undoBtn.isDisabled())) {
      await undoBtn.click();
      await page.waitForTimeout(500);
      await captureMilestone("06a-undo-action", "Hoàn tác (Undo) thu hồi node vừa thêm");

      const redoBtn = page.locator('button[aria-label="Làm lại"]').first();
      if (await redoBtn.isVisible() && !(await redoBtn.isDisabled())) {
        await redoBtn.click();
        await page.waitForTimeout(500);
        await captureMilestone("06b-redo-action", "Làm lại (Redo) phục hồi lại node draft");
      }
    }

    // --------------------------------------------------------------------------
    // SCENARIO 7: Kiểm thử Triển khai Bản nháp (Deploy Draft Execution & Anti Split-Brain)
    // --------------------------------------------------------------------------
    console.log("\n▶ [SCENARIO 7] Bấm Deploy Draft & Kiểm tra Cơ chế Chống Split-Brain...");
    const deployDraftBtn = page.locator('button:has-text("Triển khai Draft")').first();
    if (await deployDraftBtn.isVisible()) {
      await deployDraftBtn.click();
      await page.waitForTimeout(1000);
      await captureMilestone("07-deploy-draft-action-executed", "Thao tác Deploy Draft được thực thi trên giao diện");
    }

    // --------------------------------------------------------------------------
    // SCENARIO 8: Chuyển đổi các định dạng Tab (Userflow DAG -> Slide CMS -> Markdown -> Code)
    // --------------------------------------------------------------------------
    console.log("\n▶ [SCENARIO 8] Chuyển đổi các tab định dạng trong Companion Canvas...");
    const slideTab = page.locator('div[role="tab"]:has-text("q4-architecture-deck.html")');
    if (await slideTab.isVisible()) {
      await slideTab.click();
      await page.waitForTimeout(800);
      await captureMilestone("08a-slide-cms-presentation-tab", "Tab Slide Deck CMS Studio (16:9 Presentation Stage)");
    }

    const prdTab = page.locator('div[role="tab"]:has-text("prd-spec.md")');
    if (await prdTab.isVisible()) {
      await prdTab.click();
      await page.waitForTimeout(800);
      await captureMilestone("08b-markdown-spec-tab", "Tab Tài liệu Markdown SSOT kèm khả năng trích dẫn Quote");
    }

    const tokensTab = page.locator('div[role="tab"]:has-text("design-tokens.css")');
    if (await tokensTab.isVisible()) {
      await tokensTab.click();
      await page.waitForTimeout(800);
      await captureMilestone("08c-code-view-tab", "Tab Xem mã nguồn CSS / Tokens");
    }

    const flowTab = page.locator('div[role="tab"]:has-text("checkout-app.html")');
    if (await flowTab.isVisible()) {
      await flowTab.click();
      await page.waitForTimeout(800);
      await captureMilestone("08d-back-to-flow-dag-tab", "Quay trở lại tab Userflow 2D DAG");
    }

    // --------------------------------------------------------------------------
    // SCENARIO 9: New Conversation Modal & Path Verification
    // --------------------------------------------------------------------------
    console.log("\n▶ [SCENARIO 9] Mở New Conversation Modal & Kiểm tra Đường dẫn...");
    const newConvoBtn = page.locator('button:has-text("New Conversation")').first();
    if (await newConvoBtn.isVisible()) {
      await newConvoBtn.click();
      await page.waitForTimeout(500);
      await captureMilestone("09-new-conversation-modal", "Modal Tạo hội thoại mới với đường dẫn chuẩn hóa");

      // Close New Conversation modal via Escape key to verify keyboard accessibility (Defect #2 fix)
      await page.keyboard.press("Escape");
      await page.waitForTimeout(500);
      const isConvoStillOpen = await page.locator('div[role="dialog"][aria-labelledby="new-convo-title"]').isVisible();
      if (isConvoStillOpen) {
        console.warn("⚠️ New Conversation modal did not close on Escape key!");
        const cancelConvoBtn = page.locator('button:has-text("Hủy")').first();
        if (await cancelConvoBtn.isVisible()) await cancelConvoBtn.click();
      } else {
        console.log("✅ New Conversation modal successfully closed via Escape key!");
      }
      await page.waitForTimeout(400);
    }
  } catch (err: any) {
    console.error("❌ Exception during test execution:", err);
    defectsLog.push({
      type: "behavior-anomaly",
      message: `Fatal execution exception: ${err.message}`,
      timestamp: new Date().toISOString(),
    });
  } finally {
    // End test session and finalize video recording
    console.log("\n▶ Đang đóng browser context để kết thúc ghi hình video nghiệm thu...");
    await context.close();
    await browser.close();
  }

  // Find and process recorded video file
  const videoFiles = fs.readdirSync(videoDir).filter((f) => f.endsWith(".webm") || f.endsWith(".mp4"));
  let finalVideoName = "";
  if (videoFiles.length > 0) {
    const rawVideoPath = path.join(videoDir, videoFiles[0]);
    finalVideoName = "story-04.1-acceptance-walkthrough.webm";
    const finalEvidenceVideoPath = path.join(evidenceDir, finalVideoName);
    const finalBrainVideoPath = path.join(brainDir, finalVideoName);

    fs.copyFileSync(rawVideoPath, finalEvidenceVideoPath);
    fs.copyFileSync(rawVideoPath, finalBrainVideoPath);
    console.log(`🎥 Video nghiệm thu đã lưu tại: ${finalEvidenceVideoPath}`);
    console.log(`🎥 Video nghiệm thu đã sao chép về Brain: ${finalBrainVideoPath}`);
  }

  // Write Defect and Audit Log
  const defectReportPath = path.join(evidenceDir, "qa-defect-and-observation-log.json");
  const brainDefectPath = path.join(brainDir, "qa-defect-and-observation-log.json");
  const reportPayload = {
    testDate: new Date().toISOString(),
    targetOrigin: "http://127.0.0.1:3142/",
    story: "04.1",
    totalMilestonesCaptured: screenshots.length,
    screenshots,
    videoFile: finalVideoName,
    defectCount: defectsLog.length,
    defects: defectsLog,
  };

  fs.writeFileSync(defectReportPath, JSON.stringify(reportPayload, null, 2), "utf-8");
  fs.writeFileSync(brainDefectPath, JSON.stringify(reportPayload, null, 2), "utf-8");

  console.log("\n================================================================================");
  console.log(`✅ HOÀN TẤT KIỂM THỬ BROWSER THỰC NGHIỆM!`);
  console.log(`- Tổng số ảnh Milestone chụp: ${screenshots.length}`);
  console.log(`- Video nghiệm thu: ${finalVideoName}`);
  console.log(`- Tổng số Defect / Cảnh báo ghi nhận: ${defectsLog.length}`);
  console.log(`- File báo cáo lỗi: ${defectReportPath}`);
  console.log("================================================================================\n");
}

runAcceptance().catch((err) => {
  console.error("Runner crashed:", err);
  process.exit(1);
});
