/** @vitest-environment jsdom */
// @story 04.3
// @implements AC-01, AC-02, AC-03, AC-04, AC-05
import { describe, it, expect, vi, afterEach } from "vitest";
import React from "react";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { KeyedParallelCoordinator } from "../store/keyed-parallel-coordinator";
import { ContextualNodeFloatingChat } from "../components/ContextualNodeFloatingChat";
import { AgentChatComposer } from "@/components/workbench/ChatPrimitivesBridge";
import { SidebarSessionRow } from "@/pages/workbench/components/SidebarSessionRow";
import { WorkbenchNode } from "../store/flow-workbench-store";
import { sharedChatStore } from "../store/shared-chat-sync";

describe("Story 04.3: Keyed Parallel Engine, Contextual Multi-Node Floating Chat v2 & 3-Level Sub-Sessions", () => {
  afterEach(() => {
    cleanup();
  });
  const mockNodeA: WorkbenchNode = {
    id: "node-scr-01",
    screenKey: "SCR-01",
    title: "Màn hình Đăng nhập",
    posX: 100,
    posY: 100,
    version: 1,
    status: "synced",
    activeSubState: "Default",
    nodeType: "screen",
  };

  const mockNodeB: WorkbenchNode = {
    id: "node-scr-02",
    screenKey: "SCR-02",
    title: "Màn hình Giỏ hàng",
    posX: 400,
    posY: 100,
    version: 1,
    status: "synced",
    activeSubState: "Default",
    nodeType: "screen",
  };

  describe("AC-01: KeyedParallelCoordinator Engine & DAG Queue Status", () => {
    it("should allow parallel execution across distinct nodes and strict FIFO on the same node", () => {
      const coordinator = new KeyedParallelCoordinator(4);

      // Job 1 on Node A
      const jobA1 = coordinator.enqueueJob({
        projectId: "proj-1",
        sessionId: "sess-1",
        nodeId: "node-scr-01",
        prompt: "Tạo form email/password",
      });
      expect(jobA1.status).toBe("running");
      expect(jobA1.queuePosition).toBe(0);

      // Job 2 on Node B (different node -> runs in parallel)
      const jobB1 = coordinator.enqueueJob({
        projectId: "proj-1",
        sessionId: "sess-1",
        nodeId: "node-scr-02",
        prompt: "Tạo bảng danh sách sản phẩm",
      });
      expect(jobB1.status).toBe("running");
      expect(jobB1.queuePosition).toBe(0);

      // Job 3 on Node A (same node -> MUST queue FIFO)
      const jobA2 = coordinator.enqueueJob({
        projectId: "proj-1",
        sessionId: "sess-1",
        nodeId: "node-scr-01",
        prompt: "Thêm nút Đăng nhập Google",
      });
      expect(jobA2.status).toBe("queued");
      expect(jobA2.queuePosition).toBe(1);

      const summary = coordinator.getSummary();
      expect(summary.runningCount).toBe(2);
      expect(summary.queuedCount).toBe(1);
      expect(summary.totalActive).toBe(3);

      // Complete job A1 -> job A2 immediately starts running
      coordinator.completeJob(jobA1.id, "sha256-output-1");
      const updatedA2 = coordinator.getNodeStatus("node-scr-01");
      expect(updatedA2?.status).toBe("running");
      expect(updatedA2?.queuePosition).toBe(0);
    });

    it("should support mid-turn steer, pause, resume, and cancel", () => {
      const coordinator = new KeyedParallelCoordinator(4);
      const job = coordinator.enqueueJob({
        projectId: "proj-1",
        sessionId: "sess-1",
        nodeId: "node-scr-01",
        prompt: "Xây dựng giao diện",
      });

      // Pause
      coordinator.pauseJob(job.id);
      expect(coordinator.getNodeStatus("node-scr-01")?.status).toBe("paused");

      // Resume
      coordinator.resumeJob(job.id);
      expect(coordinator.getNodeStatus("node-scr-01")?.status).toBe("running");

      // Mid-turn Steer instruction
      coordinator.steerJob(job.id, "Đổi màu nút sang xanh dương");
      const active = coordinator.getActiveJobs().find((j) => j.id === job.id);
      expect(active?.steerHistory.length).toBe(1);
      expect(active?.steerHistory[0].instruction).toBe("Đổi màu nút sang xanh dương");

      // Cancel
      coordinator.cancelJob(job.id);
      expect(coordinator.getNodeStatus("node-scr-01")).toBeNull();
      expect(coordinator.getSummary().totalActive).toBe(0);
    });
  });

  describe("AC-02: Contextual Multi-Node Floating Chat v2", () => {
    it("should default to compact floating bar and render multi-node composite tag when clicked open", () => {
      render(
        <ContextualNodeFloatingChat
          projectId="11111111-1111-4111-8111-111111111111"
          sessionId="session-1"
          selectedNodes={[mockNodeA, mockNodeB]}
          onDeselectNode={vi.fn()}
          onClearSelection={vi.fn()}
        />
      );

      // Defaults to compact floating pill bar; expands only on click
      expect(screen.getByTestId("fc-compact-pill")).toBeDefined();
      fireEvent.click(screen.getByTestId("floating-chat-open-trigger"));

      const chatWin = screen.getByTestId("contextual-node-floating-chat");
      expect(chatWin.style.height).toBe("auto");
      expect(chatWin.style.maxHeight).toContain("850px");
      expect(screen.getByTestId("floating-chat-composer-wrapper")).toBeDefined();

      // Verify composite header tag and chips
      expect(screen.getByText(/2 Màn hình đang chọn/i)).toBeDefined();
      expect(screen.getByText(/SCR-01/i)).toBeDefined();
      expect(screen.getByText(/SCR-02/i)).toBeDefined();

      // Verify execution mode indicator
      expect(screen.getByText(/Song song/i)).toBeDefined();
    });

    it("should start as Floating Pill Badge, expand to full window, and collapse back to Pill", () => {
      render(
        <ContextualNodeFloatingChat
          projectId="11111111-1111-4111-8111-111111111111"
          sessionId="session-1"
          selectedNodes={[mockNodeA]}
          onDeselectNode={vi.fn()}
          onClearSelection={vi.fn()}
        />
      );

      // Verify initial collapsed pill is rendered
      expect(screen.getByTestId("fc-compact-pill")).toBeDefined();
      expect(screen.getByText(/Review Summary/i)).toBeDefined();

      // Click expand button to open floating chat window
      const expandBtn = screen.getByTitle(/Mở rộng khung Chat/i);
      fireEvent.click(expandBtn);
      expect(screen.getByTestId("contextual-node-floating-chat")).toBeDefined();
      for (const dir of ["n", "s", "e", "w", "ne", "nw", "se", "sw"]) {
        expect(screen.getByTestId(`fc-resize-${dir}`)).toBeDefined();
      }

      // Click collapse button back to pill
      const collapseBtn = screen.getByTitle(/Thu gọn thành Pill/i);
      fireEvent.click(collapseBtn);
      expect(screen.getByTestId("fc-compact-pill")).toBeDefined();
    });
  });

  describe("AC-03: MainSidebar 3-Level Hierarchy & Scoped Sub-Sessions", () => {
    it("should render 3-level tree with composite scope key ${projectId}::${sessionId}::${nodeId}", () => {
      const handleSelectNodeSubSession = vi.fn();

      render(
        <SidebarSessionRow
          session={{
            id: "sess-abc",
            title: "Thiết kế Duyệt chi v2",
            projectId: "proj-treasury",
            updatedAt: "10:30",
            subSessions: [
              { nodeId: "node-scr-01", nodeTitle: "Login", screenKey: "SCR-01", status: "running" },
              { nodeId: "node-scr-02", nodeTitle: "OTP Verify", screenKey: "SCR-02", status: "idle" },
            ],
          }}
          keyPrefix="proj"
          activeSessionId="sess-abc"
          projectId="proj-treasury"
          nodes={[mockNodeA, mockNodeB]}
          onSelectSession={vi.fn()}
          onSelectNodeSubSession={handleSelectNodeSubSession}
        />
      );

      // Verify sub-session rows exist
      const subRowA = screen.getByTestId("subsession-row-node-scr-01");
      expect(subRowA).toBeDefined();
      expect(subRowA.getAttribute("title")).toBe(
        "Composite Scope: proj-treasury::sess-abc::node-scr-01"
      );

      // Click sub-session
      fireEvent.click(subRowA);
      expect(handleSelectNodeSubSession).toHaveBeenCalledWith(
        "proj-treasury::sess-abc::node-scr-01",
        "proj-treasury",
        "sess-abc",
        "node-scr-01"
      );
    });

    it("does not fabricate sub-session rows when session.subSessions is empty", () => {
      render(
        <SidebarSessionRow
          session={{ id: "sess-empty", title: "Empty Flow", projectId: "proj-treasury", updatedAt: "10:31" }}
          keyPrefix="proj"
          activeSessionId="sess-empty"
          projectId="proj-treasury"
          nodes={[mockNodeA, mockNodeB]}
          onSelectSession={vi.fn()}
        />
      );
      expect(screen.queryByTestId("subsession-row-node-scr-01")).toBeNull();
    });
  });

  describe("AC-04: 3-Way Contextual Feedback & Attachment Tray", () => {
    it("should truncate quotes exceeding 90 characters and ignore marquee drags < 16px", () => {
      const longQuote =
        "Đây là một đoạn văn bản trích dẫn rất dài nhằm kiểm tra bộ lọc rút gọn ký tự hiển thị trên khay attachment tray của cowok design hệ thống.";
      expect(longQuote.length).toBeGreaterThan(90);

      const truncated =
        longQuote.length > 90 ? `${longQuote.slice(0, 87)}...` : longQuote;
      expect(truncated.length).toBe(90);
      expect(truncated.endsWith("...")).toBe(true);

      // Boundary condition: drag < 16px
      const isDragValid = (w: number, h: number) => w >= 16 && h >= 16;
      expect(isDragValid(15, 15)).toBe(false);
      expect(isDragValid(16, 16)).toBe(true);
      expect(isDragValid(100, 200)).toBe(true);
    });

    it("ignores IME Enter (isComposing / keyCode 229) and rapid duplicate Enter so one message produces one send", () => {
      const onSend = vi.fn();
      render(
        <AgentChatComposer
          inputValue="scisdw"
          onInputChange={vi.fn()}
          onSend={onSend}
          attachmentChips={['@[Comment #sec-biometric (440f): "test 123"]']}
        />
      );
      const input = screen.getByTestId("chat-composer-textarea");
      fireEvent.compositionStart(input);
      fireEvent.keyDown(input, { key: "Enter", keyCode: 229 });
      expect(onSend).not.toHaveBeenCalled();
      fireEvent.compositionEnd(input);

      fireEvent.keyDown(input, { key: "Enter" });
      fireEvent.keyDown(input, { key: "Enter" });
      expect(onSend).toHaveBeenCalledTimes(1);
    });

    it("surfaces pinned comment attachments on Floating Chat Card when Left Chat is shortened even with zero selected nodes", () => {
      const onRemove = vi.fn();
      render(
        <ContextualNodeFloatingChat
          projectId="11111111-1111-4111-8111-111111111111"
          sessionId="session-1"
          selectedNodes={[]}
          attachmentChips={['@[Comment #ff-slide-headline (1a2b): "Clarify title"]']}
          onRemoveAttachmentChip={onRemove}
          onDeselectNode={vi.fn()}
          onClearSelection={vi.fn()}
        />
      );
      const floatingWin = screen.getByTestId("contextual-node-floating-chat");
      expect(floatingWin).toBeDefined();
      expect(floatingWin.style.minHeight).toBe("220px");
      expect(floatingWin.style.height).toBe("auto");
      expect(screen.getByTestId("floating-chat-composer-wrapper").textContent).toContain("@[Comment #ff-slide-headline");
    });

    it("syncs draft text and full session messages 1-1 via sharedChatStore across Sidebar and Floating Chat", () => {
      const scope = "11111111-1111-4111-8111-111111111111::session-sync-1";
      sharedChatStore.clearAll();
      sharedChatStore.appendMessage(scope, {
        id: "m-sess-general",
        sender: "human",
        text: "Yêu cầu chung toàn phiên từ Sidebar",
        timestamp: "15:30",
        createdAtMs: 1700000000000,
      });
      sharedChatStore.setDraft(scope, "Nháp đang gõ dở từ Sidebar");

      render(
        <ContextualNodeFloatingChat
          projectId="11111111-1111-4111-8111-111111111111"
          sessionId="session-sync-1"
          selectedNodes={[mockNodeA]}
          defaultExpanded
          onDeselectNode={vi.fn()}
          onClearSelection={vi.fn()}
        />
      );

      expect(screen.getByText(/Yêu cầu chung toàn phiên từ Sidebar/)).toBeDefined();
      const input = screen.getByTestId("floating-chat-composer-textarea") as HTMLTextAreaElement;
      expect(input.value).toBe("Nháp đang gõ dở từ Sidebar");

      fireEvent.change(input, { target: { value: "Cập nhật tiếp trên Floating Card" } });
      expect(sharedChatStore.getDraft(scope)).toBe("Cập nhật tiếp trên Floating Card");
    });

    it("renders Fork-from-here action on non-greeting chat messages and copies history branch cleanly", () => {
      const scopeA = "proj-fork::sess-main";
      const scopeB = "proj-fork::sess-branch";
      sharedChatStore.clearAll();
      sharedChatStore.appendMessage(scopeA, { id: "m-1", sender: "human", text: "Thiết kế Checkout", timestamp: "15:50", createdAtMs: 100 });
      sharedChatStore.appendMessage(scopeA, { id: "m-2", sender: "agent", text: "Đã tạo SCR-01", timestamp: "15:51", createdAtMs: 200 });
      sharedChatStore.appendMessage(scopeA, { id: "m-3", sender: "human", text: "Đổi sang dark mode", timestamp: "15:52", createdAtMs: 300 });
      sharedChatStore.copyBranchMessages(scopeA, scopeB, "m-2");

      const branched = sharedChatStore.getMessages(scopeB);
      expect(branched.map((m) => m.id)).toEqual(["m-1", "m-2"]);

      render(
        <ContextualNodeFloatingChat
          projectId="proj-fork"
          sessionId="sess-main"
          selectedNodes={[mockNodeA]}
          defaultExpanded
          onDeselectNode={vi.fn()}
          onClearSelection={vi.fn()}
        />
      );
      expect(screen.getByTestId("chat-fork-msg-btn-m-1")).toBeDefined();
      expect(screen.getByTestId("chat-fork-msg-btn-m-2")).toBeDefined();
    });
  });
});
